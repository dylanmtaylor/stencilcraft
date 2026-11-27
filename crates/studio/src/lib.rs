//! Adapter from the shared Silhouette Studio reader to the StencilCraft document model.

use kurbo::{Affine, BezPath};
use stencilcraft_doc::{
    CutStyle, Document, FontSpec, Group, GroupChild, Layer, TextAlign, TextFlow, TextSpec,
};

pub const MAX_FILE_BYTES: usize = craft_silhouette_studio::MAX_FILE_BYTES;

pub fn is_studio_path(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .and_then(craft_file_formats::from_extension)
        == Some(craft_file_formats::Format::Studio)
}

#[derive(Debug, thiserror::Error)]
pub enum StudioError {
    #[error("{0}: not a Silhouette Studio document")]
    NotStudio(String),
    #[error("{name}: Silhouette Studio document version {version} is not supported yet")]
    Unsupported { name: String, version: String },
    #[error(
        "{0}: this design is protected or encrypted (for example a store purchase); StencilCraft does not open protected designs"
    )]
    Protected(String),
    #[error("{0}: file exceeds the 64 MiB import limit")]
    TooLarge(String),
    #[error("{name}: damaged Silhouette Studio document: {what}")]
    Malformed { name: String, what: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    V2,
    V3,
}

#[derive(Debug, Clone)]
pub struct Imported {
    pub doc: Document,
    pub version: Version,
    pub warnings: Vec<String>,
}

pub fn import(bytes: &[u8], name: &str) -> Result<Imported, StudioError> {
    let parsed = craft_silhouette_studio::import(bytes).map_err(|error| {
        use craft_silhouette_studio::Error as E;
        match error {
            E::NotStudio => StudioError::NotStudio(name.to_owned()),
            E::Unsupported { version } => StudioError::Unsupported {
                name: name.to_owned(),
                version,
            },
            E::Protected => StudioError::Protected(name.to_owned()),
            E::TooLarge => StudioError::TooLarge(name.to_owned()),
            E::Malformed(what) => StudioError::Malformed {
                name: name.to_owned(),
                what,
            },
        }
    })?;
    let mut doc = Document {
        layers: Vec::new(),
        ..Document::default()
    };
    if let Some((w, h)) = parsed.page_mm {
        doc.board.width_mm = w;
        doc.board.height_mm = h;
    }
    // Silhouette's page and StencilCraft's document are both millimetres from the top-left,
    // y down, so outlines come across as they are.
    let to_doc = Affine::IDENTITY;
    let malformed = |e: stencilcraft_doc::DocError| StudioError::Malformed {
        name: name.to_owned(),
        what: e.to_string(),
    };
    // The letters of a text object become one live text shape: its outlines as read, and the
    // characters, font and size to edit it with.
    let mut letters: Vec<TextParts> = parsed.texts.iter().map(|_| TextParts::default()).collect();
    // The document id of each read object (`None` for letters merged into their text).
    let mut ids: Vec<Option<u64>> = Vec::with_capacity(parsed.shapes.len());
    for shape in parsed.shapes.iter() {
        let path: BezPath = to_doc * shape.path.clone();
        if let Some(entry) = shape.text.and_then(|t| letters.get_mut(t)) {
            entry.path.extend(path.elements().iter().copied());
            entry.line = entry.line.or(shape.line_color);
            entry.fill = entry.fill.or(shape.fill_color);
            entry.group = entry.group.or(shape.group);
            ids.push(None);
            continue;
        }
        let layer = layer_for(&mut doc, shape.line_color);
        let added = doc
            .push_shape(layer, path, CutStyle::Cut)
            .map_err(malformed)?;
        set_fill(&mut doc, added.id, shape.fill_color);
        ids.push(Some(added.id));
    }
    let mut text_ids = Vec::with_capacity(letters.len());
    for (
        text,
        TextParts {
            path, line, fill, ..
        },
    ) in parsed.texts.iter().zip(&letters)
    {
        if path.elements().is_empty() {
            text_ids.push(None);
            continue;
        }
        let layer = layer_for(&mut doc, *line);
        let added = doc
            .push_shape(layer, path.clone(), CutStyle::Cut)
            .map_err(malformed)?;
        set_fill(&mut doc, added.id, *fill);
        if let Some(shape) = doc.get_shape_mut(added.id) {
            shape.text = Some(Box::new(live_text(text)));
        }
        text_ids.push(Some(added.id));
    }
    add_groups(&mut doc, &parsed, &ids, &letters, &text_ids);
    if doc.layers.is_empty() {
        doc.layers.push(Layer::new("cut", CutStyle::Cut));
    }
    doc.validate().map_err(|e| StudioError::Malformed {
        name: name.to_owned(),
        what: e.to_string(),
    })?;
    Ok(Imported {
        doc,
        version: match parsed.version {
            craft_silhouette_studio::Version::V2 => Version::V2,
            craft_silhouette_studio::Version::V3 => Version::V3,
        },
        warnings: parsed.warnings,
    })
}

/// The document as a `.studio3` file Silhouette Studio opens: the board as the page, each
/// visible shape as one Silhouette shape with its contours and curves, its line colour, line
/// width and fill. Placed images, text styling and cut settings are not written.
pub fn export(doc: &Document) -> Result<Vec<u8>, StudioError> {
    export_as(doc, false)
}

/// The document as a legacy `.studio` (Studio V2) file; otherwise like [`export`].
pub fn export_v2(doc: &Document) -> Result<Vec<u8>, StudioError> {
    export_as(doc, true)
}

fn export_as(doc: &Document, v2: bool) -> Result<Vec<u8>, StudioError> {
    let to_page = Affine::IDENTITY;
    let hex = |c: Option<&str>| {
        let d = c?.trim().trim_start_matches('#');
        let ch = |i: usize| u8::from_str_radix(d.get(i..i + 2)?, 16).ok();
        (d.len() == 6).then_some(())?;
        Some([ch(0)?, ch(2)?, ch(4)?])
    };
    let mut shapes = Vec::new();
    for layer in doc.layers.iter().filter(|l| l.visible && !l.template) {
        for shape in layer.shapes.iter().filter(|s| !s.hidden) {
            let path = to_page * shape.cut_path().into_owned();
            let line_rgb = hex(shape
                .production
                .line_color
                .as_deref()
                .or(layer.color.as_deref()));
            let fill_rgb = hex(shape.production.fill_color.as_deref());
            let width_mm = shape.production.stroke_width_mm.unwrap_or(0.0);
            // One Silhouette shape per StencilCraft shape: its contours with their curves.
            let contours = craft_silhouette_studio::WriteContour::from_path(&path);
            if !contours.is_empty() {
                shapes.push(craft_silhouette_studio::WriteShape {
                    contours,
                    line_rgb,
                    fill_rgb,
                    width_mm,
                });
            }
        }
    }
    let document = craft_silhouette_studio::WriteDocument {
        page_mm: (doc.board.width_mm, doc.board.height_mm),
        shapes,
    };
    if v2 {
        craft_silhouette_studio::write_v2(&document)
    } else {
        craft_silhouette_studio::write(&document)
    }
    .map_err(|e| StudioError::Malformed {
        name: "export".to_owned(),
        what: e.to_string(),
    })
}

/// Detect and import a Silhouette document from disk. Format recognition stays in the shared
/// file-format registry; this crate only adapts the recognized Studio document into Stencil data.
pub fn import_path(path: &str) -> Result<Option<Imported>, String> {
    if std::path::Path::new(path)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .and_then(craft_file_formats::from_extension)
        == Some(craft_file_formats::Format::StencilCraft)
    {
        return Ok(None);
    }
    let size = std::fs::metadata(path)
        .map_err(|error| format!("{path}: {error}"))?
        .len();
    if size > MAX_FILE_BYTES as u64 {
        let is_studio_extension = std::path::Path::new(path)
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .and_then(craft_file_formats::from_extension)
            == Some(craft_file_formats::Format::Studio);
        if is_studio_extension {
            return Err(StudioError::TooLarge(path.to_owned()).to_string());
        }
        return Ok(None);
    }
    let bytes = std::fs::read(path).map_err(|error| format!("{path}: {error}"))?;
    if craft_file_formats::detect(path, &bytes) != Some(craft_file_formats::Format::Studio) {
        return Ok(None);
    }
    import(&bytes, path)
        .map(Some)
        .map_err(|error| error.to_string())
}

/// The letters read for one text object: their outlines together, the first line and fill
/// colours and group found.
#[derive(Default)]
struct TextParts {
    path: BezPath,
    line: Option<[u8; 3]>,
    fill: Option<[u8; 3]>,
    group: Option<usize>,
}

/// A read text object as StencilCraft's live text.
fn live_text(text: &craft_silhouette_studio::Text) -> TextSpec {
    TextSpec {
        text: text.text.clone(),
        font: FontSpec {
            family: text.font.clone().unwrap_or_else(|| "Arial".to_owned()),
            style: "Regular".to_owned(),
            file: None,
        },
        size: text.size_mm * 72.0 / 25.4,
        x: text.x_mm,
        y: text.y_mm,
        alignment: TextAlign::Left,
        letter_spacing: 0.0,
        line_spacing: 100.0,
        kerning: true,
        vertical: false,
        weld: false,
        flow: TextFlow::Straight,
        wrap_width_mm: None,
        underline: false,
        letters: Vec::new(),
        transform: stencilcraft_doc::IDENTITY_TRANSFORM,
    }
}

fn set_fill(doc: &mut Document, id: u64, fill: Option<[u8; 3]>) {
    if let (Some([r, g, b]), Some(shape)) = (fill, doc.get_shape_mut(id)) {
        shape.production.fill_color = Some(format!("#{r:02x}{g:02x}{b:02x}"));
    }
}

/// Silhouette's groups as document groups, nested as read; a text's letters count once,
/// as the text.
fn add_groups(
    doc: &mut Document,
    parsed: &craft_silhouette_studio::Imported,
    ids: &[Option<u64>],
    letters: &[TextParts],
    text_ids: &[Option<u64>],
) {
    let first = doc.next_group_id();
    let group_id = |i: usize| first.saturating_add(i as u64);
    let mut children: Vec<Vec<GroupChild>> = vec![Vec::new(); parsed.group_parents.len()];
    for (i, parent) in parsed.group_parents.iter().enumerate() {
        if let Some(list) = parent.and_then(|p| children.get_mut(p)) {
            list.push(GroupChild::Group(group_id(i)));
        }
    }
    let members = parsed
        .shapes
        .iter()
        .zip(ids)
        .filter_map(|(shape, id)| Some((shape.group?, (*id)?)))
        .chain(
            letters
                .iter()
                .zip(text_ids)
                .filter_map(|(entry, id)| Some((entry.group?, (*id)?))),
        );
    for (group, id) in members {
        if let Some(list) = children.get_mut(group) {
            list.push(GroupChild::Shape(id));
        }
    }
    for (i, children) in children.into_iter().enumerate() {
        if !children.is_empty() {
            doc.groups.push(Group {
                id: group_id(i),
                name: String::new(),
                children,
            });
        }
    }
}

/// One cut layer per line colour, like Silhouette's Send by Line Color; shapes whose
/// style was not found go on a plain `cut` layer.
fn layer_for(doc: &mut Document, color: Option<[u8; 3]>) -> usize {
    let (name, color) = match color {
        Some([r, g, b]) => {
            let hex = format!("#{r:02x}{g:02x}{b:02x}");
            (format!("Line {hex}"), Some(hex))
        }
        None => ("cut".to_string(), None),
    };
    if let Some(i) = doc.layers.iter().position(|l| l.name == name) {
        return i;
    }
    let mut layer = Layer::new(name, CutStyle::Cut);
    layer.color = color;
    doc.layers.push(layer);
    doc.layers.len() - 1
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Documents saved in Silhouette Studio 5.0.556 (not committed: `STUDIO_SAMPLES` names
    /// their folder). Each differs from the others in one thing.
    fn sample(name: &str) -> Option<Imported> {
        let dir = std::env::var("STUDIO_SAMPLES").ok()?;
        let bytes = std::fs::read(std::path::Path::new(&dir).join(name)).ok()?;
        import(&bytes, name).ok()
    }

    #[test]
    fn v2_exports_open_again() {
        let mut doc = Document::default();
        let mut tri = BezPath::new();
        tri.move_to((10.0, 10.0));
        tri.line_to((40.0, 10.0));
        tri.line_to((25.0, 30.0));
        tri.close_path();
        doc.push_shape(0, tri, CutStyle::Cut).unwrap();
        let back = import(&export_v2(&doc).unwrap(), "tri.studio").unwrap();
        assert_eq!(back.version, Version::V2);
        assert_eq!(
            back.doc
                .layers
                .iter()
                .map(|l| l.shapes.len())
                .sum::<usize>(),
            1
        );
    }

    #[test]
    fn exported_curves_stay_curves() {
        let mut doc = Document::default();
        let circle = kurbo::Shape::to_path(&kurbo::Circle::new((50.0, 50.0), 20.0), 0.1);
        doc.push_shape(0, circle, CutStyle::Cut).unwrap();
        let back = import(&export(&doc).unwrap(), "circle.studio3").unwrap();
        let shape = back
            .doc
            .layers
            .iter()
            .flat_map(|l| &l.shapes)
            .next()
            .unwrap();
        let curves = shape
            .path
            .elements()
            .iter()
            .filter(|e| matches!(e, kurbo::PathEl::CurveTo(..)))
            .count();
        assert!(curves >= 4, "{curves} curves");
        let b = kurbo::Shape::bounding_box(&shape.path);
        assert!(
            (b.x0 - 30.0).abs() < 0.01 && (b.y1 - 70.0).abs() < 0.01,
            "{b:?}"
        );
    }

    #[test]
    fn exports_open_again_with_their_shapes_and_colours() {
        let mut doc = Document::default();
        doc.board.width_mm = 200.0;
        doc.board.height_mm = 100.0;
        let mut square = BezPath::new();
        square.move_to((10.0, 10.0));
        square.line_to((60.0, 10.0));
        square.line_to((60.0, 40.0));
        square.line_to((10.0, 40.0));
        square.close_path();
        let id = doc.push_shape(0, square, CutStyle::Cut).unwrap().id;
        doc.get_shape_mut(id).unwrap().production.line_color = Some("#0066cc".into());
        let bytes = export(&doc).unwrap();
        let back = import(&bytes, "round.studio3").unwrap();
        assert_eq!(
            (back.doc.board.width_mm, back.doc.board.height_mm),
            (200.0, 100.0)
        );
        let shapes: Vec<_> = back.doc.layers.iter().flat_map(|l| &l.shapes).collect();
        assert_eq!(shapes.len(), 1);
        let b = kurbo::Shape::bounding_box(&shapes[0].path);
        assert!(
            (b.x0 - 10.0).abs() < 1e-3 && (b.y0 - 10.0).abs() < 1e-3 && (b.x1 - 60.0).abs() < 1e-3,
            "{b:?}"
        );
        assert!(
            back.doc
                .layers
                .iter()
                .any(|l| l.color.as_deref() == Some("#0066cc"))
        );
    }

    #[test]
    fn fills_groups_and_live_text_come_across() {
        let Some(fill) = sample("fillC.studio3") else {
            return;
        };
        let fills: Vec<_> = fill
            .doc
            .layers
            .iter()
            .flat_map(|l| &l.shapes)
            .filter_map(|s| s.production.fill_color.clone())
            .collect();
        assert_eq!(fills, vec!["#1b7cb8".to_string()]);
        let grouped = sample("grp1.studio3").map(|i| i.doc.groups);
        assert_eq!(
            grouped.map(|g| g.first().map(|g| g.children.len())),
            Some(Some(2))
        );
        let text = sample("txt1.studio3").map(|i| {
            i.doc
                .layers
                .iter()
                .flat_map(|l| &l.shapes)
                .filter_map(|s| {
                    s.text
                        .as_ref()
                        .map(|t| (t.text.clone(), t.font.family.clone(), t.size.round()))
                })
                .collect::<Vec<_>>()
        });
        assert_eq!(
            text,
            Some(vec![("Hello".to_string(), "Arial".to_string(), 72.0)])
        );
    }
}
