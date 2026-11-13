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
    /// Shared format identifiers accepted by this app handler.
    fn formats(&self) -> &'static [craft_file_formats::Format];
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
    let format = craft_file_formats::detect(path, bytes)?;
    handlers
        .iter()
        .find(|h| h.formats().contains(&format))
        .copied()
}

/// Opens `path` with whichever handler claims it. Native `.stencilcraft` files and
/// files no handler claims give `Ok(None)` so the caller falls back to the native loader.
pub fn read_path(
    handlers: &[&dyn FormatHandler],
    path: &str,
) -> Result<Option<Converted<Document>>, String> {
    if std::path::Path::new(path)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .and_then(craft_file_formats::from_extension)
        == Some(craft_file_formats::Format::StencilCraft)
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

    struct Fake(&'static str, &'static [craft_file_formats::Format]);
    impl FormatHandler for Fake {
        fn name(&self) -> &'static str {
            self.0
        }
        fn formats(&self) -> &'static [craft_file_formats::Format] {
            self.1
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
        static PDF: &[craft_file_formats::Format] = &[craft_file_formats::Format::Pdf];
        static SVG: &[craft_file_formats::Format] = &[craft_file_formats::Format::Svg];
        let (a, b) = (Fake("a", PDF), Fake("b", SVG));
        let all: [&dyn FormatHandler; 2] = [&a, &b];
        assert_eq!(pick(&all, "x.pdf", b"<svg/>").map(|h| h.name()), Some("b"));
        assert_eq!(pick(&all, "x.svg", b"???").map(|h| h.name()), Some("b"));
        assert!(pick(&all, "x.cc", b"???").is_none());
        assert!(b.write(&Document::default()).is_err());
    }
}
