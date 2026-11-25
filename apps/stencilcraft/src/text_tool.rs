//! Silhouette Studio's Text tool, typed on the canvas: click the page and type. The click
//! point is the first line's baseline start; letters appear as you type, each keystroke one
//! undo step; Backspace, Delete, the arrows, Home/End and Enter edit as in a text box; a click
//! inside the text moves the caret; Esc (switching to the Select tool) or a click elsewhere
//! ends editing. Double-clicking text with the Select tool edits it again. The Text Style panel
//! sets the font, size, alignment, line and character spacing and kerning of the text being
//! typed or the selected text, and of new text.

use craft_theme::ControlSize;
use craft_ui::choice::{Checkbox, Segmented};
use craft_ui::contract::{Bind, Entry, Props as _, Show as _, ShowContainer as _};
use craft_ui::pickers::Select;
use craft_ui::sections::{Panel, Section};
use craft_ui::text::TextField;
use eframe::egui;
use stencilcraft_doc::{FontSpec, TextAlign, TextSpec};
use stencilcraft_engine::text::{TextAdd, TextEdit};
use stencilcraft_engine::{Command, Effect};
use stencilcraft_geom::{BezPath, Point, Shape as _};

use crate::panels::{button, hint, labelled, number};
use crate::{ActiveTool, StencilApp, WorkspacePanel};

/// Silhouette's Character Spacing: each 100 % above normal adds this share of the em between
/// letters (measured in SS 5.0.556 with Arial: 200 % and 300 % add 0.366 and 0.732 em).
pub const CHAR_SPACING_EM_PER_100_PCT: f64 = 0.366;

/// Points per millimetre (text sizes are in points).
const POINTS_PER_MM: f64 = 72.0 / 25.4;

/// Text size for new text, as in Silhouette.
pub const DEFAULT_TEXT_SIZE_PT: f64 = 72.0;

/// Outline fonts tried, in order, for new text (Silhouette uses Arial).
const PREFERRED_FAMILIES: [&str; 6] = [
    "Arial",
    "Liberation Sans",
    "Helvetica",
    "DejaVu Sans",
    "Noto Sans",
    "Segoe UI",
];

/// The engine's letter spacing (percent of the em) for a Silhouette Character Spacing.
pub fn letter_spacing_for(pct: f64) -> f64 {
    (pct - 100.0) * CHAR_SPACING_EM_PER_100_PCT
}

/// The Silhouette Character Spacing for an engine letter spacing.
pub fn char_spacing_pct(letter_spacing: f64) -> f64 {
    100.0 + letter_spacing / CHAR_SPACING_EM_PER_100_PCT
}

/// Style for new text and the Text Style panel (the family and size live beside it on the
/// app as `text_family` and `text_size`).
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    pub align: TextAlign,
    /// Silhouette's Character Spacing, percent (100 = the font's own spacing).
    pub char_pct: f64,
    /// Line Spacing, percent of the normal line distance.
    pub line_pct: f64,
    pub kerning: bool,
    /// Letters stacked top to bottom (Silhouette's vertical text direction).
    pub vertical: bool,
    /// The Text Style panel's font search.
    pub query: String,
    /// The font style (Bold, Italic…); empty is the family's regular style.
    pub font_style: String,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            align: TextAlign::Left,
            char_pct: 100.0,
            line_pct: 100.0,
            kerning: true,
            vertical: false,
            query: String::new(),
            font_style: String::new(),
        }
    }
}

/// Text being typed on the canvas.
#[derive(Clone, Debug, PartialEq)]
pub struct TextSession {
    /// The text shape, once the first letter is typed.
    pub id: Option<u64>,
    /// The first line's baseline start for new text (millimetres).
    pub anchor: Point,
    /// The caret as a byte index into the text, always on a character boundary.
    pub caret: usize,
}

/// The character boundary before `at` in `text` (or 0).
fn prev_boundary(text: &str, at: usize) -> usize {
    text.get(..at)
        .and_then(|head| head.char_indices().last())
        .map_or(0, |(i, _)| i)
}

/// The character boundary after `at` in `text` (or its end).
fn next_boundary(text: &str, at: usize) -> usize {
    text.get(at..)
        .and_then(|tail| tail.chars().next())
        .map_or(text.len(), |c| at.saturating_add(c.len_utf8()))
}

/// `at` brought onto a character boundary within `text`.
fn clamp_boundary(text: &str, at: usize) -> usize {
    let mut at = at.min(text.len());
    while at > 0 && !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

/// The start of the line holding `at`.
fn line_start(text: &str, at: usize) -> usize {
    text.get(..at)
        .and_then(|head| head.rfind('\n'))
        .map_or(0, |i| i + 1)
}

/// The end of the line holding `at` (before its newline).
fn line_end(text: &str, at: usize) -> usize {
    text.get(at..)
        .and_then(|tail| tail.find('\n'))
        .map_or(text.len(), |i| at.saturating_add(i))
}

/// Typed text without control characters (newlines come from Enter).
fn typed(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).collect()
}

/// Pasted text with line breaks kept and other control characters dropped.
fn pasted(s: &str) -> String {
    s.replace("\r\n", "\n")
        .chars()
        .filter(|c| *c == '\n' || !c.is_control())
        .collect()
}

/// The caret nearest `p` among `spec`'s caret positions (by distance to the caret line).
fn nearest_caret(spec: &TextSpec, p: Point) -> usize {
    let Ok(carets) = stencilcraft_engine::text::carets(spec) else {
        return spec.text.len();
    };
    carets
        .iter()
        .map(|c| {
            let mid = c.base.midpoint(c.top);
            (c.byte, (mid - p).hypot())
        })
        .filter(|(_, d)| d.is_finite())
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map_or(spec.text.len(), |(byte, _)| byte)
}

fn rect_path(x0: f64, y0: f64, x1: f64, y1: f64) -> BezPath {
    let mut path = BezPath::new();
    path.move_to((x0, y0));
    path.line_to((x1, y0));
    path.line_to((x1, y1));
    path.line_to((x0, y1));
    path.close_path();
    path
}

const BOX_COLOUR: egui::Color32 = egui::Color32::from_rgb(40, 190, 60);
const CARET_COLOUR: egui::Color32 = egui::Color32::from_rgb(225, 30, 30);

impl StencilApp {
    /// Load the font list once, and choose a family for new text if none is set.
    pub fn ensure_fonts(&mut self) {
        if self.text_fonts.is_empty()
            && let Ok(Effect::Fonts { families }) = self.engine.run(Command::TextFonts {
                query: String::new(),
            })
        {
            self.text_fonts = families;
        }
        if self.text_family.is_empty() {
            let has = |name: &str| self.text_fonts.iter().any(|f| f.family == name);
            self.text_family = PREFERRED_FAMILIES
                .iter()
                .find(|name| has(name))
                .map(|name| (*name).to_string())
                .or_else(|| {
                    self.text_fonts
                        .iter()
                        .find(|f| !f.single_line)
                        .map(|f| f.family.clone())
                })
                .unwrap_or_else(|| stencilcraft_engine::text::SINGLE_LINE_FAMILY.to_string());
        }
    }

    /// The font for `text_family`: its Regular style, else its first.
    fn text_font(&self) -> FontSpec {
        let wanted = self.text_style.font_style.as_str();
        let style = self
            .text_fonts
            .iter()
            .find(|f| f.family == self.text_family)
            .and_then(|f| {
                f.styles
                    .iter()
                    .find(|s| !wanted.is_empty() && s.eq_ignore_ascii_case(wanted))
                    .or_else(|| f.styles.iter().find(|s| s.eq_ignore_ascii_case("regular")))
                    .or_else(|| f.styles.first())
            })
            .cloned()
            .unwrap_or_else(|| "Regular".to_string());
        FontSpec {
            family: self.text_family.clone(),
            style,
            file: None,
        }
    }

    fn new_text_spec(&self, text: String, at: Point) -> TextSpec {
        TextSpec {
            text,
            font: self.text_font(),
            size: self.text_size,
            x: at.x,
            y: at.y,
            alignment: self.text_style.align,
            letter_spacing: letter_spacing_for(self.text_style.char_pct),
            line_spacing: self.text_style.line_pct,
            kerning: self.text_style.kerning,
            vertical: self.text_style.vertical,
            weld: false,
            flow: stencilcraft_doc::TextFlow::Straight,
            letters: Vec::new(),
            transform: stencilcraft_doc::IDENTITY_TRANSFORM,
        }
    }

    fn text_spec_of(&self, id: u64) -> Option<TextSpec> {
        self.engine
            .doc
            .get_shape(id)
            .and_then(|shape| shape.text.as_deref().cloned())
    }

    /// The text being typed ("" before the first letter).
    pub fn session_text(&self) -> String {
        self.text_session
            .as_ref()
            .and_then(|s| s.id)
            .and_then(|id| self.text_spec_of(id))
            .map(|spec| spec.text)
            .unwrap_or_default()
    }

    /// Start typing new text with its first baseline starting at `at`.
    pub fn begin_text_at(&mut self, at: Point) {
        self.end_text_session();
        self.ensure_fonts();
        self.text_session = Some(TextSession {
            id: None,
            anchor: at,
            caret: 0,
        });
        self.set_selection([]);
        self.active_tool = ActiveTool::Text;
        self.pending_panel = Some(WorkspacePanel::TextStyle);
        self.status = "type your text; Esc when done".to_string();
    }

    /// Edit the text shape `id` on the canvas, the caret at `caret` (or its end).
    pub fn begin_text_edit(&mut self, id: u64, caret: Option<usize>) {
        let Some(spec) = self.text_spec_of(id) else {
            self.status = "select a live text object to edit".to_string();
            return;
        };
        self.end_text_session();
        self.ensure_fonts();
        let caret = clamp_boundary(&spec.text, caret.unwrap_or(spec.text.len()));
        self.text_session = Some(TextSession {
            id: Some(id),
            anchor: Point::new(spec.x, spec.y),
            caret,
        });
        self.load_text_style(&spec);
        self.active_tool = ActiveTool::Text;
        // The text box and caret show what is being edited; a selection highlight would hide it.
        self.set_selection([]);
        self.pending_panel = Some(WorkspacePanel::TextStyle);
        self.status = "editing text; Esc when done".to_string();
    }

    /// Stop typing. Nothing stays selected, as in Silhouette.
    pub fn end_text_session(&mut self) {
        if self.text_session.take().is_some() {
            self.set_selection([]);
        }
    }

    /// Put `family` first in the recently used fonts (at most six).
    fn note_recent_font(&mut self, family: &str) {
        if family.is_empty() || self.prefs.recent_fonts.first().is_some_and(|f| f == family) {
            return;
        }
        self.prefs.recent_fonts.retain(|f| f != family);
        self.prefs.recent_fonts.insert(0, family.to_string());
        self.prefs.recent_fonts.truncate(6);
        self.save_prefs();
    }

    fn load_text_style(&mut self, spec: &TextSpec) {
        self.text_family = spec.font.family.clone();
        self.text_style.font_style = spec.font.style.clone();
        self.text_size = spec.size;
        self.text_style.align = spec.alignment;
        self.text_style.char_pct = char_spacing_pct(spec.letter_spacing);
        self.text_style.line_pct = spec.line_spacing;
        self.text_style.kerning = spec.kerning;
        self.text_style.vertical = spec.vertical;
    }

    /// Make the session's text `text` with the caret at `caret`: the first letter creates the
    /// shape, later changes edit it, and emptying it removes it. One undo step each.
    fn set_session_text(&mut self, text: String, caret: usize) -> bool {
        let Some(session) = self.text_session.clone() else {
            return false;
        };
        let result = match session.id {
            None if text.is_empty() => Ok(None),
            None => {
                let spec = self.new_text_spec(text.clone(), session.anchor);
                match self
                    .engine
                    .run(Command::TextAdd(TextAdd { spec, style: None }))
                {
                    Ok(Effect::ShapeAdded { id }) => Ok(Some(id)),
                    Ok(_) => Ok(None),
                    Err(e) => Err(e),
                }
            }
            Some(id) if text.is_empty() => {
                self.engine.run(Command::ShapeRemove { id }).map(|_| None)
            }
            Some(id) => self
                .engine
                .run(Command::TextEdit(TextEdit {
                    id,
                    text: Some(text.clone()),
                    ..Default::default()
                }))
                .map(|_| Some(id)),
        };
        match result {
            Ok(id) => {
                if let Some(s) = self.text_session.as_mut() {
                    s.id = id;
                    s.caret = clamp_boundary(&text, caret);
                }
                true
            }
            Err(e) => {
                self.status = e.to_string();
                false
            }
        }
    }

    /// Type `s` at the caret.
    pub fn text_insert(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        let Some(caret) = self.text_session.as_ref().map(|t| t.caret) else {
            return;
        };
        let mut text = self.session_text();
        let caret = clamp_boundary(&text, caret);
        text.insert_str(caret, s);
        self.set_session_text(text, caret.saturating_add(s.len()));
    }

    /// Backspace (`forward` false) or Delete (`forward` true) one character.
    pub fn text_erase(&mut self, forward: bool) {
        let Some(caret) = self.text_session.as_ref().map(|t| t.caret) else {
            return;
        };
        let mut text = self.session_text();
        let caret = clamp_boundary(&text, caret);
        let (from, to) = if forward {
            (caret, next_boundary(&text, caret))
        } else {
            (prev_boundary(&text, caret), caret)
        };
        if from >= to {
            return;
        }
        text.replace_range(from..to, "");
        self.set_session_text(text, from);
    }

    fn move_caret(&mut self, to: impl FnOnce(&str, usize) -> usize) {
        let text = self.session_text();
        if let Some(session) = self.text_session.as_mut() {
            let caret = clamp_boundary(&text, session.caret);
            session.caret = clamp_boundary(&text, to(&text, caret));
        }
    }

    /// Undo or redo while typing: the caret stays in the text; undoing its first letter ends
    /// the shape but keeps typing at the same place.
    fn text_session_history(&mut self, redo: bool) {
        let done = if redo {
            self.engine.redo()
        } else {
            self.engine.undo()
        };
        if !done {
            return;
        }
        let gone = self
            .text_session
            .as_ref()
            .and_then(|s| s.id)
            .is_some_and(|id| self.text_spec_of(id).is_none());
        if gone && let Some(s) = self.text_session.as_mut() {
            s.id = None;
        }
        let text = self.session_text();
        if let Some(s) = self.text_session.as_mut() {
            s.caret = clamp_boundary(&text, s.caret);
        }
        self.retain_valid_selection();
    }

    /// While typing, the keyboard belongs to the text: letters, Backspace/Delete, arrows,
    /// Home/End, Enter and paste edit it, Esc ends it, Cmd+Z/Cmd+Shift+Z undo and redo.
    /// Plain keys never reach the tool shortcuts. Returns whether a key was used.
    pub fn handle_text_session_keys(&mut self, ctx: &egui::Context) -> bool {
        if self.text_session.is_none() || ctx.text_edit_focused() {
            return false;
        }
        let taken: Vec<egui::Event> = ctx.input_mut(|input| {
            let mut taken = Vec::new();
            input.events.retain(|event| {
                let take = match event {
                    egui::Event::Text(_)
                    | egui::Event::Paste(_)
                    | egui::Event::Copy
                    | egui::Event::Cut => true,
                    egui::Event::Key { key, modifiers, .. } => {
                        !modifiers.command || matches!(key, egui::Key::Z | egui::Key::Y)
                    }
                    _ => false,
                };
                if take {
                    taken.push(event.clone());
                }
                !take
            });
            taken
        });
        let used = !taken.is_empty();
        for event in taken {
            if self.text_session.is_none() {
                break;
            }
            match event {
                egui::Event::Text(s) => self.text_insert(&typed(&s)),
                egui::Event::Paste(s) => self.text_insert(&pasted(&s)),
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => match key {
                    egui::Key::Z if modifiers.command => self.text_session_history(modifiers.shift),
                    egui::Key::Y if modifiers.command => self.text_session_history(true),
                    egui::Key::Backspace => self.text_erase(false),
                    egui::Key::Delete => self.text_erase(true),
                    egui::Key::ArrowLeft => self.move_caret(prev_boundary),
                    egui::Key::ArrowRight => self.move_caret(next_boundary),
                    egui::Key::Home => self.move_caret(line_start),
                    egui::Key::End => self.move_caret(line_end),
                    egui::Key::Enter => self.text_insert("\n"),
                    egui::Key::Escape => self.finish_typing(),
                    _ => {}
                },
                _ => {}
            }
        }
        // The canvas holds no egui focus while typing, so Escape arrives as the window's Cancel.
        if self.text_session.is_some() && crate::framework::take_window_cancel(ctx) {
            self.finish_typing();
            return true;
        }
        used
    }

    /// Esc while typing: ends the session and returns to the Select tool.
    fn finish_typing(&mut self) {
        self.end_text_session();
        self.active_tool = ActiveTool::Select;
        self.status = "text done".to_string();
    }

    /// The topmost live text under `p` (millimetres) and the caret nearest the click.
    fn text_hit(&self, p: Point) -> Option<(u64, usize)> {
        let pad = 1.0;
        self.engine
            .doc
            .layers
            .iter()
            .rev()
            .filter(|layer| layer.visible && !layer.locked)
            .flat_map(|layer| layer.shapes.iter().rev())
            .filter(|shape| stencilcraft_ui_egui::is_selectable(shape))
            .find_map(|shape| {
                let spec = shape.text.as_deref()?;
                let b = shape.path.bounding_box().inflate(pad, pad);
                b.contains(p).then(|| (shape.id, nearest_caret(spec, p)))
            })
    }

    /// The canvas part of the Text tool: clicks place, re-enter or end text, and the text box
    /// and caret are painted. Returns true while the Text tool owns the pointer.
    pub fn text_canvas(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        painter: &egui::Painter,
    ) -> bool {
        if self.text_session.is_some() && self.active_tool != ActiveTool::Text {
            self.end_text_session();
        }
        if self.active_tool != ActiveTool::Text {
            return false;
        }
        let view = self.view;
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
        }
        if response.clicked()
            && let Some(pos) = response.interact_pointer_pos()
        {
            let p = view.screen_to_world(pos.to_vec2());
            let inside = self
                .text_session
                .as_ref()
                .and_then(|s| s.id)
                .and_then(|id| self.engine.doc.get_shape(id))
                .is_some_and(|shape| shape.path.bounding_box().inflate(1.0, 1.0).contains(p));
            if inside {
                let spec = self
                    .text_session
                    .as_ref()
                    .and_then(|s| s.id)
                    .and_then(|id| self.text_spec_of(id));
                if let (Some(spec), Some(session)) = (spec, self.text_session.as_mut()) {
                    session.caret = nearest_caret(&spec, p);
                }
            } else {
                let had = self.text_session.is_some();
                self.end_text_session();
                if let Some((id, caret)) = self.text_hit(p) {
                    self.begin_text_edit(id, Some(caret));
                } else if !had {
                    self.begin_text_at(p);
                }
            }
        }
        self.paint_text_session(painter);
        true
    }

    fn paint_text_session(&self, painter: &egui::Painter) {
        let Some(session) = &self.text_session else {
            return;
        };
        let view = self.view;
        let em = self.text_size / POINTS_PER_MM;
        let spec = session.id.and_then(|id| self.text_spec_of(id));
        let bounds = session
            .id
            .and_then(|id| self.engine.doc.get_shape(id))
            .map(|shape| shape.path.bounding_box())
            .filter(|b| b.width().is_finite() && b.height().is_finite());
        let (x0, y0, x1, y1) = match bounds {
            Some(b) => (b.x0 - 1.0, b.y0 - 1.0, b.x1 + 1.0, b.y1 + 1.0),
            None => (
                session.anchor.x - 1.0,
                session.anchor.y - 0.8 * em,
                session.anchor.x + 1.0 + 0.1 * em,
                session.anchor.y + 0.25 * em,
            ),
        };
        stencilcraft_ui_egui::paint_preview_paths(
            painter,
            &view,
            &[rect_path(x0, y0, x1, y1)],
            BOX_COLOUR,
        );
        let caret = match &spec {
            Some(spec) => stencilcraft_engine::text::carets(spec)
                .ok()
                .and_then(|carets| carets.into_iter().find(|c| c.byte == session.caret))
                .map(|c| (c.base, c.top)),
            None => Some((
                session.anchor,
                Point::new(session.anchor.x, session.anchor.y - 0.75 * em),
            )),
        };
        if let Some((base, top)) = caret {
            let (a, b) = (view.world_to_screen(base), view.world_to_screen(top));
            painter.line_segment([a, b], egui::Stroke::new(2.0, CARET_COLOUR));
        }
    }

    /// The text the Text Style panel acts on: what is being typed, else the selected text.
    fn text_style_targets(&self) -> Vec<u64> {
        match self.text_session.as_ref().and_then(|s| s.id) {
            Some(id) => vec![id],
            None if self.text_session.is_some() => Vec::new(),
            None => self.selected_text_ids().collect(),
        }
    }

    /// Apply a Text Style change to the targeted text, one undo step per text.
    fn apply_text_style(&mut self, edit: TextEdit) {
        for id in self.text_style_targets() {
            if let Err(e) = self
                .engine
                .run(Command::TextEdit(TextEdit { id, ..edit.clone() }))
            {
                self.status = e.to_string();
            }
        }
    }

    pub fn text_style_panel(&mut self, ui: &mut egui::Ui) {
        self.ensure_fonts();
        if let Some(spec) = self
            .text_style_targets()
            .first()
            .and_then(|id| self.text_spec_of(*id))
        {
            self.load_text_style(&spec);
        }
        let numbers = self.locale_numbers.clone();
        let before_family = self.text_family.clone();
        let mut size = self.text_size;
        let mut style = self.text_style.clone();
        let mut family = self.text_family.clone();
        let query = style.query.to_lowercase();
        let matching: Vec<String> = self
            .text_fonts
            .iter()
            .map(|f| f.family.clone())
            .filter(|name| query.is_empty() || name.to_lowercase().contains(&query))
            .collect();
        let targets = self.text_style_targets().len();
        let recent = self.prefs.recent_fonts.clone();
        let styles: Vec<String> = self
            .text_fonts
            .iter()
            .find(|f| f.family == self.text_family)
            .map(|f| f.styles.clone())
            .unwrap_or_default();
        let before_style = self.text_font().style;
        let mut font_style = before_style.clone();
        let before_flow = self
            .text_style_targets()
            .first()
            .and_then(|id| self.text_spec_of(*id))
            .map(|spec| spec.flow);
        let mut flow = before_flow.clone();
        let mut family_entries: Vec<Entry<String>> = Vec::new();
        if !matching.contains(&before_family) {
            family_entries.push(Entry::new(before_family.clone(), before_family.clone()));
        }
        family_entries.extend(
            matching
                .iter()
                .map(|name| Entry::new(name.clone(), name.clone())),
        );
        let style_entries: Vec<Entry<String>> = styles
            .iter()
            .map(|s| Entry::new(s.clone(), s.clone()))
            .collect();
        let align_entries = [
            Entry::new(TextAlign::Left, "Left"),
            Entry::new(TextAlign::Centre, "Centre"),
            Entry::new(TextAlign::Right, "Right"),
            Entry::new(TextAlign::Justify, "Justify"),
        ];
        let direction_entries = [
            Entry::new(false, "Horizontal"),
            Entry::new(true, "Vertical"),
        ];
        let _ = Panel::new("Text Style")
            .id_salt("stencilcraft-text-style-panel")
            .show(ui, |ui, _| {
                // The field writes the query as it is typed.
                let _ = TextField::new("Find a font", Bind::bound(&mut style.query))
                    .id_salt("text-font-search")
                    .limit(200)
                    .show(ui);
                if !recent.is_empty() {
                    ui.label("Recently used");
                    ui.horizontal_wrapped(|ui| {
                        for name in &recent {
                            if button(ui, name, true) {
                                family = name.clone();
                            }
                        }
                    });
                }
                let _ = Select::new("Font", Bind::bound(&mut family), &family_entries)
                    .id_salt("text-font")
                    .show(ui);
                if styles.len() > 1 {
                    labelled(ui, "Style", |ui| {
                        let _ =
                            Select::new("Font style", Bind::bound(&mut font_style), &style_entries)
                                .id_salt("text-font-style")
                                .show(ui);
                    });
                }
                labelled(ui, "Size", |ui| {
                    number(
                        ui,
                        "text-size",
                        "Text size",
                        &mut size,
                        1.0..=2000.0,
                        "{n} pt",
                        &numbers,
                    );
                });
                whole_width(ui, "text-align-strip", |ui| {
                    let _ = Segmented::new(
                        "Justification",
                        Bind::bound(&mut style.align),
                        &align_entries,
                    )
                    .id_salt("text-align")
                    .size(ControlSize::Sm)
                    .show(ui);
                });
                let _ = Section::new("Spacing")
                    .id_salt("text-spacing")
                    .show(ui, |ui, _| {
                        labelled(ui, "Line Spacing", |ui| {
                            number(
                                ui,
                                "text-line-spacing",
                                "Line spacing",
                                &mut style.line_pct,
                                10.0..=1000.0,
                                "{n} %",
                                &numbers,
                            );
                        });
                        labelled(ui, "Character Spacing", |ui| {
                            number(
                                ui,
                                "text-char-spacing",
                                "Character spacing",
                                &mut style.char_pct,
                                0.0..=1000.0,
                                "{n} %",
                                &numbers,
                            );
                        });
                        let _ = Checkbox::new(Bind::bound(&mut style.kerning))
                            .label("Kerning")
                            .show(ui);
                        whole_width(ui, "text-direction-strip", |ui| {
                            let _ = Segmented::new(
                                "Direction",
                                Bind::bound(&mut style.vertical),
                                &direction_entries,
                            )
                            .id_salt("text-direction")
                            .size(ControlSize::Sm)
                            .show(ui);
                        });
                    });
                if let Some(flow) = flow.as_mut() {
                    let _ = Section::new("Flow").id_salt("text-flow").show(ui, |ui, _| {
                        flow_controls(ui, flow, &numbers);
                    });
                }
                hint(
                    ui,
                    if targets > 0 {
                        "Changes apply to the text being typed or selected."
                    } else {
                        "Changes apply to new text: choose the Text tool and click the page."
                    },
                );
            });
        let mut edit = TextEdit::default();
        let mut changed = false;
        if family != before_family {
            self.text_family = family.clone();
            self.text_style.font_style.clear();
            style.font_style.clear();
            edit.font = Some(self.text_font());
            changed = true;
            self.note_recent_font(&family);
        } else if font_style != before_style {
            self.text_style.font_style = font_style.clone();
            style.font_style = font_style;
            edit.font = Some(self.text_font());
            changed = true;
        }
        if size != self.text_size && size.is_finite() {
            self.text_size = size;
            edit.size = Some(size);
            changed = true;
        }
        if style.align != self.text_style.align {
            edit.alignment = Some(style.align);
            changed = true;
        }
        if style.line_pct != self.text_style.line_pct && style.line_pct.is_finite() {
            edit.line_spacing = Some(style.line_pct);
            changed = true;
        }
        if style.char_pct != self.text_style.char_pct && style.char_pct.is_finite() {
            edit.letter_spacing = Some(letter_spacing_for(style.char_pct));
            changed = true;
        }
        if style.kerning != self.text_style.kerning {
            edit.kerning = Some(style.kerning);
            changed = true;
        }
        if style.vertical != self.text_style.vertical {
            edit.vertical = Some(style.vertical);
            changed = true;
        }
        if flow != before_flow
            && let Some(flow) = flow
        {
            edit.flow = Some(flow);
            changed = true;
        }
        self.text_style = style;
        if changed {
            self.apply_text_style(edit);
        }
    }

    /// Type › Type on a Path: the selected text follows the other selected shape's outline.
    pub fn type_on_path(&mut self) {
        let text = self.selected_text_ids().next();
        let path = self
            .selected_shapes
            .iter()
            .copied()
            .find(|id| Some(*id) != text && self.text_spec_of(*id).is_none());
        let (Some(text), Some(path)) = (text, path) else {
            self.status = "select a text and the shape it should follow".to_string();
            return;
        };
        let edit = TextEdit {
            id: text,
            flow: Some(stencilcraft_doc::TextFlow::OnPath {
                path: BezPath::new(),
                shape: Some(path),
                start: 0.0,
                offset_mm: 0.0,
                flip: false,
            }),
            ..Default::default()
        };
        match self.engine.run(Command::TextEdit(edit)) {
            Ok(_) => {
                self.set_selection([text]);
                self.pending_panel = Some(WorkspacePanel::TextStyle);
                self.status = "text on path".to_string();
            }
            Err(e) => self.status = e.to_string(),
        }
    }

    /// Type › Curve Text: bend the selected text into an arc about as wide as the text.
    pub fn curve_text(&mut self) {
        let ids: Vec<u64> = self.selected_text_ids().collect();
        if ids.is_empty() {
            self.status = "select text to curve".to_string();
            return;
        }
        for id in ids {
            let width = self
                .engine
                .doc
                .get_shape(id)
                .map_or(50.0, |s| s.path.bounding_box().width());
            let diameter = (width * 1.2).max(10.0);
            if let Err(e) = self.engine.run(Command::TextEdit(TextEdit {
                id,
                flow: Some(stencilcraft_doc::TextFlow::Curve {
                    diameter_mm: diameter,
                }),
                ..Default::default()
            })) {
                self.status = e.to_string();
            }
        }
        self.pending_panel = Some(WorkspacePanel::TextStyle);
    }

    /// Insert `glyph` (Type › Glyphs): into the text being typed, else at the end of the
    /// selected text.
    pub fn insert_glyph(&mut self, glyph: &str) {
        if self.text_session.is_some() {
            self.text_insert(glyph);
            return;
        }
        let Some(id) = self.selected_text_ids().next() else {
            self.status = "click the page with the Text tool, or select text, first".to_string();
            return;
        };
        let Some(mut text) = self.text_spec_of(id).map(|s| s.text) else {
            return;
        };
        text.push_str(glyph);
        if let Err(e) = self.engine.run(Command::TextEdit(TextEdit {
            id,
            text: Some(text),
            ..Default::default()
        })) {
            self.status = e.to_string();
        }
    }

    /// The Glyphs panel: characters that are hard to type, a click inserts one.
    pub fn glyphs_panel(&mut self, ui: &mut egui::Ui) {
        let mut picked = None;
        let _ = Panel::new("Glyphs")
            .id_salt("stencilcraft-glyphs-panel")
            .show(ui, |ui, _| {
                for (title, glyphs) in GLYPH_SETS {
                    ui.label(*title);
                    ui.horizontal_wrapped(|ui| {
                        for glyph in glyphs.chars() {
                            let s = glyph.to_string();
                            if craft_ui::buttons::Button::new(s.clone())
                                .size(ControlSize::Sm)
                                .show(ui)
                                .clicked()
                            {
                                picked = Some(s);
                            }
                        }
                    });
                }
                hint(
                    ui,
                    "A click inserts the character into the text being typed or selected.",
                );
            });
        if let Some(glyph) = picked {
            self.insert_glyph(&glyph);
        }
    }

    /// Put the caret of the text being typed nearest `p` (millimetres).
    pub fn place_text_caret(&mut self, p: Point) {
        let spec = self
            .text_session
            .as_ref()
            .and_then(|s| s.id)
            .and_then(|id| self.text_spec_of(id));
        if let (Some(spec), Some(session)) = (spec, self.text_session.as_mut()) {
            session.caret = nearest_caret(&spec, p);
        }
    }

    /// Type › Edit Selected Text: edit the selected text on the canvas.
    pub fn edit_selected_text(&mut self) {
        let first = self.selected_text_ids().next();
        match first {
            Some(id) => self.begin_text_edit(id, None),
            None => self.status = "select a live text object to edit".to_string(),
        }
    }
}

/// The Glyphs panel's characters, by group.
const GLYPH_SETS: &[(&str, &str)] = &[
    ("Punctuation", "¡¿«»‹›‘’‚“”„–—…•·§¶†‡"),
    ("Symbols", "©®™°±×÷µ¢£¥€¤ªº¹²³¼½¾"),
    (
        "Accented letters",
        "ÀÁÂÃÄÅÆÇÈÉÊËÌÍÎÏÑÒÓÔÕÖØÙÚÛÜÝßàáâãäåæçèéêëìíîïñòóôõöøùúûüýÿ",
    ),
    ("Shapes and arrows", "←↑→↓↔★☆♥♦♣♠✓✗♪♫☀☂☺"),
];

/// The Flow section: straight, curved (diameter; negative dips) or on a path (start,
/// offset, flip).
fn flow_controls(
    ui: &mut egui::Ui,
    flow: &mut stencilcraft_doc::TextFlow,
    numbers: &crate::panels::LocaleNumbers,
) {
    use stencilcraft_doc::TextFlow;
    let mut kind = match flow {
        TextFlow::Straight => 0u8,
        TextFlow::Curve { .. } => 1,
        TextFlow::OnPath { .. } => 2,
    };
    let was = kind;
    let mut entries = vec![Entry::new(0u8, "Straight"), Entry::new(1u8, "Curve")];
    if was == 2 {
        entries.push(Entry::new(2u8, "On Path"));
    }
    let _ = Segmented::new("Flow", Bind::bound(&mut kind), &entries)
        .id_salt("text-flow-kind")
        .size(ControlSize::Sm)
        .show(ui);
    if kind != was {
        *flow = match kind {
            1 => TextFlow::Curve { diameter_mm: 100.0 },
            _ => TextFlow::Straight,
        };
    }
    match flow {
        TextFlow::Straight => {}
        TextFlow::Curve { diameter_mm } => {
            labelled(ui, "Diameter", |ui| {
                number(
                    ui,
                    "text-curve-diameter",
                    "Curve diameter",
                    diameter_mm,
                    -10_000.0..=10_000.0,
                    "{n} mm",
                    numbers,
                );
            });
            hint(ui, "A negative diameter bends the text downwards.");
        }
        TextFlow::OnPath {
            start,
            offset_mm,
            flip,
            ..
        } => {
            let mut pct = *start * 100.0;
            labelled(ui, "Start", |ui| {
                number(
                    ui,
                    "text-path-start",
                    "Start along the path",
                    &mut pct,
                    0.0..=100.0,
                    "{n} %",
                    numbers,
                );
            });
            *start = pct / 100.0;
            labelled(ui, "Offset", |ui| {
                number(
                    ui,
                    "text-path-offset",
                    "Offset from the path",
                    offset_mm,
                    -1000.0..=1000.0,
                    "{n} mm",
                    numbers,
                );
            });
            let _ = Checkbox::new(Bind::bound(flip))
                .label("Flip to the other side")
                .show(ui);
        }
    }
}

/// Shows `add` (a segmented strip) at its whole width, its labels in full as before the pin:
/// the pin narrows a control wider than the room its row leaves, eliding the segments
/// ("Ce…", "Horizo…"). In a horizontal scroll area the room is unbounded (egui gives its
/// content no width limit along a scrolling axis), so the strip keeps its natural width; the
/// area is as wide as the strip where it fits and scrolls where the panel is narrower.
fn whole_width(ui: &mut egui::Ui, salt: &str, add: impl FnOnce(&mut egui::Ui)) {
    egui::ScrollArea::horizontal()
        .id_salt(salt)
        .show(ui, add);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn character_spacing_maps_to_silhouettes_scale() {
        assert_eq!(letter_spacing_for(100.0), 0.0);
        assert!((letter_spacing_for(200.0) - 36.6).abs() < 1e-9);
        assert!((char_spacing_pct(letter_spacing_for(250.0)) - 250.0).abs() < 1e-9);
    }

    #[test]
    fn caret_boundaries_respect_characters_and_lines() {
        let text = "aé\nxyz";
        assert_eq!(next_boundary(text, 1), 3, "é is two bytes");
        assert_eq!(prev_boundary(text, 3), 1);
        assert_eq!(prev_boundary(text, 0), 0);
        assert_eq!(next_boundary(text, text.len()), text.len());
        assert_eq!(clamp_boundary(text, 2), 1);
        assert_eq!(clamp_boundary(text, 99), text.len());
        assert_eq!(line_start(text, 6), 4);
        assert_eq!(line_end(text, 0), 3);
        assert_eq!(typed("a\u{7}b\n"), "ab");
        assert_eq!(pasted("a\r\nb\u{7}"), "a\nb");
    }
}

#[cfg(test)]
mod flow_tests {
    use super::*;

    #[test]
    fn vertical_text_stacks_the_letters() {
        let mut app = StencilApp::new(None);
        app.text_family = stencilcraft_engine::text::SINGLE_LINE_FAMILY.to_string();
        app.begin_text_at(Point::new(10.0, 40.0));
        app.text_insert("ABC");
        let text = app.text_session.as_ref().and_then(|s| s.id).expect("text");
        app.end_text_session();
        app.set_selection([text]);
        let wide = app
            .engine
            .doc
            .get_shape(text)
            .expect("text")
            .path
            .bounding_box();
        app.apply_text_style(TextEdit {
            vertical: Some(true),
            ..TextEdit::default()
        });
        let shape = app.engine.doc.get_shape(text).expect("text");
        assert!(shape.text.as_ref().is_some_and(|s| s.vertical));
        let tall = shape.path.bounding_box();
        assert!(
            tall.height() > tall.width() && wide.width() > wide.height(),
            "{wide:?} → {tall:?}"
        );
    }

    #[test]
    fn type_on_path_curve_and_glyphs() {
        let mut app = StencilApp::new(None);
        app.text_family = stencilcraft_engine::text::SINGLE_LINE_FAMILY.to_string();
        app.begin_text_at(Point::new(10.0, 40.0));
        app.text_insert("Path");
        let text = app.text_session.as_ref().and_then(|s| s.id).expect("text");
        app.insert_glyph("★");
        assert_eq!(app.session_text(), "Path★");
        app.end_text_session();
        app.run_command(Command::ShapeAddEllipse {
            x: 0.0,
            y: 60.0,
            width: 80.0,
            height: 50.0,
        });
        let ellipse = app
            .engine
            .doc
            .shapes()
            .map(|(_, s)| s.id)
            .find(|id| *id != text)
            .expect("ellipse");
        app.set_selection([text, ellipse]);
        app.type_on_path();
        let flow = app.text_spec_of(text).map(|s| s.flow);
        assert!(
            matches!(flow, Some(stencilcraft_doc::TextFlow::OnPath { .. })),
            "{flow:?} {}",
            app.status
        );
        app.set_selection([text]);
        app.curve_text();
        assert!(matches!(
            app.text_spec_of(text).map(|s| s.flow),
            Some(stencilcraft_doc::TextFlow::Curve { .. })
        ));
        app.insert_glyph("!");
        assert_eq!(
            app.text_spec_of(text).map(|s| s.text),
            Some("Path★!".to_string())
        );
    }
}

#[cfg(test)]
mod style_tests {
    use super::*;

    #[test]
    fn font_style_and_recent_fonts() {
        let mut app = StencilApp::new(None);
        app.text_fonts = vec![stencilcraft_engine::text::FontFamily {
            family: "Demo".into(),
            styles: vec!["Regular".into(), "Bold".into(), "Italic".into()],
            single_line: false,
        }];
        app.text_family = "Demo".into();
        assert_eq!(app.text_font().style, "Regular");
        app.text_style.font_style = "bold".into();
        assert_eq!(app.text_font().style, "Bold", "matched without case");
        app.text_style.font_style = "Heavy".into();
        assert_eq!(
            app.text_font().style,
            "Regular",
            "a missing style falls back"
        );
        for name in ["A", "B", "A", "C", "D", "E", "F", "G"] {
            app.note_recent_font(name);
        }
        assert_eq!(app.prefs.recent_fonts, ["G", "F", "E", "D", "C", "A"]);
    }
}
