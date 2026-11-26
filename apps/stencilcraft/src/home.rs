//! Silhouette Studio's Home: the framework's start screen in the design area, with New, Open and
//! the recent files. The title bar's Home button shows it; choosing anything goes back to the
//! design.

use craft_ui::contract::Bind;
use craft_ui::shell::{RecentFile, RecentSort, RecentView, StartAction, StartScreen};
use eframe::egui;

use crate::{StencilApp, TopMenuAction};

/// Home's state: shown or not, and the recent list's view and sort.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HomeState {
    pub shown: bool,
    pub view: RecentView,
    pub sort: RecentSort,
}

/// Seconds since the Unix epoch of `time`, when it is after the epoch and fits.
fn epoch_seconds(time: std::time::SystemTime) -> Option<i64> {
    let secs = time.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    i64::try_from(secs).ok()
}

impl StencilApp {
    /// The recent files as the start screen lists them, newest first, with the file's
    /// modification time when it can be read.
    pub fn home_recent(&self) -> Vec<RecentFile> {
        self.recent_files
            .iter()
            .map(|path| {
                let p = std::path::Path::new(path);
                let name = p
                    .file_stem()
                    .map_or_else(|| path.clone(), |s| s.to_string_lossy().into_owned());
                let file = RecentFile::new(p, name);
                match std::fs::metadata(p)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(epoch_seconds)
                {
                    Some(at) => file.opened(at),
                    None => file,
                }
            })
            .collect()
    }

    /// Run one start-screen action. Opening anything leaves Home.
    pub fn home_action(&mut self, action: StartAction) {
        match action {
            StartAction::New => {
                self.home.shown = false;
                self.handle_menu_action(TopMenuAction::NewDoc);
            }
            StartAction::Open => {
                self.home.shown = false;
                self.handle_menu_action(TopMenuAction::OpenDoc);
            }
            StartAction::OpenRecent(path) => {
                self.home.shown = false;
                self.open_document(&path.to_string_lossy());
            }
            StartAction::RemoveRecent(path) => {
                let path = path.to_string_lossy();
                self.recent_files.retain(|p| *p != path);
                self.prefs.recent_files.clone_from(&self.recent_files);
                self.save_prefs();
            }
            StartAction::ClearRecent => {
                self.recent_files.clear();
                self.prefs.recent_files.clear();
                self.save_prefs();
            }
            _ => {}
        }
    }

    /// Home, in place of the design: the framework's start screen.
    pub fn home_body(&mut self, ui: &mut egui::Ui) {
        let recent = self.home_recent();
        let mut view = self.home.view;
        let mut sort = self.home.sort;
        let out =
            StartScreen::new(Bind::bound(&mut view), Bind::bound(&mut sort), &recent).show(ui);
        self.home.view = view;
        self.home.sort = sort;
        for action in out.actions {
            self.home_action(action);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_lists_recent_files_and_removes_them() {
        let mut app = StencilApp::new(None);
        app.recent_files = vec![
            "/tmp/a/card.stencilcraft".into(),
            "/tmp/b/tag.stencilcraft".into(),
        ];
        let names: Vec<String> = app.home_recent().into_iter().map(|f| f.name).collect();
        assert_eq!(names, ["card", "tag"]);
        app.home.shown = true;
        app.home_action(StartAction::RemoveRecent("/tmp/a/card.stencilcraft".into()));
        assert_eq!(app.recent_files, ["/tmp/b/tag.stencilcraft"]);
        assert!(app.home.shown, "removing keeps Home shown");
        app.home_action(StartAction::ClearRecent);
        assert!(app.recent_files.is_empty());
        app.home_action(StartAction::New);
        assert!(!app.home.shown, "New leaves Home");
    }
}
