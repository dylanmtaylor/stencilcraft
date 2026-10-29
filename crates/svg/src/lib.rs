//! SVG import/export for StencilCraft documents.
//!
//! Export: one `<g>` per layer with `data-kind` recording cut/engrave; the viewBox
//! is millimetres, so 1 SVG user unit = 1 mm and the sheet prints true to size.
//! Import: minimal — bare `<path d="…">` elements only (no groups, transforms,
//! styles; see `docs/gaps.md` 1). kurbo's own SVG path grammar handles `d`.

use std::fmt::Write as _;

use kurbo::BezPath;
use stencilcraft_doc::{CutStyle, Document, Layer};
use stencilcraft_geom::{Polyline, bounds_of_polylines, path_to_polylines};

#[derive(Debug, thiserror::Error)]
pub enum SvgError {
    #[error("svg export: failure to build markup: {0}")]
    Write(String),
    #[error("svg import: {0}")]
    Import(String),
    #[error("svg: {0}")]
    Model(String),
}

/// SVG class names, one per production style, stable for downstream tools that
/// colour cut paths by class.
fn class_for(style: CutStyle) -> &'static str {
    match style {
        CutStyle::Cut => "stencilcraft-cut",
        CutStyle::Engrave => "stencilcraft-engrave",
    }
}

/// Board and art bounds combined, defaulting to a 100 × 150 mm sheet for an empty
/// document (the export must always be plottable in a viewbox).
fn view_bounds(doc: &Document) -> kurbo::Rect {
    let mut acc = kurbo::Rect::new(0.0, 0.0, doc.board.width_mm, doc.board.height_mm);
    for layer in &doc.layers {
        if let Some(b) = layer.bounds() {
            acc = acc.union(b);
        }
    }
    acc
}

/// Serialise a document to SVG, millimetres as user units, one `<g>` per layer
/// (hidden layers included but marked invisible).
pub fn export(doc: &Document) -> Result<String, SvgError> {
    let board = view_bounds(doc);
    let mut svg = String::with_capacity(1024);
    svg.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    svg.push_str("<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" ");
    let _ = write!(
        svg,
        "width=\"{:.3}mm\" height=\"{:.3}mm\" viewBox=\"{:.3} {:.3} {:.3} {:.3}\"",
        board.width(),
        board.height(),
        board.x0,
        board.y0,
        board.width(),
        board.height()
    );
    svg.push('\n');
    let title = svg_title(doc);
    svg.push_str(&title);
    for layer in &doc.layers {
        let vis = if layer.visible { "visible" } else { "hidden" };
        let _ = writeln!(
            svg,
            "  <g class=\"{}\" data-kind=\"{}\" style=\"display:{}\">",
            class_for(layer.kind),
            match layer.kind {
                CutStyle::Cut => "cut",
                CutStyle::Engrave => "engrave",
            },
            vis
        );
        for shape in &layer.shapes {
            let d = path_to_svg_d(&shape.path)?;
            let _ = writeln!(
                svg,
                "    <path class=\"{}\" d=\"{}\"/>",
                class_for(shape.style),
                d
            );
        }
        svg.push_str("  </g>\n");
    }
    svg.push_str("</svg>\n");
    Ok(svg)
}

fn svg_title(doc: &Document) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "<title>stencilcraft {}: {} layers, {} shapes, kerf {:.3} mm</title>",
        stencilcraft_doc::FORMAT_NAME,
        doc.layers.len(),
        doc.shape_count(),
        doc.kerf.width_mm
    );
    s
}

/// kurbo's SVG grammar on a BezPath escapes attribute-quoting issues: no quotes
/// appear in path data. Guarded anyway against a future kurbo grammar change.
fn path_to_svg_d(path: &BezPath) -> Result<String, SvgError> {
    let s = path.to_svg();
    if s.contains('"') || s.contains('<') {
        return Err(SvgError::Write(format!(
            "path data has markup characters: {s}"
        )));
    }
    Ok(s)
}

/// Import the minimal SVG subset: every `<path>` whose `d` parses, in document
/// order, filed onto layers by production style. Groups are skipped (their
/// geometry meaning here is none — transforms are rejected above, so skipping
/// them cannot silently misread art); other SVG features are not supported yet
/// (see `docs/gaps.md` 1).
pub fn import(svg: &str, path_name: &str) -> Result<Document, SvgError> {
    if svg.contains("transform") {
        return Err(SvgError::Import(pathname_msg(
            path_name,
            "transforms are not supported yet; bake them down first",
        )));
    }
    let mut doc = Document::default();
    let mut cut_layer = -1_i64;
    let mut engrave_layer = -1_i64;
    // Walk the markup tag by tag: <path> elements are read, every other tag
    // (<g>, </g>, <svg>, comments, whitespace) is skipped. Transforms anywhere
    // were already rejected above, so skipping groups cannot silently misread
    // geometry.
    let mut rest = svg;
    while let Some(start) = rest.find('<') {
        let Some(end) = rest[start..].find('>') else {
            return Err(SvgError::Import(pathname_msg(
                path_name,
                "unterminated tag",
            )));
        };
        let tag = rest[start + 1..start + end].trim_end_matches('/').trim();
        rest = &rest[start + end + 1..];
        let tag_name = tag.split_whitespace().next().unwrap_or("");
        if tag_name != "path" {
            continue;
        }
        let Some(d) = attr_value(tag, "d") else {
            return Err(SvgError::Import(pathname_msg(
                path_name,
                "<path> has no d attribute",
            )));
        };
        let bez = BezPath::from_svg(d)
            .map_err(|e| SvgError::Import(format!("{path_name}: bad path data: {e}")))?;
        let style = class_of(tag).unwrap_or(CutStyle::Cut);
        let layer_index = match style {
            CutStyle::Cut => {
                // Document::default() already opens with a cut layer: use it.
                if cut_layer < 0 {
                    cut_layer = 0;
                }
                cut_layer as usize
            }
            CutStyle::Engrave => {
                if engrave_layer < 0 {
                    engrave_layer = doc.layers.len() as i64;
                    doc.layers.push(Layer::new("engrave", CutStyle::Engrave));
                }
                engrave_layer as usize
            }
        };
        // A valid import must be plottable relative to the board: enlarge the
        // board when art lands outside it rather than clipping silently.
        if let Some(b) = bounds_of_polylines(&Polyline::from_path(&bez)) {
            doc.board.width_mm = doc.board.width_mm.max(b.x1);
            doc.board.height_mm = doc.board.height_mm.max(b.y1);
        }
        doc.push_shape(layer_index, bez, style)
            .map_err(|e| SvgError::Model(e.to_string()))?;
    }
    Ok(doc)
}

fn class_of(tag: &str) -> Option<CutStyle> {
    let class = attr_value(tag, "class")?;
    match class {
        c if c.contains("stencilcraft-cut") => Some(CutStyle::Cut),
        c if c.contains("stencilcraft-engrave") => Some(CutStyle::Engrave),
        _ => Some(CutStyle::Cut),
    }
}

fn attr_value<'a>(tag: &'a str, attr: &str) -> Option<&'a str> {
    for quote in ['"', '\''] {
        let mut from = 0;
        while let Some(rel) = tag[from..].find(attr) {
            let at = from + rel;
            from = at + attr.len();
            // Attribute names must start after whitespace (or the tag opening)
            // so `width` will not match inside `stroke-width`.
            if !(at == 0 || tag[..at].ends_with(char::is_whitespace)) {
                continue;
            }
            let Some(rest) = tag[from..].strip_prefix('=') else {
                continue;
            };
            let Some(opened) = rest.strip_prefix(quote) else {
                continue;
            };
            let Some(end) = opened.find(quote) else {
                return None; // unterminated value
            };
            return Some(&opened[..end]);
        }
    }
    None
}

fn pathname_msg(path: &str, msg: &str) -> String {
    format!("{path}: {msg}")
}

#[cfg(test)]
mod tests_group_walk {
    use super::*;

    #[test]
    fn groups_are_skipped_not_rejected() {
        let doc = import(
            r#"<svg><g class="stencilcraft-cut"><path d="M 0 0 L 5 0 L 5 5 Z"/></g></svg>"#,
            "grouped.svg",
        )
        .unwrap();
        assert_eq!(doc.shape_count(), 1);
    }
}

/// Rebuilds into polylines for downstream code needing points instead of `d`.
pub fn polylines_of(bez: &BezPath) -> Vec<Polyline> {
    path_to_polylines(bez, stencilcraft_geom::DEFAULT_FLATTEN_STEPS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use stencilcraft_geom::Point;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(x, y));
        p.line_to(Point::new(x + w, y));
        p.line_to(Point::new(x + w, y + h));
        p.line_to(Point::new(x, y + h));
        p.close_path();
        p
    }

    #[test]
    fn export_includes_board_size_in_viewbox() {
        let doc = Document::default();
        let svg = export(&doc).unwrap();
        assert!(
            svg.contains("viewBox=\"0.000 0.000 100.000 150.000\""),
            "{svg}"
        );
        assert!(svg.contains("width=\"100.000mm\""));
        assert!(svg.contains("data-kind=\"cut\""));
    }

    #[test]
    fn export_shapes_as_paths() {
        let mut doc = Document::default();
        doc.push_shape(0, rect(10.0, 10.0, 20.0, 20.0), CutStyle::Cut)
            .unwrap();
        let svg = export(&doc).unwrap();
        assert!(svg.contains("d=\"M10"), "{svg}");
        assert!(svg.contains("class=\"stencilcraft-cut\""));
    }

    #[test]
    fn import_roundtrips_path_data() {
        let mut doc = Document::default();
        doc.push_shape(0, rect(0.0, 0.0, 30.0, 40.0), CutStyle::Cut)
            .unwrap();
        let svg = export(&doc).unwrap();
        let back = import(&svg, "test.svg").unwrap();
        assert_eq!(back.shape_count(), 1);
        assert_eq!(
            back.layers[0].shapes[0].path.to_svg(),
            doc.layers[0].shapes[0].path.to_svg()
        );
    }

    #[test]
    fn import_gives_calm_errors() {
        assert!(matches!(
            import("<path></path>", "nod.svg"),
            Err(SvgError::Import(ref m)) if m.contains("d attribute")
        ));
        assert!(matches!(
            import("<path d=\"M 0 0 Q\"/>", "bad.svg"),
            Err(SvgError::Import(_))
        ));
        assert!(matches!(
            import("<g transform=\"scale(2)\"><path d=\"M0 0 L1 0 Z\"/></g>", "tx.svg"),
            Err(SvgError::Import(ref m)) if m.contains("transform")
        ));
    }

    #[test]
    fn import_grows_board_for_outside_art() {
        let back = import("<path d=\"M 200 250 L 210 250 L 210 260 Z\"/>", "big.svg").unwrap();
        assert!(back.board.width_mm >= 210.0);
        assert!(back.board.height_mm >= 260.0);
    }
}
