//! The tool rail, drawn by the app: slots of tools, the slot's last-used tool, one or two columns and a flyout of
//! the slot's tools. At the pin `craft_ui::rails::ToolRail` takes registered commands for its tools, which the app
//! does not have, so the app keeps the rail it had (tools, order, flyouts, icons, tooltips, shortcuts) on the pin's
//! `IconButton`. Tool keys stay with the app's keymap (`handle_keyboard_shortcuts`), as before.

use craft_behavior::IconId;
use craft_ui::buttons::{IconButton, PopupMark};
use craft_ui::contract::{PopupKind, Props as _, Show as _};
use eframe::egui;
use serde::{Deserialize, Serialize};

/// Names a slot of the rail; [`LastUsed`] remembers a tool under it.
pub type SlotId = u32;

/// The rail's side of a button (the old `rail.button`).
const BUTTON: f32 = 32.0;
/// The space between buttons in a row and between rows (the old `rail.gap` and `rail.row_gap`).
const GAP: f32 = 4.0;
/// The space at each side of the buttons (the old `(rail.width - rail.button) / 2`).
const PAD_X: f32 = 6.0;
/// The space above the first button and under the last (the old `rail.pad_y`).
const PAD_Y: f32 = 6.0;

/// One tool of a slot.
#[derive(Clone, Debug, PartialEq)]
pub struct RailTool<T> {
    pub id: T,
    pub icon: IconId,
    /// The tool's name: tooltip, flyout row and accessible name.
    pub label: String,
    /// The key that selects the tool ("B"), shown in the tooltip and the flyout.
    pub shortcut: Option<String>,
}

impl<T> RailTool<T> {
    /// A tool without a shortcut.
    pub fn new(id: T, icon: IconId, label: impl Into<String>) -> Self {
        Self {
            id,
            icon,
            label: label.into(),
            shortcut: None,
        }
    }

    #[must_use]
    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        let shortcut = shortcut.into();
        self.shortcut = (!shortcut.is_empty()).then_some(shortcut);
        self
    }

    /// "Brush  (B)", the tooltip of the old rail.
    fn tooltip(&self) -> String {
        match &self.shortcut {
            Some(key) => format!("{}  ({key})", self.label),
            None => self.label.clone(),
        }
    }
}

/// A group of tools that share one button; its flyout lists them.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolSlot<T> {
    pub id: SlotId,
    pub tools: Vec<RailTool<T>>,
}

impl<T> ToolSlot<T> {
    pub fn new(id: SlotId, tools: Vec<RailTool<T>>) -> Self {
        Self { id, tools }
    }
}

/// The tool each slot showed last.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LastUsed<T> {
    tools: Vec<(SlotId, T)>,
}

impl<T> Default for LastUsed<T> {
    fn default() -> Self {
        Self { tools: Vec::new() }
    }
}

impl<T: PartialEq> LastUsed<T> {
    /// The tool remembered for `slot`.
    pub fn get(&self, slot: SlotId) -> Option<&T> {
        self.tools
            .iter()
            .find(|(id, _)| *id == slot)
            .map(|(_, tool)| tool)
    }

    /// Remembers `tool` for `slot`.
    pub fn set(&mut self, slot: SlotId, tool: T) {
        match self.tools.iter_mut().find(|(id, _)| *id == slot) {
            Some(entry) => entry.1 = tool,
            None => self.tools.push((slot, tool)),
        }
    }
}

/// The tool a slot shows: the current tool when the slot has it, else the one remembered in `last_used`, else the
/// first. `None` for an empty slot.
pub fn shown_tool<'a, T: PartialEq>(
    current: &T,
    slot: &'a ToolSlot<T>,
    last_used: &LastUsed<T>,
) -> Option<&'a RailTool<T>> {
    let find = |id: &T| slot.tools.iter().find(|tool| tool.id == *id);
    find(current)
        .or_else(|| last_used.get(slot.id).and_then(find))
        .or_else(|| slot.tools.first())
}

/// What [`show`] reports.
#[derive(Clone, Debug)]
pub struct RailOutput<T> {
    /// The tool to make current: the shown tool of a clicked slot, or the tool picked in a flyout.
    pub select: Option<T>,
    /// The column toggle was clicked; the host flips its column count.
    pub toggle_columns: bool,
}

/// Draws the rail as a side panel at the left edge, in `columns` (1 or 2) columns, with the column toggle named
/// `toggle_label` at the top. A click on a slot selects its shown tool; a right click on a slot with several tools
/// opens its flyout, whose rows select that tool. `last_used` is updated with `current`.
pub fn show<T: Clone + PartialEq>(
    ui: &mut egui::Ui,
    id_salt: &str,
    columns: usize,
    toggle_label: &str,
    slots: &[ToolSlot<T>],
    current: &T,
    last_used: &mut LastUsed<T>,
) -> RailOutput<T> {
    let columns = columns.clamp(1, 2);
    let width = PAD_X * 2.0 + BUTTON * columns as f32 + GAP * (columns as f32 - 1.0);
    let mut out = RailOutput {
        select: None,
        toggle_columns: false,
    };
    let frame = egui::Frame::NONE
        .fill(ui.visuals().panel_fill)
        .inner_margin(egui::Margin::symmetric(PAD_X as i8, PAD_Y as i8));
    let _ = egui::Panel::left((id_salt, "panel"))
        .exact_size(width)
        .resizable(false)
        .show_separator_line(false)
        .frame(frame)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(GAP, GAP);
            let chevron = if columns == 1 {
                "chevrons_right"
            } else {
                "chevrons_left"
            };
            out.toggle_columns = IconButton::new(IconId::new(chevron))
                .label(toggle_label.to_owned())
                .tooltip(toggle_label.to_owned())
                .id_salt((id_salt, "column-toggle"))
                .show(ui)
                .clicked();
            egui::ScrollArea::vertical()
                .id_salt((id_salt, "slots"))
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for row in slots.chunks(columns) {
                        ui.horizontal(|ui| {
                            for slot in row {
                                if let Some(tool) =
                                    slot_button(ui, id_salt, slot, current, last_used)
                                {
                                    out.select = Some(tool);
                                }
                            }
                        });
                    }
                });
        });
    out
}

/// One slot's button, showing the slot's shown tool, toggled on while the slot holds the current tool. Returns the
/// tool to make current.
fn slot_button<T: Clone + PartialEq>(
    ui: &mut egui::Ui,
    id_salt: &str,
    slot: &ToolSlot<T>,
    current: &T,
    last_used: &mut LastUsed<T>,
) -> Option<T> {
    let selected = slot.tools.iter().any(|tool| tool.id == *current);
    if selected {
        last_used.set(slot.id, current.clone());
    }
    let shown = shown_tool(current, slot, last_used)?;
    let several = slot.tools.len() > 1;
    let mut button = IconButton::new(shown.icon.clone())
        .label(shown.label.clone())
        .tooltip(shown.tooltip())
        .selected(selected)
        .id_salt((id_salt, "slot", slot.id));
    if let Some(key) = &shown.shortcut {
        button = button.description(key.clone());
    }
    if several {
        button = button
            .popup(PopupKind::Menu)
            .popup_mark(PopupMark::ToolGroup);
    }
    let output = button.show(ui);
    let mut select = output.clicked().then(|| shown.id.clone());
    if several {
        let _ = output.response.context_menu(|ui| {
            for tool in &slot.tools {
                let text = match &tool.shortcut {
                    Some(key) => format!("{}\t{key}", tool.label),
                    None => tool.label.clone(),
                };
                if ui.selectable_label(tool.id == shown.id, text).clicked() {
                    select = Some(tool.id.clone());
                    ui.close();
                }
            }
        });
    }
    select
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot() -> ToolSlot<u8> {
        ToolSlot::new(
            3,
            vec![
                RailTool::new(1, IconId::new("square"), "Rectangle").shortcut("R"),
                RailTool::new(2, IconId::new("circle"), "Ellipse"),
            ],
        )
    }

    #[test]
    fn a_slot_shows_its_current_then_last_used_then_first_tool() {
        let slot = slot();
        let mut last = LastUsed::default();
        assert_eq!(shown_tool(&9, &slot, &last).map(|t| t.id), Some(1));
        last.set(3, 2);
        assert_eq!(shown_tool(&9, &slot, &last).map(|t| t.id), Some(2));
        assert_eq!(shown_tool(&1, &slot, &last).map(|t| t.id), Some(1));
    }

    #[test]
    fn the_tooltip_names_the_shortcut() {
        let slot = slot();
        assert_eq!(slot.tools[0].tooltip(), "Rectangle  (R)");
        assert_eq!(slot.tools[1].tooltip(), "Ellipse");
    }
}
