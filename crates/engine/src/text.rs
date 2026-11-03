//! Live text: text objects that stay editable, laid out into cut paths.
//!
//! A text shape keeps its parameters ([`TextSpec`]) beside the outlines in `path`; `text.edit`
//! lays it out again. Outline fonts come from the system (or a font file) through
//! `vectorcraft-text` (font database, shaping, kerning, glyph outlines); the built-in
//! [`SINGLE_LINE_FAMILY`] is single-stroke lettering for pens and engraving. Everything after
//! the straight layout is ours: justification, letter nudges, curve and text on a path (each
//! letter placed rigidly on the circle or path), and weld (a union of the letter outlines).
//!
//! Every input is untrusted: sizes, counts and coordinates are capped and checked finite, and
//! font files are read with a size cap and parsed by bounds-checked readers.

mod single_line;

use std::io::Read as _;

use kurbo::{Affine, BezPath, PathEl, Point, Vec2};
use serde::{Deserialize, Serialize};
use stencilcraft_doc::{
    CutStyle, FontSpec, IDENTITY_TRANSFORM, LetterOffset, TextAlign, TextFlow, TextSpec,
};
use vectorcraft_engine::doc::{CharStyle, Justify, TextObject};
use vectorcraft_geom::{FillRule, PathData};
use vectorcraft_text::{FontDb, FontMatch};

use crate::{Command, Effect, Engine, EngineError, POINTS_PER_MM, path_element_is_finite};

/// The built-in single-stroke family (pens, scoring, engraving); always available.
pub const SINGLE_LINE_FAMILY: &str = "StencilCraft Single Line";

const MAX_TEXT_BYTES: usize = 10_000;
const MAX_LETTER_OFFSETS: usize = 10_000;
const MAX_FONT_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_COORD_MM: f64 = 1.0e6;
const MAX_SIZE_PT: f64 = 5000.0;
const MAX_TRACK_POINTS: usize = 1_000_000;
/// How far a re-laid-out outline may sit from the shape's path and still count as the same
/// text moved by an affine (mm).
const MATCH_TOLERANCE_MM: f64 = 1.0e-3;

/// `text.add` params: a [`TextSpec`] plus the cut style (default cut; engrave for the
/// single-line family).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextAdd {
    #[serde(flatten)]
    pub spec: TextSpec,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<CutStyle>,
}

/// `text.edit` params: the fields to change; the rest keep their values.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TextEdit {
    pub id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<FontSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alignment: Option<TextAlign>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub letter_spacing: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_spacing: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kerning: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertical: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weld: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flow: Option<TextFlow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub letters: Option<Vec<LetterOffset>>,
}

/// What `text.ungroup` splits a text object into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UngroupTo {
    Lines,
    Letters,
}

/// A monogram's frame, drawn in code.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MonogramFrame {
    None,
    #[default]
    Circle,
    Square,
    Diamond,
    Scallop,
}

/// `text.monogram` params: up to four initials, centred on `x, y`, with a frame round them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextMonogram {
    pub initials: String,
    pub font: FontSpec,
    #[serde(default = "monogram_size")]
    pub size: f64,
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub frame: MonogramFrame,
    /// Gap between the letters and the frame (mm).
    #[serde(default = "monogram_padding")]
    pub padding_mm: f64,
}

fn monogram_size() -> f64 {
    72.0
}

fn monogram_padding() -> f64 {
    3.0
}

/// One family for the font picker.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FontFamily {
    pub family: String,
    pub styles: Vec<String>,
    /// Single-stroke (pen) lettering rather than outlines.
    pub single_line: bool,
}

/// Runs a text command (the `text.*` group of [`Command`]).
pub(crate) fn run(engine: &mut Engine, command: Command) -> Result<Effect, EngineError> {
    match command {
        Command::TextAdd(add) => add_text(engine, add),
        Command::TextEdit(edit) => edit_text(engine, edit),
        Command::TextToOutlines { ids } => to_outlines(engine, &ids),
        Command::TextUngroup { id, to } => ungroup(engine, id, to),
        Command::TextFonts { query } => Ok(Effect::Fonts {
            families: fonts(&query),
        }),
        Command::TextMonogram(monogram) => add_monogram(engine, monogram),
        other => Err(EngineError::Internal(format!(
            "text: {other:?} is not a text command"
        ))),
    }
}

fn invalid(command: &str, message: impl std::fmt::Display) -> EngineError {
    EngineError::Invalid(format!("{command}: {message}"))
}

fn is_single_line(font: &FontSpec) -> bool {
    font.file.is_none() && font.family.trim().eq_ignore_ascii_case(SINGLE_LINE_FAMILY)
}

fn add_text(engine: &mut Engine, add: TextAdd) -> Result<Effect, EngineError> {
    const NAME: &str = "text.add";
    let TextAdd { spec, style } = add;
    let spec = prepare(engine, spec, NAME)?;
    let path = layout(&spec, NAME)?;
    let style = style.unwrap_or(if is_single_line(&spec.font) {
        CutStyle::Engrave
    } else {
        CutStyle::Cut
    });
    let id = push_text(engine, path, style, Some(spec))?;
    Ok(Effect::ShapeAdded { id })
}

/// Adds a shape on the topmost layer of `style` (a new one when there is none).
fn push_text(
    engine: &mut Engine,
    path: BezPath,
    style: CutStyle,
    spec: Option<TextSpec>,
) -> Result<u64, EngineError> {
    let layer = match engine.doc.layers.iter().rposition(|l| l.kind == style) {
        Some(index) => index,
        None => {
            let name = match style {
                CutStyle::Cut => "cut",
                CutStyle::Engrave => "engrave",
            };
            engine
                .doc
                .layers
                .push(stencilcraft_doc::Layer::new(name, style));
            engine.doc.layers.len() - 1
        }
    };
    let id = engine.doc.push_shape(layer, path, style)?.id;
    if let Some(shape) = engine.doc.get_shape_mut(id) {
        shape.text = spec.map(Box::new);
    }
    Ok(id)
}

/// Checks a spec from outside and resolves what it refers to (a path shape, a font file).
fn prepare(engine: &Engine, mut spec: TextSpec, command: &str) -> Result<TextSpec, EngineError> {
    if spec.text.len() > MAX_TEXT_BYTES {
        return Err(invalid(
            command,
            format!("text is longer than {MAX_TEXT_BYTES} bytes"),
        ));
    }
    if spec.text.trim().is_empty() {
        return Err(invalid(command, "text is empty"));
    }
    let finite = |v: f64, what: &str, lo: f64, hi: f64| {
        if v.is_finite() && (lo..=hi).contains(&v) {
            Ok(())
        } else {
            Err(invalid(
                command,
                format!("{what} must be a number from {lo} to {hi}, not {v}"),
            ))
        }
    };
    finite(spec.size, "size (pt)", 0.01, MAX_SIZE_PT)?;
    finite(spec.x, "x", -MAX_COORD_MM, MAX_COORD_MM)?;
    finite(spec.y, "y", -MAX_COORD_MM, MAX_COORD_MM)?;
    finite(spec.letter_spacing, "letter_spacing (%)", -100.0, 1000.0)?;
    finite(spec.line_spacing, "line_spacing (%)", 1.0, 1000.0)?;
    if spec.letters.len() > MAX_LETTER_OFFSETS {
        return Err(invalid(command, "too many letter offsets"));
    }
    for l in &spec.letters {
        finite(l.dx_mm, "letter dx_mm", -MAX_COORD_MM, MAX_COORD_MM)?;
        finite(l.dy_mm, "letter dy_mm", -MAX_COORD_MM, MAX_COORD_MM)?;
        finite(l.rotate_deg, "letter rotate_deg", -36_000.0, 36_000.0)?;
    }
    let xf = Affine::new(spec.transform);
    if !spec.transform.iter().all(|v| v.is_finite()) || xf.determinant().abs() < 1e-12 {
        return Err(invalid(command, "transform must be finite and invertible"));
    }
    if spec.font.family.trim().is_empty() && spec.font.file.is_none() {
        return Err(invalid(command, "font needs a family or a file"));
    }
    if let Some(file) = spec.font.file.clone() {
        let families = load_font_file(FontDb::global(), &file)
            .map_err(|e| invalid(command, format!("font file {file}: {e}")))?;
        if spec.font.family.trim().is_empty() {
            spec.font.family = families.into_iter().next().unwrap_or_default();
        }
    }
    let one_line = !spec.text.contains('\n') && !spec.vertical;
    match &mut spec.flow {
        TextFlow::Straight => {}
        TextFlow::Curve { diameter_mm } => {
            if *diameter_mm == 0.0 {
                spec.flow = TextFlow::Straight;
            } else {
                finite(diameter_mm.abs(), "curve diameter_mm", 0.1, MAX_COORD_MM)?;
                if !one_line {
                    return Err(invalid(
                        command,
                        "curve works on one horizontal line; ungroup to lines first",
                    ));
                }
            }
        }
        TextFlow::OnPath {
            path,
            shape,
            start,
            offset_mm,
            flip: _,
        } => {
            finite(*start, "path start", -1000.0, 1000.0)?;
            finite(*offset_mm, "path offset_mm", -MAX_COORD_MM, MAX_COORD_MM)?;
            if !one_line {
                return Err(invalid(
                    command,
                    "text on a path works on one horizontal line; ungroup to lines first",
                ));
            }
            if path.elements().is_empty() {
                let Some(id) = *shape else {
                    return Err(invalid(command, "on-path flow needs a path or a shape id"));
                };
                let source = engine
                    .doc
                    .get_shape(id)
                    .ok_or_else(|| invalid(command, format!("no shape has id {id}")))?;
                // The path lives in the text's own space, before `transform`.
                *path = xf.inverse() * source.path.clone();
            }
            if !path.elements().iter().all(path_element_is_finite)
                || Track::new(path, false).is_none()
            {
                return Err(invalid(command, "the path to follow has no length"));
            }
        }
    }
    Ok(spec)
}

/// A glyph in straight text space: mm, y up, the anchor at the origin.
#[derive(Debug, Clone)]
struct Glyph {
    outline: BezPath,
    /// Pen position on the baseline.
    origin: Point,
    advance: f64,
    line: usize,
    byte: usize,
    len: usize,
}

/// Lays a spec out to its cut path in document mm.
fn layout(spec: &TextSpec, command: &str) -> Result<BezPath, EngineError> {
    let glyphs = straight(spec, command)?;
    let placements = placements(spec, &glyphs);
    let pieces: Vec<BezPath> = glyphs
        .iter()
        .zip(&placements)
        .filter(|(g, _)| !g.outline.elements().is_empty())
        .map(|(g, xf)| *xf * g.outline.clone())
        .collect();
    let path = if spec.weld && !is_single_line(&spec.font) {
        weld(&pieces)
    } else {
        let mut all = BezPath::new();
        for piece in &pieces {
            all.extend(piece.iter());
        }
        all
    };
    if path.elements().is_empty() {
        return Err(invalid(command, "the text has no letters to cut"));
    }
    if !path.elements().iter().all(path_element_is_finite) {
        return Err(invalid(
            command,
            "the text lays out beyond finite coordinates",
        ));
    }
    Ok(path)
}

/// Merges overlapping letters into one outline (nonzero fill keeps the counters).
fn weld(pieces: &[BezPath]) -> BezPath {
    let data: Vec<PathData> = pieces.iter().map(PathData::from_bezpath).collect();
    let inputs: Vec<(&PathData, FillRule)> = data.iter().map(|d| (d, FillRule::NonZero)).collect();
    match vectorcraft_pathops::try_unite_all(&inputs) {
        Ok(united) if !united.subpaths.is_empty() => united.to_bezpath(),
        // A union the boolean engine refuses keeps the letters as they are.
        _ => {
            let mut all = BezPath::new();
            for piece in pieces {
                all.extend(piece.iter());
            }
            all
        }
    }
}

/// The straight layout (lines or columns at the anchor), justified.
fn straight(spec: &TextSpec, command: &str) -> Result<Vec<Glyph>, EngineError> {
    let mut glyphs = if is_single_line(&spec.font) {
        single_line_glyphs(spec)
    } else {
        outline_glyphs(spec, FontDb::global(), command)?
    };
    if spec.alignment == TextAlign::Justify && !spec.vertical {
        justify(&mut glyphs);
    }
    Ok(glyphs)
}

fn outline_glyphs(spec: &TextSpec, db: &FontDb, command: &str) -> Result<Vec<Glyph>, EngineError> {
    if let Some(file) = &spec.font.file {
        load_font_file(db, file).map_err(|e| invalid(command, format!("font file {file}: {e}")))?;
    }
    let face = match db.resolve(&spec.font.family, &spec.font.style) {
        Some((face, m)) if !matches!(m, FontMatch::Missing) => face,
        _ => {
            return Err(invalid(
                command,
                format!("font family {:?} is not installed", spec.font.family),
            ));
        }
    };
    let style = CharStyle {
        font_family: face.family.clone(),
        font_style: face.style.clone(),
        size: spec.size,
        leading: Some(spec.size * 1.2 * spec.line_spacing / 100.0),
        // Tracking is in 1/1000 em.
        tracking: spec.letter_spacing * 10.0,
        kerning: (!spec.kerning).then_some(0.0),
        ..CharStyle::default()
    };
    let mut object = TextObject::point(Point::ZERO, &spec.text, style);
    object.vertical = spec.vertical;
    object.para.justify = match spec.alignment {
        TextAlign::Left | TextAlign::Justify => Justify::Left,
        TextAlign::Centre => Justify::Center,
        TextAlign::Right => Justify::Right,
    };
    let laid = vectorcraft_text::layout(db, &object);
    // Text space is points, y down; ours is mm, y up.
    let to_mm = Affine::scale_non_uniform(1.0 / POINTS_PER_MM, -1.0 / POINTS_PER_MM);
    Ok(laid
        .glyphs
        .iter()
        .map(|g| Glyph {
            outline: to_mm * g.outline.clone(),
            origin: to_mm * g.origin,
            advance: g.advance / POINTS_PER_MM,
            line: g.line,
            byte: g.byte,
            len: g.len.max(1),
        })
        .collect())
}

fn single_line_glyphs(spec: &TextSpec) -> Vec<Glyph> {
    let em = spec.size / POINTS_PER_MM;
    let unit = em / single_line::UNITS_PER_EM;
    let track = spec.letter_spacing / 100.0 * em;
    let line_gap = 1.2 * em * spec.line_spacing / 100.0;
    let mut out = Vec::new();
    let mut line_start = 0;
    for (line, text) in spec.text.split('\n').enumerate() {
        let mut pen = 0.0;
        let mut row = Vec::new();
        for (byte, c) in text.char_indices() {
            let (advance, strokes) = single_line::glyph(c);
            let width = advance * unit;
            let step = if spec.vertical { em } else { width } + track;
            row.push((line_start + byte, c.len_utf8(), pen, step, width, strokes));
            pen += step;
        }
        let length = (pen - track).max(0.0);
        let start = match spec.alignment {
            TextAlign::Left | TextAlign::Justify => 0.0,
            TextAlign::Centre => -length / 2.0,
            TextAlign::Right => -length,
        };
        let across = -(line as f64) * line_gap;
        for (byte, len, at, step, width, strokes) in row {
            let origin = if spec.vertical {
                // Columns run top to bottom, right to left; letters centred on the column.
                Point::new(across - width / 2.0, -(start + at) - 0.75 * em)
            } else {
                Point::new(start + at, across)
            };
            let outline = Affine::translate(origin.to_vec2()) * Affine::scale(unit) * strokes;
            out.push(Glyph {
                outline,
                origin,
                advance: step,
                line,
                byte,
                len,
            });
        }
        line_start += text.len() + 1;
    }
    out
}

/// Stretches every line but the last to the widest line, spreading the extra space evenly
/// between its letters.
fn justify(glyphs: &mut [Glyph]) {
    let Some(last) = glyphs.iter().map(|g| g.line).max() else {
        return;
    };
    let extent = |glyphs: &[Glyph], line: usize| {
        glyphs
            .iter()
            .filter(|g| g.line == line && !g.outline.elements().is_empty())
            .fold(None, |acc: Option<(f64, f64)>, g| {
                let (a, b) = (g.origin.x, g.origin.x + g.advance);
                Some(acc.map_or((a, b), |(lo, hi)| (lo.min(a), hi.max(b))))
            })
    };
    let widest = (0..=last)
        .filter_map(|line| extent(glyphs, line).map(|(a, b)| b - a))
        .fold(0.0, f64::max);
    for line in 0..last {
        let Some((lo, hi)) = extent(glyphs, line) else {
            continue;
        };
        let count = glyphs.iter().filter(|g| g.line == line).count();
        if count < 2 {
            continue;
        }
        let step = (widest - (hi - lo)) / (count - 1) as f64;
        for (k, g) in glyphs.iter_mut().filter(|g| g.line == line).enumerate() {
            let shift = Vec2::new(step * k as f64, 0.0);
            g.origin += shift;
            g.outline.apply_affine(Affine::translate(shift));
        }
    }
}

/// Each glyph's transform from straight text space to the document.
fn placements(spec: &TextSpec, glyphs: &[Glyph]) -> Vec<Affine> {
    let doc = Affine::new(spec.transform);
    let anchor = Affine::translate((spec.x, spec.y));
    let starts: Vec<usize> = spec.text.char_indices().map(|(b, _)| b).collect();
    let char_index = |byte: usize| starts.partition_point(|&b| b < byte);
    let nudge = |g: &Glyph| {
        let index = char_index(g.byte);
        spec.letters
            .iter()
            .rev()
            .find(|l| l.index == index)
            .map_or(Affine::IDENTITY, |l| {
                Affine::translate(g.origin.to_vec2() + Vec2::new(l.dx_mm, l.dy_mm))
                    * Affine::rotate(l.rotate_deg.to_radians())
                    * Affine::translate(-g.origin.to_vec2())
            })
    };
    let centre = |g: &Glyph| g.origin.x + g.advance / 2.0;
    match &spec.flow {
        TextFlow::Straight => glyphs.iter().map(|g| doc * anchor * nudge(g)).collect(),
        TextFlow::Curve { diameter_mm } => {
            let radius = diameter_mm.abs() / 2.0;
            let (lo, hi) = glyphs.iter().fold((f64::MAX, f64::MIN), |(lo, hi), g| {
                (lo.min(g.origin.x), hi.max(g.origin.x + g.advance))
            });
            let middle = if lo <= hi { (lo + hi) / 2.0 } else { 0.0 };
            glyphs
                .iter()
                .map(|g| {
                    let gx = centre(g);
                    let theta = (gx - middle) / radius;
                    let (at, turn) = if *diameter_mm > 0.0 {
                        let c = Vec2::new(middle, -radius);
                        (c + radius * Vec2::new(theta.sin(), theta.cos()), -theta)
                    } else {
                        let c = Vec2::new(middle, radius);
                        (c + radius * Vec2::new(theta.sin(), -theta.cos()), theta)
                    };
                    doc * anchor
                        * Affine::translate(at)
                        * Affine::rotate(turn)
                        * Affine::translate((-gx, 0.0))
                        * nudge(g)
                })
                .collect()
        }
        TextFlow::OnPath {
            path,
            start,
            offset_mm,
            flip,
            ..
        } => {
            let Some(track) = Track::new(path, *flip) else {
                return glyphs.iter().map(|g| doc * anchor * nudge(g)).collect();
            };
            let from = start * track.length();
            glyphs
                .iter()
                .map(|g| {
                    let gx = centre(g);
                    let (at, tangent) = track.at(from + gx);
                    let normal = Vec2::new(-tangent.y, tangent.x);
                    doc * Affine::translate(at.to_vec2() + normal * *offset_mm)
                        * Affine::rotate(tangent.y.atan2(tangent.x))
                        * Affine::translate((-gx, 0.0))
                        * nudge(g)
                })
                .collect()
        }
    }
}

/// A path flattened for walking along by distance.
struct Track {
    points: Vec<Point>,
    /// Distance along the track at each point.
    at: Vec<f64>,
    closed: bool,
}

impl Track {
    /// The first contour of `path`, reversed when `flip`; `None` without length.
    fn new(path: &BezPath, flip: bool) -> Option<Track> {
        let bounds = kurbo::Shape::bounding_box(path);
        let tolerance = (bounds.width().hypot(bounds.height()) * 1e-5).max(1e-4);
        if !tolerance.is_finite() {
            return None;
        }
        let mut points: Vec<Point> = Vec::new();
        let mut closed = false;
        let mut contours = 0;
        kurbo::flatten(path.iter(), tolerance, |el| match el {
            PathEl::MoveTo(p) => {
                contours += 1;
                if contours == 1 {
                    points.push(p);
                }
            }
            PathEl::LineTo(p) if contours == 1 && points.len() < MAX_TRACK_POINTS => {
                points.push(p);
            }
            PathEl::ClosePath if contours == 1 => closed = true,
            _ => {}
        });
        if closed && let Some(first) = points.first().copied() {
            points.push(first);
        }
        points.dedup_by(|a, b| a.distance(*b) < 1e-9);
        if flip {
            points.reverse();
        }
        let mut at = Vec::with_capacity(points.len());
        let mut total = 0.0;
        let mut previous = points.first().copied()?;
        for p in &points {
            total += previous.distance(*p);
            at.push(total);
            previous = *p;
        }
        (points.len() >= 2 && total > 1e-9 && total.is_finite()).then_some(Track {
            points,
            at,
            closed,
        })
    }

    fn length(&self) -> f64 {
        self.at.last().copied().unwrap_or(0.0)
    }

    /// The point `s` along the track and the unit tangent there. Closed tracks wrap; open
    /// ones run on straight past their ends.
    fn at(&self, s: f64) -> (Point, Vec2) {
        let total = self.length();
        let s = if self.closed { s.rem_euclid(total) } else { s };
        let i = self
            .at
            .partition_point(|&d| d <= s)
            .clamp(1, self.points.len().saturating_sub(1));
        let (Some(&a), Some(&b), Some(&da), Some(&db)) = (
            self.points.get(i - 1),
            self.points.get(i),
            self.at.get(i - 1),
            self.at.get(i),
        ) else {
            return (Point::ZERO, Vec2::new(1.0, 0.0));
        };
        let span = (db - da).max(1e-12);
        let tangent = (b - a) / span;
        (a + tangent * (s - da), tangent)
    }
}

/// The affine that takes `expected` (a fresh layout) to `actual` (the shape's path), when the
/// shape has only been moved, turned, scaled or mirrored since; `None` after a path edit.
fn infer_affine(expected: &BezPath, actual: &BezPath) -> Option<Affine> {
    let (p, q) = (anchor_points(expected), anchor_points(actual));
    if p.len() != q.len() || p.is_empty() {
        return None;
    }
    let n = p.len() as f64;
    let mean = |v: &[Point]| v.iter().fold(Vec2::ZERO, |acc, p| acc + p.to_vec2()) / n;
    let (mp, mq) = (mean(&p), mean(&q));
    let (mut sxx, mut sxy, mut syy) = (0.0, 0.0, 0.0);
    let (mut xx, mut yx, mut xy, mut yy) = (0.0, 0.0, 0.0, 0.0);
    for (a, b) in p.iter().zip(&q) {
        let (u, v) = (a.to_vec2() - mp, b.to_vec2() - mq);
        sxx += u.x * u.x;
        sxy += u.x * u.y;
        syy += u.y * u.y;
        xx += u.x * v.x;
        yx += u.y * v.x;
        xy += u.x * v.y;
        yy += u.y * v.y;
    }
    let det = sxx * syy - sxy * sxy;
    let linear = if det.abs() > 1e-12 * (sxx + syy).powi(2).max(1e-300) {
        // Least squares: [a c; b d] from the normal equations.
        let a = (xx * syy - yx * sxy) / det;
        let c = (yx * sxx - xx * sxy) / det;
        let b = (xy * syy - yy * sxy) / det;
        let d = (yy * sxx - xy * sxy) / det;
        [a, b, c, d]
    } else {
        // Collinear points (one straight stroke): a turn and scale about them.
        let norm = sxx + syy;
        if norm <= 1e-18 {
            [1.0, 0.0, 0.0, 1.0]
        } else {
            let cos = (xx + yy) / norm;
            let sin = (xy - yx) / norm;
            [cos, sin, -sin, cos]
        }
    };
    let [a, b, c, d] = linear;
    let t = mq - Vec2::new(a * mp.x + c * mp.y, b * mp.x + d * mp.y);
    let affine = Affine::new([a, b, c, d, t.x, t.y]);
    let fits = p
        .iter()
        .zip(&q)
        .all(|(a, b)| (affine * *a).distance(*b) <= MATCH_TOLERANCE_MM);
    (fits && affine.as_coeffs().iter().all(|v| v.is_finite())).then_some(affine)
}

/// On-curve points of a path, without repeats or explicit closing points, so paths that
/// differ only in how curves and closes are spelled compare equal.
fn anchor_points(path: &BezPath) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::new();
    let mut start: Option<Point> = None;
    let push = |out: &mut Vec<Point>, p: Point| {
        if out
            .last()
            .is_none_or(|l| l.distance(p) > MATCH_TOLERANCE_MM / 10.0)
        {
            out.push(p);
        }
    };
    for el in path.elements() {
        match *el {
            PathEl::MoveTo(p) => {
                start = Some(p);
                push(&mut out, p);
            }
            PathEl::LineTo(p) | PathEl::QuadTo(_, p) | PathEl::CurveTo(_, _, p) => {
                push(&mut out, p);
            }
            PathEl::ClosePath => {
                if let (Some(s), Some(l)) = (start, out.last())
                    && s.distance(*l) <= MATCH_TOLERANCE_MM / 10.0
                    && out.len() > 1
                {
                    out.pop();
                }
            }
        }
    }
    out
}

/// The live spec of shape `id` with the moves made to the shape since its layout folded in.
fn current_spec(engine: &Engine, id: u64, command: &str) -> Result<TextSpec, EngineError> {
    let shape = engine
        .doc
        .get_shape(id)
        .ok_or_else(|| invalid(command, format!("no shape has id {id}")))?;
    let mut spec = shape
        .text
        .as_deref()
        .cloned()
        .ok_or_else(|| invalid(command, format!("shape {id} is not live text")))?;
    let expected = layout(&spec, command)?;
    let moved = if expected == shape.path {
        Affine::IDENTITY
    } else {
        infer_affine(&expected, &shape.path).ok_or_else(|| {
            invalid(
                command,
                format!(
                    "shape {id} was edited as a path since its text was laid out; \
                     undo the edit or keep it with text.to-outlines"
                ),
            )
        })?
    };
    spec.transform = (moved * Affine::new(spec.transform)).as_coeffs();
    fold_translation(&mut spec);
    Ok(spec)
}

/// A transform that only moves is kept in `x, y` (and an on-path path) instead.
fn fold_translation(spec: &mut TextSpec) {
    let [a, b, c, d, e, f] = spec.transform;
    let near = |v: f64, w: f64| (v - w).abs() <= 1e-12;
    if !(near(a, 1.0) && near(b, 0.0) && near(c, 0.0) && near(d, 1.0)) {
        return;
    }
    spec.x += e;
    spec.y += f;
    if let TextFlow::OnPath { path, .. } = &mut spec.flow {
        path.apply_affine(Affine::translate((e, f)));
    }
    spec.transform = IDENTITY_TRANSFORM;
}

fn edit_text(engine: &mut Engine, edit: TextEdit) -> Result<Effect, EngineError> {
    const NAME: &str = "text.edit";
    let id = edit.id;
    let mut spec = current_spec(engine, id, NAME)?;
    let TextEdit {
        id: _,
        text,
        font,
        size,
        x,
        y,
        alignment,
        letter_spacing,
        line_spacing,
        kerning,
        vertical,
        weld,
        flow,
        letters,
    } = edit;
    if let Some(v) = text {
        spec.text = v;
    }
    if let Some(v) = font {
        spec.font = v;
    }
    if let Some(v) = size {
        spec.size = v;
    }
    if let Some(v) = x {
        spec.x = v;
    }
    if let Some(v) = y {
        spec.y = v;
    }
    if let Some(v) = alignment {
        spec.alignment = v;
    }
    if let Some(v) = letter_spacing {
        spec.letter_spacing = v;
    }
    if let Some(v) = line_spacing {
        spec.line_spacing = v;
    }
    if let Some(v) = kerning {
        spec.kerning = v;
    }
    if let Some(v) = vertical {
        spec.vertical = v;
    }
    if let Some(v) = weld {
        spec.weld = v;
    }
    if let Some(v) = flow {
        spec.flow = v;
    }
    if let Some(v) = letters {
        spec.letters = v;
    }
    let spec = prepare(engine, spec, NAME)?;
    let path = layout(&spec, NAME)?;
    let shape = engine
        .doc
        .get_shape_mut(id)
        .ok_or_else(|| invalid(NAME, format!("no shape has id {id}")))?;
    shape.path = path;
    shape.text = Some(Box::new(spec));
    Ok(Effect::ShapesEdited { ids: vec![id] })
}

fn to_outlines(engine: &mut Engine, ids: &[u64]) -> Result<Effect, EngineError> {
    const NAME: &str = "text.to-outlines";
    let ids = crate::unique_ids(ids, NAME)?;
    for id in &ids {
        let shape = engine
            .doc
            .get_shape(*id)
            .ok_or_else(|| invalid(NAME, format!("no shape has id {id}")))?;
        if shape.text.is_none() {
            return Err(invalid(NAME, format!("shape {id} is not live text")));
        }
    }
    for id in &ids {
        if let Some(shape) = engine.doc.get_shape_mut(*id) {
            shape.text = None;
        }
    }
    Ok(Effect::ShapesEdited { ids })
}

/// The first glyph's straight origin of `spec` laid out at the origin.
fn first_origin(spec: &TextSpec, command: &str) -> Result<Point, EngineError> {
    let glyphs = straight(spec, command)?;
    Ok(glyphs
        .iter()
        .find(|g| !g.outline.elements().is_empty())
        .or(glyphs.first())
        .map_or(Point::ZERO, |g| g.origin))
}

/// A piece of `spec` (its text and letter nudges from `range`) without position.
fn piece(spec: &TextSpec, range: std::ops::Range<usize>) -> Option<TextSpec> {
    let text = spec.text.get(range.clone())?.to_string();
    let first = spec.text.get(..range.start)?.chars().count();
    let count = text.chars().count();
    let letters = spec
        .letters
        .iter()
        .filter(|l| (first..first + count).contains(&l.index))
        .map(|l| LetterOffset {
            index: l.index - first,
            ..*l
        })
        .collect();
    Some(TextSpec {
        text,
        x: 0.0,
        y: 0.0,
        letters,
        transform: IDENTITY_TRANSFORM,
        ..spec.clone()
    })
}

fn ungroup(engine: &mut Engine, id: u64, to: UngroupTo) -> Result<Effect, EngineError> {
    const NAME: &str = "text.ungroup";
    let spec = current_spec(engine, id, NAME)?;
    let glyphs = straight(&spec, NAME)?;
    let mut pieces: Vec<TextSpec> = Vec::new();
    match to {
        UngroupTo::Letters => {
            let placed = placements(&spec, &glyphs);
            for (g, xf) in glyphs.iter().zip(placed) {
                if g.outline.elements().is_empty() {
                    continue;
                }
                let Some(mut letter) = piece(&spec, g.byte..g.byte + g.len) else {
                    continue;
                };
                letter.alignment = TextAlign::Left;
                letter.flow = TextFlow::Straight;
                letter.letters.clear();
                // The nudge, curve or path placement becomes the letter's own transform.
                let own = first_origin(&letter, NAME)?;
                let whole = xf * Affine::translate(g.origin - own);
                let at = whole * Point::ZERO;
                letter.x = at.x;
                letter.y = at.y;
                letter.transform = (whole * Affine::translate(-at.to_vec2())).as_coeffs();
                fold_translation(&mut letter);
                pieces.push(letter);
            }
        }
        UngroupTo::Lines => {
            if !spec.flow.is_straight() {
                return Err(invalid(NAME, "curved or on-path text is one line already"));
            }
            let mut start = 0;
            for (index, text) in spec.text.split('\n').enumerate() {
                let range = start..start + text.len();
                start = range.end + 1;
                let Some(first) = glyphs
                    .iter()
                    .find(|g| g.line == index && !g.outline.elements().is_empty())
                else {
                    continue;
                };
                let Some(mut line) = piece(&spec, range) else {
                    continue;
                };
                if line.alignment == TextAlign::Justify {
                    line.alignment = TextAlign::Left;
                }
                let shift = first.origin - first_origin(&line, NAME)?;
                line.x = spec.x + shift.x;
                line.y = spec.y + shift.y;
                line.transform = spec.transform;
                pieces.push(line);
            }
        }
    }
    if pieces.len() < 2 {
        return Err(invalid(NAME, format!("shape {id} has nothing to split")));
    }
    let mut laid = Vec::with_capacity(pieces.len());
    for p in pieces {
        laid.push((layout(&p, NAME)?, p));
    }
    // The pieces take the shape's place: the first keeps its id, the rest follow it.
    let mut next_id = engine.doc.next_shape_id;
    let (layer, index) = engine
        .doc
        .layers
        .iter_mut()
        .find_map(|l| Some((l.shapes.iter().position(|s| s.id == id)?, l)))
        .map(|(i, l)| (l, i))
        .ok_or_else(|| invalid(NAME, format!("no shape has id {id}")))?;
    let template = layer
        .shapes
        .get(index)
        .cloned()
        .ok_or_else(|| EngineError::Internal(format!("{NAME}: shape {id} vanished")))?;
    let mut ids = Vec::with_capacity(laid.len());
    let mut shapes = Vec::with_capacity(laid.len());
    for (k, (path, spec)) in laid.into_iter().enumerate() {
        let shape_id = if k == 0 {
            id
        } else {
            let fresh = next_id;
            next_id = next_id
                .checked_add(1)
                .ok_or_else(|| EngineError::Internal("shape ids exhausted".to_string()))?;
            fresh
        };
        ids.push(shape_id);
        shapes.push(stencilcraft_doc::Shape {
            id: shape_id,
            path,
            text: Some(Box::new(spec)),
            ..template.clone()
        });
    }
    layer.shapes.splice(index..=index, shapes);
    engine.doc.next_shape_id = next_id;
    Ok(Effect::ShapesAdded { ids })
}

/// Families for the font picker: the built-in single-line family, then the installed ones
/// (and font files loaded so far) whose name contains `query` (any case).
pub fn fonts(query: &str) -> Vec<FontFamily> {
    let query = query.trim().to_lowercase();
    let wanted = |name: &str| query.is_empty() || name.to_lowercase().contains(&query);
    let mut out = Vec::new();
    if wanted(SINGLE_LINE_FAMILY) {
        out.push(FontFamily {
            family: SINGLE_LINE_FAMILY.to_string(),
            styles: vec!["Regular".to_string()],
            single_line: true,
        });
    }
    let db = FontDb::global();
    for family in db.families() {
        if wanted(&family) {
            out.push(FontFamily {
                styles: db.styles(&family),
                family,
                single_line: false,
            });
        }
    }
    out
}

/// Reads a font file (at most 64 MiB) into `db`; returns the families it holds.
fn load_font_file(db: &FontDb, path: &str) -> Result<Vec<String>, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(MAX_FONT_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_FONT_FILE_BYTES {
        return Err(format!(
            "larger than {} MiB",
            MAX_FONT_FILE_BYTES / 1024 / 1024
        ));
    }
    load_font_bytes(db, bytes)
}

/// Adds font bytes (TTF, OTF, TTC; untrusted) to `db`; returns the families they hold, or why
/// none could be read. Parsing is bounds-checked: bad bytes are an error, never a crash.
pub fn load_font_bytes(db: &FontDb, bytes: Vec<u8>) -> Result<Vec<String>, String> {
    // A private database tells which families these bytes hold.
    let probe = FontDb::with_font_dirs(Vec::new());
    probe.set_system_fallback(false);
    let before = probe.families();
    if probe.add_font(bytes.clone()) == 0 {
        return Err("no font faces could be read".to_string());
    }
    let families: Vec<String> = probe
        .families()
        .into_iter()
        .filter(|f| !before.contains(f))
        .collect();
    if families.is_empty() {
        return Err("the font faces have no family name".to_string());
    }
    db.add_font(bytes);
    Ok(families)
}

fn add_monogram(engine: &mut Engine, m: TextMonogram) -> Result<Effect, EngineError> {
    const NAME: &str = "text.monogram";
    let initials: String = m.initials.chars().filter(|c| !c.is_whitespace()).collect();
    let count = initials.chars().count();
    if !(1..=4).contains(&count) {
        return Err(invalid(NAME, "a monogram has one to four initials"));
    }
    if !(m.padding_mm.is_finite() && (0.0..=1000.0).contains(&m.padding_mm)) {
        return Err(invalid(NAME, "padding_mm must be from 0 to 1000"));
    }
    let mut spec = TextSpec {
        text: initials,
        font: m.font,
        size: m.size,
        x: m.x,
        y: m.y,
        alignment: TextAlign::Centre,
        letter_spacing: 0.0,
        line_spacing: 100.0,
        kerning: true,
        vertical: false,
        weld: false,
        flow: TextFlow::Straight,
        letters: Vec::new(),
        transform: IDENTITY_TRANSFORM,
    };
    spec = prepare(engine, spec, NAME)?;
    let first = layout(&spec, NAME)?;
    let bounds = kurbo::Shape::bounding_box(&first);
    spec.x += m.x - bounds.center().x;
    spec.y += m.y - bounds.center().y;
    let path = layout(&spec, NAME)?;
    let style = if is_single_line(&spec.font) {
        CutStyle::Engrave
    } else {
        CutStyle::Cut
    };
    let bounds = kurbo::Shape::bounding_box(&path);
    let frame = frame_path(m.frame, bounds, m.padding_mm);
    let text_id = push_text(engine, path, style, Some(spec))?;
    let mut ids = vec![text_id];
    if let Some(frame) = frame {
        ids.push(push_text(engine, frame, CutStyle::Cut, None)?);
    }
    Ok(Effect::ShapesAdded { ids })
}

/// Our built-in monogram frames round `bounds`, `padding` clear of it.
fn frame_path(frame: MonogramFrame, bounds: kurbo::Rect, padding: f64) -> Option<BezPath> {
    use kurbo::Shape as _;
    let c = bounds.center();
    let (hw, hh) = (
        bounds.width() / 2.0 + padding,
        bounds.height() / 2.0 + padding,
    );
    let radius = hw.hypot(hh);
    Some(match frame {
        MonogramFrame::None => return None,
        MonogramFrame::Circle => kurbo::Circle::new(c, radius).to_path(0.01),
        MonogramFrame::Square => {
            let h = hw.max(hh);
            kurbo::Rect::new(c.x - h, c.y - h, c.x + h, c.y + h).to_path(0.01)
        }
        MonogramFrame::Diamond => {
            let h = hw + hh;
            let mut p = BezPath::new();
            p.move_to((c.x, c.y + h));
            p.line_to((c.x + h, c.y));
            p.line_to((c.x, c.y - h));
            p.line_to((c.x - h, c.y));
            p.close_path();
            p
        }
        MonogramFrame::Scallop => {
            const BUMPS: usize = 16;
            let step = std::f64::consts::TAU / BUMPS as f64;
            let at = |k: usize| c + radius * Vec2::from_angle(step * k as f64);
            let mut p = BezPath::new();
            p.move_to(at(0));
            for k in 0..BUMPS {
                let (a, b) = (at(k), at(k + 1));
                let mid = a.midpoint(b);
                let start = (a - mid).atan2();
                let arc = kurbo::Arc::new(
                    mid,
                    Vec2::splat(a.distance(b) / 2.0),
                    start,
                    std::f64::consts::PI,
                    0.0,
                );
                for el in arc.append_iter(0.01) {
                    p.push(el);
                }
            }
            p.close_path();
            p
        }
    })
}

#[cfg(test)]
mod tests;
