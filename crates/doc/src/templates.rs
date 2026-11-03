//! New-project templates: documents pre-set for a workflow (blank page, folded card, gift
//! tag, and starters for emboss, sketch, stamp, stipple and print & cut). All geometry is
//! generated here; nothing is copied from another application.

use kurbo::{BezPath, Point, Shape as _};
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
    GiftTag,
    Emboss,
    Sketch,
    Stamp,
    Stipple,
    PrintAndCut,
}

impl ProjectTemplate {
    pub const ALL: [ProjectTemplate; 8] = [
        ProjectTemplate::Blank,
        ProjectTemplate::Card,
        ProjectTemplate::GiftTag,
        ProjectTemplate::Emboss,
        ProjectTemplate::Sketch,
        ProjectTemplate::Stamp,
        ProjectTemplate::Stipple,
        ProjectTemplate::PrintAndCut,
    ];

    /// A short label for a new-project picker.
    pub fn label(self) -> &'static str {
        match self {
            ProjectTemplate::Blank => "Blank",
            ProjectTemplate::Card => "Card",
            ProjectTemplate::GiftTag => "Gift tag",
            ProjectTemplate::Emboss => "Emboss",
            ProjectTemplate::Sketch => "Sketch",
            ProjectTemplate::Stamp => "Stamp",
            ProjectTemplate::Stipple => "Stipple",
            ProjectTemplate::PrintAndCut => "Print & Cut",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FoldAxis {
    /// A fold line running top to bottom through the middle (a side-folding card).
    #[default]
    Vertical,
    Horizontal,
}

/// Optional template settings; each template uses the ones that apply.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TemplateParams {
    /// Page preset (default: 12×12 mat; Letter for print & cut).
    #[serde(default)]
    pub preset: Option<PagePreset>,
    /// Item size: the open card (default 10 × 7 in) or the tag (default 2 × 3.5 in).
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

/// A dashed straight line from `a` to `b` as open subpaths of one path.
fn dashed(a: Point, b: Point) -> BezPath {
    let length = a.distance(b);
    let mut path = BezPath::new();
    let mut t = 0.0;
    // At most a few thousand dashes: length is capped by MAX_ITEM_MM.
    while t < length {
        let end = (t + FOLD_DASH_MM).min(length);
        path.move_to(a.lerp(b, t / length));
        path.line_to(a.lerp(b, end / length));
        t = end + FOLD_GAP_MM;
    }
    path
}

/// A new document for `template`. The board fits the item; shapes are centred on it, and
/// a card or tag's pieces are grouped. Canvas y grows downwards, so a tag's top is min y.
pub fn new_from_template(
    template: ProjectTemplate,
    params: &TemplateParams,
) -> Result<Document, DocError> {
    let default_preset = match template {
        ProjectTemplate::PrintAndCut => PagePreset::Letter,
        _ => PagePreset::Mat12x12,
    };
    let preset = params.preset.unwrap_or(default_preset);
    let (mut page_w, mut page_h) = preset
        .size_mm()
        .or(PagePreset::Mat12x12.size_mm())
        .unwrap_or((304.8, 304.8));
    let mut doc = Document::default();
    doc.meta.title = format!("{} project", template.label());
    doc.board_setup.mat = preset.mat_label().map(str::to_string);
    let item = match template {
        ProjectTemplate::Card => Some((
            size(params.width_mm, 10.0 * INCH_MM, "card width")?,
            size(params.height_mm, 7.0 * INCH_MM, "card height")?,
        )),
        ProjectTemplate::GiftTag => Some((
            size(params.width_mm, 2.0 * INCH_MM, "tag width")?,
            size(params.height_mm, 3.5 * INCH_MM, "tag height")?,
        )),
        _ => None,
    };
    if let Some((w, h)) = item {
        // Grow the page (with a 1 in margin) when the item does not fit it.
        if w > page_w || h > page_h {
            page_w = page_w.max(w + INCH_MM);
            page_h = page_h.max(h + INCH_MM);
            doc.board_setup.mat = None;
        }
    }
    doc.board.width_mm = page_w;
    doc.board.height_mm = page_h;
    let (cx, cy) = (page_w / 2.0, page_h / 2.0);
    match template {
        ProjectTemplate::Blank => {}
        ProjectTemplate::Card | ProjectTemplate::GiftTag => {
            let Some((w, h)) = item else {
                return Err(DocError::Model("template: missing item size".to_string()));
            };
            let (x0, y0, x1, y1) = (cx - w / 2.0, cy - h / 2.0, cx + w / 2.0, cy + h / 2.0);
            let mut members = Vec::new();
            if template == ProjectTemplate::Card {
                let outline = polygon(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)]);
                members.push(doc.push_shape(0, outline, CutStyle::Cut)?.id);
                let (a, b) = match params.fold.unwrap_or_default() {
                    FoldAxis::Vertical => (Point::new(cx, y0), Point::new(cx, y1)),
                    FoldAxis::Horizontal => (Point::new(x0, cy), Point::new(x1, cy)),
                };
                doc.layers.push(Layer::new("score", CutStyle::Engrave));
                members.push(doc.push_shape(1, dashed(a, b), CutStyle::Engrave)?.id);
                doc.meta.notes = "Cut the outline, score the dashed fold line.".to_string();
            } else {
                let hole = size(params.hole_mm, 0.25 * INCH_MM, "hole diameter")?;
                let chamfer = (w.min(h) / 4.0).max(0.0);
                if hole >= w / 2.0 || hole + chamfer >= h / 2.0 {
                    return Err(DocError::Model(
                        "template: the hole is too large for the tag".to_string(),
                    ));
                }
                let outline = polygon(&[
                    (x0 + chamfer, y0),
                    (x1 - chamfer, y0),
                    (x1, y0 + chamfer),
                    (x1, y1),
                    (x0, y1),
                    (x0, y0 + chamfer),
                ]);
                members.push(doc.push_shape(0, outline, CutStyle::Cut)?.id);
                let center = Point::new(cx, y0 + chamfer.max(hole));
                let circle = kurbo::Circle::new(center, hole / 2.0).to_path(0.01);
                members.push(doc.push_shape(0, circle, CutStyle::Cut)?.id);
            }
            doc.groups.push(Group {
                id: 1,
                name: template.label().to_string(),
                children: members.into_iter().map(GroupChild::Shape).collect(),
            });
        }
        ProjectTemplate::Emboss
        | ProjectTemplate::Sketch
        | ProjectTemplate::Stamp
        | ProjectTemplate::Stipple
        | ProjectTemplate::PrintAndCut => {
            let (name, kind, notes) = match template {
                ProjectTemplate::Emboss => (
                    "emboss",
                    CutStyle::Engrave,
                    "Emboss: shapes on this layer are pressed, not cut.",
                ),
                ProjectTemplate::Sketch => (
                    "sketch",
                    CutStyle::Engrave,
                    "Sketch: shapes on this layer are drawn with a pen.",
                ),
                ProjectTemplate::Stamp => (
                    "stamp",
                    CutStyle::Cut,
                    "Stamp: mirror the design before cutting the stamp material.",
                ),
                ProjectTemplate::Stipple => (
                    "stipple",
                    CutStyle::Engrave,
                    "Stipple: shapes on this layer are marked as dots.",
                ),
                _ => (
                    "cut contour",
                    CutStyle::Cut,
                    "Print & Cut: export with registration marks (print-cut) and cut the contour.",
                ),
            };
            doc.layers = vec![Layer::new(name, kind)];
            doc.meta.notes = notes.to_string();
        }
    }
    doc.validate()?;
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_template_builds_and_round_trips() {
        for template in ProjectTemplate::ALL {
            let doc = new_from_template(template, &TemplateParams::default()).unwrap();
            let back = Document::load_json(&doc.save_json().unwrap()).unwrap();
            assert_eq!(back, doc, "{template:?}");
        }
    }

    #[test]
    fn card_is_a_grouped_outline_with_a_dashed_score_fold() {
        let doc = new_from_template(ProjectTemplate::Card, &TemplateParams::default()).unwrap();
        assert!((doc.board.width_mm - 304.8).abs() < 1e-9);
        let outline = doc.layers[0].shapes[0].bounds().unwrap();
        assert!((outline.width() - 254.0).abs() < 1e-9 && (outline.height() - 177.8).abs() < 1e-9);
        let fold = &doc.layers[1].shapes[0];
        assert_eq!(fold.style, CutStyle::Engrave);
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
    fn gift_tag_has_a_hole_and_large_items_grow_the_page() {
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
    fn starters_set_the_layer_operation() {
        let doc = new_from_template(ProjectTemplate::Sketch, &TemplateParams::default()).unwrap();
        assert_eq!((doc.layers[0].name.as_str(), doc.layers[0].kind), ("sketch", CutStyle::Engrave));
        let doc =
            new_from_template(ProjectTemplate::PrintAndCut, &TemplateParams::default()).unwrap();
        assert!((doc.board.width_mm - 215.9).abs() < 1e-9);
    }
}
