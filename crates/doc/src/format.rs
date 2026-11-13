//! The format-handler seam: every foreign file format (Silhouette Studio, SVG, DXF,
//! PDF, raster, …) plugs in as a [`FormatHandler`] so open, import and save dispatch
//! the same way and surface the same kind of warnings. See `docs/formats/README.md`.

use crate::Document;

/// How faithfully a direction (read or write) carries a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fidelity {
    /// Not available.
    None,
    /// Some content is dropped or approximated; the handler warns about what.
    Lossy,
    /// Everything StencilCraft models survives.
    Lossless,
}

/// What a handler can do, declared up front so a UI can say so before opening or saving
/// (like a "this format keeps only outlines" warning).
#[derive(Debug, Clone, Copy)]
pub struct Capabilities {
    pub read: Fidelity,
    pub write: Fidelity,
    /// Plain-language notes on what round-trips and what does not.
    pub notes: &'static [&'static str],
}

/// A read or write result plus the warnings the user should see.
#[derive(Debug, Clone)]
pub struct Converted<T> {
    pub value: T,
    pub warnings: Vec<String>,
}

pub trait FormatHandler: Sync {
    /// Short human name, e.g. "Silhouette Studio (.studio3)".
    fn name(&self) -> &'static str;
    /// Lower-case extensions without the dot.
    fn extensions(&self) -> &'static [&'static str];
    /// True when the leading bytes are this format's (content beats extension).
    fn sniff(&self, bytes: &[u8]) -> bool;
    fn capabilities(&self) -> Capabilities;
    /// Reads untrusted bytes. Must be bounded and never panic; `name` labels errors.
    fn read(&self, bytes: &[u8], name: &str) -> Result<Converted<Document>, String>;
    /// Writes the document. Handlers without a writer keep this default.
    fn write(&self, _doc: &Document) -> Result<Converted<Vec<u8>>, String> {
        Err(format!("{}: writing is not supported", self.name()))
    }
}

/// Picks the handler for a file: a content match first, then the extension.
pub fn pick<'a>(
    handlers: &[&'a dyn FormatHandler],
    path: &str,
    bytes: &[u8],
) -> Option<&'a dyn FormatHandler> {
    if let Some(h) = handlers.iter().find(|h| h.sniff(bytes)) {
        return Some(*h);
    }
    let ext = path.rsplit_once('.')?.1.to_ascii_lowercase();
    handlers
        .iter()
        .find(|h| h.extensions().contains(&ext.as_str()))
        .copied()
}

/// Opens `path` with whichever handler claims it. Native `.stencilcraft` files and
/// files no handler claims give `Ok(None)` so the caller falls back to the native loader.
pub fn read_path(
    handlers: &[&dyn FormatHandler],
    path: &str,
) -> Result<Option<Converted<Document>>, String> {
    if path
        .to_ascii_lowercase()
        .ends_with(&format!(".{}", crate::FORMAT_EXTENSION))
    {
        return Ok(None);
    }
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    match pick(handlers, path, &bytes) {
        Some(handler) => handler.read(&bytes, path).map(Some),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake(&'static str, &'static [u8]);
    impl FormatHandler for Fake {
        fn name(&self) -> &'static str {
            self.0
        }
        fn extensions(&self) -> &'static [&'static str] {
            if self.0 == "a" { &["aa"] } else { &["bb"] }
        }
        fn sniff(&self, bytes: &[u8]) -> bool {
            bytes.starts_with(self.1)
        }
        fn capabilities(&self) -> Capabilities {
            Capabilities {
                read: Fidelity::Lossy,
                write: Fidelity::None,
                notes: &[],
            }
        }
        fn read(&self, _: &[u8], _: &str) -> Result<Converted<Document>, String> {
            Err("fake".into())
        }
    }

    #[test]
    fn content_beats_extension_and_extension_is_the_fallback() {
        let (a, b) = (Fake("a", b"AAA"), Fake("b", b"BBB"));
        let all: [&dyn FormatHandler; 2] = [&a, &b];
        assert_eq!(pick(&all, "x.aa", b"BBB...").map(|h| h.name()), Some("b"));
        assert_eq!(pick(&all, "x.AA", b"???").map(|h| h.name()), Some("a"));
        assert!(pick(&all, "x.cc", b"???").is_none());
        assert!(b.write(&Document::default()).is_err());
    }
}
