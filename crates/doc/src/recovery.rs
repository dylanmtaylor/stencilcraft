//! Autosave snapshots for crash recovery, and the recent-files list.
//!
//! The UI decides when to snapshot (on a timer while the document is dirty); this module
//! owns where snapshots go and keeps the directory bounded. Every write is atomic
//! (`craft_atomic`): a crash mid-write leaves the previous snapshot intact, never a
//! half-written file under a snapshot name.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::{DocError, Document, load_from};

/// File extension of a recovery snapshot.
pub const RECOVERY_EXTENSION: &str = "stencilcraft-recovery";
/// Longest snapshot id, in bytes.
pub const MAX_RECOVERY_ID: usize = 64;
/// Most entries in the recent-files list.
pub const MAX_RECENT_FILES: usize = 10;

/// A recovery directory with bounds on how many snapshots and bytes it keeps.
#[derive(Debug, Clone, PartialEq)]
pub struct Recovery {
    pub dir: PathBuf,
    /// Oldest snapshots beyond this count are removed after each snapshot.
    pub max_entries: usize,
    /// Oldest snapshots are removed while the directory holds more than this; a single
    /// document larger than this is refused.
    pub max_bytes: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveryEntry {
    /// The id the snapshot was saved under (one per open document).
    pub id: String,
    pub path: PathBuf,
    pub bytes: u64,
    pub modified: Option<SystemTime>,
}

impl Recovery {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            max_entries: 20,
            max_bytes: 64 * 1024 * 1024,
        }
    }

    fn path_for(&self, id: &str) -> Result<PathBuf, DocError> {
        let ok = !id.is_empty()
            && id.len() <= MAX_RECOVERY_ID
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
        if !ok {
            return Err(DocError::Model(format!(
                "recovery: id must be 1-{MAX_RECOVERY_ID} letters, digits, `-` or `_`"
            )));
        }
        Ok(self.dir.join(format!("{id}.{RECOVERY_EXTENSION}")))
    }

    /// Writes (or replaces) snapshot `id` atomically, then prunes the oldest others.
    pub fn snapshot(&self, id: &str, doc: &Document) -> Result<RecoveryEntry, DocError> {
        let path = self.path_for(id)?;
        let json = doc.save_json()?;
        let bytes = json.len() as u64;
        if bytes > self.max_bytes {
            return Err(DocError::Model(format!(
                "recovery: document is {bytes} bytes, more than the {} byte limit",
                self.max_bytes
            )));
        }
        let io = |source| DocError::Io {
            path: path.display().to_string(),
            source,
        };
        std::fs::create_dir_all(&self.dir).map_err(io)?;
        craft_atomic::atomic_write(&path, json.as_bytes()).map_err(io)?;
        self.prune(id)?;
        Ok(RecoveryEntry {
            id: id.to_string(),
            modified: std::fs::metadata(&path).and_then(|m| m.modified()).ok(),
            path,
            bytes,
        })
    }

    /// Snapshots in the directory, newest first. A missing directory has none; files that
    /// are not snapshots (temporary files included) are ignored.
    pub fn list(&self) -> Result<Vec<RecoveryEntry>, DocError> {
        let entries = match std::fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => {
                return Err(DocError::Io {
                    path: self.dir.display().to_string(),
                    source,
                });
            }
        };
        let mut out = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(id) = snapshot_id(&path) else {
                continue;
            };
            if self.path_for(&id).is_err() {
                continue;
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if !meta.is_file() {
                continue;
            }
            out.push(RecoveryEntry {
                id,
                path,
                bytes: meta.len(),
                modified: meta.modified().ok(),
            });
        }
        out.sort_by(|a, b| b.modified.cmp(&a.modified).then(a.id.cmp(&b.id)));
        Ok(out)
    }

    /// Loads snapshot `id` (a damaged one is an error, not a crash).
    pub fn restore(&self, id: &str) -> Result<Document, DocError> {
        let path = self.path_for(id)?;
        load_from(&path.display().to_string())
    }

    /// Deletes snapshot `id`; a missing one is not an error.
    pub fn discard(&self, id: &str) -> Result<(), DocError> {
        let path = self.path_for(id)?;
        match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(DocError::Io {
                path: path.display().to_string(),
                source: e,
            }),
            _ => Ok(()),
        }
    }

    /// Removes the oldest snapshots (never `keep`) until within both bounds.
    fn prune(&self, keep: &str) -> Result<(), DocError> {
        let mut entries = self.list()?;
        let mut total: u64 = entries.iter().map(|e| e.bytes).sum();
        let mut count = entries.len();
        // Newest first, so pop from the back.
        while count > self.max_entries.max(1) || total > self.max_bytes {
            let Some(oldest) = entries.iter().rposition(|e| e.id != keep) else {
                break;
            };
            let entry = entries.remove(oldest);
            self.discard(&entry.id)?;
            total = total.saturating_sub(entry.bytes);
            count -= 1;
        }
        Ok(())
    }
}

fn snapshot_id(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    let id = name.strip_suffix(RECOVERY_EXTENSION)?.strip_suffix('.')?;
    Some(id.to_string())
}

/// Most recently used document paths, newest first, without duplicates.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct RecentFiles {
    pub paths: Vec<String>,
}

impl RecentFiles {
    /// Moves `path` to the front, keeping at most [`MAX_RECENT_FILES`].
    pub fn push(&mut self, path: &str) {
        self.paths.retain(|p| p != path);
        self.paths.insert(0, path.to_string());
        self.paths.truncate(MAX_RECENT_FILES);
    }

    pub fn remove(&mut self, path: &str) {
        self.paths.retain(|p| p != path);
    }

    /// Reads the list; a missing or damaged file gives an empty list.
    pub fn load(file: &Path) -> Self {
        let mut list: Self = std::fs::read_to_string(file)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        list.paths.truncate(MAX_RECENT_FILES);
        list
    }

    pub fn save(&self, file: &Path) -> Result<(), DocError> {
        let json = serde_json::to_string_pretty(self).map_err(|e| DocError::Structure {
            path: file.display().to_string(),
            message: e.to_string(),
        })?;
        craft_atomic::atomic_write(file, json.as_bytes()).map_err(|source| DocError::Io {
            path: file.display().to_string(),
            source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "stencilcraft-recovery-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn snapshot_list_restore_discard() {
        let dir = scratch("cycle");
        let recovery = Recovery::new(&dir);
        assert!(recovery.list().unwrap().is_empty(), "missing dir has none");
        let mut doc = Document::default();
        doc.meta.title = "Card".to_string();
        recovery.snapshot("doc-1", &doc).unwrap();
        doc.meta.title = "Card v2".to_string();
        recovery.snapshot("doc-1", &doc).unwrap();
        let list = recovery.list().unwrap();
        assert_eq!(list.len(), 1, "a re-snapshot replaces, never duplicates");
        assert_eq!(recovery.restore("doc-1").unwrap().meta.title, "Card v2");
        recovery.discard("doc-1").unwrap();
        recovery.discard("doc-1").unwrap();
        assert!(recovery.list().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn writes_are_atomic_and_strays_are_ignored() {
        let dir = scratch("atomic");
        let recovery = Recovery::new(&dir);
        recovery.snapshot("a", &Document::default()).unwrap();
        // Only the snapshot itself remains: no temporary file is left behind.
        let names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(
            names,
            vec![std::ffi::OsString::from("a.stencilcraft-recovery")]
        );
        // A crash's leftovers (a temp file, a foreign file) are not listed.
        std::fs::write(dir.join(".a.stencilcraft-recovery.tmp123"), "{").unwrap();
        std::fs::write(dir.join("notes.txt"), "x").unwrap();
        let ids: Vec<_> = recovery.list().unwrap().into_iter().map(|e| e.id).collect();
        assert_eq!(ids, vec!["a".to_string()]);
        // A damaged snapshot restores to an error, not a crash.
        std::fs::write(dir.join("b.stencilcraft-recovery"), "{\"stencil").unwrap();
        assert!(recovery.restore("b").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bad_ids_and_bounds() {
        let dir = scratch("bounds");
        let mut recovery = Recovery::new(&dir);
        for bad in ["", "../x", "a/b", "a.b", &"x".repeat(65)] {
            assert!(recovery.snapshot(bad, &Document::default()).is_err());
        }
        recovery.max_entries = 2;
        for id in ["one", "two", "three"] {
            recovery.snapshot(id, &Document::default()).unwrap();
        }
        let list = recovery.list().unwrap();
        assert_eq!(list.len(), 2);
        assert!(list.iter().any(|e| e.id == "three"), "the newest is kept");
        recovery.max_bytes = 10;
        assert!(recovery.snapshot("big", &Document::default()).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn recent_files_dedupe_cap_and_survive_damage() {
        let dir = scratch("recent");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("recent.json");
        assert_eq!(RecentFiles::load(&file), RecentFiles::default());
        let mut recent = RecentFiles::default();
        for i in 0..12 {
            recent.push(&format!("/d/{i}.stencilcraft"));
        }
        recent.push("/d/5.stencilcraft");
        assert_eq!(recent.paths.len(), MAX_RECENT_FILES);
        assert_eq!(recent.paths[0], "/d/5.stencilcraft");
        assert_eq!(
            recent
                .paths
                .iter()
                .filter(|p| p.ends_with("/5.stencilcraft"))
                .count(),
            1
        );
        recent.save(&file).unwrap();
        assert_eq!(RecentFiles::load(&file), recent);
        recent.remove("/d/5.stencilcraft");
        assert!(!recent.paths.contains(&"/d/5.stencilcraft".to_string()));
        std::fs::write(&file, "not json").unwrap();
        assert_eq!(RecentFiles::load(&file), RecentFiles::default());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
