//! The Library panel and crash recovery. While the document has unsaved changes it is
//! snapshotted every [`AUTOSAVE_SECS`] to the recovery folder (saving discards the snapshot);
//! File › Open Recovered lists the snapshots to open or discard. The Library is a local folder
//! of designs (Silhouette's Library, without an account): list, search, open, and save the
//! current design into it. File › Print opens the page as a PDF in the system viewer.

use craft_ui::controls::TextField;
use craft_ui::edit::EditEvent;
use craft_ui::patterns::Panel;
use eframe::egui;
use stencilcraft_doc::library::{Library, LibraryItem};
use stencilcraft_doc::recovery::{Recovery, RecoveryEntry};

use crate::StencilApp;
use crate::panels::{button, hint};

/// Seconds between recovery snapshots of a changed document.
pub const AUTOSAVE_SECS: u64 = 30;

/// Where recovery snapshots and the library live, and the panel's fields.
#[derive(Default)]
pub struct FileStores {
    pub recovery: Option<Recovery>,
    pub library: Option<Library>,
    /// This window's snapshot id.
    pub session_id: String,
    pub last_snapshot: Option<std::time::Instant>,
    pub query: String,
    pub save_name: String,
    /// Snapshots of earlier sessions, listed once at start.
    pub recovered: Vec<RecoveryEntry>,
}

impl FileStores {
    /// Stores under the platform data folder (`None` folders on the web build).
    pub fn open() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let base = craft_settings::paths::data_dir("stencilcraft");
        #[cfg(target_arch = "wasm32")]
        let base: Option<std::path::PathBuf> = None;
        let recovery = base.as_ref().map(|d| Recovery::new(d.join("recovery")));
        let library = base.as_ref().map(|d| Library::new(d.join("library")));
        let session_id = format!(
            "session-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs())
        );
        let recovered = recovery
            .as_ref()
            .and_then(|r| r.list().ok())
            .unwrap_or_default();
        Self {
            recovery,
            library,
            session_id,
            last_snapshot: None,
            query: String::new(),
            save_name: "design".to_string(),
            recovered,
        }
    }
}

impl StencilApp {
    /// Snapshot the document when it has unsaved changes and the last snapshot is old enough.
    pub fn autosave_tick(&mut self) {
        let due = self
            .stores
            .last_snapshot
            .is_none_or(|t| t.elapsed().as_secs() >= AUTOSAVE_SECS);
        if !due || self.engine.doc == self.saved_document {
            return;
        }
        self.stores.last_snapshot = Some(std::time::Instant::now());
        if let Some(recovery) = &self.stores.recovery
            && let Err(e) = recovery.snapshot(&self.stores.session_id, &self.engine.doc)
        {
            self.status = format!("autosave: {e}");
        }
    }

    /// The document was saved: its recovery snapshot is no longer needed.
    pub fn discard_own_snapshot(&mut self) {
        if let Some(recovery) = &self.stores.recovery {
            let _ = recovery.discard(&self.stores.session_id);
        }
    }

    /// Open `doc` as the current document. A recovered or library design is unsaved until
    /// saved to a file.
    fn load_design(&mut self, doc: stencilcraft_doc::Document, what: &str) {
        self.engine = stencilcraft_engine::Engine::new(doc);
        self.saved_document = stencilcraft_doc::Document::default();
        self.set_selection([]);
        self.open_path = None;
        self.view_initialized = false;
        self.last_board = None;
        self.kerf_preview_cache = None;
        self.status = format!("opened {what}");
    }

    pub fn open_recovered(&mut self, id: &str) {
        let restored = self.stores.recovery.as_ref().map(|r| r.restore(id));
        match restored {
            Some(Ok(doc)) => {
                self.load_design(doc, "the recovered design (save it to keep it)");
                if let Some(r) = &self.stores.recovery {
                    let _ = r.discard(id);
                }
                self.stores.recovered.retain(|e| e.id != id);
            }
            Some(Err(e)) => self.status = format!("recovery: {e}"),
            None => self.status = "no recovery folder".to_string(),
        }
    }

    pub fn discard_recovered(&mut self, id: &str) {
        if let Some(r) = &self.stores.recovery {
            let _ = r.discard(id);
        }
        self.stores.recovered.retain(|e| e.id != id);
    }

    fn library_items(&self) -> Vec<LibraryItem> {
        let Some(library) = &self.stores.library else {
            return Vec::new();
        };
        if self.stores.query.trim().is_empty() {
            library.list("").map(|l| l.items).unwrap_or_default()
        } else {
            library.search(self.stores.query.trim()).unwrap_or_default()
        }
    }

    pub fn open_library_item(&mut self, rel: &str) {
        let opened = self.stores.library.as_ref().map(|l| l.open(rel));
        match opened {
            Some(Ok(doc)) => self.load_design(doc, rel),
            Some(Err(e)) => self.status = e.to_string(),
            None => {}
        }
    }

    /// Save the current design into the library as `name`.
    pub fn save_to_library(&mut self, name: &str) {
        let name: String = name
            .trim()
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        if name.is_empty() {
            self.status = "name the design first".to_string();
            return;
        }
        let rel = format!("{name}.{}", stencilcraft_doc::FORMAT_EXTENSION);
        let saved = self
            .stores
            .library
            .as_ref()
            .map(|l| l.save(&rel, &self.engine.doc));
        self.status = match saved {
            Some(Ok(())) => format!("saved {rel} to the library"),
            Some(Err(e)) => e.to_string(),
            None => "no library folder".to_string(),
        };
    }

    /// File › Print: the page opened in the system viewer to print from there: with Print &
    /// Cut marks on, the alignment sheet (registration marks, fills and line colours, at
    /// 100 %); otherwise the page as a PDF.
    pub fn print_via_pdf(&mut self) {
        let (bytes, name) = if self.prefs.print_cut_marks {
            match self.print_sheet_svg() {
                Ok(svg) => (svg.into_bytes(), "stencilcraft-print-and-cut.svg"),
                Err(e) => {
                    self.status = format!("print: {e}");
                    return;
                }
            }
        } else {
            match self.export_bytes(crate::file_menu::ExportKind::Pdf) {
                Ok(bytes) => (bytes, "stencilcraft-print.pdf"),
                Err(e) => {
                    self.status = format!("print: {e}");
                    return;
                }
            }
        };
        let path = std::env::temp_dir().join(name);
        if let Err(e) = std::fs::write(&path, bytes) {
            self.status = format!("print: {e}");
            return;
        }
        self.status = match open_with_system(&path) {
            Ok(()) => "opened the page to print (print at 100 %)".to_string(),
            Err(e) => format!("print: {e}; the PDF is at {}", path.display()),
        };
    }

    pub fn library_panel(&mut self, ui: &mut egui::Ui) {
        let recovered: Vec<(String, String)> = self
            .stores
            .recovered
            .iter()
            .map(|e| {
                let age = e
                    .modified
                    .and_then(|m| m.elapsed().ok())
                    .map_or(String::new(), |d| format!(", {} min ago", d.as_secs() / 60));
                (
                    e.id.clone(),
                    format!("{} ({} KB{age})", e.id, e.bytes / 1024),
                )
            })
            .collect();
        let items = self.library_items();
        let has_library = self.stores.library.is_some();
        let mut open_recovered = None;
        let mut discard = None;
        let mut open_item = None;
        let mut save = false;
        let mut query = self.stores.query.clone();
        let mut save_name = self.stores.save_name.clone();
        let _ = Panel::new("stencilcraft-library-panel").show(ui, |ui| {
            if !recovered.is_empty() {
                ui.label("Recovered files");
                for (id, label) in &recovered {
                    ui.horizontal(|ui| {
                        ui.label(label);
                        if button(ui, "Open", true) {
                            open_recovered = Some(id.clone());
                        }
                        if button(ui, "Discard", true) {
                            discard = Some(id.clone());
                        }
                    });
                }
                ui.separator();
            }
            ui.label("Library");
            if !has_library {
                hint(ui, "No library folder on this system.");
                return;
            }
            let edit = TextField::new("library-search", "Search the library", query.as_str())
                .char_limit(200)
                .show(ui);
            if let Some(
                EditEvent::Update { value, .. }
                | EditEvent::Commit { value, .. }
                | EditEvent::Set { value, .. },
            ) = edit.event
            {
                query = value;
            }
            if items.is_empty() {
                hint(ui, "No designs yet: save one below.");
            }
            for item in &items {
                ui.horizontal(|ui| {
                    let title = if item.title.is_empty() {
                        item.path.as_str()
                    } else {
                        item.title.as_str()
                    };
                    ui.label(title);
                    if button(ui, "Open", !item.damaged) {
                        open_item = Some(item.path.clone());
                    }
                });
            }
            ui.separator();
            let edit = TextField::new("library-save-name", "Design name", save_name.as_str())
                .char_limit(120)
                .show(ui);
            if let Some(
                EditEvent::Update { value, .. }
                | EditEvent::Commit { value, .. }
                | EditEvent::Set { value, .. },
            ) = edit.event
            {
                save_name = value;
            }
            save = button(ui, "Save Design to Library", true);
        });
        self.stores.query = query;
        self.stores.save_name = save_name;
        if let Some(id) = open_recovered {
            self.open_recovered(&id);
        }
        if let Some(id) = discard {
            self.discard_recovered(&id);
        }
        if let Some(rel) = open_item {
            self.open_library_item(&rel);
        }
        if save {
            let name = self.stores.save_name.clone();
            self.save_to_library(&name);
        }
    }
}

/// Open `path` with the system's default application.
fn open_with_system(path: &std::path::Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", ""]).arg(path);
        c
    };
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut c = std::process::Command::new("open");
        c.arg(path);
        c
    };
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let mut command = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(path);
        c
    };
    command.spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use stencilcraft_engine::Command;

    fn app_in(dir: &std::path::Path) -> StencilApp {
        let mut app = StencilApp::new(None);
        app.stores = FileStores {
            recovery: Some(Recovery::new(dir.join("recovery"))),
            library: Some(Library::new(dir.join("library"))),
            session_id: "session-test".into(),
            ..FileStores::default()
        };
        app
    }

    #[test]
    fn changed_documents_are_snapshotted_and_recovered() {
        let dir = std::env::temp_dir().join(format!("sc-recovery-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = app_in(&dir);
        app.autosave_tick();
        assert!(
            app.stores
                .recovery
                .as_ref()
                .is_some_and(|r| r.list().unwrap_or_default().is_empty()),
            "an unchanged document is not snapshotted"
        );
        app.run_command(Command::ShapeAddRect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        });
        app.autosave_tick();
        let listed = app
            .stores
            .recovery
            .as_ref()
            .map(|r| r.list().unwrap_or_default());
        assert_eq!(listed.map(|l| l.len()), Some(1));

        let mut next = app_in(&dir);
        next.stores.recovered = next
            .stores
            .recovery
            .as_ref()
            .and_then(|r| r.list().ok())
            .unwrap_or_default();
        next.open_recovered("session-test");
        assert_eq!(next.engine.doc.shapes().count(), 1);
        assert!(
            next.engine.doc != next.saved_document,
            "recovered work is unsaved"
        );
        assert!(next.stores.recovered.is_empty());

        next.save_to_library("my design");
        let mut third = app_in(&dir);
        let items = third.library_items();
        assert_eq!(items.len(), 1, "{}", next.status);
        let rel = items[0].path.clone();
        third.open_library_item(&rel);
        assert_eq!(third.engine.doc.shapes().count(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
