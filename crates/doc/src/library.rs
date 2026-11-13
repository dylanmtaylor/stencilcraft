//! A local design library: a directory of `.stencilcraft` files with folders, favourites,
//! search, thumbnails (from each file's metadata) and a recycle bin. No account, no cloud;
//! users sync the directory with their own tools if they want to.
//!
//! Paths are relative to the library root, `/`-separated, and checked (no `..`, no hidden
//! names, bounded depth) because they may come from a UI field or a control message.
//! Library bookkeeping (favourites, where trashed files came from) lives in
//! `.library.json` in the root, written atomically; the bin is the `.trash` folder.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::{DocError, Document, load_from, save_to};

pub const DOCUMENT_EXTENSION: &str = "stencilcraft";
const INDEX_FILE: &str = ".library.json";
const TRASH_DIR: &str = ".trash";
/// Deepest folder nesting, and most files a listing or search visits.
pub const MAX_DEPTH: usize = 16;
pub const MAX_FILES: usize = 10_000;
const MAX_PATH_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq)]
pub struct LibraryItem {
    /// Relative path, e.g. `cards/birthday.stencilcraft`.
    pub path: String,
    pub title: String,
    pub notes: String,
    pub thumbnail_svg: Option<String>,
    pub favourite: bool,
    pub modified: Option<SystemTime>,
    pub bytes: u64,
    /// The file could not be read as a document (it is still listed, so it can be trashed).
    pub damaged: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct LibraryListing {
    /// Subfolders (relative paths), sorted.
    pub folders: Vec<String>,
    /// Documents in the folder, sorted by path.
    pub items: Vec<LibraryItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrashEntry {
    /// File name inside the bin.
    pub name: String,
    /// Where it was, for restoring.
    pub original: String,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
struct Index {
    #[serde(default)]
    favourites: Vec<String>,
    #[serde(default)]
    trash: Vec<TrashEntry>,
}

pub struct Library {
    root: PathBuf,
}

fn io_err(path: &Path) -> impl FnOnce(std::io::Error) -> DocError + '_ {
    move |source| DocError::Io {
        path: path.display().to_string(),
        source,
    }
}

fn bad(message: String) -> DocError {
    DocError::Model(format!("library: {message}"))
}

/// Checks a relative library path; `""` is the root folder when `allow_root`.
fn check_rel(rel: &str, allow_root: bool) -> Result<Vec<&str>, DocError> {
    if rel.is_empty() && allow_root {
        return Ok(Vec::new());
    }
    let parts: Vec<&str> = rel.split('/').collect();
    let ok = rel.len() <= MAX_PATH_BYTES
        && parts.len() <= MAX_DEPTH
        && parts.iter().all(|p| {
            !p.is_empty() && !p.starts_with('.') && !p.contains(['\\', ':', '\0']) && p.trim() == *p
        });
    if ok {
        Ok(parts)
    } else {
        Err(bad(format!(
            "`{}` is not a valid library path",
            rel.chars().take(80).collect::<String>()
        )))
    }
}

fn is_document(rel: &str) -> bool {
    std::path::Path::new(rel)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .and_then(craft_file_formats::from_extension)
        == Some(craft_file_formats::Format::StencilCraft)
}

impl Library {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn abs(&self, rel: &str, allow_root: bool) -> Result<PathBuf, DocError> {
        let mut path = self.root.clone();
        for part in check_rel(rel, allow_root)? {
            path.push(part);
        }
        Ok(path)
    }

    fn doc_path(&self, rel: &str) -> Result<PathBuf, DocError> {
        if !is_document(rel) {
            return Err(bad(format!("`{rel}` is not a .{DOCUMENT_EXTENSION} file")));
        }
        self.abs(rel, false)
    }

    fn index(&self) -> Index {
        std::fs::read_to_string(self.root.join(INDEX_FILE))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    fn save_index(&self, index: &Index) -> Result<(), DocError> {
        let path = self.root.join(INDEX_FILE);
        let json = serde_json::to_string_pretty(index).map_err(|e| bad(e.to_string()))?;
        std::fs::create_dir_all(&self.root).map_err(io_err(&self.root))?;
        craft_atomic::atomic_write(&path, json.as_bytes()).map_err(io_err(&path))
    }

    fn item(&self, rel: String, favourites: &[String]) -> LibraryItem {
        let path = self.root.join(&rel);
        let meta = std::fs::metadata(&path).ok();
        let doc = load_from(&path.display().to_string()).ok();
        LibraryItem {
            favourite: favourites.contains(&rel),
            path: rel,
            title: doc
                .as_ref()
                .map(|d| d.meta.title.clone())
                .unwrap_or_default(),
            notes: doc
                .as_ref()
                .map(|d| d.meta.notes.clone())
                .unwrap_or_default(),
            thumbnail_svg: doc.as_ref().and_then(|d| d.meta.thumbnail_svg.clone()),
            modified: meta.as_ref().and_then(|m| m.modified().ok()),
            bytes: meta.map_or(0, |m| m.len()),
            damaged: doc.is_none(),
        }
    }

    /// Folders and documents directly inside `folder` (`""` for the root).
    pub fn list(&self, folder: &str) -> Result<LibraryListing, DocError> {
        let dir = self.abs(folder, true)?;
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && folder.is_empty() => {
                return Ok(LibraryListing::default());
            }
            Err(e) => return Err(io_err(&dir)(e)),
        };
        let favourites = self.index().favourites;
        let mut listing = LibraryListing::default();
        for entry in entries.flatten().take(MAX_FILES) {
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            let rel = if folder.is_empty() {
                name.clone()
            } else {
                format!("{folder}/{name}")
            };
            if check_rel(&rel, false).is_err() {
                continue;
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                listing.folders.push(rel);
            } else if kind.is_file() && is_document(&rel) {
                listing.items.push(self.item(rel, &favourites));
            }
        }
        listing.folders.sort();
        listing.items.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(listing)
    }

    pub fn create_folder(&self, folder: &str) -> Result<(), DocError> {
        let dir = self.abs(folder, false)?;
        std::fs::create_dir_all(&dir).map_err(io_err(&dir))
    }

    /// Saves `doc` at `rel` (atomically), creating folders as needed.
    pub fn save(&self, rel: &str, doc: &Document) -> Result<(), DocError> {
        let path = self.doc_path(rel)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(io_err(parent))?;
        }
        save_to(doc, &path.display().to_string())
    }

    pub fn open(&self, rel: &str) -> Result<Document, DocError> {
        load_from(&self.doc_path(rel)?.display().to_string())
    }

    /// Moves or renames a document; refuses to overwrite one.
    pub fn move_item(&self, from: &str, to: &str) -> Result<(), DocError> {
        let (src, dst) = (self.doc_path(from)?, self.doc_path(to)?);
        if dst.exists() {
            return Err(bad(format!("`{to}` already exists")));
        }
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).map_err(io_err(parent))?;
        }
        std::fs::rename(&src, &dst).map_err(io_err(&src))?;
        let mut index = self.index();
        for f in &mut index.favourites {
            if f == from {
                *f = to.to_string();
            }
        }
        self.save_index(&index)
    }

    pub fn set_favourite(&self, rel: &str, favourite: bool) -> Result<(), DocError> {
        let path = self.doc_path(rel)?;
        if !path.is_file() {
            return Err(bad(format!("`{rel}` does not exist")));
        }
        let mut index = self.index();
        index.favourites.retain(|f| f != rel);
        if favourite {
            index.favourites.push(rel.to_string());
        }
        self.save_index(&index)
    }

    /// Every document below `folder`, depth-first, up to [`MAX_FILES`].
    fn walk(&self, folder: &str, out: &mut Vec<String>, depth: usize) -> Result<(), DocError> {
        if depth > MAX_DEPTH || out.len() >= MAX_FILES {
            return Ok(());
        }
        let listing = self.list(folder)?;
        out.extend(listing.items.into_iter().map(|i| i.path));
        for sub in listing.folders {
            self.walk(&sub, out, depth + 1)?;
        }
        out.truncate(MAX_FILES);
        Ok(())
    }

    /// Favourite documents that still exist.
    pub fn favourites(&self) -> Vec<LibraryItem> {
        let favourites = self.index().favourites;
        favourites
            .iter()
            .filter(|rel| self.doc_path(rel).is_ok_and(|p| p.is_file()))
            .map(|rel| self.item(rel.clone(), &favourites))
            .collect()
    }

    /// Documents whose path, title or notes contain `query` (case-insensitive).
    pub fn search(&self, query: &str) -> Result<Vec<LibraryItem>, DocError> {
        let needle = query.trim().to_lowercase();
        let mut paths = Vec::new();
        self.walk("", &mut paths, 0)?;
        let favourites = self.index().favourites;
        Ok(paths
            .into_iter()
            .map(|rel| self.item(rel, &favourites))
            .filter(|item| {
                [&item.path, &item.title, &item.notes]
                    .iter()
                    .any(|text| text.to_lowercase().contains(&needle))
            })
            .collect())
    }

    /// Moves a document to the bin.
    pub fn trash(&self, rel: &str) -> Result<TrashEntry, DocError> {
        let src = self.doc_path(rel)?;
        let bin = self.root.join(TRASH_DIR);
        std::fs::create_dir_all(&bin).map_err(io_err(&bin))?;
        let mut index = self.index();
        let base = rel.replace('/', "_");
        let name = (0..10_000)
            .map(|n| {
                if n == 0 {
                    base.clone()
                } else {
                    format!("{n}-{base}")
                }
            })
            .find(|n| !bin.join(n).exists())
            .ok_or_else(|| bad("the bin is full; empty it first".to_string()))?;
        std::fs::rename(&src, bin.join(&name)).map_err(io_err(&src))?;
        let entry = TrashEntry {
            name,
            original: rel.to_string(),
        };
        index.favourites.retain(|f| f != rel);
        index.trash.push(entry.clone());
        self.save_index(&index)?;
        Ok(entry)
    }

    /// Bin entries whose files are still there.
    pub fn trash_list(&self) -> Vec<TrashEntry> {
        let bin = self.root.join(TRASH_DIR);
        self.index()
            .trash
            .into_iter()
            .filter(|e| check_rel(&e.name, false).is_ok() && bin.join(&e.name).is_file())
            .collect()
    }

    /// Puts a binned document back where it was (refusing to overwrite).
    pub fn restore(&self, name: &str) -> Result<String, DocError> {
        check_rel(name, false)?;
        let mut index = self.index();
        let Some(at) = index.trash.iter().position(|e| e.name == name) else {
            return Err(bad(format!("`{name}` is not in the bin")));
        };
        let entry = index.trash.remove(at);
        let dst = self.doc_path(&entry.original)?;
        if dst.exists() {
            return Err(bad(format!("`{}` already exists", entry.original)));
        }
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).map_err(io_err(parent))?;
        }
        let src = self.root.join(TRASH_DIR).join(&entry.name);
        std::fs::rename(&src, &dst).map_err(io_err(&src))?;
        self.save_index(&index)?;
        Ok(entry.original)
    }

    /// Deletes everything in the bin for good.
    pub fn empty_trash(&self) -> Result<(), DocError> {
        let bin = self.root.join(TRASH_DIR);
        match std::fs::remove_dir_all(&bin) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(io_err(&bin)(e)),
            _ => {}
        }
        let mut index = self.index();
        index.trash.clear();
        self.save_index(&index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "stencilcraft-library-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn titled(title: &str) -> Document {
        let mut doc = Document::default();
        doc.meta.title = title.to_string();
        doc.meta.thumbnail_svg = Some(doc.render_thumbnail(1024));
        doc
    }

    #[test]
    fn folders_favourites_search_and_thumbnails() {
        let dir = scratch("browse");
        let lib = Library::new(&dir);
        assert_eq!(lib.list("").unwrap(), LibraryListing::default());
        lib.create_folder("cards").unwrap();
        lib.save("cards/birthday.stencilcraft", &titled("Birthday card"))
            .unwrap();
        lib.save("tags/gift.stencilcraft", &titled("Gift tag"))
            .unwrap();
        std::fs::write(dir.join("readme.txt"), "x").unwrap();
        std::fs::write(dir.join("broken.stencilcraft"), "{").unwrap();
        let root = lib.list("").unwrap();
        assert_eq!(root.folders, vec!["cards".to_string(), "tags".to_string()]);
        assert_eq!(root.items.len(), 1);
        assert!(root.items[0].damaged);
        let cards = lib.list("cards").unwrap();
        assert_eq!(cards.items[0].title, "Birthday card");
        assert!(
            cards.items[0]
                .thumbnail_svg
                .as_ref()
                .unwrap()
                .starts_with("<svg")
        );
        lib.set_favourite("tags/gift.stencilcraft", true).unwrap();
        assert_eq!(lib.favourites().len(), 1);
        assert!(lib.set_favourite("tags/none.stencilcraft", true).is_err());
        let hits = lib.search("BIRTH").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "cards/birthday.stencilcraft");
        assert_eq!(lib.search("").unwrap().len(), 3);
        lib.move_item("tags/gift.stencilcraft", "cards/gift.stencilcraft")
            .unwrap();
        assert_eq!(lib.favourites()[0].path, "cards/gift.stencilcraft");
        assert!(
            lib.move_item("cards/gift.stencilcraft", "cards/birthday.stencilcraft")
                .is_err()
        );
        assert_eq!(
            lib.open("cards/gift.stencilcraft").unwrap().meta.title,
            "Gift tag"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn recycle_bin_trash_restore_empty() {
        let dir = scratch("bin");
        let lib = Library::new(&dir);
        lib.save("a/x.stencilcraft", &titled("one")).unwrap();
        let entry = lib.trash("a/x.stencilcraft").unwrap();
        assert!(lib.list("a").unwrap().items.is_empty());
        assert!(
            lib.search("").unwrap().is_empty(),
            "the bin is not searched"
        );
        lib.save("a/x.stencilcraft", &titled("two")).unwrap();
        let second = lib.trash("a/x.stencilcraft").unwrap();
        assert_ne!(entry.name, second.name);
        assert_eq!(lib.trash_list().len(), 2);
        assert_eq!(lib.restore(&entry.name).unwrap(), "a/x.stencilcraft");
        assert_eq!(lib.open("a/x.stencilcraft").unwrap().meta.title, "one");
        assert!(lib.restore(&second.name).is_err(), "would overwrite");
        lib.empty_trash().unwrap();
        assert!(lib.trash_list().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unsafe_paths_are_refused() {
        let lib = Library::new(scratch("paths"));
        for bad in [
            "../x.stencilcraft",
            "/x.stencilcraft",
            "a//x.stencilcraft",
            ".trash/x.stencilcraft",
            "a\\..\\x.stencilcraft",
            "x.txt",
            "",
        ] {
            assert!(lib.save(bad, &Document::default()).is_err(), "{bad}");
            assert!(lib.open(bad).is_err(), "{bad}");
        }
        assert!(lib.list("..").is_err());
        assert!(lib.restore("../etc").is_err());
    }
}
