//! Silhouette Studio's Text tool, typed on the canvas: click the page and type. The click
//! point is the first line's baseline start; letters appear as you type, each keystroke one
//! undo step; Backspace, Delete, the arrows, Home/End and Enter edit as in a text box; a click
//! inside the text moves the caret; Esc (switching to the Select tool) or a click elsewhere
//! ends editing. Double-clicking text with the Select tool edits it again. The Text Style panel
//! sets the font, size, alignment, line and character spacing and kerning of the text being
//! typed or the selected text, and of new text.

use craft_ui::composites::SectionHeader;
use craft_ui::controls::{Checkbox, ControlSize, Segmented, Select, TextField};
use craft_ui::edit::EditEvent;
use craft_ui::patterns::Panel;
use eframe::egui;
use stencilcraft_doc::{FontSpec, TextAlign, TextSpec};
use stencilcraft_engine::text::{TextAdd, TextEdit};
use stencilcraft_engine::{Command, Effect};
use stencilcraft_geom::{BezPath, Point, Shape as _};

use crate::panels::{hint, labelled, number};
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
    /// The Text Style panel's font search.
    pub query: String,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            align: TextAlign::Left,
            char_pct: 100.0,
            line_pct: 100.0,
            kerning: true,
            query: String::new(),
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
        let style = self
            .text_fonts
            .iter()
            .find(|f| f.family == self.text_family)
            .and_then(|f| {
                f.styles
                    .iter()
                    .find(|s| s.eq_ignore_ascii_case("regular"))
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
            vertical: false,
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

    fn load_text_style(&mut self, spec: &TextSpec) {
        self.text_family = spec.font.family.clone();
        self.text_size = spec.size;
        self.text_style.align = spec.alignment;
        self.text_style.char_pct = char_spacing_pct(spec.letter_spacing);
        self.text_style.line_pct = spec.line_spacing;
        self.text_style.kerning = spec.kerning;
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
                    egui::Key::Escape => {
                        self.end_text_session();
                        self.active_tool = ActiveTool::Select;
                        self.status = "text done".to_string();
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        used
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
        let _ = Panel::new("stencilcraft-text-style-panel").show(ui, |ui| {
            let edit = TextField::new("text-font-search", "Find a font", style.query.as_str())
                .char_limit(200)
                .show(ui);
            if let Some(
                EditEvent::Update { value, .. }
                | EditEvent::Commit { value, .. }
                | EditEvent::Set { value, .. },
            ) = edit.event
            {
                style.query = value;
            }
            let mut select = Select::new("text-font", "Font", &mut family);
            if !matching.contains(&before_family) {
                select = select.option(before_family.clone(), &before_family);
            }
            for name in &matching {
                select = select.option(name.clone(), name);
            }
            let _ = select.show(ui);
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
            let _ = Segmented::new("text-align", "Justification", &mut style.align)
                .item(TextAlign::Left, "Left")
                .item(TextAlign::Centre, "Centre")
                .item(TextAlign::Right, "Right")
                .item(TextAlign::Justify, "Justify")
                .size(ControlSize::Sm)
                .show(ui);
            let _ = SectionHeader::new("text-spacing", "Spacing").show(ui, |ui| {
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
                let _ = Checkbox::bool("Kerning", &mut style.kerning).show(ui);
            });
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
            self.text_family = family;
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
        self.text_style = style;
        if changed {
            self.apply_text_style(edit);
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
