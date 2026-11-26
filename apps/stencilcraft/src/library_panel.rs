//! The Library panel and crash recovery. While the document has unsaved changes it is
//! snapshotted every [`AUTOSAVE_SECS`] to the recovery folder (saving discards the snapshot);
//! File › Open Recovered lists the snapshots to open or discard. The Library is a local folder
//! of designs (Silhouette's Library, without an account): list, search, open, and save the
//! current design into it. File › Print opens the page as a PDF in the system viewer.

use craft_ui::contract::{Bind, Props as _, Show as _, ShowContainer as _};
use craft_ui::sections::Panel;
use craft_ui::text::TextField;
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
    /// The library folder the panel shows (`""` is the top).
    pub folder: String,
    /// The New Folder field.
    pub new_folder: String,
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
            folder: String::new(),
            new_folder: String::new(),
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
            && let Err(e) =
                recovery.snapshot(&self.recovery_key(self.tabs.active), &self.engine.doc)
        {
            self.status = format!("autosave: {e}");
        }
    }

    /// The document was saved: its recovery snapshot is no longer needed.
    pub fn discard_own_snapshot(&mut self) {
        if let Some(recovery) = &self.stores.recovery {
            let _ = recovery.discard(&self.recovery_key(self.tabs.active));
        }
    }

    /// Open `doc` as the current document. A recovered or library design is unsaved until
    /// saved to a file.
    fn load_design(&mut self, doc: stencilcraft_doc::Document, what: &str) {
        if !self.tab_for_new_document() {
            return;
        }
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
            library
                .list(&self.stores.folder)
                .map(|l| l.items)
                .unwrap_or_default()
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
        let rel = if self.stores.folder.is_empty() {
            format!("{name}.{}", stencilcraft_doc::FORMAT_EXTENSION)
        } else {
            format!(
                "{}/{name}.{}",
                self.stores.folder,
                stencilcraft_doc::FORMAT_EXTENSION
            )
        };
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
            let dpi = if self.prefs.print_dpi.is_finite() {
                self.prefs.print_dpi.clamp(72.0, 2400.0)
            } else {
                600.0
            };
            match stencilcraft_import::write_document(&self.engine.doc, "pdf", dpi)
                .map_err(|e| e.to_string())
            {
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

    /// The folders inside the shown one.
    fn library_folders(&self) -> Vec<String> {
        self.stores
            .library
            .as_ref()
            .and_then(|l| l.list(&self.stores.folder).ok())
            .map(|l| l.folders)
            .unwrap_or_default()
    }

    /// New Folder: a folder inside the shown one; names keep to letters, digits, spaces, `-`, `_`.
    pub fn create_library_folder(&mut self, name: &str) {
        let name: String = name
            .trim()
            .chars()
            .filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | ' '))
            .collect();
        if name.is_empty() {
            self.status = "name the folder first".to_string();
            return;
        }
        let rel = if self.stores.folder.is_empty() {
            name
        } else {
            format!("{}/{name}", self.stores.folder)
        };
        self.status = match self.stores.library.as_ref().map(|l| l.create_folder(&rel)) {
            Some(Ok(())) => format!("made the folder {rel}"),
            Some(Err(e)) => e.to_string(),
            None => "no library folder".to_string(),
        };
    }

    /// The library action of a button, run after the panel is drawn.
    pub fn library_action(&mut self, action: LibraryAction) {
        let Some(library) = self.stores.library.as_ref() else {
            return;
        };
        let result = match &action {
            LibraryAction::Enter(folder) => {
                self.stores.folder.clone_from(folder);
                Ok(String::new())
            }
            LibraryAction::Up => {
                self.stores.folder = self
                    .stores
                    .folder
                    .rsplit_once('/')
                    .map_or_else(String::new, |(parent, _)| parent.to_string());
                Ok(String::new())
            }
            LibraryAction::Favourite(rel, on) => library.set_favourite(rel, *on).map(|()| {
                if *on {
                    format!("{rel} is a favourite")
                } else {
                    format!("{rel} is no longer a favourite")
                }
            }),
            LibraryAction::Trash(rel) => library
                .trash(rel)
                .map(|_| format!("moved {rel} to the Recycle Bin")),
            LibraryAction::Restore(name) => {
                library.restore(name).map(|rel| format!("restored {rel}"))
            }
            LibraryAction::EmptyBin => library
                .empty_trash()
                .map(|()| "emptied the Recycle Bin".to_string()),
        };
        match result {
            Ok(message) if message.is_empty() => {}
            Ok(message) => self.status = message,
            Err(e) => self.status = e.to_string(),
        }
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
        let folders = self.library_folders();
        let favourites = self
            .stores
            .library
            .as_ref()
            .map(Library::favourites)
            .unwrap_or_default();
        let bin = self
            .stores
            .library
            .as_ref()
            .map(Library::trash_list)
            .unwrap_or_default();
        let folder = self.stores.folder.clone();
        let mut action = None;
        let mut new_folder = self.stores.new_folder.clone();
        let mut make_folder = false;
        let has_library = self.stores.library.is_some();
        let mut open_recovered = None;
        let mut discard = None;
        let mut open_item = None;
        let mut save = false;
        let mut query = self.stores.query.clone();
        let mut save_name = self.stores.save_name.clone();
        let _ = Panel::new("Library")
            .id_salt("stencilcraft-library-panel")
            .show(ui, |ui, _| {
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
                // The field writes `query` as it is typed.
                let _ = TextField::new("Search the library", Bind::bound(&mut query))
                    .id_salt("library-search")
                    .limit(200)
                    .show(ui);
                if !favourites.is_empty() {
                    hint(ui, "Favourites");
                    for item in &favourites {
                        ui.horizontal(|ui| {
                            ui.label(item_title(item));
                            if button(ui, "Open", !item.damaged) {
                                open_item = Some(item.path.clone());
                            }
                        });
                    }
                }
                ui.horizontal(|ui| {
                    ui.label(if folder.is_empty() {
                        "Library".to_string()
                    } else {
                        format!("Library / {folder}")
                    });
                    if !folder.is_empty() && button(ui, "Up", true) {
                        action = Some(LibraryAction::Up);
                    }
                });
                for sub in &folders {
                    ui.horizontal(|ui| {
                        let name = sub.rsplit('/').next().unwrap_or(sub);
                        ui.label(format!("{name}/"));
                        if button(ui, "Open folder", true) {
                            action = Some(LibraryAction::Enter(sub.clone()));
                        }
                    });
                }
                if items.is_empty() && folders.is_empty() {
                    hint(ui, "No designs here yet: save one below.");
                }
                for item in &items {
                    ui.horizontal(|ui| {
                        ui.label(item_title(item));
                        if button(ui, "Open", !item.damaged) {
                            open_item = Some(item.path.clone());
                        }
                        // Plain words: the interface font has no star glyphs.
                        let star = if item.favourite {
                            "Unfavourite"
                        } else {
                            "Favourite"
                        };
                        if button(ui, star, true) {
                            action =
                                Some(LibraryAction::Favourite(item.path.clone(), !item.favourite));
                        }
                        if button(ui, "Delete", true) {
                            action = Some(LibraryAction::Trash(item.path.clone()));
                        }
                    });
                }
                ui.horizontal(|ui| {
                    let _ = TextField::new("New folder name", Bind::bound(&mut new_folder))
                        .id_salt("library-new-folder")
                        .limit(80)
                        .show(ui);
                    make_folder = button(ui, "New Folder", true);
                });
                if !bin.is_empty() {
                    ui.separator();
                    hint(ui, &format!("Recycle Bin ({})", bin.len()));
                    for entry in &bin {
                        ui.horizontal(|ui| {
                            ui.label(&entry.original);
                            if button(ui, "Restore", true) {
                                action = Some(LibraryAction::Restore(entry.name.clone()));
                            }
                        });
                    }
                    if button(ui, "Empty Recycle Bin", true) {
                        action = Some(LibraryAction::EmptyBin);
                    }
                }
                ui.separator();
                let _ = TextField::new("Design name", Bind::bound(&mut save_name))
                    .id_salt("library-save-name")
                    .limit(120)
                    .show(ui);
                save = button(ui, "Save Design to Library", true);
            });
        self.stores.query = query;
        self.stores.save_name = save_name;
        self.stores.new_folder = new_folder;
        if make_folder {
            let name = std::mem::take(&mut self.stores.new_folder);
            self.create_library_folder(&name);
        }
        if let Some(action) = action {
            self.library_action(action);
        }
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

/// A Library panel button's action.
#[derive(Clone, Debug, PartialEq)]
pub enum LibraryAction {
    Enter(String),
    Up,
    Favourite(String, bool),
    Trash(String),
    Restore(String),
    EmptyBin,
}

/// An item's shown name: its title, else its path.
fn item_title(item: &stencilcraft_doc::library::LibraryItem) -> String {
    if item.title.is_empty() {
        item.path.clone()
    } else {
        item.title.clone()
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
    fn library_folders_favourites_and_the_bin() {
        let dir = std::env::temp_dir().join(format!("sc-lib-folders-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = app_in(&dir);
        app.create_library_folder("Cards / Birthday!");
        assert!(app.status.contains("Cards  Birthday"), "{}", app.status);
        app.create_library_folder("Cards");
        app.library_action(LibraryAction::Enter("Cards".into()));
        app.run_command(Command::ShapeAddRect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        });
        app.save_to_library("tag");
        assert!(
            app.status.contains("Cards/tag"),
            "saved into the folder: {}",
            app.status
        );
        let items = app.library_items();
        assert_eq!(items.len(), 1);
        let rel = items[0].path.clone();
        app.library_action(LibraryAction::Favourite(rel.clone(), true));
        assert!(app.library_items()[0].favourite);
        app.library_action(LibraryAction::Trash(rel.clone()));
        assert!(app.library_items().is_empty());
        let bin = app
            .stores
            .library
            .as_ref()
            .map(Library::trash_list)
            .unwrap_or_default();
        assert_eq!(bin.len(), 1);
        app.library_action(LibraryAction::Restore(bin[0].name.clone()));
        assert_eq!(app.library_items().len(), 1, "restored");
        app.library_action(LibraryAction::Trash(rel));
        app.library_action(LibraryAction::EmptyBin);
        assert!(
            app.stores
                .library
                .as_ref()
                .map(Library::trash_list)
                .unwrap_or_default()
                .is_empty()
        );
        app.library_action(LibraryAction::Up);
        assert!(app.stores.folder.is_empty());
        let _ = std::fs::remove_dir_all(dir);
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
