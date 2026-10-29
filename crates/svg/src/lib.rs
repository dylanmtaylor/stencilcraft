//! SVG import/export for StencilCraft documents.
//!
//! Export: one `<g>` per layer with `data-kind` recording cut/engrave; the viewBox
//! is millimetres, so 1 SVG user unit = 1 mm and the sheet prints true to size.
//! Import supports common SVG shapes, groups, transforms, production styles, and
//! local `<use>` references; see `docs/gaps.md` for remaining gaps.

use std::fmt::Write as _;

use kurbo::BezPath;
use stencilcraft_doc::{CutStyle, Document, Layer};
use stencilcraft_geom::{Polyline, path_to_polylines};

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
    svg.push_str(">\n");
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

const MAX_SVG_BYTES: usize = 16 * 1024 * 1024;
const MAX_SVG_NODES: usize = 100_000;
const MAX_SVG_DEPTH: usize = 128;

#[derive(Clone, Copy)]
struct ImportState {
    transform: kurbo::Affine,
    kind: CutStyle,
    display_none: bool,
    visibility_hidden: bool,
}

impl Default for ImportState {
    fn default() -> Self {
        Self {
            transform: kurbo::Affine::IDENTITY,
            kind: CutStyle::Cut,
            display_none: false,
            visibility_hidden: false,
        }
    }
}

/// Import common SVG artwork into a cutting document.
///
/// Supported geometry: `<path>`, `<rect>` (including rounded corners),
/// `<circle>`, `<ellipse>`, `<line>`, `<polyline>`, and `<polygon>`. Nested
/// groups, transforms (`matrix`, `translate`, `scale`, `rotate`, `skewX/Y`),
/// and inherited presentation/inline styles (`display`, `visibility`, CSS
/// `style`, `class`, `data-kind`) are applied. SVG paths are flattened nowhere
/// here: curves remain curves in the document. Each imported shape is assigned
/// to cut or engrave according to inherited `data-kind`/`data-style` or class.
/// Stroke/fill colors are not represented by the current document model.
///
/// Physical root dimensions accept unitless SVG px, `px`, `mm`, `cm`, `in`,
/// `pt`, and `pc`; a `viewBox` maps its user coordinates onto that physical
/// viewport, following `preserveAspectRatio`. Without physical dimensions,
/// `viewBox` units are interpreted as millimetres to preserve StencilCraft's
/// 1-unit = 1-mm convention. A fragment without an SVG root uses that same
/// convention. Local fragment-only `<use>` references can instantiate supported
/// shapes and groups; external references, symbols, text, CSS selectors/stylesheets,
/// gradients, clipping, and percentage lengths are not expanded. Unsupported
/// drawable elements are reported rather than silently lost.
pub fn import(svg: &str, path_name: &str) -> Result<Document, SvgError> {
    if svg.len() > MAX_SVG_BYTES {
        return Err(svg_error(path_name, "SVG exceeds the 16 MiB import limit"));
    }
    let xml = roxmltree::Document::parse(svg)
        .map_err(|e| svg_error(path_name, &format!("invalid XML: {e}")))?;
    let root = xml.root_element();
    let (width_mm, height_mm, viewport) = viewport_transform(root, path_name)?;

    let mut doc = Document::default();
    doc.board.width_mm = width_mm;
    doc.board.height_mm = height_mm;
    let mut id_index = std::collections::HashMap::new();
    let mut xml_node_count = 0usize;
    for node in root.descendants() {
        xml_node_count += 1;
        if xml_node_count > MAX_SVG_NODES {
            return Err(svg_error(
                path_name,
                "SVG exceeds the 100,000-node import limit",
            ));
        }
        if let Some(id) = node.attribute("id")
            && id_index.insert(id.to_string(), node).is_some()
        {
            return Err(svg_error(path_name, &format!("duplicate SVG id {id:?}")));
        }
    }
    let mut context = ImportContext {
        visited: 0,
        path_name,
        id_index,
        reference_stack: Vec::new(),
        doc: &mut doc,
        engrave_layer: None,
    };
    let base = ImportState {
        transform: viewport,
        ..ImportState::default()
    };
    import_node(root, base, 0, &mut context)?;

    // SVG art may legitimately extend beyond its nominal viewport; do not clip
    // cut geometry silently. Normalize negative coordinates onto the sheet.
    let mut bounds: Option<kurbo::Rect> = None;
    for layer in &doc.layers {
        if let Some(b) = layer.bounds() {
            bounds = Some(bounds.map_or(b, |prev| prev.union(b)));
        }
    }
    if let Some(b) = bounds {
        let shift_x = (-b.x0).max(0.0);
        let shift_y = (-b.y0).max(0.0);
        if shift_x != 0.0 || shift_y != 0.0 {
            let shift = kurbo::Affine::translate((shift_x, shift_y));
            for layer in &mut doc.layers {
                for shape in &mut layer.shapes {
                    shape.path = transform_path(&shape.path, shift);
                }
            }
        }
        doc.board.width_mm = doc.board.width_mm.max(b.x1 + shift_x).max(1.0);
        doc.board.height_mm = doc.board.height_mm.max(b.y1 + shift_y).max(1.0);
    }
    doc.validate()
        .map_err(|e| svg_error(path_name, &format!("invalid imported document: {e}")))?;
    Ok(doc)
}

struct ImportContext<'a, 'input, 'doc> {
    visited: usize,
    path_name: &'a str,
    id_index: std::collections::HashMap<String, roxmltree::Node<'a, 'input>>,
    reference_stack: Vec<String>,
    doc: &'doc mut Document,
    engrave_layer: Option<usize>,
}

fn import_node<'a, 'input>(
    node: roxmltree::Node<'a, 'input>,
    inherited: ImportState,
    depth: usize,
    context: &mut ImportContext<'a, 'input, '_>,
) -> Result<(), SvgError> {
    context.visited = context
        .visited
        .checked_add(1)
        .ok_or_else(|| svg_error(context.path_name, "SVG node counter overflow"))?;
    if context.visited > MAX_SVG_NODES {
        return Err(svg_error(
            context.path_name,
            "SVG exceeds the 100,000-node import limit",
        ));
    }
    if depth > MAX_SVG_DEPTH {
        return Err(svg_error(
            context.path_name,
            "SVG nesting exceeds 128 levels",
        ));
    }
    let tag = node.tag_name().name();
    // Definitions are not drawable content unless instantiated via `<use>`;
    // references resolve directly into supported targets inside these containers.
    if matches!(tag, "defs" | "symbol" | "clipPath" | "mask" | "pattern") {
        return Ok(());
    }
    let mut state = inherited;
    // SVG presentation attributes and inline declarations inherit. `display`
    // is different: a descendant cannot override an ancestor's display:none.
    if node.attribute("display") == Some("none") {
        state.display_none = true;
    }
    if let Some(transform) = node.attribute("transform") {
        let local = parse_transform(transform)
            .map_err(|e| svg_error(context.path_name, &format!("<{}> transform: {e}", tag)))?;
        state.transform = compose(inherited.transform, local);
    }
    if let Some(kind) = style_kind(node) {
        state.kind = kind;
    }
    let style = style_properties(node);
    if style.get("display").is_some_and(|v| *v == "none") {
        state.display_none = true;
    }
    if let Some(visibility) = style.get("visibility") {
        state.visibility_hidden = *visibility == "hidden" || *visibility == "collapse";
    }
    let color_property = style.get("stroke").or_else(|| style.get("fill"));
    if state.kind == CutStyle::Cut && color_property.is_some_and(|v| *v == "none") {
        // A line with fill:none can still be a cut path; stroke:none plus
        // fill:none is empty and can safely be ignored.
        if style.get("stroke").is_some_and(|v| *v == "none")
            && style.get("fill").is_some_and(|v| *v == "none")
        {
            state.display_none = true;
        }
    }
    if tag == "use" {
        if !state.display_none {
            let href = node
                .attribute("href")
                .or_else(|| node.attribute(("http://www.w3.org/1999/xlink", "href")))
                .ok_or_else(|| svg_error(context.path_name, "<use> has no href reference"))?;
            let id = href
                .strip_prefix('#')
                .filter(|id| !id.is_empty())
                .ok_or_else(|| {
                    svg_error(
                        context.path_name,
                        "<use> supports only local fragment references",
                    )
                })?;
            if context.reference_stack.iter().any(|active| active == id) {
                return Err(svg_error(
                    context.path_name,
                    &format!("cyclic <use> reference to #{id}"),
                ));
            }
            let target = context.id_index.get(id).copied().ok_or_else(|| {
                svg_error(
                    context.path_name,
                    &format!("<use> references missing id #{id}"),
                )
            })?;
            if !matches!(
                target.tag_name().name(),
                "g" | "path" | "rect" | "circle" | "ellipse" | "line" | "polyline" | "polygon"
            ) {
                return Err(svg_error(
                    context.path_name,
                    &format!(
                        "<use> reference #{id} targets unsupported <{}>",
                        target.tag_name().name()
                    ),
                ));
            }
            let x = match node.attribute("x") {
                Some(value) => parse_number(value).ok_or_else(|| {
                    svg_error(context.path_name, "<use> has invalid x coordinate")
                })?,
                None => 0.0,
            };
            let y = match node.attribute("y") {
                Some(value) => parse_number(value).ok_or_else(|| {
                    svg_error(context.path_name, "<use> has invalid y coordinate")
                })?,
                None => 0.0,
            };
            if x.abs() > 1.0e9 || y.abs() > 1.0e9 {
                return Err(svg_error(
                    context.path_name,
                    "<use> coordinates exceed ±1e9",
                ));
            }
            state.transform = compose(state.transform, kurbo::Affine::translate((x, y)));
            context.reference_stack.push(id.to_string());
            let result = import_node(target, state, depth + 1, context);
            context.reference_stack.pop();
            return result;
        }
        return Ok(());
    }
    if !state.display_none
        && !state.visibility_hidden
        && let Some(path) = geometry_for(node, context.path_name)?
    {
        let transformed = transform_path(&path, state.transform);
        validate_import_path(&transformed, context.path_name)?;
        let index = match state.kind {
            CutStyle::Cut => 0,
            CutStyle::Engrave => match context.engrave_layer {
                Some(i) => i,
                None => {
                    let i = context.doc.layers.len();
                    context
                        .doc
                        .layers
                        .push(Layer::new("engrave", CutStyle::Engrave));
                    context.engrave_layer = Some(i);
                    i
                }
            },
        };
        context
            .doc
            .push_shape(index, transformed, state.kind)
            .map_err(|e| svg_error(context.path_name, &e.to_string()))?;
    }
    for child in node.children().filter(|n| n.is_element()) {
        import_node(child, state, depth + 1, context)?;
    }
    Ok(())
}

fn geometry_for(
    node: roxmltree::Node<'_, '_>,
    path_name: &str,
) -> Result<Option<BezPath>, SvgError> {
    let tag = node.tag_name().name();
    let number = |name: &str, default: f64| -> Result<f64, SvgError> {
        match node.attribute(name) {
            Some(value) => parse_number(value).ok_or_else(|| {
                svg_error(path_name, &format!("<{tag}> has invalid {name}={value:?}"))
            }),
            None => Ok(default),
        }
    };
    let path = match tag {
        "svg" | "g" | "defs" | "symbol" | "clipPath" | "mask" | "pattern" | "title" | "desc"
        | "metadata" | "style" | "stop" | "namedview" | "guide" | "perspective" => return Ok(None),
        "path" => {
            let d = node
                .attribute("d")
                .ok_or_else(|| svg_error(path_name, "<path> has no d attribute"))?;
            BezPath::from_svg(d)
                .map_err(|e| svg_error(path_name, &format!("bad path data: {e}")))?
        }
        "rect" => {
            let x = number("x", 0.0)?;
            let y = number("y", 0.0)?;
            let w = number("width", 0.0)?;
            let h = number("height", 0.0)?;
            if w < 0.0 || h < 0.0 {
                return Err(svg_error(
                    path_name,
                    "<rect> width/height cannot be negative",
                ));
            }
            let mut rx = number("rx", number("ry", 0.0)?)?;
            let mut ry = number("ry", rx)?;
            if rx < 0.0 || ry < 0.0 {
                return Err(svg_error(
                    path_name,
                    "<rect> corner radii cannot be negative",
                ));
            }
            if rx == 0.0 {
                rx = ry;
            }
            if ry == 0.0 {
                ry = rx;
            }
            rounded_rect(x, y, w, h, rx.min(w / 2.0), ry.min(h / 2.0))
        }
        "circle" => {
            let cx = number("cx", 0.0)?;
            let cy = number("cy", 0.0)?;
            let r = number("r", 0.0)?;
            if r < 0.0 {
                return Err(svg_error(path_name, "<circle> radius cannot be negative"));
            }
            ellipse_path(cx, cy, r, r)
        }
        "ellipse" => {
            let cx = number("cx", 0.0)?;
            let cy = number("cy", 0.0)?;
            let rx = number("rx", 0.0)?;
            let ry = number("ry", 0.0)?;
            if rx < 0.0 || ry < 0.0 {
                return Err(svg_error(path_name, "<ellipse> radii cannot be negative"));
            }
            ellipse_path(cx, cy, rx, ry)
        }
        "line" => {
            let mut p = BezPath::new();
            p.move_to(kurbo::Point::new(number("x1", 0.0)?, number("y1", 0.0)?));
            p.line_to(kurbo::Point::new(number("x2", 0.0)?, number("y2", 0.0)?));
            p
        }
        "polyline" | "polygon" => {
            let points = node
                .attribute("points")
                .ok_or_else(|| svg_error(path_name, &format!("<{tag}> has no points attribute")))?;
            points_path(points, tag == "polygon", path_name)?
        }
        "text" | "tspan" | "image" | "foreignObject" => {
            return Err(svg_error(
                path_name,
                &format!("<{tag}> is not supported yet"),
            ));
        }
        name if is_supported_non_drawable(name) => return Ok(None),
        _ => {
            return Err(svg_error(
                path_name,
                &format!(
                    "unsupported SVG element <{tag}>; import stopped to avoid silently dropping art"
                ),
            ));
        }
    };
    Ok(Some(path))
}

fn is_supported_non_drawable(tag: &str) -> bool {
    matches!(
        tag,
        "a" | "switch"
            | "marker"
            | "linearGradient"
            | "radialGradient"
            | "filter"
            | "feGaussianBlur"
            | "feOffset"
            | "feBlend"
            | "feColorMatrix"
            | "feComponentTransfer"
            | "feComposite"
            | "feConvolveMatrix"
            | "feDiffuseLighting"
            | "feDisplacementMap"
            | "feDistantLight"
            | "feDropShadow"
            | "feFlood"
            | "feFuncA"
            | "feFuncB"
            | "feFuncG"
            | "feFuncR"
            | "feImage"
            | "feMerge"
            | "feMergeNode"
            | "feMorphology"
            | "fePointLight"
            | "feSpecularLighting"
            | "feSpotLight"
            | "feTile"
            | "feTurbulence"
            | "animate"
            | "animateMotion"
            | "animateTransform"
            | "set"
            | "view"
            | "cursor"
            | "color-profile"
            | "font"
            | "font-face"
            | "glyph"
            | "missing-glyph"
    )
}

fn points_path(points: &str, closed: bool, path_name: &str) -> Result<BezPath, SvgError> {
    let values =
        parse_number_list(points).ok_or_else(|| svg_error(path_name, "invalid points list"))?;
    if values.len() % 2 != 0 {
        return Err(svg_error(
            path_name,
            "points list must contain coordinate pairs",
        ));
    }
    if values.len() / 2 > 500_000 {
        return Err(svg_error(
            path_name,
            "a polygon exceeds the 500,000-point limit",
        ));
    }
    let mut path = BezPath::new();
    let mut pairs = values.chunks_exact(2);
    if let Some(first) = pairs.next() {
        path.move_to(kurbo::Point::new(first[0], first[1]));
        for pair in pairs {
            path.line_to(kurbo::Point::new(pair[0], pair[1]));
        }
        if closed {
            path.close_path();
        }
    }
    Ok(path)
}

fn rounded_rect(x: f64, y: f64, w: f64, h: f64, rx: f64, ry: f64) -> BezPath {
    if rx <= 0.0 || ry <= 0.0 {
        let mut p = BezPath::new();
        p.move_to(kurbo::Point::new(x, y));
        p.line_to(kurbo::Point::new(x + w, y));
        p.line_to(kurbo::Point::new(x + w, y + h));
        p.line_to(kurbo::Point::new(x, y + h));
        p.close_path();
        return p;
    }
    let k = 0.552_284_749_830_793_6;
    let mut p = BezPath::new();
    p.move_to(kurbo::Point::new(x + rx, y));
    p.line_to(kurbo::Point::new(x + w - rx, y));
    p.curve_to(
        kurbo::Point::new(x + w - rx + k * rx, y),
        kurbo::Point::new(x + w, y + ry - k * ry),
        kurbo::Point::new(x + w, y + ry),
    );
    p.line_to(kurbo::Point::new(x + w, y + h - ry));
    p.curve_to(
        kurbo::Point::new(x + w, y + h - ry + k * ry),
        kurbo::Point::new(x + w - rx + k * rx, y + h),
        kurbo::Point::new(x + w - rx, y + h),
    );
    p.line_to(kurbo::Point::new(x + rx, y + h));
    p.curve_to(
        kurbo::Point::new(x + rx - k * rx, y + h),
        kurbo::Point::new(x, y + h - ry + k * ry),
        kurbo::Point::new(x, y + h - ry),
    );
    p.line_to(kurbo::Point::new(x, y + ry));
    p.curve_to(
        kurbo::Point::new(x, y + ry - k * ry),
        kurbo::Point::new(x + rx - k * rx, y),
        kurbo::Point::new(x + rx, y),
    );
    p.close_path();
    p
}

fn ellipse_path(cx: f64, cy: f64, rx: f64, ry: f64) -> BezPath {
    let k = 0.552_284_749_830_793_6;
    let mut p = BezPath::new();
    p.move_to(kurbo::Point::new(cx + rx, cy));
    p.curve_to(
        kurbo::Point::new(cx + rx, cy + k * ry),
        kurbo::Point::new(cx + k * rx, cy + ry),
        kurbo::Point::new(cx, cy + ry),
    );
    p.curve_to(
        kurbo::Point::new(cx - k * rx, cy + ry),
        kurbo::Point::new(cx - rx, cy + k * ry),
        kurbo::Point::new(cx - rx, cy),
    );
    p.curve_to(
        kurbo::Point::new(cx - rx, cy - k * ry),
        kurbo::Point::new(cx - k * rx, cy - ry),
        kurbo::Point::new(cx, cy - ry),
    );
    p.curve_to(
        kurbo::Point::new(cx + k * rx, cy - ry),
        kurbo::Point::new(cx + rx, cy - k * ry),
        kurbo::Point::new(cx + rx, cy),
    );
    p.close_path();
    p
}

fn style_properties<'a, 'input: 'a>(
    node: roxmltree::Node<'a, 'input>,
) -> std::collections::HashMap<&'a str, &'a str> {
    let mut props = std::collections::HashMap::new();
    for key in [
        "display",
        "visibility",
        "data-kind",
        "data-style",
        "fill",
        "stroke",
        "stroke-width",
    ] {
        if let Some(value) = node.attribute(key) {
            props.insert(key, value.trim());
        }
    }
    if let Some(style) = node.attribute("style") {
        for item in style.split(';') {
            if let Some((key, value)) = item.split_once(':') {
                props.insert(key.trim(), value.trim());
            }
        }
    }
    props
}

fn style_kind(node: roxmltree::Node<'_, '_>) -> Option<CutStyle> {
    let props = style_properties(node);
    let explicit = props.get("data-kind").or_else(|| props.get("data-style"));
    if let Some(value) = explicit {
        match *value {
            "cut" => return Some(CutStyle::Cut),
            "engrave" | "score" | "mark" => return Some(CutStyle::Engrave),
            _ => {}
        }
    }
    let class = node.attribute("class").unwrap_or("").to_ascii_lowercase();
    if class
        .split_ascii_whitespace()
        .any(|c| c.contains("engrave") || c.contains("score"))
    {
        Some(CutStyle::Engrave)
    } else if class.split_ascii_whitespace().any(|c| c.contains("cut")) {
        Some(CutStyle::Cut)
    } else {
        None
    }
}

fn viewport_transform(
    root: roxmltree::Node<'_, '_>,
    path_name: &str,
) -> Result<(f64, f64, kurbo::Affine), SvgError> {
    let view_box = match root.attribute("viewBox") {
        Some(raw) => {
            let values = parse_number_list(raw)
                .filter(|v| v.len() == 4)
                .ok_or_else(|| svg_error(path_name, "viewBox must contain four finite numbers"))?;
            if values[2] <= 0.0 || values[3] <= 0.0 {
                return Err(svg_error(
                    path_name,
                    "viewBox width and height must be positive",
                ));
            }
            Some((values[0], values[1], values[2], values[3]))
        }
        None => None,
    };
    let width_attr = root.attribute("width");
    let height_attr = root.attribute("height");
    let explicit_viewport = width_attr.is_some() || height_attr.is_some();
    let fallback = view_box
        .map(|(_, _, w, h)| (w, h))
        .unwrap_or((100.0, 150.0));
    let width = width_attr
        .map(svg_length_mm)
        .transpose()
        .map_err(|e| svg_error(path_name, &format!("root width: {e}")))?
        .unwrap_or(fallback.0);
    let height = height_attr
        .map(svg_length_mm)
        .transpose()
        .map_err(|e| svg_error(path_name, &format!("root height: {e}")))?
        .unwrap_or(fallback.1);
    if !width.is_finite()
        || !height.is_finite()
        || width <= 0.0
        || height <= 0.0
        || width > 1.0e9
        || height > 1.0e9
    {
        return Err(svg_error(
            path_name,
            "viewport dimensions must be positive, finite, and no larger than 1e9 mm",
        ));
    }
    let Some((min_x, min_y, vb_w, vb_h)) = view_box else {
        // Without viewBox, physical SVG user units are CSS px (96/in). For a
        // bare SVG fragment there is no viewport and paths remain mm-based.
        let sx = if explicit_viewport { 25.4 / 96.0 } else { 1.0 };
        let sy = sx;
        return Ok((width, height, kurbo::Affine::scale_non_uniform(sx, sy)));
    };
    let preserve = root
        .attribute("preserveAspectRatio")
        .unwrap_or("xMidYMid meet");
    let tokens: Vec<&str> = preserve.split_ascii_whitespace().collect();
    let align = tokens.first().copied().unwrap_or("xMidYMid");
    let mode = tokens.get(1).copied().unwrap_or("meet");
    if align == "none" {
        return Ok((
            width,
            height,
            kurbo::Affine::new([
                width / vb_w,
                0.0,
                0.0,
                height / vb_h,
                -min_x * width / vb_w,
                -min_y * height / vb_h,
            ]),
        ));
    }
    if !matches!(mode, "meet" | "slice") {
        return Err(svg_error(
            path_name,
            "preserveAspectRatio supports only meet, slice, or none",
        ));
    }
    let x_align = if align.contains("xMin") {
        0.0
    } else if align.contains("xMax") {
        1.0
    } else if align.contains("xMid") {
        0.5
    } else {
        return Err(svg_error(
            path_name,
            "unsupported preserveAspectRatio x alignment",
        ));
    };
    let y_align = if align.contains("YMin") {
        0.0
    } else if align.contains("YMax") {
        1.0
    } else if align.contains("YMid") {
        0.5
    } else {
        return Err(svg_error(
            path_name,
            "unsupported preserveAspectRatio y alignment",
        ));
    };
    let sx = width / vb_w;
    let sy = height / vb_h;
    let scale = if mode == "meet" {
        sx.min(sy)
    } else {
        sx.max(sy)
    };
    let tx = (width - vb_w * scale) * x_align - min_x * scale;
    let ty = (height - vb_h * scale) * y_align - min_y * scale;
    Ok((
        width,
        height,
        kurbo::Affine::new([scale, 0.0, 0.0, scale, tx, ty]),
    ))
}

fn svg_length_mm(raw: &str) -> Result<f64, String> {
    let raw = raw.trim();
    let split = raw
        .find(|c: char| !(c.is_ascii_digit() || matches!(c, '+' | '-' | '.' | 'e' | 'E')))
        .unwrap_or(raw.len());
    let value = raw[..split]
        .parse::<f64>()
        .map_err(|_| format!("invalid length {raw:?}"))?;
    if !value.is_finite() || value <= 0.0 {
        return Err(format!("length {raw:?} must be positive and finite"));
    }
    let unit = raw[split..].trim();
    let scale = match unit {
        "" | "px" => 25.4 / 96.0,
        "mm" => 1.0,
        "cm" => 10.0,
        "in" => 25.4,
        "pt" => 25.4 / 72.0,
        "pc" => 25.4 / 6.0,
        _ => return Err(format!("unsupported length unit {unit:?}")),
    };
    Ok(value * scale)
}

fn parse_number(raw: &str) -> Option<f64> {
    let value = raw.trim().parse::<f64>().ok()?;
    value.is_finite().then_some(value)
}

fn parse_number_list(raw: &str) -> Option<Vec<f64>> {
    let mut values = Vec::new();
    let mut token = String::new();
    let mut comma_pending = false;
    let mut saw_any = false;
    for ch in raw.chars() {
        if ch.is_ascii_whitespace() {
            if !token.is_empty() {
                values.push(parse_number(&token)?);
                token.clear();
                saw_any = true;
            }
        } else if ch == ',' {
            if !token.is_empty() {
                values.push(parse_number(&token)?);
                token.clear();
                saw_any = true;
                comma_pending = true;
            } else if !comma_pending && saw_any {
                // Whitespace before a comma is valid; consecutive commas or
                // a leading comma are not part of SVG's number-list grammar.
                comma_pending = true;
            } else {
                return None;
            }
        } else if (ch == '+' || ch == '-')
            && !token.is_empty()
            && !token.ends_with('e')
            && !token.ends_with('E')
        {
            values.push(parse_number(&token)?);
            token.clear();
            token.push(ch);
            saw_any = true;
            comma_pending = false;
        } else if ch.is_ascii_digit() || matches!(ch, '+' | '-' | '.' | 'e' | 'E') {
            token.push(ch);
            comma_pending = false;
        } else {
            return None;
        }
        if values.len() > 200_000 {
            return None;
        }
    }
    if comma_pending {
        return None;
    }
    if !token.is_empty() {
        values.push(parse_number(&token)?);
    }
    Some(values)
}

fn parse_transform(raw: &str) -> Result<kurbo::Affine, String> {
    let mut rest = raw.trim();
    let mut result = kurbo::Affine::IDENTITY;
    while !rest.is_empty() {
        rest = rest.trim_start_matches(|c: char| c.is_ascii_whitespace() || c == ',');
        if rest.is_empty() {
            break;
        }
        let open = rest
            .find('(')
            .ok_or_else(|| "expected '(' after transform name".to_string())?;
        let name = rest[..open].trim();
        let close = rest[open + 1..]
            .find(')')
            .map(|i| i + open + 1)
            .ok_or_else(|| "unterminated transform".to_string())?;
        let values = parse_number_list(&rest[open + 1..close])
            .ok_or_else(|| "transform has invalid numeric arguments".to_string())?;
        let local = match (name, values.as_slice()) {
            ("matrix", [a, b, c, d, e, f]) => kurbo::Affine::new([*a, *b, *c, *d, *e, *f]),
            ("translate", [x]) => kurbo::Affine::translate((*x, 0.0)),
            ("translate", [x, y]) => kurbo::Affine::translate((*x, *y)),
            ("scale", [s]) => kurbo::Affine::scale(*s),
            ("scale", [x, y]) => kurbo::Affine::scale_non_uniform(*x, *y),
            ("rotate", [deg]) => kurbo::Affine::rotate(deg.to_radians()),
            ("rotate", [deg, cx, cy]) => {
                let to = kurbo::Affine::translate((*cx, *cy));
                let rot = kurbo::Affine::rotate(deg.to_radians());
                let from = kurbo::Affine::translate((-*cx, -*cy));
                compose(compose(to, rot), from)
            }
            ("skewX", [deg]) => {
                kurbo::Affine::new([1.0, 0.0, deg.to_radians().tan(), 1.0, 0.0, 0.0])
            }
            ("skewY", [deg]) => {
                kurbo::Affine::new([1.0, deg.to_radians().tan(), 0.0, 1.0, 0.0, 0.0])
            }
            _ => return Err(format!("unsupported or malformed transform {name:?}")),
        };
        if !local.as_coeffs().iter().all(|v| v.is_finite()) {
            return Err("transform contains non-finite values".to_string());
        }
        result = compose(result, local);
        rest = &rest[close + 1..];
    }
    Ok(result)
}

/// Matrix composition `parent(local(point))` for kurbo's [a,b,c,d,e,f]
/// affine coefficient order.
fn compose(parent: kurbo::Affine, local: kurbo::Affine) -> kurbo::Affine {
    let [a, b, c, d, e, f] = parent.as_coeffs();
    let [g, h, i, j, k, l] = local.as_coeffs();
    kurbo::Affine::new([
        a * g + c * h,
        b * g + d * h,
        a * i + c * j,
        b * i + d * j,
        a * k + c * l + e,
        b * k + d * l + f,
    ])
}

fn transform_path(path: &BezPath, affine: kurbo::Affine) -> BezPath {
    let mut out = BezPath::new();
    for elem in path.elements() {
        match *elem {
            kurbo::PathEl::MoveTo(p) => out.move_to(affine * p),
            kurbo::PathEl::LineTo(p) => out.line_to(affine * p),
            kurbo::PathEl::QuadTo(c, p) => out.quad_to(affine * c, affine * p),
            kurbo::PathEl::CurveTo(c1, c2, p) => out.curve_to(affine * c1, affine * c2, affine * p),
            kurbo::PathEl::ClosePath => out.close_path(),
        }
    }
    out
}

fn validate_import_path(path: &BezPath, path_name: &str) -> Result<(), SvgError> {
    if path.elements().len() > 1_000_000 {
        return Err(svg_error(
            path_name,
            "a shape exceeds the 1,000,000-segment limit",
        ));
    }
    let valid = |p: kurbo::Point| {
        p.x.is_finite() && p.y.is_finite() && p.x.abs() <= 1.0e9 && p.y.abs() <= 1.0e9
    };
    for element in path.elements() {
        let safe = match *element {
            kurbo::PathEl::MoveTo(p) | kurbo::PathEl::LineTo(p) => valid(p),
            kurbo::PathEl::QuadTo(c, p) => valid(c) && valid(p),
            kurbo::PathEl::CurveTo(c1, c2, p) => valid(c1) && valid(c2) && valid(p),
            kurbo::PathEl::ClosePath => true,
        };
        if !safe {
            return Err(svg_error(
                path_name,
                "geometry coordinates must be finite and within ±1e9 mm",
            ));
        }
    }
    Ok(())
}

fn svg_error(path_name: &str, message: &str) -> SvgError {
    SvgError::Import(format!("{path_name}: {message}"))
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
        let transformed = import(
            "<g transform=\"scale(nope)\"><path d=\"M0 0 L1 0 Z\"/></g>",
            "tx.svg",
        );
        assert!(matches!(
            transformed,
            Err(SvgError::Import(ref m)) if m.contains("transform")
        ));
    }

    #[test]
    fn import_grows_board_for_outside_art() {
        let back = import("<path d=\"M 200 250 L 210 250 L 210 260 Z\"/>", "big.svg").unwrap();
        assert!(back.board.width_mm >= 210.0);
        assert!(back.board.height_mm >= 260.0);
    }

    #[test]
    fn imports_all_common_basic_shapes() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
          <rect x="1" y="2" width="10" height="8"/>
          <circle cx="20" cy="20" r="5"/>
          <ellipse cx="40" cy="20" rx="8" ry="4"/>
          <line x1="1" y1="30" x2="20" y2="30"/>
          <polyline points="1,40 10,45 20,40"/>
          <polygon points="30,40 40,40 35,50"/>
          <path d="M50 50 C55 45 60 55 65 50 Z"/>
        </svg>"#;
        let doc = import(svg, "shapes.svg").unwrap();
        assert_eq!(doc.shape_count(), 7);
        assert!(doc.layers[0].shapes[0].path.elements().len() >= 5);
        assert!(doc.layers[0].shapes[1].path.elements().len() >= 6);
    }

    #[test]
    fn imports_nested_group_transforms_and_inherited_production_style() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
          <g transform="translate(10,20)" data-kind="engrave">
            <g scale="2" transform="scale(2)">
              <rect x="1" y="2" width="3" height="4"/>
            </g>
          </g>
        </svg>"#;
        let doc = import(svg, "groups.svg").unwrap();
        assert_eq!(doc.shape_count(), 1);
        assert_eq!(doc.layers.len(), 2);
        assert_eq!(doc.layers[1].kind, CutStyle::Engrave);
        let b = doc.layers[1].shapes[0].bounds().unwrap();
        assert!((b.x0 - 12.0).abs() < 1e-9, "{b:?}");
        assert!((b.y0 - 24.0).abs() < 1e-9, "{b:?}");
        assert!((b.width() - 6.0).abs() < 1e-9, "{b:?}");
    }

    #[test]
    fn imports_local_use_with_x_y_transform_and_inherited_style() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
          <defs>
            <g id="shape" data-kind="engrave" transform="scale(2)">
              <rect x="1" y="2" width="3" height="4"/>
            </g>
          </defs>
          <use href="#shape" x="10" y="20" transform="translate(5,0)"/>
          <use xlink:href="#shape" x="30" y="5"
            xmlns:xlink="http://www.w3.org/1999/xlink"/>
        </svg>"##;
        let doc = import(svg, "use.svg").unwrap();
        assert_eq!(doc.shape_count(), 2);
        assert_eq!(doc.layers[1].shapes.len(), 2);
        let first = doc.layers[1].shapes[0].bounds().unwrap();
        assert!((first.x0 - 17.0).abs() < 1e-9, "{first:?}");
        assert!((first.y0 - 24.0).abs() < 1e-9, "{first:?}");
        assert!((first.width() - 6.0).abs() < 1e-9, "{first:?}");
        let second = doc.layers[1].shapes[1].bounds().unwrap();
        assert!((second.x0 - 32.0).abs() < 1e-9, "{second:?}");
        assert!((second.y0 - 9.0).abs() < 1e-9, "{second:?}");
    }

    #[test]
    fn local_use_cycles_missing_and_external_references_fail_calmly() {
        for svg in [
            r##"<svg><defs><g id="a"><use href="#b"/></g><g id="b"><use href="#a"/></g></defs><use href="#a"/></svg>"##,
            r##"<svg><use href="#missing"/></svg>"##,
            r#"<svg><use href="other.svg#shape"/></svg>"#,
            r##"<svg><defs><text id="label">no</text></defs><use href="#label"/></svg>"##,
            r##"<svg><rect id="same"/><circle id="same"/></svg>"##,
        ] {
            assert!(import(svg, "use-error.svg").is_err(), "{svg}");
        }
    }

    #[test]
    fn viewbox_maps_physical_mm_and_preserves_aspect_ratio() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200mm" height="100mm"
          viewBox="0 0 100 100" preserveAspectRatio="none">
          <rect x="10" y="20" width="5" height="4"/>
        </svg>"#;
        let doc = import(svg, "viewbox.svg").unwrap();
        assert_eq!(doc.board.width_mm, 200.0);
        assert_eq!(doc.board.height_mm, 100.0);
        let b = doc.layers[0].shapes[0].bounds().unwrap();
        assert!((b.x0 - 20.0).abs() < 1e-9, "{b:?}");
        assert!((b.y0 - 20.0).abs() < 1e-9, "{b:?}");
        assert!((b.width() - 10.0).abs() < 1e-9, "{b:?}");
        assert!((b.height() - 4.0).abs() < 1e-9, "{b:?}");
    }

    #[test]
    fn imports_hidden_visibility_and_inline_styles() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
          <g style="display:none"><rect width="10" height="10"/></g>
          <g style="visibility:hidden" data-kind="engrave">
            <line style="visibility:visible" x1="1" y1="2" x2="3" y2="4"/>
            <line x1="5" y1="6" x2="7" y2="8"/>
          </g>
        </svg>"#;
        let doc = import(svg, "style.svg").unwrap();
        assert_eq!(doc.shape_count(), 1);
        assert_eq!(doc.layers.len(), 2);
        assert_eq!(doc.layers[1].shapes.len(), 1);
    }

    #[test]
    fn reports_unsupported_drawables_instead_of_dropping_them() {
        let svg =
            r#"<svg xmlns="http://www.w3.org/2000/svg"><text x="0" y="10">hello</text></svg>"#;
        assert!(matches!(
            import(svg, "text.svg"),
            Err(SvgError::Import(ref message)) if message.contains("<text> is not supported")
        ));
    }

    #[test]
    fn rejects_nonfinite_geometry_and_malformed_number_lists() {
        assert!(import(r#"<svg viewBox="0 0 NaN 100"/>"#, "nan.svg").is_err());
        assert!(import(r#"<svg><polygon points="0,0,,1,1"/></svg>"#, "points.svg").is_err());
        assert!(
            import(
                r#"<svg><rect x="1e400" width="10" height="10"/></svg>"#,
                "huge.svg"
            )
            .is_err()
        );
        assert!(
            import(
                &format!("<svg>{}</svg>", " ".repeat(MAX_SVG_BYTES)),
                "large.svg"
            )
            .is_err()
        );
    }

    #[test]
    fn parses_transform_lists_and_keeps_physical_px_units() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="96px" height="48px">
          <g transform="translate(10 0) scale(2)"><line x1="0" y1="0" x2="4" y2="0"/></g>
        </svg>"#;
        let doc = import(svg, "px.svg").unwrap();
        assert!((doc.board.width_mm - 25.4).abs() < 1e-9);
        assert!((doc.board.height_mm - 12.7).abs() < 1e-9);
        let b = doc.layers[0].shapes[0].bounds().unwrap();
        assert!((b.x0 - 10.0 * 25.4 / 96.0).abs() < 1e-9, "{b:?}");
        assert!((b.width() - 8.0 * 25.4 / 96.0).abs() < 1e-9, "{b:?}");
    }

    #[test]
    fn malformed_xml_and_resource_limits_are_errors_not_panics() {
        assert!(matches!(
            import("<svg><g></svg>", "bad-xml.svg"),
            Err(SvgError::Import(ref e)) if e.contains("invalid XML")
        ));
        let deep = format!(
            "{}<path d=\"M0 0L1 0\"/>{}",
            "<g>".repeat(MAX_SVG_DEPTH + 2),
            "</g>".repeat(MAX_SVG_DEPTH + 2)
        );
        assert!(matches!(
            import(&deep, "deep.svg"),
            Err(SvgError::Import(ref e)) if e.contains("nesting exceeds")
        ));
        let nodes = format!("<svg>{}</svg>", "<g/>".repeat(MAX_SVG_NODES + 1));
        assert!(matches!(
            import(&nodes, "nodes.svg"),
            Err(SvgError::Import(ref e)) if e.contains("100,000-node")
        ));
    }

    #[test]
    fn deterministically_mutated_svg_never_panics() {
        let seed = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect x="2" y="3" width="7" height="8"/></svg>"#;
        let bytes = seed.as_bytes();
        let mut state = 0x51_6f_9a_u64;
        for _ in 0..512 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let index = (state as usize) % bytes.len();
            let mut mutated = bytes.to_vec();
            mutated[index] = (state >> 32) as u8;
            if let Ok(text) = std::str::from_utf8(&mutated)
                && let Ok(doc) = import(text, "mutated.svg")
            {
                doc.validate().unwrap();
            }
        }
    }
}
