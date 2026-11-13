# File formats

Every foreign file format plugs into one seam:
[`stencilcraft_doc::format::FormatHandler`](../../crates/doc/src/format.rs).

```rust
pub trait FormatHandler: Sync {
    fn name(&self) -> &'static str;                 // "Silhouette Studio (.studio, .studio3)"
    fn extensions(&self) -> &'static [&'static str]; // lower case, no dot
    fn sniff(&self, bytes: &[u8]) -> bool;           // magic-byte match
    fn capabilities(&self) -> Capabilities;          // read/write Fidelity + notes
    fn read(&self, bytes: &[u8], name: &str) -> Result<Converted<Document>, String>;
    fn write(&self, doc: &Document) -> Result<Converted<Vec<u8>>, String>; // default: unsupported
}
```

- **Detection.** `format::pick(handlers, path, bytes)` tries content first (`sniff`), then
  the extension. `format::read_path(handlers, path)` reads a file through whichever handler
  claims it. It returns `Ok(None)` for native `.stencilcraft` files and for files no handler
  claims, so the caller falls back to the native loader.
- **Capabilities.** `Fidelity::{None, Lossy, Lossless}` for each direction, plus
  plain-language `notes` on what does not round-trip. A UI shows these before open or save,
  the way Silhouette Studio warns when you save in a legacy format.
- **Warnings.** `Converted { value, warnings }` carries what a particular file lost, such as
  skipped records or an assumed page size. The CLI prints them to stderr. MCP `doc.open`
  returns them as `warnings`.
- **Untrusted input.** `read` must be bounded (size and count caps), must check every length
  against the bytes really present, must never panic, and must fail closed. It rejects what
  it cannot interpret rather than guessing. Each reader ships a truncation and bit-flip corpus
  test.
- **Imports are never save targets.** Saving an opened foreign file writes `.stencilcraft`
  unless the user picks a format with a writer.

## Handlers

| Format | Crate | Read | Write | Notes |
| --- | --- | --- | --- | --- |
| Silhouette Studio `.studio` (v2), `.studio3` (v3) | `crates/studio` | lossy | none yet | [silhouette-studio.md](silhouette-studio.md). Protected store designs are rejected |
| Silhouette/Graphtec GSD, GSP | none yet | none | none | no samples yet |
| SVG / SVGZ | `crates/svg` | yes | yes | SVGZ compression and text decoding are shared through `craft-svg` |

Wired in: CLI `import --in FILE` and every `--in FILE` (`run`, `tile-preview`, `cut`,
`print-cut`), and MCP `doc.open` / `doc_open`.

## Cricut Design Space

Design Space has no local proprietary document format. Projects are stored in Cricut's
cloud behind an account sign-in, and the app imports and exports only standard files (SVG,
DXF, PNG, JPG, PDF and similar). So there is nothing Cricut-specific to read or write. Its
interchange formats are covered by the SVG/DXF/PDF/raster handlers. (Cricut help centre:
<https://help.cricut.com/hc/en-us>. Sign-in requirement observed on 9.83.76, see
`docs/reference-apps.md`.)

## Clean-room rule for proprietary formats

We learn formats only from files we create ourselves in the reference apps, by comparing
samples that differ in one thing, plus public prose documentation cited by URL. We never
disassemble or debug the apps, never read or copy other projects' code for the format, and
never decrypt or work around DRM. Vendor-generated samples stay in the gitignored
`corpora/` directory. They are not committed.
