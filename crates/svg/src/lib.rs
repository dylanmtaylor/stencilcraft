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
    for layer in doc.layers.iter().filter(|layer| !layer.template) {
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
    let parents = doc.group_parent_map();
    // Template (guide) layers are canvas-only: never exported.
    for layer in doc.layers.iter().filter(|layer| !layer.template) {
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
        // Groups nest inside the layer as `<g class="group" data-group="id">`; a group
        // whose shapes are not adjacent in stacking order appears more than once.
        let mut open: Vec<u64> = Vec::new();
        for shape in layer.shapes.iter().filter(|shape| !shape.hidden) {
            let chain = stencilcraft_doc::canvas::group_chain_in(
                &parents,
                stencilcraft_doc::GroupChild::Shape(shape.id),
            );
            let common = open.iter().zip(&chain).take_while(|(a, b)| a == b).count();
            for _ in common..open.len() {
                svg.push_str("    </g>\n");
            }
            open.truncate(common);
            for id in chain.iter().skip(common) {
                let _ = writeln!(svg, "    <g class=\"group\" data-group=\"{id}\">");
                open.push(*id);
            }
            let d = path_to_svg_d(&shape.cut_path())?;
            let _ = writeln!(
                svg,
                "    <path class=\"{}\" d=\"{}\"/>",
                class_for(shape.style),
                d
            );
        }
        for _ in &open {
            svg.push_str("    </g>\n");
        }
        svg.push_str("  </g>\n");
    }
    svg.push_str("</svg>\n");
    Ok(svg)
}

/// How to lay tiled sheets out on one preview page.
#[derive(Debug, Clone, PartialEq)]
pub struct TilePreviewOptions {
    /// Cells per row; clamped to the number of sheets.
    pub columns: usize,
    /// Page millimetres per sheet millimetre. 0.25 is a comfortable overview.
    pub scale: f64,
}

impl Default for TilePreviewOptions {
    fn default() -> Self {
        Self {
            columns: 3,
            scale: 0.25,
        }
    }
}

/// Space between preview cells and around the page, millimetres.
pub const TILE_PREVIEW_GAP_MM: f64 = 6.0;

/// Largest preview page this will build, millimetres per side.
pub const TILE_PREVIEW_MAX_MM: f64 = 100_000.0;

/// Render every sheet of a tiling onto one preview page.
///
/// Unlike a cut export, a preview is for looking at: paths are stroked, not filled
/// (cut lines red, engrave/marks blue), and each sheet sits in a framed cell with a
/// `sheet N of M` label so it is obvious where one sheet ends and the next begins.
/// Cells follow the tiling order (first sheet top-left of the board) but rows are
/// placed bottom-up and sheet content is flipped, because SVG y grows downwards while
/// documents are y-up: the preview reads the way the assembled board does.
pub fn export_tile_preview(
    pieces: &[Document],
    options: &TilePreviewOptions,
) -> Result<String, SvgError> {
    if pieces.is_empty() {
        return Err(SvgError::Model(
            "preview: the tiling produced no sheets to show".to_string(),
        ));
    }
    if options.columns == 0 {
        return Err(SvgError::Model(
            "preview: columns must be at least 1".to_string(),
        ));
    }
    if !options.scale.is_finite() || options.scale <= 0.0 {
        return Err(SvgError::Model(format!(
            "preview: scale {} is not a positive number",
            options.scale
        )));
    }
    let columns = options.columns.min(pieces.len());
    let rows = pieces.len().div_ceil(columns);
    let mut cell_w: f64 = 0.0;
    let mut cell_h: f64 = 0.0;
    for piece in pieces {
        let (w, h) = (piece.board.width_mm, piece.board.height_mm);
        if !(w.is_finite() && h.is_finite()) || w <= 0.0 || h <= 0.0 {
            return Err(SvgError::Model(format!(
                "preview: sheet {w}×{h} mm is not a usable size"
            )));
        }
        cell_w = cell_w.max(w * options.scale);
        cell_h = cell_h.max(h * options.scale);
    }
    let gap = TILE_PREVIEW_GAP_MM;
    let page_w = columns as f64 * cell_w + (columns as f64 + 1.0) * gap;
    let page_h = rows as f64 * cell_h + (rows as f64 + 1.0) * gap;
    if !(page_w.is_finite() && page_h.is_finite())
        || page_w > TILE_PREVIEW_MAX_MM
        || page_h > TILE_PREVIEW_MAX_MM
    {
        return Err(SvgError::Model(format!(
            "preview: a {page_w:.0}×{page_h:.0} mm page is too big; use a smaller scale or fewer sheets"
        )));
    }

    let mut svg = String::with_capacity(2048 + pieces.len() * 512);
    svg.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let _ = writeln!(
        svg,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" width=\"{page_w:.3}mm\" height=\"{page_h:.3}mm\" viewBox=\"0 0 {page_w:.3} {page_h:.3}\">"
    );
    let _ = writeln!(
        svg,
        "  <title>StencilCraft tiling preview: {} sheets, {} × {} cells</title>",
        pieces.len(),
        columns,
        rows
    );
    // A preview shows outlines, not filled artwork: fill would hide the very detail
    // the preview exists to show. Colours follow cutting-software convention.
    svg.push_str("  <style>\n");
    svg.push_str("    .preview-frame { fill: none; stroke: #9aa0a6; stroke-width: 0.25; }\n");
    svg.push_str("    .stencilcraft-cut { fill: none; stroke: #c0392b; stroke-width: 0.3; }\n");
    svg.push_str("    .stencilcraft-engrave { fill: none; stroke: #2d6cdf; stroke-width: 0.2; }\n");
    svg.push_str(
        "    .preview-label { fill: #5f6368; font-family: sans-serif; font-size: 3.2px; }\n",
    );
    svg.push_str("  </style>\n");
    for (index, piece) in pieces.iter().enumerate() {
        let col = index % columns;
        let row = index / columns;
        let x = gap + col as f64 * (cell_w + gap);
        // SVG y grows downwards while documents are y-up: the tiling's first row is
        // the bottom of the board, so it belongs at the bottom of the page.
        let y = gap + (rows - 1 - row) as f64 * (cell_h + gap);
        let cell_height = piece.board.height_mm * options.scale;
        let _ = writeln!(
            svg,
            "  <g class=\"sheet\" data-sheet=\"{}\" data-size=\"{:.1}x{:.1}\">",
            index, piece.board.width_mm, piece.board.height_mm
        );
        let _ = writeln!(
            svg,
            "    <text class=\"preview-label\" x=\"{x:.3}\" y=\"{:.3}\">sheet {} of {} · {:.0}×{:.0} mm</text>",
            y - 1.6,
            index + 1,
            pieces.len(),
            piece.board.width_mm,
            piece.board.height_mm
        );
        let _ = writeln!(
            svg,
            "    <rect class=\"preview-frame\" x=\"{x:.3}\" y=\"{y:.3}\" width=\"{:.3}\" height=\"{cell_height:.3}\"/>",
            piece.board.width_mm * options.scale
        );
        // Flip the sheet's y-up millimetres into the page's y-down space, so each cell
        // reads the same way the sheet does on the mat.
        let _ = writeln!(
            svg,
            "    <g transform=\"translate({x:.3} {:.3}) scale({:.4} {:.4})\">",
            y + cell_height,
            options.scale,
            -options.scale
        );
        for layer in &piece.layers {
            if !layer.is_output() {
                continue;
            }
            for shape in layer.shapes.iter().filter(|shape| !shape.hidden) {
                let d = path_to_svg_d(&shape.cut_path())?;
                let _ = writeln!(
                    svg,
                    "      <path class=\"{}\" d=\"{}\"/>",
                    class_for(shape.style),
                    d
                );
            }
        }
        svg.push_str("    </g>\n  </g>\n");
    }
    svg.push_str("</svg>\n");
    Ok(svg)
}

/// Export a physical-size Print & Cut SVG sheet with three high-contrast
/// registration marks (top-left origin square, top-right and bottom-left L-corners)
/// around the document board.
pub fn export_print_and_cut(doc: &Document) -> Result<String, SvgError> {
    const PAGE_MARGIN_MM: f64 = 12.0;
    const MARK_SIZE_MM: f64 = 6.0;

    doc.validate().map_err(|e| SvgError::Model(e.to_string()))?;
    let artwork = export(doc)?;
    let page_width = doc.board.width_mm + 2.0 * PAGE_MARGIN_MM;
    let page_height = doc.board.height_mm + 2.0 * PAGE_MARGIN_MM;
    if !page_width.is_finite() || !page_height.is_finite() {
        return Err(SvgError::Model(
            "print-cut: page dimensions are not finite".to_string(),
        ));
    }

    let mut svg = String::with_capacity(artwork.len() + 1024);
    svg.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let _ = writeln!(
        svg,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" width=\"{page_width:.3}mm\" height=\"{page_height:.3}mm\" viewBox=\"0 0 {page_width:.3} {page_height:.3}\">"
    );
    svg.push_str("  <title>StencilCraft print-and-cut alignment sheet</title>\n");
    let _ = writeln!(
        svg,
        "  <g id=\"registration-marks\" fill=\"#000000\" aria-label=\"Registration marks: top-left square, top-right and bottom-left corners\">"
    );
    let _ = writeln!(
        svg,
        "    <rect x=\"0\" y=\"0\" width=\"{MARK_SIZE_MM:.3}\" height=\"{MARK_SIZE_MM:.3}\"/>"
    );
    let _ = writeln!(
        svg,
        "    <path d=\"M {:.3} 0 H {:.3} V {:.3} H {:.3} V {:.3} H {:.3} Z\"/>",
        page_width,
        page_width - MARK_SIZE_MM,
        MARK_SIZE_MM,
        page_width - 1.5,
        1.5,
        page_width - MARK_SIZE_MM,
    );
    let _ = writeln!(
        svg,
        "    <path d=\"M 0 {:.3} H {:.3} V {:.3} H {:.3} V {:.3} H 0 Z\"/>",
        page_height,
        MARK_SIZE_MM,
        page_height - MARK_SIZE_MM,
        1.5,
        page_height - 1.5,
    );
    svg.push_str("  </g>\n");
    let _ = writeln!(
        svg,
        "  <svg x=\"{PAGE_MARGIN_MM:.3}\" y=\"{PAGE_MARGIN_MM:.3}\" width=\"{:.3}mm\" height=\"{:.3}mm\" viewBox=\"0 0 {:.3} {:.3}\" overflow=\"hidden\">",
        doc.board.width_mm, doc.board.height_mm, doc.board.width_mm, doc.board.height_mm,
    );
    if let Some(root_start) = artwork.find('>')
        && let Some(root_end) = artwork.rfind("</svg>")
        && root_start < root_end
    {
        svg.push_str(&artwork[root_start + 1..root_end]);
    } else {
        return Err(SvgError::Write(
            "print-cut: could not embed SVG artwork".to_string(),
        ));
    }
    svg.push_str("  </svg>\n</svg>\n");
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

/// The cut or engrave tag an SVG element carries, as `craft-svg-paths` follows it down the tree.
#[derive(Clone)]
struct Kind(CutStyle);

impl Default for Kind {
    fn default() -> Self {
        Kind(CutStyle::Cut)
    }
}

impl craft_svg_paths::Kind for Kind {
    const PROPERTIES: &'static [&'static str] = &["data-kind", "data-style"];

    fn of(style: &craft_svg_paths::StyleMap, class: &str) -> Option<Self> {
        let explicit = style.get("data-kind").or_else(|| style.get("data-style"));
        if let Some(value) = explicit {
            match value.as_str() {
                "cut" => return Some(Kind(CutStyle::Cut)),
                "engrave" | "score" | "mark" => return Some(Kind(CutStyle::Engrave)),
                _ => {}
            }
        }
        let class = class.to_ascii_lowercase();
        if class
            .split_ascii_whitespace()
            .any(|c| c.contains("engrave") || c.contains("score"))
        {
            Some(Kind(CutStyle::Engrave))
        } else if class.split_ascii_whitespace().any(|c| c.contains("cut")) {
            Some(Kind(CutStyle::Cut))
        } else {
            None
        }
    }

    // A line with fill:none can still be a cut path; stroke:none plus fill:none is empty
    // and can safely be ignored.
    fn skips_unpainted(&self) -> bool {
        self.0 == CutStyle::Cut
    }
}

/// A lenient import: the document plus every piece of the file that was skipped or
/// approximated (text, masks, filters, ignored CSS, clips it could not apply).
#[derive(Debug)]
pub struct SvgImport {
    pub document: Document,
    pub warnings: Vec<String>,
}

/// Import common SVG artwork into a cutting document.
///
/// Supported geometry: `<path>`, `<rect>` (including rounded corners),
/// `<circle>`, `<ellipse>`, `<line>`, `<polyline>`, and `<polygon>`. Nested
/// groups, transforms (`matrix`, `translate`, `scale`, `rotate`, `skewX/Y`),
/// and inherited presentation/inline styles (`display`, `visibility`, CSS
/// `style`, `class`, `data-kind`) are applied. Stylesheets support simple type,
/// class and ID selectors (including compound, descendant and child selectors)
/// with specificity-based cascade. SVG paths are flattened nowhere
/// here: curves remain curves in the document. Each imported shape is assigned
/// to cut or engrave according to inherited `data-kind`/`data-style` or class.
/// Embedded stylesheets support type, class, ID and attribute selectors (`=`, `~=`,
/// `^=`, `$=`, `*=`, `|=`), descendant/child/sibling combinators, `:not()`,
/// `:first-child`, `:last-child`, `:root`, selector lists, and a bounded
/// presentation-property cascade. Shapes with a stroke colour (any CSS Color 4 form,
/// or `currentColor`) go on one layer per colour — `Line #rrggbb` (or
/// `engrave #rrggbb`) with the layer colour set. `clip-path` references to
/// `<clipPath>` elements clip the geometry.
///
/// This strict form refuses a file with anything [`import_with_warnings`] would only
/// warn about (text, masks, filters, ignored CSS), so nothing is lost silently.
///
/// Physical root dimensions accept unitless SVG px, `px`, `mm`, `cm`, `in`,
/// `pt`, and `pc`; a `viewBox` maps its user coordinates onto that physical
/// viewport, following `preserveAspectRatio`. Without physical dimensions,
/// `viewBox` units are interpreted as millimetres to preserve StencilCraft's
/// 1-unit = 1-mm convention. A fragment without an SVG root uses that same
/// convention. Local fragment-only `<use>` references can instantiate supported
/// shapes, groups and `<symbol>` templates with viewBox sizing; external references,
/// text, masks, filters and percentage lengths are not expanded. Unsupported drawable
/// elements are reported rather than silently lost.
pub fn import(svg: &str, path_name: &str) -> Result<Document, SvgError> {
    let imported = import_with_warnings(svg, path_name)?;
    if imported.warnings.is_empty() {
        Ok(imported.document)
    } else {
        Err(svg_error(path_name, &imported.warnings.join("; ")))
    }
}

/// [`import`] that skips what it cannot represent and says so: `<text>` (no fonts
/// here; convert text to paths first), masks and filters (the art is imported unmasked
/// and unfiltered), ignored CSS at-rules and selectors, and clip paths it cannot apply.
pub fn import_with_warnings(svg: &str, path_name: &str) -> Result<SvgImport, SvgError> {
    let read =
        craft_svg_paths::import::<Kind>(svg, path_name).map_err(|e| SvgError::Import(e.0))?;
    let mut doc = Document::default();
    doc.board.width_mm = read.width_mm;
    doc.board.height_mm = read.height_mm;
    let (mut warnings, mut dropped) = (read.warnings, read.dropped_warnings);
    // (engrave?, stroke colour) → layer index; (cut, no colour) is layer 0.
    let mut layer_index = std::collections::HashMap::from([((false, None), 0)]);
    // Each clip region is unioned once, on first use.
    let mut regions: Vec<Option<BezPath>> = vec![None; read.clips.len()];
    for shape in read.shapes {
        let mut path = shape.path;
        for clip in &shape.clips {
            let (Some(pieces), Some(slot)) = (read.clips.get(*clip), regions.get_mut(*clip)) else {
                continue;
            };
            let region = match slot {
                Some(region) => region,
                None => slot.insert(
                    clip_union(pieces)
                        .map_err(|e| svg_error(path_name, &format!("clip-path: {e}")))?,
                ),
            };
            path = clip_to_region(&path, region)
                .map_err(|e| svg_error(path_name, &format!("clip-path: {e}")))?;
        }
        if path.elements().is_empty() {
            continue;
        }
        let kind = shape.kind.0;
        let color = shape.stroke.and_then(|paint| {
            match stencilcraft_color::parse_color(&paint) {
                Ok(color) => Some(color.to_css()),
                Err(_) => {
                    craft_svg_paths::push_warning(
                        &mut warnings,
                        &mut dropped,
                        format!(
                            "{}: stroke colour {:?} not understood; shape put on the uncoloured layer",
                            shape.label,
                            paint.chars().take(64).collect::<String>()
                        ),
                    );
                    None
                }
            }
        });
        let index = layer_for(&mut doc, &mut layer_index, kind, color);
        doc.push_shape(index, path, kind)
            .map_err(|e| svg_error(path_name, &e.to_string()))?;
    }
    if dropped > 0 {
        warnings.push(format!("…and {dropped} more warnings"));
    }

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
    Ok(SvgImport {
        document: doc,
        warnings,
    })
}

/// The layer for a shape of `kind` stroked in `color`, created on first use.
fn layer_for(
    doc: &mut Document,
    layer_index: &mut std::collections::HashMap<(bool, Option<String>), usize>,
    kind: CutStyle,
    color: Option<String>,
) -> usize {
    let key = (kind == CutStyle::Engrave, color);
    if let Some(index) = layer_index.get(&key) {
        return *index;
    }
    let name = match (&key.1, kind) {
        (Some(hex), CutStyle::Cut) => format!("Line {hex}"),
        (Some(hex), CutStyle::Engrave) => format!("engrave {hex}"),
        (None, _) => "engrave".to_string(),
    };
    let mut layer = Layer::new(name, kind);
    layer.color = key.1.clone();
    let index = doc.layers.len();
    doc.layers.push(layer);
    layer_index.insert(key, index);
    index
}

/// One clip region: the union of a `<clipPath>`'s pieces, each under its own `clip-rule`.
fn clip_union(clip: &craft_svg_paths::Clip) -> Result<BezPath, String> {
    // An empty clip path clips everything away (CSS Masking §6.1).
    if clip.pieces.is_empty() {
        return Ok(BezPath::new());
    }
    let pieces: Vec<_> = clip
        .pieces
        .iter()
        .map(|(path, rule)| {
            let rule = match rule {
                craft_svg_paths::FillRule::EvenOdd => vectorcraft_geom::FillRule::EvenOdd,
                craft_svg_paths::FillRule::NonZero => vectorcraft_geom::FillRule::NonZero,
            };
            (vectorcraft_geom::PathData::from_bezpath(path), rule)
        })
        .collect();
    let inputs: Vec<_> = pieces.iter().map(|(p, rule)| (p, *rule)).collect();
    vectorcraft_pathops::try_boolean_n(&inputs, |inside: &[bool]| inside.iter().any(|b| *b))
        .map(|region| region.to_bezpath())
        .map_err(|e| e.to_string())
}

/// `path` cut down to `region`: closed contours by boolean intersection (curves kept),
/// open contours flattened and clipped as polylines.
fn clip_to_region(path: &BezPath, region: &BezPath) -> Result<BezPath, String> {
    if region.elements().is_empty() {
        return Ok(BezPath::new());
    }
    let (mut closed, mut open) = (BezPath::new(), BezPath::new());
    let mut contour = Vec::new();
    for el in path
        .elements()
        .iter()
        .copied()
        .chain([kurbo::PathEl::MoveTo(kurbo::Point::ZERO)])
    {
        if matches!(el, kurbo::PathEl::MoveTo(_)) && !contour.is_empty() {
            let target = if matches!(contour.last(), Some(kurbo::PathEl::ClosePath)) {
                &mut closed
            } else {
                &mut open
            };
            target.extend(contour.drain(..));
        }
        contour.push(el);
    }
    let mut out = BezPath::new();
    if !closed.elements().is_empty() {
        // Cut contours follow the engine's even-odd convention, so holes stay holes.
        let kept = vectorcraft_pathops::try_boolean(
            &vectorcraft_geom::PathData::from_bezpath(&closed),
            vectorcraft_geom::FillRule::EvenOdd,
            &vectorcraft_geom::PathData::from_bezpath(region),
            vectorcraft_geom::FillRule::NonZero,
            vectorcraft_pathops::BoolOp::Intersect,
            0.001,
        )
        .map_err(|e| e.to_string())?;
        out.extend(kept.to_bezpath());
    }
    if !open.elements().is_empty() {
        let polygons: Vec<Vec<kurbo::Point>> = stencilcraft_generate::flatten(region)
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|c| c.closed && c.points.len() >= 3)
            .map(|c| c.points)
            .collect();
        for line in stencilcraft_generate::flatten(&open).map_err(|e| e.to_string())? {
            let pieces = stencilcraft_generate::clip::clip_polyline(&line.points, &polygons, true);
            out.extend(stencilcraft_generate::polylines_path(&pieces, false));
        }
    }
    Ok(out)
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
    use craft_svg_paths::{
        MAX_CSS_BYTES, MAX_SVG_BYTES, MAX_SVG_DEPTH, MAX_SVG_NODES, MAX_SVG_WARNINGS,
    };
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
    fn hidden_shapes_are_not_exported() {
        let mut doc = Document::default();
        doc.push_shape(0, rect(10.0, 10.0, 20.0, 20.0), CutStyle::Cut)
            .unwrap();
        doc.push_shape(0, rect(50.0, 50.0, 5.0, 5.0), CutStyle::Cut)
            .unwrap();
        doc.layers[0].shapes[1].hidden = true;
        let svg = export(&doc).unwrap();
        assert!(svg.contains("d=\"M10"), "{svg}");
        assert!(!svg.contains("d=\"M50"), "{svg}");
    }

    fn preview_piece(w: f64, h: f64, style: CutStyle) -> Document {
        let mut doc = Document {
            board: stencilcraft_doc::Board {
                width_mm: w,
                height_mm: h,
            },
            ..Document::default()
        };
        doc.layers.clear();
        doc.layers.push(stencilcraft_doc::Layer::new("cut", style));
        doc.push_shape(0, rect(0.0, 0.0, w, h), style).unwrap();
        doc
    }

    #[test]
    fn tile_preview_pages_every_sheet_with_stroked_frames() {
        let pieces = vec![
            preview_piece(60.0, 80.0, CutStyle::Cut),
            preview_piece(60.0, 80.0, CutStyle::Engrave),
        ];
        let svg = export_tile_preview(
            &pieces,
            &TilePreviewOptions {
                columns: 2,
                scale: 0.5,
            },
        )
        .unwrap();
        // Side by side: 2 cells of 30 × 40 mm plus three 6 mm gaps across, two 6 mm
        // gaps down.
        assert!(svg.contains("width=\"78.000mm\""), "{svg}");
        assert!(svg.contains("height=\"52.000mm\""), "{svg}");
        assert_eq!(svg.matches("class=\"sheet\"").count(), 2, "{svg}");
        assert_eq!(svg.matches("class=\"preview-frame\"").count(), 2, "{svg}");
        // One cut sheet and one engrave sheet, each drawn with its own class.
        assert_eq!(svg.matches("class=\"stencilcraft-cut\"").count(), 1);
        assert_eq!(svg.matches("class=\"stencilcraft-engrave\"").count(), 1);
        // Outlines, not fills: the whole point of the preview is to show the detail.
        assert!(svg.contains(".preview-frame { fill: none;"), "{svg}");
        assert!(svg.contains(".stencilcraft-cut { fill: none;"), "{svg}");
        // Content is flipped into the page's y-down space and anchored to the cell's
        // bottom edge: 6 mm gap + 40 mm cell = 46 mm.
        assert!(
            svg.contains("translate(6.000 46.000) scale(0.5000 -0.5000)"),
            "{svg}"
        );
        assert!(svg.contains("data-sheet=\"1\""), "{svg}");
    }

    #[test]
    fn tile_preview_labels_each_sheet() {
        let pieces = vec![
            preview_piece(60.0, 80.0, CutStyle::Cut),
            preview_piece(60.0, 80.0, CutStyle::Cut),
        ];
        let svg = export_tile_preview(
            &pieces,
            &TilePreviewOptions {
                columns: 1,
                scale: 0.5,
            },
        )
        .unwrap();
        assert!(svg.contains(">sheet 1 of 2 · 60×80 mm</text>"), "{svg}");
        assert!(svg.contains(">sheet 2 of 2 · 60×80 mm</text>"), "{svg}");
        // Column-major tiling stacks the first sheet at the bottom of the page.
        let first = svg.find("data-sheet=\"0\"").unwrap();
        let second = svg.find("data-sheet=\"1\"").unwrap();
        let first_y = &svg[first..second];
        assert!(
            first_y.contains("y=\"52.000\""),
            "sheet 0 should sit below sheet 1: {svg}"
        );
    }

    #[test]
    fn tile_preview_refuses_impossible_pages() {
        let pieces = vec![preview_piece(60.0, 80.0, CutStyle::Cut)];
        assert!(matches!(
            export_tile_preview(&[], &TilePreviewOptions::default()),
            Err(SvgError::Model(_))
        ));
        assert!(matches!(
            export_tile_preview(
                &pieces,
                &TilePreviewOptions {
                    columns: 0,
                    scale: 1.0
                }
            ),
            Err(SvgError::Model(_))
        ));
        assert!(matches!(
            export_tile_preview(
                &pieces,
                &TilePreviewOptions {
                    columns: 1,
                    scale: 0.0
                }
            ),
            Err(SvgError::Model(_))
        ));
        let huge = vec![preview_piece(200_000.0, 200_000.0, CutStyle::Cut)];
        assert!(matches!(
            export_tile_preview(
                &huge,
                &TilePreviewOptions {
                    columns: 1,
                    scale: 1.0
                }
            ),
            Err(SvgError::Model(_))
        ));
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
            <symbol id="badge" viewBox="10 20 20 10" preserveAspectRatio="xMidYMid meet" data-kind="engrave">
              <rect x="10" y="20" width="20" height="10"/>
            </symbol>
          </defs>
          <use href="#shape" x="10" y="20" transform="translate(5,0)"/>
          <use xlink:href="#shape" x="30" y="5"
            xmlns:xlink="http://www.w3.org/1999/xlink"/>
          <use href="#badge" x="50" y="10" width="40" height="40"/>
        </svg>"##;
        let doc = import(svg, "use.svg").unwrap();
        assert_eq!(doc.shape_count(), 3);
        assert_eq!(doc.layers[0].shapes.len(), 0);
        assert_eq!(doc.layers[1].shapes.len(), 3);
        let first = doc.layers[1].shapes[0].bounds().unwrap();
        assert!((first.x0 - 17.0).abs() < 1e-9, "{first:?}");
        assert!((first.y0 - 24.0).abs() < 1e-9, "{first:?}");
        assert!((first.width() - 6.0).abs() < 1e-9, "{first:?}");
        let second = doc.layers[1].shapes[1].bounds().unwrap();
        assert!((second.x0 - 32.0).abs() < 1e-9, "{second:?}");
        assert!((second.y0 - 9.0).abs() < 1e-9, "{second:?}");
        let badge = doc.layers[1].shapes[2].bounds().unwrap();
        assert!((badge.x0 - 50.0).abs() < 1e-9, "{badge:?}");
        assert!((badge.y0 - 20.0).abs() < 1e-9, "{badge:?}");
        assert!((badge.width() - 40.0).abs() < 1e-9, "{badge:?}");
        assert!((badge.height() - 20.0).abs() < 1e-9, "{badge:?}");
        assert_eq!(doc.layers[1].shapes[2].style, CutStyle::Engrave);
    }

    #[test]
    fn local_use_cycles_missing_and_external_references_fail_calmly() {
        for svg in [
            r##"<svg><defs><g id="a"><use href="#b"/></g><g id="b"><use href="#a"/></g></defs><use href="#a"/></svg>"##,
            r##"<svg><use href="#missing"/></svg>"##,
            r#"<svg><use href="other.svg#shape"/></svg>"#,
            r##"<svg><defs><text id="label">no</text></defs><use href="#label"/></svg>"##,
            r##"<svg><rect id="same"/><circle id="same"/></svg>"##,
            r##"<svg><defs><symbol id="cycle"><use href="#cycle"/></symbol></defs><use href="#cycle"/></svg>"##,
            r##"<svg><defs><symbol id="bad" viewBox="0 0 0 4"><rect width="2" height="2"/></symbol></defs><use href="#bad"/></svg>"##,
            r##"<svg><defs><symbol id="zero" viewBox="0 0 4 4"><rect width="2" height="2"/></symbol></defs><use href="#zero" width="-1"/></svg>"##,
        ] {
            assert!(import(svg, "use-error.svg").is_err(), "{svg}");
        }
    }

    #[test]
    fn viewbox_maps_physical_mm_and_preserves_aspect_ratio() {
        for preserve in ["xMidYMid meet extra", "notxMidYMid"] {
            let svg = format!(
                r#"<svg width="200mm" height="100mm" viewBox="0 0 100 100" preserveAspectRatio="{preserve}"/>"#
            );
            assert!(import(&svg, "aspect.svg").is_err(), "{preserve}");
        }
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
    fn applies_css_selector_specificity_cascade_and_inherited_visibility() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
          <style>
            /* supported selector subset */
            path { data-kind: cut; }
            .mark { data-kind: engrave; visibility: hidden; }
            g#source > path.mark { visibility: visible; }
            path.mark[id="first"] { display: none; }
            path.mark[data-kind="engrave"] { stroke: red; }
            .mark { data-kind: cut !important; }
          </style>
          <g id="source">
            <path id="first" class="mark" d="M1 1L2 1"/>
            <path id="second" class="mark" style="data-kind:engrave" d="M3 3L4 3"/>
          </g>
        </svg>"##;
        let doc = import(svg, "css.svg").unwrap();
        assert_eq!(doc.shape_count(), 1);
        assert_eq!(doc.layers[0].shapes.len(), 1);
        assert_eq!(doc.layers[0].shapes[0].bounds().unwrap().x0, 3.0);
        assert_eq!(doc.layers.len(), 1);
    }

    #[test]
    fn rejects_unsupported_css_selectors_and_caps_stylesheet_size() {
        assert!(
            import(
                r##"<svg><style>path:hover { display:none; }</style><path d="M0 0L1 1"/></svg>"##,
                "pseudo.svg"
            )
            .is_err()
        );
        let large = format!(
            "<svg><style>{}</style></svg>",
            " ".repeat(MAX_CSS_BYTES + 1)
        );
        assert!(import(&large, "css-large.svg").is_err());
    }

    #[test]
    fn reports_unsupported_drawables_instead_of_dropping_them() {
        let svg =
            r#"<svg xmlns="http://www.w3.org/2000/svg"><text x="0" y="10">hello</text></svg>"#;
        assert!(matches!(
            import(svg, "text.svg"),
            Err(SvgError::Import(ref message)) if message.contains("<text> \"hello\" skipped")
        ));
        for svg in [
            r#"<svg><image href="a.png" width="1" height="1"/></svg>"#,
            r#"<svg><foreignObject/></svg>"#,
            r#"<svg><blink/></svg>"#,
        ] {
            assert!(import_with_warnings(svg, "drawable.svg").is_err(), "{svg}");
        }
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
        let seeds = [
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect x="2" y="3" width="7" height="8"/></svg>"#,
            // Colour, CSS, clip, text, mask and filter paths in one seed.
            r##"<svg viewBox="0 0 20 20"><style>@media print{g{x:y}} g>rect:not(.a)+rect[id^="r"]{stroke:hsl(9,50%,50%)}</style><clipPath id="c" transform="scale(1)"><circle cx="5" cy="5" r="4"/></clipPath><g clip-path="url(#c)" color="red"><rect width="9" height="9" stroke="currentColor"/><rect id="r2" width="3" height="3"/><path d="M0 0L20 20"/></g><text id="t">hi</text><g mask="url(#m)" filter="url(#f)"><rect width="1" height="1" stroke="#0f08"/></g></svg>"##,
        ];
        for (seed_index, seed) in seeds.iter().enumerate() {
            let bytes = seed.as_bytes();
            let mut state = 0x51_6f_9a_u64 + seed_index as u64;
            for _ in 0..1024 {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                let index = (state as usize) % bytes.len();
                let mut mutated = bytes.to_vec();
                mutated[index] = (state >> 32) as u8;
                if let Ok(text) = std::str::from_utf8(&mutated)
                    && let Ok(imported) = import_with_warnings(text, "mutated.svg")
                {
                    imported.document.validate().unwrap();
                    assert!(imported.warnings.len() <= MAX_SVG_WARNINGS + 1);
                }
            }
        }
    }

    fn lenient(svg: &str) -> SvgImport {
        import_with_warnings(svg, "test.svg").unwrap()
    }

    fn layer<'a>(doc: &'a Document, name: &str) -> &'a stencilcraft_doc::Layer {
        doc.layers
            .iter()
            .find(|layer| layer.name == name)
            .unwrap_or_else(|| panic!("no layer {name}: {:?}", doc.layers))
    }

    #[test]
    fn stroke_colours_become_one_layer_each() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
          <style>.blue { stroke: rgb(0 0 100%); }</style>
          <rect width="1" height="1" stroke="red"/>
          <rect width="1" height="1" style="stroke:#F00"/>
          <rect width="1" height="1" stroke="hsl(0, 100%, 50%)"/>
          <rect width="1" height="1" class="blue"/>
          <g color="rgba(0, 128, 0, 0.5)"><rect width="1" height="1" stroke="currentColor"/></g>
          <rect width="1" height="1" stroke="url(#gradient) cornflowerblue"/>
          <rect width="1" height="1" stroke="url(#gradient)"/>
          <rect width="1" height="1" fill="black"/>
          <rect width="1" height="1" stroke="none" fill="black"/>
          <g data-kind="engrave"><rect width="1" height="1" stroke="#00800080"/></g>
        </svg>"##;
        let imported = lenient(svg);
        assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
        let doc = imported.document;
        assert_eq!(doc.shape_count(), 10);
        assert_eq!(doc.layers[0].name, "cut");
        assert_eq!(doc.layers[0].color, None);
        assert_eq!(doc.layers[0].shapes.len(), 3);
        let red = layer(&doc, "Line #ff0000");
        assert_eq!(red.shapes.len(), 3);
        assert_eq!(red.color.as_deref(), Some("#ff0000"));
        assert_eq!(red.kind, CutStyle::Cut);
        assert_eq!(layer(&doc, "Line #0000ff").shapes.len(), 1);
        assert_eq!(layer(&doc, "Line #008000").shapes.len(), 1);
        assert_eq!(layer(&doc, "Line #6495ed").shapes.len(), 1);
        let engrave = layer(&doc, "engrave #008000");
        assert_eq!(engrave.kind, CutStyle::Engrave);
        assert_eq!(engrave.color.as_deref(), Some("#008000"));
        assert_eq!(engrave.shapes[0].style, CutStyle::Engrave);
        // The same document comes through the strict import unchanged.
        assert_eq!(
            import(svg, "colours.svg").unwrap().layers.len(),
            doc.layers.len()
        );
    }

    #[test]
    fn unreadable_stroke_colour_warns_and_strict_import_refuses() {
        let svg = r#"<svg><rect id="odd" width="1" height="1" stroke="color(srgb 1 0 0)"/></svg>"#;
        let imported = lenient(svg);
        assert_eq!(imported.document.layers[0].shapes.len(), 1);
        assert_eq!(imported.warnings.len(), 1);
        assert!(
            imported.warnings[0].contains("<rect id=\"odd\">"),
            "{:?}",
            imported.warnings
        );
        assert!(import(svg, "odd.svg").is_err());
    }

    fn bounds_of(doc: &Document) -> Vec<kurbo::Rect> {
        doc.layers
            .iter()
            .flat_map(|layer| layer.shapes.iter())
            .map(|shape| shape.bounds().unwrap())
            .collect()
    }

    fn near(a: kurbo::Rect, b: (f64, f64, f64, f64)) -> bool {
        (a.x0 - b.0).abs() < 1e-3
            && (a.y0 - b.1).abs() < 1e-3
            && (a.x1 - b.2).abs() < 1e-3
            && (a.y1 - b.3).abs() < 1e-3
    }

    #[test]
    fn clip_paths_clip_closed_and_open_geometry() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
          <defs>
            <clipPath id="half"><rect x="0" y="0" width="10" height="20"/></clipPath>
            <clipPath id="moved" transform="translate(50 0)"><rect width="5" height="5"/></clipPath>
            <clipPath id="disc"><circle cx="30" cy="30" r="5"/></clipPath>
            <clipPath id="nothing"/>
          </defs>
          <g clip-path="url(#half)">
            <rect width="20" height="20"/>
            <line x1="-5" y1="5" x2="25" y2="5"/>
          </g>
          <rect x="40" y="0" width="20" height="20" clip-path="url('#moved')"/>
          <rect x="20" y="20" width="20" height="20" style="clip-path: url(#disc)"/>
          <g transform="translate(0 60)"><rect width="20" height="20" clip-path="url(#half)"/></g>
          <rect width="20" height="20" clip-path="url(#nothing)"/>
          <rect x="80" y="80" width="5" height="5" clip-path="url(#half)"/>
        </svg>"##;
        let imported = lenient(svg);
        assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
        let bounds = bounds_of(&imported.document);
        assert_eq!(bounds.len(), 5, "{bounds:?}");
        assert!(near(bounds[0], (0.0, 0.0, 10.0, 20.0)), "{bounds:?}");
        assert!(near(bounds[1], (0.0, 5.0, 10.0, 5.0)), "{bounds:?}");
        assert!(near(bounds[2], (50.0, 0.0, 55.0, 5.0)), "{bounds:?}");
        assert!(near(bounds[3], (25.0, 25.0, 35.0, 35.0)), "{bounds:?}");
        // The clip region lives in the referencing element's user space.
        assert!(near(bounds[4], (0.0, 60.0, 10.0, 80.0)), "{bounds:?}");
        // The clipped closed shape stays closed (a cut contour, not an open run).
        let first = &imported.document.layers[0].shapes[0].path;
        assert!(matches!(
            first.elements().last(),
            Some(kurbo::PathEl::ClosePath)
        ));
    }

    #[test]
    fn clip_paths_that_cannot_be_applied_warn() {
        for (svg, needle) in [
            (
                r##"<svg><clipPath id="c" clipPathUnits="objectBoundingBox"><rect width="1" height="1"/></clipPath><rect width="9" height="9" clip-path="url(#c)"/></svg>"##,
                "objectBoundingBox",
            ),
            (
                r##"<svg><rect width="9" height="9" clip-path="url(#missing)"/></svg>"##,
                "not a local <clipPath>",
            ),
            (
                r##"<svg><rect width="9" height="9" clip-path="inset(1px)"/></svg>"##,
                "not a local <clipPath>",
            ),
            (
                r##"<svg><rect id="r" width="1" height="1"/><rect width="9" height="9" clip-path="url(#r)"/></svg>"##,
                "not a <clipPath>",
            ),
            (
                r##"<svg><clipPath id="c"><text>A</text><rect width="1" height="1"/></clipPath><rect width="9" height="9" clip-path="url(#c)"/></svg>"##,
                "inside a clip path",
            ),
        ] {
            let imported = lenient(svg);
            assert!(
                imported.warnings.iter().any(|w| w.contains(needle)),
                "{svg}: {:?}",
                imported.warnings
            );
            assert!(imported.document.shape_count() >= 1, "{svg}");
            assert!(import(svg, "clip.svg").is_err(), "{svg}");
        }
    }

    #[test]
    fn advanced_css_selectors_match_per_selectors_level_3() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
          <style>
            rect[data-tag~="b"] { stroke: #000001; }
            rect[id^="pre"] { stroke: #000002; }
            rect[id$="post"] { stroke: #000003; }
            rect[id*="mid"] { stroke: #000004; }
            rect[lang|="en"] { stroke: #000005; }
            g > line:first-child { stroke: #000006; }
            rect:not(.keep):last-child { stroke: #000007; }
            #anchor + rect { stroke: #000008; }
            #anchor ~ circle { stroke: #000009; }
            :root > rect.top { stroke: #00000a; }
          </style>
          <rect class="top" width="1" height="1"/>
          <g>
            <line x2="1"/>
            <rect class="keep" data-tag="a b c" width="1" height="1"/>
            <rect id="prefix" width="1" height="1"/>
            <rect id="the-post" width="1" height="1"/>
            <rect id="amidb" width="1" height="1"/>
            <rect lang="en-GB" width="1" height="1"/>
            <rect id="anchor" class="keep" width="1" height="1"/>
            <rect width="1" height="1"/>
            <circle r="1"/>
            <rect class="keep" width="1" height="1"/>
            <rect width="1" height="1"/>
          </g>
        </svg>"##;
        let imported = lenient(svg);
        assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
        let names: Vec<_> = imported
            .document
            .layers
            .iter()
            .map(|layer| (layer.name.as_str(), layer.shapes.len()))
            .collect();
        for n in 1..=10 {
            let name = format!("Line #{n:06x}");
            assert!(
                names
                    .iter()
                    .any(|(layer, count)| *layer == name && *count == 1),
                "{name}: {names:?}"
            );
        }
        // Unmatched: the anchor, and the last-but-one `.keep` rect (excluded by :not).
        assert_eq!(imported.document.layers[0].shapes.len(), 2);
    }

    #[test]
    fn css_at_rules_and_unsupported_selectors_warn_instead_of_failing() {
        let svg = r##"<svg><style>
            @charset "utf-8";
            @import url(other.css);
            @font-face { font-family: X; src: url(x.woff); }
            @media (max-width: 10px) { rect { display: none; } }
            rect:hover, rect.ok { stroke: red; }
            a::before { stroke: blue; }
          </style><rect class="ok" width="1" height="1"/></svg>"##;
        let imported = lenient(svg);
        let warnings = imported.warnings.join("\n");
        assert!(warnings.contains("@import"), "{warnings}");
        assert!(warnings.contains("@media"), "{warnings}");
        assert!(!warnings.contains("font-face"), "{warnings}");
        assert!(!warnings.contains("charset"), "{warnings}");
        assert!(warnings.contains("rect:hover"), "{warnings}");
        assert!(warnings.contains("a::before"), "{warnings}");
        // The @media rule is not applied; the plain rule after it still is.
        assert_eq!(layer(&imported.document, "Line #ff0000").shapes.len(), 1);
        assert!(import(svg, "media.svg").is_err());
        for broken in [
            "<svg><style>@media screen { rect { x: y; }</style></svg>",
            "<svg><style>@import</style></svg>",
            "<svg><style>rect } {</style></svg>",
            "<svg><style>rect:not(:not(.a)) { stroke: red; }</style><rect/></svg>",
        ] {
            let result = import_with_warnings(broken, "broken.svg");
            assert!(
                result.is_err() || result.is_ok_and(|i| !i.warnings.is_empty()),
                "{broken}"
            );
        }
    }

    #[test]
    fn text_masks_and_filters_warn_by_name() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg">
          <text id="title" x="0" y="10" font-size="12">Hello <tspan>  wide   world</tspan></text>
          <text display="none">hidden</text>
          <g visibility="hidden"><text>also hidden</text></g>
          <rect id="masked" width="1" height="1" mask="url(#m)"/>
          <g filter="url(#shadow)"><rect width="1" height="1"/></g>
          <rect width="1" height="1" style="filter:none; mask:none"/>
        </svg>"##;
        let imported = lenient(svg);
        assert_eq!(imported.document.shape_count(), 3);
        assert_eq!(imported.warnings.len(), 3, "{:?}", imported.warnings);
        assert!(imported.warnings[0].contains(r#"<text id="title"> "Hello wide world""#));
        assert!(imported.warnings[0].contains("convert text to paths"));
        assert!(imported.warnings[1].contains(r#"<rect id="masked">: mask ignored"#));
        assert!(imported.warnings[2].contains("<g>: filter ignored"));
        // Masks and filters do not inherit, so the filtered group warns once.
        assert!(import(svg, "text.svg").is_err());
    }

    #[test]
    fn warnings_are_capped() {
        let svg = format!(
            "<svg>{}</svg>",
            (0..200)
                .map(|i| format!("<text id=\"t{i}\">x</text>"))
                .collect::<String>()
        );
        let imported = lenient(&svg);
        assert_eq!(imported.warnings.len(), MAX_SVG_WARNINGS + 1);
        assert!(imported.warnings[MAX_SVG_WARNINGS].contains("136 more"));
    }
}
