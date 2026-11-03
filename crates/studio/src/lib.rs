//! Reader for Silhouette Studio documents: `.studio` (header `silhouette04;`) and
//! `.studio3` (header `silhouette05;`).
//!
//! Everything here comes from differential analysis of documents we drew and saved
//! ourselves in Silhouette Studio; see `docs/formats/silhouette-studio.md` for the
//! layout, the sample pairs behind each fact, and what is still unknown.
//!
//! The reader is bounded and fails closed: input size and shape/point counts are
//! capped, every length is checked against the bytes that are really there, and a
//! body that is not in the plain layout we know (for example a protected store
//! design) is rejected instead of guessed at. Records we do not understand are skipped.

use kurbo::{Affine, BezPath, Point, Vec2};
use stencilcraft_doc::format::{Capabilities, Converted, Fidelity, FormatHandler};
use stencilcraft_doc::{CutStyle, Document, Layer};

/// Largest file we read (store designs and our samples are far smaller).
pub const MAX_FILE_BYTES: usize = 64 * 1024 * 1024;
const MAX_SHAPES: usize = 100_000;
const MAX_POINTS: usize = 2_000_000;
/// Coordinates are millimetres; anything past a kilometre is corruption.
const MAX_COORD_MM: f32 = 1.0e6;

const MAGIC_LEN: usize = 13;
/// `.studio3` files start with a 1024-byte header holding a table of section lengths.
const V3_HEADER_LEN: usize = 1024;
const V3_TABLE_START: usize = 0x10;
const V3_TABLE_END: usize = 0x3e8;

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

/// Which container the bytes use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    /// `.studio`: the document body alone, header `silhouette04;`.
    V2,
    /// `.studio3`: a 1024-byte section table, then the body (header `silhouette05;`),
    /// a thumbnail PNG and further sections.
    V3,
}

/// A successful import: the document plus notes about what was skipped or assumed.
#[derive(Debug, Clone)]
pub struct Imported {
    pub doc: Document,
    pub version: Version,
    pub warnings: Vec<String>,
}

/// Is this file name one we read?
pub fn is_studio_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".studio") || lower.ends_with(".studio3")
}

/// Cheap header check, for dispatch by content rather than extension.
pub fn sniff(bytes: &[u8]) -> Option<Version> {
    match bytes.get(..MAGIC_LEN)? {
        b"silhouette04;" => Some(Version::V2),
        b"silhouette05;" => Some(Version::V3),
        _ => None,
    }
}

/// Reads a `.studio` / `.studio3` document into a StencilCraft document.
pub fn import(bytes: &[u8], name: &str) -> Result<Imported, StudioError> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(StudioError::TooLarge(name.to_string()));
    }
    let bad = |what: &str| StudioError::Malformed {
        name: name.to_string(),
        what: what.to_string(),
    };
    let magic = bytes
        .get(..MAGIC_LEN)
        .ok_or_else(|| StudioError::NotStudio(name.to_string()))?;
    if !magic.starts_with(b"silhouette") || magic.last() != Some(&b';') {
        return Err(StudioError::NotStudio(name.to_string()));
    }
    let (version, body) = match magic {
        b"silhouette04;" => (Version::V2, bytes),
        // A bare body (header followed directly by the root record) is read as is.
        b"silhouette05;" if bytes.get(MAGIC_LEN) == Some(&1) => (Version::V3, bytes),
        b"silhouette05;" => (Version::V3, v3_body(bytes, name)?),
        _ => {
            let version = String::from_utf8_lossy(magic.get(10..12).unwrap_or_default());
            return Err(StudioError::Unsupported {
                name: name.to_string(),
                version: version.into_owned(),
            });
        }
    };
    let mut warnings = Vec::new();
    let root = block(body, MAGIC_LEN).ok_or_else(|| bad("root record is truncated"))?;

    let mut doc = Document {
        layers: vec![Layer::new("cut", CutStyle::Cut)],
        ..Document::default()
    };
    let mut points_total = 0usize;
    let mut cursor = root.start;
    while let Some((at, path)) = find_path(body, cursor, root.end) {
        cursor = path.end;
        if doc.shape_count() >= MAX_SHAPES {
            return Err(bad("more than 100,000 shapes"));
        }
        points_total = points_total.saturating_add(path.nodes.len());
        if points_total > MAX_POINTS {
            return Err(bad("more than 2,000,000 points"));
        }
        let bez = to_bez(&path);
        let limit = f64::from(MAX_COORD_MM);
        let wild = |p: Point| !p.is_finite() || p.x.abs() > limit || p.y.abs() > limit;
        if bez
            .elements()
            .iter()
            .any(|el| el.end_point().is_some_and(wild))
        {
            warnings.push(format!(
                "skipped a path at byte {at}: coordinates out of range"
            ));
            continue;
        }
        doc.push_shape(0, bez, CutStyle::Cut)
            .map_err(|e| bad(&e.to_string()))?;
    }

    match page_size(body, &root) {
        Some((w, h)) => {
            doc.board.width_mm = w;
            doc.board.height_mm = h;
        }
        None => {
            warnings.push("page size not found; using the default sheet".to_string());
        }
    }
    doc.validate().map_err(|e| bad(&e.to_string()))?;
    Ok(Imported {
        doc,
        version,
        warnings,
    })
}

/// `.studio3`: the header is a table of little-endian u64 section lengths; the
/// sections follow the header back to back and section 0 is the document body.
fn v3_body<'a>(bytes: &'a [u8], name: &str) -> Result<&'a [u8], StudioError> {
    let bad = |what: &str| StudioError::Malformed {
        name: name.to_string(),
        what: what.to_string(),
    };
    if bytes.len() < V3_HEADER_LEN {
        return Err(bad("header is truncated"));
    }
    let mut total: u64 = 0;
    let mut first: Option<u64> = None;
    let mut at = V3_TABLE_START;
    while at < V3_TABLE_END {
        let len = u64_at(bytes, at).ok_or_else(|| bad("section table is truncated"))?;
        first.get_or_insert(len);
        total = total
            .checked_add(len)
            .ok_or_else(|| bad("section table overflows"))?;
        at += 8;
    }
    let available = (bytes.len() - V3_HEADER_LEN) as u64;
    if total > available {
        return Err(bad("sections run past the end of the file"));
    }
    let first = usize::try_from(first.unwrap_or(0)).map_err(|_| bad("section 0 is too large"))?;
    let body = bytes
        .get(V3_HEADER_LEN..V3_HEADER_LEN.saturating_add(first))
        .ok_or_else(|| bad("section 0 is truncated"))?;
    if body.get(..MAGIC_LEN) != Some(b"silhouette05;".as_slice()) || body.get(MAGIC_LEN) != Some(&1)
    {
        // The table parsed but the body is not the plain record layout we know:
        // the shape of an encrypted or protected design. Never try to read it.
        return Err(StudioError::Protected(name.to_string()));
    }
    Ok(body)
}

/// A record: marker byte `0x01`, then a u32 length that counts itself, then payload.
/// `start..end` is the payload.
#[derive(Debug, Clone, Copy)]
struct Block {
    start: usize,
    end: usize,
}

fn block(d: &[u8], at: usize) -> Option<Block> {
    if *d.get(at)? != 1 {
        return None;
    }
    let len = usize::try_from(u32_at(d, at + 1)?).ok()?;
    if len < 4 {
        return None;
    }
    let end = (at + 1).checked_add(len)?;
    (end <= d.len()).then_some(Block { start: at + 5, end })
}

/// A path record, as seen around every shape in our samples:
///
/// ```text
/// 01 L1  u32(?)  01 L2  01 u32(?)  n × node      L1 == L2 + 9
/// node:  01 len  f32 x  f32 y  in-handle  out-handle  3 bytes(?)
/// handle: 00 (none) | 01 f32 dx f32 dy (relative to the node)
/// ```
/// followed by the object's 3 × 3 transform (f32, row-major). Every length must
/// agree before we accept it, so a false match on unrelated bytes is vanishingly
/// unlikely.
struct PathRecord {
    nodes: Vec<Node>,
    transform: Affine,
    end: usize,
}

#[derive(Debug, Clone, Copy)]
struct Node {
    at: Point,
    handle_in: Option<Vec2>,
    handle_out: Option<Vec2>,
}

fn find_path(d: &[u8], from: usize, to: usize) -> Option<(usize, PathRecord)> {
    let window = d.get(from..to)?;
    let mut offset = 0;
    while let Some(rel) = window.get(offset..)?.iter().position(|&b| b == 1) {
        let at = from + offset + rel;
        if let Some(path) = path_at(d, at, to) {
            return Some((at, path));
        }
        offset += rel + 1;
    }
    None
}

fn path_at(d: &[u8], at: usize, limit: usize) -> Option<PathRecord> {
    let outer = block(d, at)?;
    if outer.end > limit {
        return None;
    }
    let inner = block(d, outer.start + 4)?;
    if inner.end != outer.end || *d.get(inner.start)? != 1 {
        return None;
    }
    let mut nodes = Vec::new();
    let mut p = inner.start + 5;
    while p < inner.end {
        let node = block(d, p)?;
        if node.end > inner.end {
            return None;
        }
        nodes.push(parse_node(d, node)?);
        p = node.end;
    }
    if nodes.is_empty() {
        return None;
    }
    Some(PathRecord {
        nodes,
        transform: transform_at(d, outer.end)?,
        end: outer.end,
    })
}

fn parse_node(d: &[u8], b: Block) -> Option<Node> {
    let at = point_at(d, b.start)?;
    let mut o = b.start + 8;
    let mut handle = || -> Option<Option<Vec2>> {
        let flag = *d.get(o)?;
        o += 1;
        match flag {
            0 => Some(None),
            1 => {
                let v = point_at(d, o)?.to_vec2();
                o += 8;
                Some(Some(v))
            }
            _ => None,
        }
    };
    let handle_in = handle()?;
    let handle_out = handle()?;
    // Three trailing bytes of unknown meaning close every node record.
    (o + 3 == b.end).then_some(Node {
        at,
        handle_in,
        handle_out,
    })
}

/// The 3 × 3 row-major matrix after a path; only affine ones (last row 0 0 1) pass.
fn transform_at(d: &[u8], at: usize) -> Option<Affine> {
    let mut m = [0f64; 9];
    for (i, v) in m.iter_mut().enumerate() {
        let f = f32_at(d, at + 4 * i)?;
        if !f.is_finite() || f.abs() > MAX_COORD_MM {
            return None;
        }
        *v = f64::from(f);
    }
    let [a, b, c, dd, e, f, g, h, i] = m;
    (g == 0.0 && h == 0.0 && i == 1.0).then_some(Affine::new([a, dd, b, e, c, f]))
}

fn point_at(d: &[u8], at: usize) -> Option<Point> {
    let (x, y) = (f32_at(d, at)?, f32_at(d, at + 4)?);
    let ok = |v: f32| v.is_finite() && v.abs() <= MAX_COORD_MM;
    (ok(x) && ok(y)).then(|| Point::new(f64::from(x), f64::from(y)))
}

/// Segments are cubic where either end has a handle, straight otherwise.
/// shortcut: every path is closed, as in all samples so far (rectangle, ellipse);
/// upgrade once the line/open-polyline samples show the open flag.
fn to_bez(path: &PathRecord) -> BezPath {
    let mut bez = BezPath::new();
    let Some(first) = path.nodes.first() else {
        return bez;
    };
    bez.move_to(first.at);
    let next = path.nodes.iter().skip(1).chain(std::iter::once(first));
    let last = path.nodes.len() - 1;
    for (i, (from, to)) in path.nodes.iter().zip(next).enumerate() {
        if from.handle_out.is_none() && to.handle_in.is_none() {
            // The closing straight edge is the close itself.
            if i != last {
                bez.line_to(to.at);
            }
        } else {
            bez.curve_to(
                from.at + from.handle_out.unwrap_or_default(),
                to.at + to.handle_in.unwrap_or_default(),
                to.at,
            );
        }
    }
    bez.close_path();
    path.transform * bez
}

/// Page width and height (mm): the root's second record, 0x29 bytes into its payload.
/// Seen in one page size only (12 × 12 in), so it is checked hard and optional.
fn page_size(d: &[u8], root: &Block) -> Option<(f64, f64)> {
    let first = block(d, root.start)?;
    let second = block(d, first.end)?;
    let at = second.start + 0x29;
    if at + 8 > second.end {
        return None;
    }
    let (w, h) = (f32_at(d, at)?, f32_at(d, at + 4)?);
    let sane = |v: f32| v.is_finite() && (1.0..=10_000.0).contains(&v);
    // Stored as f32: round to a micrometre so 304.8 stays 304.8.
    let um = |v: f32| (f64::from(v) * 1000.0).round() / 1000.0;
    (sane(w) && sane(h)).then_some((um(w), um(h)))
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        d.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn u64_at(d: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        d.get(at..at.checked_add(8)?)?.try_into().ok()?,
    ))
}

fn f32_at(d: &[u8], at: usize) -> Option<f32> {
    u32_at(d, at).map(f32::from_bits)
}

/// The Silhouette Studio handler for the shared format seam (`.studio` and `.studio3`).
pub struct StudioHandler;

impl FormatHandler for StudioHandler {
    fn name(&self) -> &'static str {
        "Silhouette Studio (.studio, .studio3)"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["studio", "studio3"]
    }
    fn sniff(&self, bytes: &[u8]) -> bool {
        sniff(bytes).is_some()
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            read: Fidelity::Lossy,
            write: Fidelity::None,
            notes: &[
                "Reads outlines (lines and Bézier curves, as closed paths) with each object's transform, and the page size.",
                "Open paths, text, colours, layers and cut settings are not read yet.",
                "Protected or encrypted store designs are rejected, never decrypted.",
            ],
        }
    }
    fn read(&self, bytes: &[u8], name: &str) -> Result<Converted<Document>, String> {
        let got = import(bytes, name).map_err(|e| e.to_string())?;
        Ok(Converted {
            value: got.doc,
            warnings: got.warnings,
        })
    }
}
