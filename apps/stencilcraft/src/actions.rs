//! Actions: record a sequence of edits and play it again on another selection, as
//! Illustrator's Actions panel does. Recording keeps each command (the engine's recorder);
//! playing runs them again with the current selection in place of the recorded shapes, each
//! step's new shapes becoming the selection for the next.

use craft_ui::controls::TextField;
use craft_behavior::EditEvent;
use craft_ui::patterns::Panel;
use eframe::egui;
use serde_json::{Value, json};
use stencilcraft_engine::{Command, Effect};

use crate::StencilApp;
use crate::panels::{button, hint};

/// A saved action: its name and the recorded commands (JSON, as the engine takes them).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedAction {
    pub name: String,
    pub steps: Vec<Value>,
}

/// Most steps one action keeps.
const MAX_STEPS: usize = 500;

/// `step` with the selection in place of the shapes it was recorded on.
fn retarget(step: &Value, selection: &[u64]) -> Value {
    let mut step = step.clone();
    if let Some(params) = step.get_mut("params").and_then(Value::as_object_mut) {
        if params.contains_key("ids") {
            params.insert("ids".into(), json!(selection));
        }
        if params.contains_key("id")
            && let Some(first) = selection.first()
        {
            params.insert("id".into(), json!(first));
        }
    }
    step
}

impl StencilApp {
    pub fn start_recording(&mut self) {
        self.engine.recorder = Some(Vec::new());
        self.status = "recording an action".to_string();
    }

    /// Stop recording and keep the steps as action `name`.
    pub fn stop_recording(&mut self, name: &str) {
        let Some(mut steps) = self.engine.recorder.take() else {
            return;
        };
        steps.truncate(MAX_STEPS);
        let name = name.trim();
        if steps.is_empty() || name.is_empty() {
            self.status = "nothing recorded (or no name)".to_string();
            return;
        }
        self.prefs.actions.retain(|a| a.name != name);
        self.prefs.actions.push(SavedAction {
            name: name.to_string(),
            steps,
        });
        self.save_prefs();
        self.status = format!("saved action {name}");
    }

    /// Play action `index` on the selection; stops at the first step that fails.
    pub fn play_action(&mut self, index: usize) {
        let Some(action) = self.prefs.actions.get(index).cloned() else {
            return;
        };
        let mut selection = self.selected_shapes.clone();
        for (n, step) in action.steps.iter().enumerate() {
            let step = retarget(step, &selection);
            let command: Command = match serde_json::from_value(step) {
                Ok(c) => c,
                Err(e) => {
                    self.status = format!("{}: step {} is not a command: {e}", action.name, n + 1);
                    return;
                }
            };
            match self.engine.run(command) {
                Ok(Effect::ShapesAdded { ids }) if !ids.is_empty() => selection = ids,
                Ok(Effect::ShapeAdded { id }) => selection = vec![id],
                Ok(_) => {}
                Err(e) => {
                    self.status = format!("{}: step {} failed: {e}", action.name, n + 1);
                    self.set_selection(selection);
                    return;
                }
            }
        }
        self.set_selection(selection);
        self.retain_valid_selection();
        self.status = format!("played {}", action.name);
    }

    pub fn actions_panel(&mut self, ui: &mut egui::Ui) {
        let recording = self.engine.recorder.as_ref().map(Vec::len);
        let actions = self.prefs.actions.clone();
        let mut name = self.action_name_draft.clone();
        let (mut start, mut stop) = (false, false);
        let mut play = None;
        let mut remove = None;
        let _ = Panel::new("stencilcraft-actions-panel").show(ui, |ui| {
            match recording {
                None => {
                    start = button(ui, "Start Recording", true);
                }
                Some(steps) => {
                    ui.label(format!("Recording… {steps} steps"));
                    let edit = TextField::new("action-name", "Action name", name.as_str())
                        .char_limit(80)
                        .show(ui);
                    if let Some(
                        EditEvent::Update { value, .. }
                        | EditEvent::Commit { value, .. }
                        | EditEvent::Set { value, .. },
                    ) = edit.event
                    {
                        name = value;
                    }
                    stop = button(ui, "Stop and Save", true);
                }
            }
            for (i, a) in actions.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("{} ({} steps)", a.name, a.steps.len()));
                    if button(ui, "Play", recording.is_none()) {
                        play = Some(i);
                    }
                    if button(ui, "Delete", true) {
                        remove = Some(i);
                    }
                });
            }
            hint(
                ui,
                "Record edits, then play them on another selection: the recorded shapes are \
                 replaced by what is selected.",
            );
        });
        self.action_name_draft = name.clone();
        if start {
            self.start_recording();
        }
        if stop {
            self.stop_recording(&name);
        }
        if let Some(i) = play {
            self.play_action(i);
        }
        if let Some(i) = remove
            && i < self.prefs.actions.len()
        {
            self.prefs.actions.remove(i);
            self.save_prefs();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_play_on_another_selection() {
        let mut app = StencilApp::new(None);
        for x in [0.0, 50.0] {
            app.run_command(Command::ShapeAddRect {
                x,
                y: 0.0,
                width: 20.0,
                height: 20.0,
            });
        }
        let ids: Vec<u64> = app.engine.doc.shapes().map(|(_, s)| s.id).collect();
        app.set_selection([ids[0]]);
        app.start_recording();
        // Offset the selection, then perforate the offset.
        app.select_added(Command::ShapesOffset {
            ids: vec![ids[0]],
            distance_mm: 2.0,
            join: stencilcraft_kerf::JoinStyle::Round,
            miter_limit: 4.0,
            weld: false,
        });
        let offset = app.selected_shapes.clone();
        app.run_command(Command::GeneratePerforate(
            stencilcraft_engine::generate::PerforateArgs {
                ids: offset,
                cut_mm: 3.0,
                gap_mm: 1.0,
            },
        ));
        app.stop_recording("Offset and perforate");
        assert_eq!(app.prefs.actions.len(), 1);
        assert_eq!(app.prefs.actions[0].steps.len(), 2);
        let before = app.engine.doc.shapes().count();
        app.set_selection([ids[1]]);
        app.play_action(0);
        assert_eq!(
            app.engine.doc.shapes().count(),
            before + 1,
            "{}",
            app.status
        );
        assert!(app.status.starts_with("played"), "{}", app.status);
    }
}
