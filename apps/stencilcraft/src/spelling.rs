//! Type › Spelling… (F7): the framework's Check Spelling dialog over the document's live text,
//! with the bundled English dictionary and the user's own words (kept in the preferences).

use craft_text::spell::{Dictionary, UserWords, words};
use craft_text::ui::spelling::{
    CheckSpellingDialog, SUGGESTION_COUNT, SpellingAction, SpellingItem,
};
use eframe::egui;
use stencilcraft_engine::Command;
use stencilcraft_engine::text::TextEdit;

use crate::StencilApp;

/// The most misspellings listed in one pass.
const MAX_ITEMS: usize = 2_000;

/// `text` with every whole word equal to `word` replaced by `to`.
fn replace_word(text: &str, word: &str, to: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for range in words(text) {
        if text.get(range.clone()) == Some(word) {
            out.push_str(text.get(at..range.start).unwrap_or_default());
            out.push_str(to);
            at = range.end;
        }
    }
    out.push_str(text.get(at..).unwrap_or_default());
    out
}

impl StencilApp {
    /// The live text objects to check: every one with `all_layers`, else the selected ones
    /// (every one when no text is selected).
    fn spelling_targets(&self, all_layers: bool) -> Vec<(u64, String)> {
        let selected: Vec<u64> = self.selected_text_ids().collect();
        self.engine
            .doc
            .shapes()
            .filter_map(|(_, shape)| Some((shape.id, shape.text.as_deref()?.text.clone())))
            .filter(|(id, _)| all_layers || selected.is_empty() || selected.contains(id))
            .collect()
    }

    /// Every misspelling in the text to check, skipping the user's words and `ignored`.
    pub fn find_misspellings(&self, all_layers: bool, ignored: &[String]) -> Vec<SpellingItem> {
        let dictionary = Dictionary::english();
        let user = UserWords::from_words(self.prefs.spelling_words.iter().chain(ignored).cloned());
        let mut out = Vec::new();
        for (id, text) in self.spelling_targets(all_layers) {
            for miss in dictionary.misspellings(&text, &user) {
                if out.len() >= MAX_ITEMS {
                    return out;
                }
                let before = text.get(..miss.range.start).unwrap_or_default();
                let after = text.get(miss.range.end..).unwrap_or_default();
                let suggestions = dictionary.suggest(&miss.word, SUGGESTION_COUNT);
                out.push(
                    SpellingItem::new(id, miss.range, miss.word, suggestions)
                        .context(before, after),
                );
            }
        }
        out
    }

    /// Type › Spelling…: open the dialog over the document's text.
    pub fn open_spelling(&mut self) {
        let items = self.find_misspellings(true, &[]);
        self.spelling = Some(CheckSpellingDialog::new(items).all_layers(true));
    }

    fn set_text(&mut self, id: u64, text: String) {
        if let Err(e) = self.engine.run(Command::TextEdit(TextEdit {
            id,
            text: Some(text),
            ..Default::default()
        })) {
            self.status = e.to_string();
        }
    }

    /// Change: `word` at `range` of `object` becomes `to`, while the word is still there.
    pub fn spelling_change(
        &mut self,
        object: u64,
        range: std::ops::Range<usize>,
        word: &str,
        to: &str,
    ) {
        let Some(mut text) = self.text_spec_of(object).map(|s| s.text) else {
            return;
        };
        if text.get(range.clone()) == Some(word) {
            text.replace_range(range, to);
            self.set_text(object, text);
        }
    }

    /// Change All: every whole `word` becomes `to`, in `object` or in all the text.
    pub fn spelling_change_all(&mut self, object: u64, word: &str, to: &str, all_layers: bool) {
        let targets: Vec<u64> = if all_layers {
            self.spelling_targets(true)
                .into_iter()
                .map(|(id, _)| id)
                .collect()
        } else {
            vec![object]
        };
        for id in targets {
            let Some(text) = self.text_spec_of(id).map(|s| s.text) else {
                continue;
            };
            let changed = replace_word(&text, word, to);
            if changed != text {
                self.set_text(id, changed);
            }
        }
    }

    /// Add: `word` joins the person's dictionary.
    pub fn spelling_add(&mut self, word: &str) {
        let mut user = UserWords::from_words(self.prefs.spelling_words.clone());
        if user.add(word) {
            self.prefs.spelling_words = user.words().to_vec();
            self.save_prefs();
        }
    }

    /// Run one of the dialog's buttons on the document.
    pub fn spelling_action(&mut self, action: SpellingAction) {
        match action {
            SpellingAction::Change { item, to, .. } => {
                self.spelling_change(item.object, item.range, &item.word, &to);
            }
            SpellingAction::ChangeAll {
                item,
                to,
                all_layers,
                ..
            } => self.spelling_change_all(item.object, &item.word, &to, all_layers),
            SpellingAction::Add(word) => self.spelling_add(&word),
            _ => {}
        }
    }

    /// F7 opens Spelling… (not while typing).
    pub fn handle_spelling_key(&mut self, ctx: &egui::Context) -> bool {
        if ctx.text_edit_focused() || self.text_session.is_some() {
            return false;
        }
        let pressed = ctx
            .input_mut(|input| crate::consume_exact(input, egui::Modifiers::NONE, egui::Key::F7));
        if pressed {
            self.open_spelling();
        }
        pressed
    }

    /// The dialog, while open: its buttons change the text, then it lists again.
    pub fn spelling_dialog(&mut self, ui: &mut egui::Ui) {
        let Some(mut dialog) = self.spelling.take() else {
            return;
        };
        let shown = dialog.show(ui);
        if let Some(action) = dialog.take_action() {
            self.spelling_action(action);
            dialog.set_items(self.find_misspellings(dialog.all_layers_checked(), dialog.ignored()));
        }
        if shown.closed.is_none() {
            self.spelling = Some(dialog);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stencilcraft_engine::text::{SINGLE_LINE_FAMILY, TextAdd};
    use stencilcraft_geom::Point;

    fn app_with(texts: &[&str]) -> (StencilApp, Vec<u64>) {
        let mut app = StencilApp::new(None);
        app.prefs.spelling_words.clear();
        for text in texts {
            let mut spec = app.new_text_spec((*text).to_string(), Point::new(10.0, 10.0));
            spec.font.family = SINGLE_LINE_FAMILY.to_string();
            spec.font.file = None;
            app.run_command(Command::TextAdd(TextAdd { spec, style: None }));
        }
        let ids = app
            .engine
            .doc
            .shapes()
            .filter(|(_, s)| s.text.is_some())
            .map(|(_, s)| s.id)
            .collect();
        (app, ids)
    }

    fn text(app: &StencilApp, id: u64) -> String {
        app.text_spec_of(id).map(|s| s.text).unwrap_or_default()
    }

    #[test]
    fn replace_word_changes_whole_words_only() {
        assert_eq!(
            replace_word("teh cat, teh hat", "teh", "the"),
            "the cat, the hat"
        );
        assert_eq!(replace_word("tehran teh", "teh", "the"), "tehran the");
    }

    #[test]
    fn spelling_finds_changes_and_learns_words() {
        let (mut app, ids) = app_with(&["Happy birthdy Stencilcrafty", "birthdy wishes"]);
        let items = app.find_misspellings(true, &[]);
        let words: Vec<&str> = items.iter().map(|i| i.word.as_str()).collect();
        assert_eq!(words, ["birthdy", "Stencilcrafty", "birthdy"]);
        assert!(
            items[0].suggestions.iter().any(|s| s == "birthday"),
            "{:?}",
            items[0].suggestions
        );
        let first = &items[0];
        app.spelling_change(first.object, first.range.clone(), &first.word, "birthday");
        assert_eq!(text(&app, ids[0]), "Happy birthday Stencilcrafty");
        assert_eq!(
            text(&app, ids[1]),
            "birthdy wishes",
            "Change is one occurrence"
        );
        // A stale item (the text moved on) changes nothing.
        app.spelling_change(first.object, first.range.clone(), &first.word, "x");
        assert_eq!(text(&app, ids[0]), "Happy birthday Stencilcrafty");
        app.spelling_change_all(ids[1], "birthdy", "birthday", true);
        assert_eq!(text(&app, ids[1]), "birthday wishes");
        assert_eq!(
            app.find_misspellings(true, &["Stencilcrafty".to_string()])
                .len(),
            0,
            "ignored"
        );
        app.spelling_add("Stencilcrafty");
        assert!(app.find_misspellings(true, &[]).is_empty());
    }
}
