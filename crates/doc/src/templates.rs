//! New-project templates (the new-project wizard): documents pre-set for a workflow. Each
//! template is our own simple geometry plus page, mat, media, operation, mirror and
//! registration-mark defaults, a name and a short description. Nothing here is copied
//! from another application.
//!
//! Operations: the document knows `cut` and `engrave`; score, pen (sketch), foil, emboss,
//! stipple and engrave all use an `engrave` layer named for the operation, so a machine
//! profile can map the layer to its tool. Canvas y grows downwards: "top" is min y.

use kurbo::{BezPath, Point, Rect, Shape as _};
use serde::{Deserialize, Serialize};

use crate::{CutStyle, DocError, Document, Group, GroupChild, Layer, PagePreset};

const INCH_MM: f64 = 25.4;
/// Dash and gap of a fold (score) line, millimetres.
const FOLD_DASH_MM: f64 = 4.0;
const FOLD_GAP_MM: f64 = 2.0;
/// Largest template item side, millimetres.
const MAX_ITEM_MM: f64 = 2000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectTemplate {
    Blank,
    Card,
    Emboss,
    GiftTag,
    Pixscan,
    PrintAndCut,
    Sketch,
    Stamping,
    Stipple,
    VinylSticker,
    HeatTransferTshirt,
    StickerSheet,
    GreetingsCard,
    Stencil,
    SketchedMessage,
    FoilStampCard,
    FoilStampNotebook,
    AcrylicLight,
    MetalEngravedDecor,
    EmbossedCard,
    EmbossedSticker,
    CoinCase,
}

/// What a new-project picker shows for one template.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TemplateInfo {
    pub id: ProjectTemplate,
    pub name: &'static str,
    pub description: &'static str,
    /// `wizard` (project types) or `starter` (ready-made starting points).
    pub category: &'static str,
    /// Which [`TemplateParams`] fields the template reads.
    pub params: &'static [&'static str],
}

const SIZE: &[&str] = &["preset", "width_mm", "height_mm"];
const CARD: &[&str] = &["preset", "width_mm", "height_mm", "fold"];
const TAG: &[&str] = &["preset", "width_mm", "height_mm", "hole_mm"];
const PAGE: &[&str] = &["preset"];

impl ProjectTemplate {
    pub const ALL: [ProjectTemplate; 22] = [
        ProjectTemplate::Blank,
        ProjectTemplate::Card,
        ProjectTemplate::Emboss,
        ProjectTemplate::GiftTag,
        ProjectTemplate::Pixscan,
        ProjectTemplate::PrintAndCut,
        ProjectTemplate::Sketch,
        ProjectTemplate::Stamping,
        ProjectTemplate::Stipple,
        ProjectTemplate::VinylSticker,
        ProjectTemplate::HeatTransferTshirt,
        ProjectTemplate::StickerSheet,
        ProjectTemplate::GreetingsCard,
        ProjectTemplate::Stencil,
        ProjectTemplate::SketchedMessage,
        ProjectTemplate::FoilStampCard,
        ProjectTemplate::FoilStampNotebook,
        ProjectTemplate::AcrylicLight,
        ProjectTemplate::MetalEngravedDecor,
        ProjectTemplate::EmbossedCard,
        ProjectTemplate::EmbossedSticker,
        ProjectTemplate::CoinCase,
    ];

    pub fn info(self) -> TemplateInfo {
        use ProjectTemplate as T;
        let (name, description, category, params) = match self {
            T::Blank => ("Blank", "An empty 12 × 12 in page with one cut layer.", "wizard", PAGE),
            T::Card => ("Card", "A 10 × 7 in card outline with a dashed score fold through the middle, grouped.", "wizard", CARD),
            T::Emboss => ("Emboss", "A page with an emboss layer: its shapes are pressed, not cut.", "wizard", PAGE),
            T::GiftTag => ("Gift tag", "A 2 × 3.5 in tag with chamfered top corners and a 1/4 in hole, grouped.", "wizard", TAG),
            T::Pixscan => ("Scan mat", "A page for a scanning mat: place the item, import its photo, trace and cut.", "wizard", PAGE),
            T::PrintAndCut => ("Print & Cut", "A Letter page with registration marks and a cut-contour layer.", "wizard", PAGE),
            T::Sketch => ("Sketch", "A page with a pen layer: its shapes are drawn, not cut.", "wizard", PAGE),
            T::Stamping => ("Stamping", "A stamp layer, mirrored so the stamp prints the right way round.", "wizard", PAGE),
            T::Stipple => ("Stipple", "A page with a stipple layer: its shapes are marked as dots.", "wizard", PAGE),
            T::VinylSticker => ("Vinyl sticker", "A 4 in circle on vinyl with a weeding box around it.", "starter", SIZE),
            T::HeatTransferTshirt => ("Heat transfer T-shirt", "A 10 × 10 in print area guide on heat-transfer material, mirrored.", "starter", SIZE),
            T::StickerSheet => ("Sticker sheet (Print & Cut)", "A Letter sheet of 2 in round stickers: artwork guides with cut borders 2 mm outside, plus registration marks.", "starter", PAGE),
            T::GreetingsCard => ("Greetings card", "An 8.5 × 5.5 in card that folds to 4.25 × 5.5 in, with a score fold.", "starter", CARD),
            T::Stencil => ("Stencil", "A 10 × 10 in stencil-film frame; add the design inside and keep bridges.", "starter", SIZE),
            T::SketchedMessage => ("Sketched message", "A folding card whose front has a pen-drawn border for a message.", "starter", CARD),
            T::FoilStampCard => ("Foil stamp card", "A folding card with a foil frame on its front.", "starter", CARD),
            T::FoilStampNotebook => ("Foil stamp notebook", "A 5.5 × 8.5 in notebook-cover guide with a foil frame.", "starter", SIZE),
            T::AcrylicLight => ("Acrylic light", "A 6 × 8 in acrylic panel with a base tab and an engraved frame.", "starter", SIZE),
            T::MetalEngravedDecor => ("Metal engraved framed decor", "An 8 × 10 in metal plate guide with an engraved frame.", "starter", SIZE),
            T::EmbossedCard => ("Embossed card", "A folding card with an embossed frame on its front.", "starter", CARD),
            T::EmbossedSticker => ("Embossed sticker", "A 3 in round sticker with an embossed inner ring.", "starter", SIZE),
            T::CoinCase => ("Coin case", "A flat coin-pouch net: cut outline with score lines to fold.", "starter", SIZE),
        };
        TemplateInfo {
            id: self,
            name,
            description,
            category,
            params,
        }
    }

    pub fn label(self) -> &'static str {
        self.info().name
    }
}

/// Every template, in picker order.
pub fn list_templates() -> Vec<TemplateInfo> {
    ProjectTemplate::ALL.iter().map(|t| t.info()).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FoldAxis {
    /// A fold line running top to bottom through the middle (a side-folding card).
    #[default]
    Vertical,
    Horizontal,
}

/// Optional template settings; each template reads the ones its `params` list names.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TemplateParams {
    #[serde(default)]
    pub preset: Option<PagePreset>,
    /// Item size (the open card, the tag, the sticker, the panel…).
    #[serde(default)]
    pub width_mm: Option<f64>,
    #[serde(default)]
    pub height_mm: Option<f64>,
    #[serde(default)]
    pub fold: Option<FoldAxis>,
    /// Gift tag hole diameter (default 1/4 in).
    #[serde(default)]
    pub hole_mm: Option<f64>,
}

fn size(value: Option<f64>, default: f64, what: &str) -> Result<f64, DocError> {
    let v = value.unwrap_or(default);
    if v.is_finite() && v > 0.0 && v <= MAX_ITEM_MM {
        Ok(v)
    } else {
        Err(DocError::Model(format!(
            "template: {what} must be positive, at most {MAX_ITEM_MM} mm"
        )))
    }
}

fn polygon(points: &[(f64, f64)]) -> BezPath {
    let mut path = BezPath::new();
    for (i, (x, y)) in points.iter().enumerate() {
        let p = Point::new(*x, *y);
        if i == 0 {
            path.move_to(p);
        } else {
            path.line_to(p);
        }
    }
    path.close_path();
    path
}

fn rect_path(r: Rect) -> BezPath {
    polygon(&[(r.x0, r.y0), (r.x1, r.y0), (r.x1, r.y1), (r.x0, r.y1)])
}

fn circle(center: Point, diameter: f64) -> BezPath {
    kurbo::Circle::new(center, diameter / 2.0).to_path(0.01)
}

/// A dashed straight line from `a` to `b` as open subpaths of one path.
fn dashed(a: Point, b: Point) -> BezPath {
    let length = a.distance(b);
    let mut path = BezPath::new();
    let mut t = 0.0;
    // At most a few thousand dashes: lengths are capped by MAX_ITEM_MM.
    while t < length {
        let end = (t + FOLD_DASH_MM).min(length);
        path.move_to(a.lerp(b, t / length));
        path.line_to(a.lerp(b, end / length));
        t = end + FOLD_GAP_MM;
    }
    path
}

/// Builds a template document: layers by name, shapes, one group of the main pieces.
struct Builder {
    doc: Document,
    grouped: Vec<u64>,
}

impl Builder {
    /// Index of layer `name` (created with `kind` if missing).
    fn layer(&mut self, name: &str, kind: CutStyle, template: bool) -> usize {
        if let Some(i) = self.doc.layers.iter().position(|l| l.name == name) {
            return i;
        }
        self.doc.layers.push(Layer {
            template,
            ..Layer::new(name, kind)
        });
        self.doc.layers.len() - 1
    }

    fn add(&mut self, layer: &str, kind: CutStyle, path: BezPath) -> Result<u64, DocError> {
        let index = self.layer(layer, kind, false);
        let id = self.doc.push_shape(index, path, kind)?.id;
        self.grouped.push(id);
        Ok(id)
    }

    /// A guide drawn on a template (non-cutting) layer; not part of the group.
    fn guide(&mut self, path: BezPath) -> Result<(), DocError> {
        let index = self.layer("guide", CutStyle::Cut, true);
        self.doc.push_shape(index, path, CutStyle::Cut)?;
        Ok(())
    }

    /// An open card outline with a score fold; returns the front panel.
    fn card(&mut self, r: Rect, fold: FoldAxis) -> Result<Rect, DocError> {
        self.add("cut", CutStyle::Cut, rect_path(r))?;
        let c = r.center();
        let (a, b, front) = match fold {
            FoldAxis::Vertical => (
                Point::new(c.x, r.y0),
                Point::new(c.x, r.y1),
                Rect::new(c.x, r.y0, r.x1, r.y1),
            ),
            FoldAxis::Horizontal => (
                Point::new(r.x0, c.y),
                Point::new(r.x1, c.y),
                Rect::new(r.x0, c.y, r.x1, r.y1),
            ),
        };
        self.add("score", CutStyle::Engrave, dashed(a, b))?;
        Ok(front)
    }
}

/// A new document for `template`. The page grows (with a 1 in margin) when the item does
/// not fit it; shapes are centred on the page; the main pieces are grouped.
pub fn new_from_template(
    template: ProjectTemplate,
    params: &TemplateParams,
) -> Result<Document, DocError> {
    use ProjectTemplate as T;
    let inch = |w: f64, h: f64| (w * INCH_MM, h * INCH_MM);
    let letter_page = matches!(template, T::PrintAndCut | T::StickerSheet);
    let preset = params.preset.unwrap_or(if letter_page {
        PagePreset::Letter
    } else {
        PagePreset::Mat12x12
    });
    // The item each template centres on the page.
    let item = match template {
        T::Card | T::EmbossedCard | T::FoilStampCard | T::SketchedMessage => Some(inch(10.0, 7.0)),
        T::GreetingsCard => Some(inch(8.5, 5.5)),
        T::GiftTag => Some(inch(2.0, 3.5)),
        T::VinylSticker => Some(inch(4.0, 4.0)),
        T::HeatTransferTshirt | T::Stencil => Some(inch(10.0, 10.0)),
        T::FoilStampNotebook => Some(inch(5.5, 8.5)),
        T::AcrylicLight => Some(inch(6.0, 8.0)),
        T::MetalEngravedDecor => Some(inch(8.0, 10.0)),
        T::EmbossedSticker => Some(inch(3.0, 3.0)),
        T::CoinCase => Some((90.0, 180.0)),
        _ => None,
    };
    let item = match item {
        Some((w, h)) => Some((
            size(params.width_mm, w, "width")?,
            size(params.height_mm, h, "height")?,
        )),
        None => None,
    };
    let (mut page_w, mut page_h) = preset.size_mm().unwrap_or(inch(12.0, 12.0));
    let mut doc = Document::default();
    doc.meta.title = format!("{} project", template.label());
    doc.meta.notes = template.info().description.to_string();
    doc.board_setup.mat = preset.mat_label().map(str::to_string);
    if let Some((w, h)) = item
        && (w > page_w || h > page_h)
    {
        page_w = page_w.max(w + INCH_MM);
        page_h = page_h.max(h + INCH_MM);
        doc.board_setup.mat = None;
    }
    doc.board.width_mm = page_w;
    doc.board.height_mm = page_h;
    let center = Point::new(page_w / 2.0, page_h / 2.0);
    let r = item.map_or(Rect::ZERO, |(w, h)| Rect::from_center_size(center, (w, h)));
    let fold = params.fold.unwrap_or_default();
    let inset = |r: Rect, by: f64| r.inset(-by);
    let mut b = Builder {
        doc,
        grouped: Vec::new(),
    };
    let media = |b: &mut Builder, m: &str| b.doc.board_setup.media = Some(m.to_string());
    let only_layer = |b: &mut Builder, name: &str, kind: CutStyle| {
        b.doc.layers = vec![Layer::new(name, kind)];
    };
    match template {
        T::Blank => {}
        T::Emboss => only_layer(&mut b, "emboss", CutStyle::Engrave),
        T::Sketch => only_layer(&mut b, "pen", CutStyle::Engrave),
        T::Stipple => only_layer(&mut b, "stipple", CutStyle::Engrave),
        T::Stamping => {
            only_layer(&mut b, "stamp", CutStyle::Cut);
            b.doc.board_setup.mirror = true;
            media(&mut b, "stamp material");
        }
        T::Pixscan => b.doc.board_setup.mat = Some("scan mat".to_string()),
        T::PrintAndCut => {
            only_layer(&mut b, "cut contour", CutStyle::Cut);
            b.doc.board_setup.registration_marks = true;
        }
        T::Card | T::GreetingsCard => {
            b.card(r, fold)?;
            media(&mut b, "cardstock");
        }
        T::EmbossedCard | T::FoilStampCard | T::SketchedMessage => {
            let front = b.card(r, fold)?;
            let (layer, frame) = match template {
                T::EmbossedCard => ("emboss", 8.0),
                T::FoilStampCard => ("foil", 10.0),
                _ => ("pen", 12.0),
            };
            b.add(layer, CutStyle::Engrave, rect_path(inset(front, frame)))?;
            media(&mut b, "cardstock");
        }
        T::GiftTag => {
            let hole = size(params.hole_mm, 0.25 * INCH_MM, "hole diameter")?;
            let chamfer = r.width().min(r.height()) / 4.0;
            if hole >= r.width() / 2.0 || hole + chamfer >= r.height() / 2.0 {
                return Err(DocError::Model(
                    "template: the hole is too large for the tag".to_string(),
                ));
            }
            b.add(
                "cut",
                CutStyle::Cut,
                polygon(&[
                    (r.x0 + chamfer, r.y0),
                    (r.x1 - chamfer, r.y0),
                    (r.x1, r.y0 + chamfer),
                    (r.x1, r.y1),
                    (r.x0, r.y1),
                    (r.x0, r.y0 + chamfer),
                ]),
            )?;
            let hole_center = Point::new(center.x, r.y0 + chamfer.max(hole));
            b.add("cut", CutStyle::Cut, circle(hole_center, hole))?;
            media(&mut b, "cardstock");
        }
        T::VinylSticker => {
            b.add("cut", CutStyle::Cut, circle(center, r.width().min(r.height())))?;
            b.add("cut", CutStyle::Cut, rect_path(r.inset(6.0)))?;
            media(&mut b, "adhesive vinyl");
        }
        T::HeatTransferTshirt => {
            b.guide(rect_path(r))?;
            b.doc.board_setup.mirror = true;
            media(&mut b, "heat-transfer vinyl");
        }
        T::StickerSheet => {
            // 2 in circles in a 3 × 4 grid below a 1 in mark margin; borders 2 mm outside.
            let d = 2.0 * INCH_MM;
            let pitch = d + 12.0;
            for row in 0..4 {
                for col in 0..3 {
                    let c = Point::new(
                        page_w / 2.0 + (f64::from(col) - 1.0) * pitch,
                        INCH_MM + 2.0 + d / 2.0 + f64::from(row) * pitch,
                    );
                    b.guide(circle(c, d))?;
                    b.add("cut", CutStyle::Cut, circle(c, d + 4.0))?;
                }
            }
            b.grouped.clear();
            b.doc.board_setup.registration_marks = true;
            media(&mut b, "printable sticker paper");
        }
        T::Stencil => {
            b.add("cut", CutStyle::Cut, rect_path(r))?;
            media(&mut b, "stencil film");
        }
        T::FoilStampNotebook => {
            b.guide(rect_path(r))?;
            b.add("foil", CutStyle::Engrave, rect_path(inset(r, 12.0)))?;
        }
        T::AcrylicLight => {
            let tab_w = r.width() / 2.0;
            let x0 = center.x - tab_w / 2.0;
            b.add(
                "cut",
                CutStyle::Cut,
                polygon(&[
                    (r.x0, r.y0),
                    (r.x1, r.y0),
                    (r.x1, r.y1),
                    (x0 + tab_w, r.y1),
                    (x0 + tab_w, r.y1 + 10.0),
                    (x0, r.y1 + 10.0),
                    (x0, r.y1),
                    (r.x0, r.y1),
                ]),
            )?;
            b.add("engrave", CutStyle::Engrave, rect_path(inset(r, 10.0)))?;
            media(&mut b, "acrylic 3 mm");
        }
        T::MetalEngravedDecor => {
            b.guide(rect_path(r))?;
            b.add("engrave", CutStyle::Engrave, rect_path(inset(r, 12.0)))?;
            media(&mut b, "anodised aluminium");
        }
        T::EmbossedSticker => {
            let d = r.width().min(r.height());
            b.add("cut", CutStyle::Cut, circle(center, d))?;
            b.add("emboss", CutStyle::Engrave, circle(center, d * 0.8))?;
            media(&mut b, "sticker paper");
        }
        T::CoinCase => {
            // A strip folded in thirds: back, front, flap; the flap end is pointed.
            let (w, h) = (r.width(), r.height());
            let third = h / 3.0;
            b.add(
                "cut",
                CutStyle::Cut,
                polygon(&[
                    (r.x0, r.y0 + third * 0.4),
                    (center.x, r.y0),
                    (r.x1, r.y0 + third * 0.4),
                    (r.x1, r.y1),
                    (r.x0, r.y1),
                ]),
            )?;
            for k in [1.0, 2.0] {
                let y = r.y0 + third * k;
                b.add(
                    "score",
                    CutStyle::Engrave,
                    dashed(Point::new(r.x0, y), Point::new(r.x0 + w, y)),
                )?;
            }
            media(&mut b, "cardstock");
        }
    }
    let mut doc = b.doc;
    if b.grouped.len() >= 2 {
        doc.groups.push(Group {
            id: 1,
            name: template.label().to_string(),
            children: b.grouped.into_iter().map(GroupChild::Shape).collect(),
        });
    }
    doc.validate()?;
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_template_builds_fits_and_round_trips() {
        let mut ids = std::collections::BTreeSet::new();
        for info in list_templates() {
            assert!(ids.insert(info.id), "{:?} listed twice", info.id);
            assert!(!info.name.is_empty() && !info.description.is_empty());
            let doc = new_from_template(info.id, &TemplateParams::default()).unwrap();
            let back = Document::load_json(&doc.save_json().unwrap()).unwrap();
            assert_eq!(back, doc, "{:?}", info.id);
            let page = kurbo::Rect::new(0.0, 0.0, doc.board.width_mm, doc.board.height_mm);
            for (_, shape) in doc.shapes() {
                let b = shape.bounds().unwrap();
                assert!(page.contains(b.origin()) && b.x1 <= page.x1 && b.y1 <= page.y1, "{:?}", info.id);
            }
        }
        let json = serde_json::to_value(list_templates()).unwrap();
        assert_eq!(json[0]["id"], "blank");
        assert_eq!(json[10]["id"], "heat-transfer-tshirt");
    }

    #[test]
    fn card_is_a_grouped_outline_with_a_dashed_score_fold() {
        let doc = new_from_template(ProjectTemplate::Card, &TemplateParams::default()).unwrap();
        assert!((doc.board.width_mm - 304.8).abs() < 1e-9);
        let outline = doc.layers[0].shapes[0].bounds().unwrap();
        assert!((outline.width() - 254.0).abs() < 1e-9 && (outline.height() - 177.8).abs() < 1e-9);
        let fold = &doc.layers[1].shapes[0];
        assert_eq!((doc.layers[1].name.as_str(), fold.style), ("score", CutStyle::Engrave));
        let b = fold.bounds().unwrap();
        assert!(b.width() < 1e-9 && (b.center().x - outline.center().x).abs() < 1e-9);
        let dashes = fold
            .path
            .elements()
            .iter()
            .filter(|e| matches!(e, kurbo::PathEl::MoveTo(_)))
            .count();
        assert_eq!(dashes, 30, "177.8 mm in 4 + 2 mm dashes");
        assert_eq!(doc.group_shapes(1), vec![1, 2]);
        let horizontal = TemplateParams {
            fold: Some(FoldAxis::Horizontal),
            ..TemplateParams::default()
        };
        let doc = new_from_template(ProjectTemplate::Card, &horizontal).unwrap();
        assert!(doc.layers[1].shapes[0].bounds().unwrap().height() < 1e-9);
    }

    #[test]
    fn gift_tag_hole_and_large_items_grow_the_page() {
        let doc = new_from_template(ProjectTemplate::GiftTag, &TemplateParams::default()).unwrap();
        assert_eq!(doc.layers[0].shapes.len(), 2);
        let hole = doc.layers[0].shapes[1].bounds().unwrap();
        assert!((hole.width() - 6.35).abs() < 0.01);
        let big = TemplateParams {
            width_mm: Some(500.0),
            height_mm: Some(100.0),
            ..TemplateParams::default()
        };
        let doc = new_from_template(ProjectTemplate::Card, &big).unwrap();
        assert!(doc.board.width_mm >= 500.0 && doc.board_setup.mat.is_none());
        for bad in [
            TemplateParams {
                hole_mm: Some(40.0),
                ..TemplateParams::default()
            },
            TemplateParams {
                width_mm: Some(f64::NAN),
                ..TemplateParams::default()
            },
            TemplateParams {
                height_mm: Some(1e9),
                ..TemplateParams::default()
            },
        ] {
            assert!(new_from_template(ProjectTemplate::GiftTag, &bad).is_err());
        }
    }

    #[test]
    fn production_defaults_follow_the_template() {
        let t = |id| new_from_template(id, &TemplateParams::default()).unwrap();
        let shirt = t(ProjectTemplate::HeatTransferTshirt);
        assert!(shirt.board_setup.mirror);
        assert_eq!(shirt.board_setup.media.as_deref(), Some("heat-transfer vinyl"));
        assert!(shirt.layers.iter().any(|l| l.template), "print area is a guide");
        let sheet = t(ProjectTemplate::StickerSheet);
        assert!(sheet.board_setup.registration_marks);
        assert!((sheet.board.width_mm - 215.9).abs() < 1e-9);
        let cut = sheet.layers.iter().find(|l| l.name == "cut").unwrap();
        assert_eq!(cut.shapes.len(), 12);
        let sketch = t(ProjectTemplate::Sketch);
        assert_eq!((sketch.layers[0].name.as_str(), sketch.layers[0].kind), ("pen", CutStyle::Engrave));
        let coin = t(ProjectTemplate::CoinCase);
        assert_eq!(coin.layers.iter().find(|l| l.name == "score").unwrap().shapes.len(), 2);
        assert!(t(ProjectTemplate::Stamping).board_setup.mirror);
    }
}
