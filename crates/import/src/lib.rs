//! File import into a StencilCraft document: SVG/SVGZ, DXF, PDF, EPS/PS, Illustrator `.ai`,
//! and raster images placed as an image object to trace. Detection, reader selection, codecs, and
//! vector parsing come from craft-libs; this crate maps their neutral results into Stencil data.
//!
//! Vector art comes in "as is" (1:1, the source page as the board) or fitted to a
//! page, and open paths whose endpoints meet within a tolerance can be joined into
//! polylines (DXF drawings are often loose `LINE`s). Only path geometry is read: fills,
//! text and embedded images in vector files are not. SVG source tags, classes, layers and stroke
//! colors are retained when they affect Stencil's layer/style mapping.

mod handlers;
mod render;

use std::collections::HashMap;

use kurbo::{BezPath, PathEl, Point, Shape as _};
use stencilcraft_doc::{CutStyle, Document, Layer};

pub use handlers::{
    Capabilities, ExportOptions, FormatHandler, handlers, write_document, writer_for,
};
pub use render::{RenderFormat, render, render_document};

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("{name}: {message}")]
    Bad { name: String, message: String },
}

/// Format families recognized by the shared import registry.
pub use craft_file_import::ImportFormat as Format;
pub const MAX_SHAPES: usize = craft_file_import::MAX_SHAPES;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImportOptions {
    /// Scale vector art uniformly to fit `page_mm` (else 1:1, "as is").
    pub fit_to_page: bool,
    /// The page `fit_to_page` fills, millimetres (width, height).
    pub page_mm: (f64, f64),
    /// Join open paths whose endpoints are within `join_tolerance_mm`.
    pub join: bool,
    pub join_tolerance_mm: f64,
    /// Raster images: pixels per inch for their physical size.
    pub dpi: f64,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            fit_to_page: false,
            page_mm: (100.0, 150.0),
            join: true,
            join_tolerance_mm: 0.05,
            dpi: 96.0,
        }
    }
}

fn bad(name: &str, message: impl std::fmt::Display) -> ImportError {
    ImportError::Bad {
        name: name.to_string(),
        message: message.to_string(),
    }
}

/// Read a file (bounded) and import it.
pub fn import_file(path: &str, options: &ImportOptions) -> Result<Document, ImportError> {
    let size = std::fs::metadata(path).map_err(|e| bad(path, e))?.len();
    if size > craft_file_import::MAX_INPUT_BYTES as u64 {
        return Err(bad(path, "the file exceeds the 64 MiB input limit"));
    }
    let bytes = std::fs::read(path).map_err(|e| bad(path, e))?;
    import_bytes(&bytes, path, options)
}

/// Import file contents; `name` picks the format when the bytes don't, and labels errors.
pub fn import_bytes(
    bytes: &[u8],
    name: &str,
    options: &ImportOptions,
) -> Result<Document, ImportError> {
    if bytes.len() > craft_file_import::MAX_INPUT_BYTES {
        return Err(bad(name, "the file exceeds the 64 MiB input limit"));
    }
    let format = craft_file_import::format_for(name, bytes)
        .ok_or_else(|| bad(name, "unrecognised or unsupported import format"))?;
    read_format(format, bytes, name, options)
}

pub(crate) fn read_format(
    format: Format,
    bytes: &[u8],
    name: &str,
    options: &ImportOptions,
) -> Result<Document, ImportError> {
    let detected = craft_file_import::format_for(name, bytes)
        .ok_or_else(|| bad(name, "unrecognised or unsupported import format"))?;
    if detected != format {
        return Err(bad(
            name,
            "selected import handler does not match the detected file format",
        ));
    }
    let imported =
        craft_file_import::import(name, bytes, Default::default()).map_err(|e| bad(name, e))?;
    imported_document(imported, name, options)
}

/// Adapt a neutral shared import result, then apply Stencil document options.
fn imported_document(
    imported: craft_file_import::Imported,
    name: &str,
    options: &ImportOptions,
) -> Result<Document, ImportError> {
    let (fit_w, fit_h) = options.page_mm;
    if options.fit_to_page
        && !(fit_w.is_finite() && fit_h.is_finite() && fit_w > 0.0 && fit_h > 0.0)
    {
        return Err(bad(name, "the page to fit must be a positive size"));
    }
    if options.join
        && !(options.join_tolerance_mm.is_finite()
            && (0.0..=10.0).contains(&options.join_tolerance_mm))
    {
        return Err(bad(name, "join tolerance must be 0–10 mm"));
    }
    let mut doc = match imported {
        craft_file_import::Imported::Raster(image) => {
            let pixels = craft_raster_codecs::common::Pixels::from_raw(
                image.width,
                image.height,
                image.rgba,
            )
            .ok_or_else(|| {
                bad(
                    name,
                    "shared raster decoder returned an invalid pixel buffer",
                )
            })?;
            let label = name.rsplit(['/', '\\']).next().unwrap_or(name);
            let image = stencilcraft_raster::place(&pixels, label, options.dpi)
                .map_err(|e| bad(name, e))?;
            let mut doc = Document {
                board: stencilcraft_doc::Board {
                    width_mm: image.width_mm,
                    height_mm: image.height_mm,
                },
                ..Document::default()
            };
            doc.push_image(image).map_err(|e| bad(name, e))?;
            return Ok(doc);
        }
        craft_file_import::Imported::Vector(scene) => {
            for warning in &scene.warnings {
                eprintln!("{name}: {warning}");
            }
            if scene.dropped_warnings > 0 {
                eprintln!(
                    "{name}: {} additional import warnings omitted",
                    scene.dropped_warnings
                );
            }
            vector_document(scene, name)?
        }
    };
    if options.join {
        join_document(&mut doc, options.join_tolerance_mm);
    }
    if options.fit_to_page {
        fit(&mut doc, options.page_mm);
    }
    doc.validate().map_err(|e| bad(name, e))?;
    Ok(doc)
}

/// Map generic shared paths and source labels into StencilCraft's layers and cut/engrave styles.
fn vector_document(
    scene: craft_file_import::VectorScene,
    name: &str,
) -> Result<Document, ImportError> {
    if !(scene.width_mm.is_finite()
        && scene.height_mm.is_finite()
        && scene.width_mm > 0.0
        && scene.height_mm > 0.0)
    {
        return Err(bad(name, "the vector page has invalid dimensions"));
    }
    if scene.shapes.len() > MAX_SHAPES {
        return Err(bad(name, format!("more than {MAX_SHAPES} paths")));
    }
    let mut doc = Document {
        board: stencilcraft_doc::Board {
            width_mm: scene.width_mm,
            height_mm: scene.height_mm,
        },
        layers: vec![Layer::new("cut", CutStyle::Cut)],
        ..Document::default()
    };
    let mut layers = HashMap::<(String, bool, Option<String>), usize>::new();
    layers.insert(("cut".into(), false, None), 0);
    for source in scene.shapes {
        if source.path.elements().is_empty() {
            continue;
        }
        let classes = source
            .class
            .as_deref()
            .unwrap_or("")
            .split_ascii_whitespace();
        let style = if source.tag.as_deref() == Some("engrave")
            || classes.clone().any(|class| class == "stencilcraft-engrave")
        {
            CutStyle::Engrave
        } else {
            CutStyle::Cut
        };
        let layer_name = source.layer.unwrap_or_else(|| match style {
            CutStyle::Cut => "cut".into(),
            CutStyle::Engrave => "engrave".into(),
        });
        let color = source
            .stroke_rgb
            .map(|[red, green, blue]| format!("#{red:02x}{green:02x}{blue:02x}"));
        let key = (
            layer_name.clone(),
            style == CutStyle::Engrave,
            color.clone(),
        );
        let layer_index = if let Some(index) = layers.get(&key) {
            *index
        } else {
            let mut layer = Layer::new(layer_name, style);
            layer.color = color;
            doc.layers.push(layer);
            let index = doc.layers.len() - 1;
            layers.insert(key, index);
            index
        };
        doc.push_shape(layer_index, source.path, style)
            .map_err(|e| bad(name, e))?;
    }
    doc.validate().map_err(|e| bad(name, e))?;
    Ok(doc)
}

/// Scale the art uniformly to fill the page, its corner at the page's.
fn fit(doc: &mut Document, (page_w, page_h): (f64, f64)) {
    let bounds = doc
        .layers
        .iter()
        .filter_map(Layer::bounds)
        .reduce(|a, b| a.union(b));
    doc.board.width_mm = page_w;
    doc.board.height_mm = page_h;
    let Some(bounds) = bounds else {
        return;
    };
    let scale = [page_w / bounds.width(), page_h / bounds.height()]
        .into_iter()
        .filter(|s| s.is_finite() && *s > 0.0)
        .reduce(f64::min)
        .unwrap_or(1.0);
    let transform =
        kurbo::Affine::scale(scale) * kurbo::Affine::translate(-bounds.origin().to_vec2());
    for shape in doc.layers.iter_mut().flat_map(|l| l.shapes.iter_mut()) {
        shape.path = transform * shape.path.clone();
    }
}

/// Join open subpaths, per layer, into chains; a chain that comes back to its start
/// within the tolerance is closed. Closed subpaths are left alone.
fn join_document(doc: &mut Document, tolerance: f64) {
    let mut next_id = doc.next_shape_id;
    for layer in &mut doc.layers {
        let mut open = Vec::new();
        let mut kept = Vec::new();
        for mut shape in std::mem::take(&mut layer.shapes) {
            let mut closed = BezPath::new();
            for sub in subpaths(&shape.path) {
                if matches!(sub.elements().last(), Some(PathEl::ClosePath)) {
                    closed.extend(sub.elements().iter().copied());
                } else if sub.elements().len() > 1 {
                    open.push((sub, shape.style));
                }
            }
            if !closed.elements().is_empty() {
                shape.path = closed;
                kept.push(shape);
            }
        }
        for style in [CutStyle::Cut, CutStyle::Engrave] {
            let group: Vec<BezPath> = open
                .iter()
                .filter(|(_, s)| *s == style)
                .map(|(p, _)| p.clone())
                .collect();
            for path in join(group, tolerance) {
                kept.push(stencilcraft_doc::Shape {
                    id: next_id,
                    path,
                    style,
                    hidden: false,
                    locked: false,
                    tool: None,
                    text: None,
                    production: Default::default(),
                });
                next_id = next_id.saturating_add(1);
            }
        }
        layer.shapes = kept;
    }
    doc.next_shape_id = next_id;
}

fn subpaths(path: &BezPath) -> Vec<BezPath> {
    let mut out: Vec<BezPath> = Vec::new();
    for element in path.elements() {
        match (element, out.last_mut()) {
            (PathEl::MoveTo(_), _) | (_, None) => out.push(BezPath::from_vec(vec![*element])),
            (_, Some(current)) => current.push(*element),
        }
    }
    out.retain(|p| matches!(p.elements().first(), Some(PathEl::MoveTo(_))));
    out
}

fn ends(path: &BezPath) -> Option<(Point, Point)> {
    let start = match path.elements().first() {
        Some(PathEl::MoveTo(p)) => *p,
        _ => return None,
    };
    Some((
        start,
        path.elements()
            .last()
            .and_then(PathEl::end_point)
            .unwrap_or(start),
    ))
}

/// Greedy endpoint chaining through a grid hash of cell size `tolerance`.
pub fn join(paths: Vec<BezPath>, tolerance: f64) -> Vec<BezPath> {
    let cell = tolerance.max(1e-9);
    let key = |p: Point| ((p.x / cell).floor() as i64, (p.y / cell).floor() as i64);
    let mut grid: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    let ends: Vec<Option<(Point, Point)>> = paths.iter().map(ends).collect();
    for (i, e) in ends.iter().enumerate() {
        if let Some((a, b)) = e {
            grid.entry(key(*a)).or_default().push(i);
            if key(*a) != key(*b) {
                grid.entry(key(*b)).or_default().push(i);
            }
        }
    }
    let mut used = vec![false; paths.len()];
    // The nearest unused path with an endpoint near `p`, and whether that is its end.
    let find = |p: Point, used: &[bool]| -> Option<(usize, bool)> {
        let (kx, ky) = key(p);
        let mut best: Option<(f64, usize, bool)> = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                let cell = (kx.saturating_add(dx), ky.saturating_add(dy));
                for &j in grid.get(&cell).into_iter().flatten() {
                    if used.get(j).copied().unwrap_or(true) {
                        continue;
                    }
                    let Some(Some((a, b))) = ends.get(j) else {
                        continue;
                    };
                    for (q, is_end) in [(*a, false), (*b, true)] {
                        let d = q.distance(p);
                        if d <= tolerance && best.is_none_or(|(bd, ..)| d < bd) {
                            best = Some((d, j, is_end));
                        }
                    }
                }
            }
        }
        best.map(|(_, j, is_end)| (j, is_end))
    };
    let mut out = Vec::new();
    for i in 0..paths.len() {
        if used[i] {
            continue;
        }
        used[i] = true;
        let mut chain = paths[i].clone();
        for _pass in 0..2 {
            while let Some((_, end)) = self::ends(&chain) {
                let Some((j, is_end)) = find(end, &used) else {
                    break;
                };
                used[j] = true;
                let mut next = paths[j].clone();
                if is_end {
                    next = next.reverse_subpaths();
                }
                chain.extend(next.elements().iter().skip(1).copied());
            }
            chain = chain.reverse_subpaths();
        }
        if let Some((start, end)) = self::ends(&chain)
            && chain.elements().len() > 2
            && start.distance(end) <= tolerance
        {
            chain.close_path();
        }
        out.push(chain);
    }
    out
}

/// Bounds of a whole document's art, for callers placing an import.
pub fn art_bounds(doc: &Document) -> Option<kurbo::Rect> {
    doc.layers
        .iter()
        .flat_map(|l| l.shapes.iter())
        .filter_map(|s| s.path.bounding_box().as_rect())
        .reduce(|a, b| a.union(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(a: (f64, f64), b: (f64, f64)) -> BezPath {
        let mut p = BezPath::new();
        p.move_to(a);
        p.line_to(b);
        p
    }

    #[test]
    fn join_chains_loose_lines_into_a_closed_polyline() {
        // A square drawn as four lines, two of them backwards, with a small gap.
        let lines = vec![
            line((0.0, 0.0), (10.0, 0.0)),
            line((10.0, 10.0), (10.0, 0.01)),
            line((10.0, 10.0), (0.0, 10.0)),
            line((0.0, 0.0), (0.0, 10.0)),
            line((50.0, 50.0), (60.0, 50.0)),
        ];
        let joined = join(lines, 0.05);
        assert_eq!(joined.len(), 2);
        let square = joined.iter().find(|p| p.elements().len() > 3).unwrap();
        assert!(matches!(square.elements().last(), Some(PathEl::ClosePath)));
        assert!(
            (square.area().abs() - 100.0).abs() < 0.2,
            "{}",
            square.area()
        );
        assert_eq!(
            join(
                vec![line((0.0, 0.0), (1.0, 0.0)), line((1.1, 0.0), (2.0, 0.0))],
                0.05
            )
            .len(),
            2
        );
    }

    #[test]
    fn detects_formats() {
        assert_eq!(
            craft_file_import::format_for("x.bin", b"%PDF-1.7"),
            Some(Format::Pdf)
        );
        assert_eq!(
            craft_file_import::format_for("x.ai", b"%PDF-1.5"),
            Some(Format::Pdf)
        );
        assert_eq!(
            craft_file_import::format_for("x.ai", b"%!PS-Adobe-3.0"),
            Some(Format::Eps)
        );
        assert_eq!(
            craft_file_import::format_for("X.DXF", b"0\nSECTION"),
            Some(Format::Dxf)
        );
        assert_eq!(
            craft_file_import::format_for("drawing", b"0\nSECTION\n2\nHEADER"),
            Some(Format::Dxf)
        );
        assert_eq!(
            craft_file_import::format_for("a.webp", b""),
            Some(Format::Raster)
        );
        assert_eq!(craft_file_import::format_for("noext", b"\x00\x01"), None);
    }
}
