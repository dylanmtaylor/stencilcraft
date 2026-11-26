//! Document tabs (Silhouette's tabs above the design area): several documents open in one
//! window, each with its own undo history, selection and view. The live document is the
//! app's own fields; the others wait in their tab as a [`DocSlot`]. Shown with the
//! framework's [`DocumentTabs`].

use craft_ui::contract::{Bind, Props as _, Show as _, ShowContainer as _};
use craft_ui::tabs::{DocumentTab, DocumentTabs, DocumentTabsEvent};
use eframe::egui;

use crate::{StencilApp, new_doc};

/// The most documents open at once.
pub const MAX_TABS: usize = 32;

/// A document waiting in a background tab.
pub struct DocSlot {
    engine: stencilcraft_engine::Engine,
    saved_document: stencilcraft_doc::Document,
    doc_title: String,
    open_path: Option<String>,
    mat: stencilcraft_machine::Mat,
    selected: Vec<u64>,
    view: stencilcraft_ui_egui::View,
    view_initialized: bool,
    last_board: Option<(f64, f64)>,
}

/// One tab: its id, and its document unless it is the live one.
pub struct TabEntry {
    pub id: u64,
    pub slot: Option<DocSlot>,
}

/// The open documents, in tab order.
pub struct DocTabs {
    pub active: u64,
    pub entries: Vec<TabEntry>,
    next_id: u64,
    /// A tab with unsaved changes waiting for Discard or Cancel.
    pub pending_close: Option<u64>,
}

impl Default for DocTabs {
    fn default() -> Self {
        DocTabs {
            active: 1,
            entries: vec![TabEntry { id: 1, slot: None }],
            next_id: 2,
            pending_close: None,
        }
    }
}

/// A tab's name: the file's name, else the document's title.
fn tab_name(open_path: Option<&str>, title: &str) -> String {
    open_path
        .and_then(|p| std::path::Path::new(p).file_stem()?.to_str())
        .map_or_else(|| title.to_string(), str::to_string)
}

impl StencilApp {
    /// The recovery snapshot key of tab `id`: the session's own for the first tab.
    pub fn recovery_key(&self, id: u64) -> String {
        if id == 1 {
            self.stores.session_id.clone()
        } else {
            format!("{}-tab{id}", self.stores.session_id)
        }
    }

    /// Whether the live document is untouched and unnamed, so New or Open can use its tab.
    fn tab_is_blank(&self) -> bool {
        self.open_path.is_none()
            && self.engine.doc == self.saved_document
            && self.engine.doc.shapes().next().is_none()
    }

    /// Whether the live document has unsaved changes.
    pub fn is_modified(&self) -> bool {
        self.engine.doc != self.saved_document
    }

    /// Drafts and previews belong to the live document; they end when it changes.
    fn end_live_drafts(&mut self) {
        self.text_session = None;
        self.distort = None;
        self.pen_draft = None;
        self.box_preview = None;
        self.kerf_preview_cache = None;
        self.snap_lines.clear();
    }

    /// The live document moves into a slot; the fields hold a blank document after.
    fn take_live(&mut self) -> DocSlot {
        self.end_live_drafts();
        // A modified document keeps a recovery snapshot while it waits in the background.
        if self.is_modified()
            && let Some(recovery) = &self.stores.recovery
            && let Err(e) =
                recovery.snapshot(&self.recovery_key(self.tabs.active), &self.engine.doc)
        {
            self.status = format!("autosave: {e}");
        }
        let slot = DocSlot {
            engine: std::mem::take(&mut self.engine),
            saved_document: std::mem::take(&mut self.saved_document),
            doc_title: std::mem::take(&mut self.doc_title),
            open_path: self.open_path.take(),
            mat: self.mat,
            selected: self.selected_shapes.clone(),
            view: self.view,
            view_initialized: self.view_initialized,
            last_board: self.last_board,
        };
        self.set_selection([]);
        slot
    }

    fn install(&mut self, slot: DocSlot) {
        self.end_live_drafts();
        self.engine = slot.engine;
        self.saved_document = slot.saved_document;
        self.doc_title = slot.doc_title;
        self.open_path = slot.open_path;
        self.mat = slot.mat;
        self.view = slot.view;
        self.view_initialized = slot.view_initialized;
        self.last_board = slot.last_board;
        self.set_selection(slot.selected);
    }

    /// Show tab `id`'s document.
    pub fn switch_tab(&mut self, id: u64) {
        if id == self.tabs.active {
            return;
        }
        let Some(at) = self.tabs.entries.iter().position(|e| e.id == id) else {
            return;
        };
        let Some(slot) = self.tabs.entries.get_mut(at).and_then(|e| e.slot.take()) else {
            return;
        };
        let live = self.take_live();
        let active = self.tabs.active;
        if let Some(entry) = self.tabs.entries.iter_mut().find(|e| e.id == active) {
            entry.slot = Some(live);
        }
        self.tabs.active = id;
        self.install(slot);
    }

    /// A new tab with a blank document, shown. `false` when the tab limit is reached.
    pub fn open_blank_tab(&mut self) -> bool {
        if self.tabs.entries.len() >= MAX_TABS {
            self.status = format!("at most {MAX_TABS} documents can be open; close one first");
            return false;
        }
        let live = self.take_live();
        let active = self.tabs.active;
        if let Some(entry) = self.tabs.entries.iter_mut().find(|e| e.id == active) {
            entry.slot = Some(live);
        }
        let id = self.tabs.next_id;
        self.tabs.next_id = self.tabs.next_id.saturating_add(1);
        let at = self
            .tabs
            .entries
            .iter()
            .position(|e| e.id == active)
            .map_or(self.tabs.entries.len(), |i| i + 1);
        self.tabs.entries.insert(at, TabEntry { id, slot: None });
        self.tabs.active = id;
        self.saved_document = self.engine.doc.clone();
        self.doc_title = format!("Untitled-{}", self.tabs.entries.len());
        self.view_initialized = false;
        self.last_board = None;
        true
    }

    /// New and Open land in the live tab when it is blank, else in a new one. `false` when
    /// no tab could be opened.
    pub fn tab_for_new_document(&mut self) -> bool {
        self.tab_is_blank() || self.open_blank_tab()
    }

    /// The titles of every open document.
    fn titles(&self) -> Vec<String> {
        std::iter::once(self.doc_title.clone())
            .chain(
                self.tabs
                    .entries
                    .iter()
                    .filter_map(|e| e.slot.as_ref().map(|s| s.doc_title.clone())),
            )
            .collect()
    }

    /// The name New proposes: "Untitled-N", the lowest N no other open document has. The live
    /// document's own name counts only when New will open a new tab.
    pub fn next_untitled(&self) -> String {
        let blank = self.tab_is_blank();
        let titles = self.titles();
        let taken = |name: &str| {
            titles
                .iter()
                .enumerate()
                .any(|(i, t)| t == name && !(i == 0 && blank))
        };
        (1..=MAX_TABS + 1)
            .map(|n| format!("Untitled-{n}"))
            .find(|name| !taken(name))
            .unwrap_or_else(|| "Untitled".to_string())
    }

    /// The tab already showing the file at `path`, if any.
    pub fn tab_with_path(&self, path: &str) -> Option<u64> {
        let same = |p: &str| {
            p == path
                || std::fs::canonicalize(p).ok() == std::fs::canonicalize(path).ok()
                    && std::fs::canonicalize(path).is_ok()
        };
        if self.open_path.as_deref().is_some_and(same) {
            return Some(self.tabs.active);
        }
        self.tabs.entries.iter().find_map(|e| {
            let slot = e.slot.as_ref()?;
            slot.open_path.as_deref().is_some_and(same).then_some(e.id)
        })
    }

    /// File › Close Tab (Ctrl+W) on tab `id`: asks first when it has unsaved changes.
    pub fn request_close_tab(&mut self, id: u64) {
        let modified = if id == self.tabs.active {
            self.is_modified()
        } else {
            self.tabs
                .entries
                .iter()
                .find(|e| e.id == id)
                .and_then(|e| e.slot.as_ref())
                .is_some_and(|s| s.engine.doc != s.saved_document)
        };
        if modified {
            self.tabs.pending_close = Some(id);
        } else {
            self.close_tab(id);
        }
    }

    /// Close tab `id` without asking. The last tab is replaced by a fresh document.
    pub fn close_tab(&mut self, id: u64) {
        let Some(at) = self.tabs.entries.iter().position(|e| e.id == id) else {
            return;
        };
        if let Some(recovery) = &self.stores.recovery {
            let _ = recovery.discard(&self.recovery_key(id));
        }
        if self.tabs.entries.len() == 1 {
            self.end_live_drafts();
            self.engine = stencilcraft_engine::Engine::default();
            self.saved_document = stencilcraft_doc::Document::default();
            self.create_document(&new_doc::Choice::default());
            return;
        }
        if id == self.tabs.active {
            // Show the neighbour: the next tab, else the one before.
            let neighbour = self
                .tabs
                .entries
                .get(at + 1)
                .or_else(|| at.checked_sub(1).and_then(|i| self.tabs.entries.get(i)))
                .map(|e| e.id);
            if let Some(next) = neighbour {
                self.switch_tab(next);
            }
        }
        self.tabs.entries.retain(|e| e.id != id);
    }

    /// The tab strip above the design area, and the discard question of a closing tab.
    pub fn tab_strip(&mut self, ui: &mut egui::Ui) {
        let active = self.tabs.active;
        let tabs: Vec<DocumentTab<u64>> = self
            .tabs
            .entries
            .iter()
            .map(|e| {
                let (name, modified) = match &e.slot {
                    Some(s) => (
                        tab_name(s.open_path.as_deref(), &s.doc_title),
                        s.engine.doc != s.saved_document,
                    ),
                    None => (
                        tab_name(self.open_path.as_deref(), &self.doc_title),
                        self.is_modified(),
                    ),
                };
                let mut tab = DocumentTab::new(e.id, name);
                tab.modified = modified;
                tab
            })
            .collect();
        let mut add = false;
        let out = DocumentTabs::new("Documents", &tabs)
            .selected(active)
            .reorderable(true)
            .trailing(|ui| {
                if craft_ui::buttons::IconButton::new(craft_behavior::IconId::new("plus"))
                    .label("New Document")
                    .show(ui)
                    .clicked()
                {
                    add = true;
                }
            })
            .show(ui);
        for event in out.events {
            match event {
                DocumentTabsEvent::Selected(id)
                | DocumentTabsEvent::Activated(id)
                | DocumentTabsEvent::Picked(id) => self.switch_tab(id),
                DocumentTabsEvent::CloseRequested(id) => self.request_close_tab(id),
                DocumentTabsEvent::Reordered {
                    documents, before, ..
                } => {
                    let moved: Vec<TabEntry> = documents
                        .iter()
                        .filter_map(|d| {
                            let i = self.tabs.entries.iter().position(|e| e.id == *d)?;
                            Some(self.tabs.entries.remove(i))
                        })
                        .collect();
                    let at = before
                        .and_then(|b| self.tabs.entries.iter().position(|e| e.id == b))
                        .unwrap_or(self.tabs.entries.len());
                    for (n, entry) in moved.into_iter().enumerate() {
                        self.tabs
                            .entries
                            .insert(at.saturating_add(n).min(self.tabs.entries.len()), entry);
                    }
                }
                DocumentTabsEvent::EmptyDoubleClick => add = true,
                _ => {}
            }
        }
        if add {
            self.open_new_document(0);
        }
        self.close_question(ui);
    }

    fn close_question(&mut self, ui: &mut egui::Ui) {
        use craft_ui::dialog::Dialog;
        use craft_ui::sections::{DialogRole, FooterButton};
        let Some(id) = self.tabs.pending_close else {
            return;
        };
        let mut open = true;
        let shown = Dialog::new("Discard unsaved changes?")
            .id_salt("discard-before-close")
            .opened(Bind::bound(&mut open))
            .buttons([
                FooterButton::new("Discard changes", DialogRole::Accept),
                FooterButton::new("Cancel", DialogRole::Cancel),
            ])
            .show(ui, |ui, _| {
                let _ = craft_ui::display::Text::new(
                    "This document has changes that are not saved. Cancel to save it first.",
                )
                .show(ui);
            });
        if shown.output.picked == Some(0) {
            self.tabs.pending_close = None;
            self.close_tab(id);
        } else if !open || shown.output.closed.is_some() {
            self.tabs.pending_close = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stencilcraft_engine::Command;

    fn add_rect(app: &mut StencilApp) {
        app.run_command(Command::ShapeAddRect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        });
    }

    #[test]
    fn tabs_keep_their_own_document_undo_and_selection() {
        let mut app = StencilApp::new(None);
        add_rect(&mut app);
        let first = app.tabs.active;
        let ids: Vec<u64> = app.engine.doc.shapes().map(|(_, s)| s.id).collect();
        app.set_selection(ids.clone());
        assert!(app.tab_for_new_document(), "a changed tab is not reused");
        let second = app.tabs.active;
        assert_ne!(first, second);
        assert_eq!(app.tabs.entries.len(), 2);
        assert!(
            app.engine.doc.shapes().next().is_none(),
            "the new tab is blank"
        );
        assert!(app.selected_shapes.is_empty());
        assert!(!app.engine.can_undo(), "its own history");
        // A blank tab is reused.
        assert!(app.tab_for_new_document());
        assert_eq!(app.tabs.entries.len(), 2);
        app.switch_tab(first);
        assert_eq!(app.engine.doc.shapes().count(), 1);
        assert_eq!(app.selected_shapes, ids);
        assert!(app.engine.undo(), "the first tab's history came back");
        // A tab that is not there changes nothing.
        app.switch_tab(999);
        assert_eq!(app.tabs.active, first);
    }

    #[test]
    fn closing_asks_for_changes_and_the_last_tab_becomes_fresh() {
        let mut app = StencilApp::new(None);
        add_rect(&mut app);
        let first = app.tabs.active;
        app.open_blank_tab();
        let second = app.tabs.active;
        app.request_close_tab(first);
        assert_eq!(
            app.tabs.pending_close,
            Some(first),
            "unsaved changes ask first"
        );
        app.tabs.pending_close = None;
        app.request_close_tab(second);
        assert_eq!(app.tabs.entries.len(), 1, "an unchanged tab closes at once");
        assert_eq!(app.tabs.active, first);
        assert_eq!(app.engine.doc.shapes().count(), 1);
        app.close_tab(first);
        assert_eq!(app.tabs.entries.len(), 1);
        assert!(app.engine.doc.shapes().next().is_none(), "a fresh document");
        assert!(!app.is_modified());
    }

    #[test]
    fn new_documents_count_up_their_untitled_names() {
        let mut app = StencilApp::new(None);
        assert_eq!(app.next_untitled(), "Untitled-1", "a blank tab is reused");
        app.doc_title = "Untitled-1".to_string();
        add_rect(&mut app);
        assert_eq!(app.next_untitled(), "Untitled-2");
        app.open_blank_tab();
        app.doc_title = "Untitled-2".to_string();
        add_rect(&mut app);
        assert_eq!(app.next_untitled(), "Untitled-3");
    }

    #[test]
    fn the_tab_limit_holds() {
        let mut app = StencilApp::new(None);
        for _ in 0..MAX_TABS + 3 {
            app.open_blank_tab();
        }
        assert_eq!(app.tabs.entries.len(), MAX_TABS);
    }
}
