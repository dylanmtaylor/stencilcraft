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
    match page_size(body, &root) {
        Some((w, h)) => {
            doc.board.width_mm = w;
            doc.board.height_mm = h;
        }
        None => {
            warnings.push("page size not found; using the default sheet".to_string());
        }
    }
    // Silhouette is y-down from the page's top-left; documents are y-up.
    let to_doc = Affine::new([1.0, 0.0, 0.0, -1.0, 0.0, doc.board.height_mm]);
    let mut points_total = 0usize;
    let mut cursor = root.start;
    while let Some((at, path)) = find_path(body, cursor, root.end) {
        cursor = path.end;
        if doc.shape_count() >= MAX_SHAPES {
            return Err(bad("more than 100,000 shapes"));
        }
        let nodes: usize = path.contours.iter().map(|c| c.nodes.len()).sum();
        points_total = points_total.saturating_add(nodes);
        if points_total > MAX_POINTS {
            return Err(bad("more than 2,000,000 points"));
        }
        let bez = to_doc * to_bez(&path);
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

/// An object's outline in page coordinates, as every drawable object (shape, line,
/// compound path, text) stores it after its drawing-frame nodes and matrix:
///
/// ```text
/// 01 L  u32 contours-1  contours × contour
/// contour: u32 Lc  u8 closed  u32 nodes-1  nodes × node      (Lc counts itself)
/// node:    f32 x  f32 y  in-handle  out-handle
/// handle:  00 (none) | 01 f32 dx f32 dy (relative to the node)
/// ```
/// Every length and count must agree before a match is accepted.
struct PathRecord {
    contours: Vec<Contour>,
    end: usize,
}

struct Contour {
    closed: bool,
    nodes: Vec<Node>,
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
    let count = usize::try_from(u32_at(d, outer.start)?)
        .ok()?
        .checked_add(1)?;
    let mut contours = Vec::new();
    let mut p = outer.start + 4;
    while p < outer.end {
        if contours.len() >= count {
            return None;
        }
        let (contour, end) = contour_at(d, p, outer.end)?;
        contours.push(contour);
        p = end;
    }
    (p == outer.end && contours.len() == count).then_some(PathRecord {
        contours,
        end: outer.end,
    })
}

fn contour_at(d: &[u8], at: usize, limit: usize) -> Option<(Contour, usize)> {
    let end = at.checked_add(usize::try_from(u32_at(d, at)?).ok()?)?;
    let closed = match *d.get(at + 4)? {
        0 => false,
        1 => true,
        _ => return None,
    };
    let count = usize::try_from(u32_at(d, at + 5)?).ok()?.checked_add(1)?;
    // Each node takes at least 10 bytes: refuse counts the bytes cannot hold.
    if end > limit || count > (end - at) / 10 {
        return None;
    }
    let mut nodes = Vec::with_capacity(count);
    let mut p = at + 9;
    for _ in 0..count {
        let point = point_at(d, p)?;
        p += 8;
        let mut handle = || -> Option<Option<Vec2>> {
            let flag = *d.get(p)?;
            p += 1;
            match flag {
                0 => Some(None),
                1 => {
                    let v = point_at(d, p)?.to_vec2();
                    p += 8;
                    Some(Some(v))
                }
                _ => None,
            }
        };
        let handle_in = handle()?;
        let handle_out = handle()?;
        nodes.push(Node {
            at: point,
            handle_in,
            handle_out,
        });
    }
    (p == end).then_some((Contour { closed, nodes }, end))
}

fn point_at(d: &[u8], at: usize) -> Option<Point> {
    let (x, y) = (f32_at(d, at)?, f32_at(d, at + 4)?);
    let ok = |v: f32| v.is_finite() && v.abs() <= MAX_COORD_MM;
    (ok(x) && ok(y)).then(|| Point::new(f64::from(x), f64::from(y)))
}

/// One subpath per contour. A segment is cubic where either end has a handle,
/// straight otherwise; a closed contour also joins its last node to its first.
fn to_bez(path: &PathRecord) -> BezPath {
    let mut bez = BezPath::new();
    for contour in &path.contours {
        let Some(first) = contour.nodes.first() else {
            continue;
        };
        bez.move_to(first.at);
        let closing = contour.closed.then_some(first);
        let next = contour.nodes.iter().skip(1).chain(closing);
        let last = contour.nodes.len() - 1;
        for (i, (from, to)) in contour.nodes.iter().zip(next).enumerate() {
            match (from.handle_out, to.handle_in) {
                // A straight closing edge is the close itself.
                (None, None) if contour.closed && i == last => {}
                (None, None) => bez.line_to(to.at),
                (out, inn) => bez.curve_to(
                    from.at + out.unwrap_or_default(),
                    to.at + inn.unwrap_or_default(),
                    to.at,
                ),
            }
        }
        if contour.closed {
            bez.close_path();
        }
    }
    bez
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
                "Reads every object's outline in page coordinates (open and closed paths, lines and Bézier curves, compound paths, text as outlines) and the page size.",
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
