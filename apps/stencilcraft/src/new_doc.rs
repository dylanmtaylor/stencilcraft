//! File › New: the New Document dialog. Category tabs (Recent, Cut Mats, Vinyl, Paper &
//! Card, Print & Cut, Laser) over a grid of preset cards drawn in code, and a Preset Details
//! column (name, width, height, units, orientation, mat, machine, kerf) with Create and Close.
//! The layout follows the suite's New Document (craft-ui `NewDocumentDialog`: same card and
//! width tokens); that dialog's details are raster settings (ppi, colour mode, depth), so this
//! one keeps the shape and swaps the fields for cutting ones. Create runs `document.new` and
//! `kerf.set` through the engine; mat and machine are app state, as on the Send tab.

use craft_theme::TypeRole;
use craft_ui::behavior::GridNav;
use craft_ui::composites::SectionLabel;
use craft_ui::controls::{NumberField, Segmented, Select, TextField};
use craft_ui::dialog::{Dialog, DialogState, Footer, Outcome, Width};
use craft_ui::edit::EditEvent;
use craft_ui::icons::Icon;
use craft_ui::localized::{LocaleNumbers, Unit};
use craft_ui::primitives::{Content, Corner, Divider, Gap, Interactive, Pad, SurfaceKind, Text};
use craft_ui::theme::{ToEgui as _, Tokens, TokensOf as _};
use eframe::egui::{
    self, Align, Color32, Id, Rect, ScrollArea, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2,
};
use serde::{Deserialize, Serialize};
use stencilcraft_engine::Command;
use stencilcraft_geom::{inches_to_mm, mm_to_inches};
use stencilcraft_machine::{Mat, Model};

/// The greatest board side the dialog creates, millimetres (a 10 m roll).
pub const MAX_SIDE_MM: f64 = 10_000.0;
/// Recent sizes remembered across runs.
pub const MAX_RECENT: usize = 8;
/// Greatest kerf the dialog sets, millimetres (the options bar's range).
const MAX_KERF_MM: f64 = 2.0;

/// The small drawing on a card, painted in code (no artwork files).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Thumb {
    Blank,
    /// A cutting mat: inch grid.
    Mat,
    /// Vinyl: a peeled corner.
    Vinyl,
    /// Heat-transfer: a mirrored "R" stand-in (two arrows facing apart).
    Transfer,
    /// A folding card: dashed centre score line.
    Card,
    /// A tag: punched hole near the top.
    Tag,
    /// Print & Cut: registration marks in three corners.
    PrintCut,
    /// A stencil: a cut-out window.
    Stencil,
    /// Laser sheet: a crosshair.
    Laser,
}

/// One built-in card.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub width_mm: f64,
    pub height_mm: f64,
    pub mat: Mat,
    pub thumb: Thumb,
    /// Kerf the preset sets (laser presets); `None` keeps the current kerf.
    pub kerf_mm: Option<f64>,
}

const fn inches(
    id: &'static str,
    name: &'static str,
    w: f64,
    h: f64,
    mat: Mat,
    thumb: Thumb,
) -> Preset {
    Preset {
        id,
        name,
        width_mm: inches_to_mm(w),
        height_mm: inches_to_mm(h),
        mat,
        thumb,
        kerf_mm: None,
    }
}

const fn mm(
    id: &'static str,
    name: &'static str,
    w: f64,
    h: f64,
    mat: Mat,
    thumb: Thumb,
) -> Preset {
    Preset {
        id,
        name,
        width_mm: w,
        height_mm: h,
        mat,
        thumb,
        kerf_mm: None,
    }
}

const fn laser(id: &'static str, name: &'static str, w: f64, h: f64, kerf: f64) -> Preset {
    Preset {
        kerf_mm: Some(kerf),
        ..mm(id, name, w, h, Mat::None, Thumb::Laser)
    }
}

/// A category tab and its cards.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Category {
    pub id: &'static str,
    pub title: &'static str,
    pub presets: &'static [Preset],
}

/// Built-in tabs after Recent. Mat sizes are the machine crate's `Mat` extents; the project
/// kinds follow the starters in docs/observations/silhouette-studio.md (our own names and sizes).
pub const CATEGORIES: &[Category] = &[
    Category {
        id: "mats",
        title: "Cut Mats",
        presets: &[
            inches(
                "mat-12x12",
                "12 × 12 in Mat",
                12.0,
                12.0,
                Mat::Cameo12x12,
                Thumb::Mat,
            ),
            inches(
                "mat-12x24",
                "12 × 24 in Mat",
                12.0,
                24.0,
                Mat::Cameo12x24,
                Thumb::Mat,
            ),
            inches(
                "mat-15x15",
                "15 × 15 in Plus Mat",
                15.0,
                15.0,
                Mat::CameoPlus15x15,
                Thumb::Mat,
            ),
            inches(
                "mat-24x24",
                "24 × 24 in Pro Mat",
                24.0,
                24.0,
                Mat::CameoPro24x24,
                Thumb::Mat,
            ),
            inches(
                "mat-8x12",
                "8 × 12 in Portrait Mat",
                8.0,
                12.0,
                Mat::Portrait8x12,
                Thumb::Mat,
            ),
            inches(
                "mat-8.5x12",
                "8.5 × 12 in Portrait Mat",
                8.5,
                12.0,
                Mat::Portrait8_5x12,
                Thumb::Mat,
            ),
        ],
    },
    Category {
        id: "vinyl",
        title: "Vinyl",
        presets: &[
            inches(
                "vinyl-sticker",
                "Vinyl Sticker",
                4.0,
                4.0,
                Mat::Cameo12x12,
                Thumb::Vinyl,
            ),
            inches(
                "vinyl-decal",
                "Wall Decal",
                12.0,
                12.0,
                Mat::Cameo12x12,
                Thumb::Vinyl,
            ),
            inches(
                "vinyl-roll",
                "Vinyl Roll Banner",
                12.0,
                48.0,
                Mat::None,
                Thumb::Vinyl,
            ),
            inches(
                "htv-shirt",
                "Heat Transfer T-Shirt",
                10.0,
                12.0,
                Mat::Cameo12x12,
                Thumb::Transfer,
            ),
            inches(
                "vinyl-stencil",
                "Stencil",
                8.5,
                11.0,
                Mat::Cameo12x12,
                Thumb::Stencil,
            ),
        ],
    },
    Category {
        id: "paper",
        title: "Paper & Card",
        presets: &[
            inches(
                "card-folded",
                "Folded Card",
                10.0,
                7.0,
                Mat::Cameo12x12,
                Thumb::Card,
            ),
            inches(
                "gift-tag",
                "Gift Tag",
                2.5,
                4.0,
                Mat::Cameo12x12,
                Thumb::Tag,
            ),
            inches("letter", "Letter", 8.5, 11.0, Mat::Cameo12x12, Thumb::Blank),
            mm("a4", "A4", 210.0, 297.0, Mat::Cameo12x12, Thumb::Blank),
            mm("a5", "A5", 148.0, 210.0, Mat::Portrait8_5x12, Thumb::Blank),
            inches(
                "cardstock-12",
                "Cardstock 12 × 12",
                12.0,
                12.0,
                Mat::Cameo12x12,
                Thumb::Blank,
            ),
        ],
    },
    Category {
        id: "print-cut",
        title: "Print & Cut",
        presets: &[
            inches(
                "pc-letter",
                "Sticker Sheet, Letter",
                8.5,
                11.0,
                Mat::Cameo12x12,
                Thumb::PrintCut,
            ),
            mm(
                "pc-a4",
                "Sticker Sheet, A4",
                210.0,
                297.0,
                Mat::Cameo12x12,
                Thumb::PrintCut,
            ),
            inches(
                "pc-labels",
                "Labels, Letter",
                8.5,
                11.0,
                Mat::Portrait8_5x12,
                Thumb::PrintCut,
            ),
        ],
    },
    Category {
        id: "laser",
        title: "Laser",
        presets: &[
            laser("laser-300x300", "Plywood 300 × 300", 300.0, 300.0, 0.15),
            laser("laser-400x300", "Acrylic 400 × 300", 400.0, 300.0, 0.10),
            laser("laser-600x400", "Bed 600 × 400", 600.0, 400.0, 0.15),
            laser("laser-a4", "Engraved Plaque A4", 297.0, 210.0, 0.10),
        ],
    },
];

/// Recent's fallback cards when nothing has been created yet.
const RECENT_DEFAULTS: &[Preset] = &[
    inches(
        "mat-12x12",
        "12 × 12 in Mat",
        12.0,
        12.0,
        Mat::Cameo12x12,
        Thumb::Mat,
    ),
    inches("letter", "Letter", 8.5, 11.0, Mat::Cameo12x12, Thumb::Blank),
    mm(
        "default",
        "Default 100 × 150 mm",
        100.0,
        150.0,
        Mat::CameoPro24x24,
        Thumb::Blank,
    ),
];

/// A size the user created, remembered in the settings file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecentDoc {
    pub name: String,
    pub width_mm: f64,
    pub height_mm: f64,
    pub mat: Mat,
}

/// The unit Width and Height show; the document is always millimetres.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Units {
    #[default]
    Millimetres,
    Inches,
}

impl Units {
    fn display_value(self, value: f64) -> f64 {
        match self {
            Units::Millimetres => value,
            Units::Inches => mm_to_inches(value),
        }
    }
    fn document_value(self, value: f64) -> f64 {
        match self {
            Units::Millimetres => value,
            Units::Inches => inches_to_mm(value),
        }
    }
    fn label(self) -> &'static str {
        match self {
            Units::Millimetres => "Millimetres",
            Units::Inches => "Inches",
        }
    }
    fn suffix(self) -> &'static str {
        match self {
            Units::Millimetres => "{n} mm",
            Units::Inches => "{n} in",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Orientation {
    Portrait,
    Landscape,
}

/// What a card sets.
#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub title: String,
    pub width_mm: f64,
    pub height_mm: f64,
    pub mat: Mat,
    pub thumb: Thumb,
    pub kerf_mm: Option<f64>,
}

impl From<&Preset> for Card {
    fn from(p: &Preset) -> Self {
        Card {
            title: p.name.to_string(),
            width_mm: p.width_mm,
            height_mm: p.height_mm,
            mat: p.mat,
            thumb: p.thumb,
            kerf_mm: p.kerf_mm,
        }
    }
}

/// The details column: everything Create makes the document from.
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    pub name: String,
    pub width_mm: f64,
    pub height_mm: f64,
    pub units: Units,
    pub mat: Mat,
    pub model: Model,
    pub kerf_mm: f64,
}

impl Default for Choice {
    fn default() -> Self {
        Choice {
            name: "Untitled-1".to_string(),
            width_mm: 100.0,
            height_mm: 150.0,
            units: Units::Millimetres,
            mat: Mat::CameoPro24x24,
            model: Model::Cameo4Pro,
            kerf_mm: 0.0,
        }
    }
}

impl Choice {
    pub fn orientation(&self) -> Orientation {
        if self.width_mm > self.height_mm {
            Orientation::Landscape
        } else {
            Orientation::Portrait
        }
    }

    /// Turns the page: swaps width and height when it stands the other way. A square stays.
    pub fn set_orientation(&mut self, orientation: Orientation) {
        if self.width_mm != self.height_mm && self.orientation() != orientation {
            std::mem::swap(&mut self.width_mm, &mut self.height_mm);
        }
    }

    fn side_ok(side: f64) -> bool {
        side.is_finite() && side > 0.0 && side <= MAX_SIDE_MM
    }

    /// Whether Create can run: finite positive sides within the cap, a sane kerf, a name.
    pub fn is_valid(&self) -> bool {
        Self::side_ok(self.width_mm)
            && Self::side_ok(self.height_mm)
            && self.kerf_mm.is_finite()
            && (0.0..=MAX_KERF_MM).contains(&self.kerf_mm)
            && !self.name.trim().is_empty()
    }

    /// The engine commands Create runs, in order.
    pub fn commands(&self) -> Vec<Command> {
        vec![
            Command::DocumentNew {
                width_mm: self.width_mm,
                height_mm: self.height_mm,
            },
            Command::KerfSet {
                width_mm: self.kerf_mm,
            },
        ]
    }

    pub fn recent(&self) -> RecentDoc {
        RecentDoc {
            name: self.name.trim().to_string(),
            width_mm: self.width_mm,
            height_mm: self.height_mm,
            mat: self.mat,
        }
    }
}

/// Puts `doc` first in `recent`, dropping an entry of the same size and mat, keeping `max`.
pub fn push_recent(recent: &mut Vec<RecentDoc>, doc: RecentDoc, max: usize) {
    recent.retain(|r| (r.width_mm, r.height_mm, r.mat) != (doc.width_mm, doc.height_mm, doc.mat));
    recent.insert(0, doc);
    recent.truncate(max);
}

/// The dialog's state between frames.
#[derive(Debug, Default)]
pub struct NewDocState {
    pub open: bool,
    pub dialog: DialogState,
    /// 0 is Recent; `i` is `CATEGORIES[i - 1]`.
    pub tab: usize,
    pub selected: Option<usize>,
    pub choice: Choice,
    /// A double-click on a card asked for Create.
    create_now: bool,
}

impl NewDocState {
    /// Opens on `tab` with the current kerf, machine and mat.
    pub fn open(&mut self, tab: usize, kerf_mm: f64, mat: Mat, model: Model) {
        self.open = true;
        self.tab = tab.min(CATEGORIES.len());
        self.selected = None;
        self.choice.kerf_mm = if kerf_mm.is_finite() {
            kerf_mm.clamp(0.0, MAX_KERF_MM)
        } else {
            0.0
        };
        self.choice.mat = mat;
        self.choice.model = model;
    }

    /// The open tab's cards. Recent lists remembered sizes (the valid ones), then the defaults.
    pub fn cards(&self, recent: &[RecentDoc]) -> Vec<Card> {
        if let Some(category) = self.tab.checked_sub(1).and_then(|i| CATEGORIES.get(i)) {
            return category.presets.iter().map(Card::from).collect();
        }
        let mut cards: Vec<Card> = recent
            .iter()
            .filter(|r| Choice::side_ok(r.width_mm) && Choice::side_ok(r.height_mm))
            .map(|r| Card {
                title: r.name.clone(),
                width_mm: r.width_mm,
                height_mm: r.height_mm,
                mat: r.mat,
                thumb: if r.mat == Mat::None {
                    Thumb::Blank
                } else {
                    Thumb::Mat
                },
                kerf_mm: None,
            })
            .collect();
        cards.extend(RECENT_DEFAULTS.iter().map(Card::from));
        cards
    }

    /// A card was clicked: the details take its size, mat and kerf; the name stays.
    pub fn pick(&mut self, index: usize, card: &Card) {
        self.selected = Some(index);
        self.choice.width_mm = card.width_mm;
        self.choice.height_mm = card.height_mm;
        self.choice.mat = card.mat;
        if let Some(kerf) = card.kerf_mm {
            self.choice.kerf_mm = kerf;
        }
    }

    /// One frame. Returns the choice when the user pressed Create (or double-clicked a card)
    /// with valid details.
    pub fn show(&mut self, ctx: &egui::Context, recent: &[RecentDoc]) -> Option<Choice> {
        if !self.open {
            return None;
        }
        let width = craft_ui::theme::current(ctx).component.new_document.width;
        let valid = self.choice.is_valid();
        let mut dialog = std::mem::take(&mut self.dialog);
        let shown = Dialog::new("new-document", "New Document")
            .width(Width::Fixed(width))
            .footer(
                Footer::standard()
                    .ok("Create")
                    .cancel("Close")
                    .ok_enabled(valid),
            )
            .show(ctx, &mut dialog, &mut (), |ui, ()| self.body(ui, recent));
        self.dialog = dialog;
        let create = shown.outcome == Some(Outcome::Ok)
            || (std::mem::take(&mut self.create_now) && self.choice.is_valid());
        if shown.closed || create {
            self.open = false;
            self.dialog.close();
        }
        (create && self.choice.is_valid()).then(|| self.choice.clone())
    }

    fn body(&mut self, ui: &mut Ui, recent: &[RecentDoc]) {
        let tokens = Tokens::of(ui);
        let nd = tokens.component.new_document;
        let row = Gap::Row.points(&tokens);
        let grid_width = nd.card_columns as f32 * nd.card[0]
            + nd.card_columns.saturating_sub(1) as f32 * row
            + nd.scroll_room;
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(grid_width);
                self.tab_strip(ui);
                ui.add_space(row);
                Divider::horizontal().show(ui);
                ui.add_space(row);
                self.card_grid(ui, recent);
            });
            ui.add_space(Gap::Section.points(&tokens));
            ui.vertical(|ui| {
                ui.set_width(ui.available_width());
                self.details(ui);
            });
        });
    }

    fn tab_strip(&mut self, ui: &mut Ui) {
        let tokens = Tokens::of(ui);
        let titles: Vec<&str> = std::iter::once("Recent")
            .chain(CATEGORIES.iter().map(|c| c.title))
            .collect();
        let ids: Vec<Id> = (0..titles.len())
            .map(|i| Interactive::id_in(ui, ("new-document-tab", i)))
            .collect();
        let stop = GridNav::roving_stop(ui, &ids, Some(self.tab), true);
        let pad = Pad::Control.points(&tokens);
        let mut picked = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = Gap::Section.points(&tokens);
            for (i, (title, id)) in titles.iter().zip(&ids).enumerate() {
                let on = i == self.tab;
                let format = Text::format(
                    &tokens,
                    TypeRole::Label,
                    if on { Content::Normal } else { Content::Muted },
                );
                let galley = ui
                    .painter()
                    .layout_job(egui::text::LayoutJob::single_section(
                        (*title).to_string(),
                        format.clone(),
                    ));
                let (_, rect) =
                    ui.allocate_space(vec2(galley.size().x, galley.size().y + 2.0 * pad));
                let mut element = Interactive::with_id(*id, egui::accesskit::Role::Tab, *title)
                    .selected(on)
                    .position_in_set(i);
                if i != stop {
                    element = element.sense(Sense::CLICK);
                }
                let response = element.interact_at(ui, rect).finish(ui);
                if ui.is_rect_visible(rect) {
                    ui.painter()
                        .galley(pos2(rect.left(), rect.top() + pad), galley, format.color);
                    if on {
                        let y = rect.bottom();
                        ui.painter().line_segment(
                            [pos2(rect.left(), y), pos2(rect.right(), y)],
                            Stroke::new(2.0, Content::Accent.color(&tokens).to_egui()),
                        );
                    }
                }
                if response.clicked() || response.gained_focus() {
                    picked = Some(i);
                }
            }
        });
        if let Some(i) = picked.filter(|i| *i != self.tab) {
            self.tab = i;
            self.selected = None;
        }
    }

    fn card_grid(&mut self, ui: &mut Ui, recent: &[RecentDoc]) {
        let tokens = Tokens::of(ui);
        let cards = self.cards(recent);
        let heading = if self.tab == 0 {
            format!("Recent ({})", cards.len())
        } else {
            format!("Blank Document Presets ({})", cards.len())
        };
        SectionLabel::new(heading).show(ui);
        let gap = Gap::Row.points(&tokens);
        ui.add_space(gap);
        let nd = tokens.component.new_document;
        let card_size = Vec2::from(nd.card);
        let columns = nd.card_columns.max(1);
        let rows = cards.len().div_ceil(columns);
        let inset = Pad::Control.points(&tokens);
        let title_format = Text::format(&tokens, TypeRole::Body, Content::Normal);
        let line_format = Text::format(&tokens, TypeRole::Caption, Content::Faint);
        let outline = Content::Muted.color(&tokens).to_egui();
        let accent = Content::Accent.color(&tokens).to_egui();
        let ids: Vec<Id> = (0..cards.len())
            .map(|i| Interactive::id_in(ui, ("new-document-card", self.tab, i)))
            .collect();
        let mut clicked = None;
        let mut double = false;
        let units = self.choice.units;
        ScrollArea::vertical()
            .id_salt(("new-document-cards", self.tab))
            .max_height(
                nd.card_rows as f32 * card_size.y + nd.card_rows.saturating_sub(1) as f32 * gap,
            )
            .auto_shrink([false, true])
            .show(ui, |ui| {
                let width = columns as f32 * card_size.x + (columns - 1) as f32 * gap;
                let height = rows as f32 * card_size.y + rows.saturating_sub(1) as f32 * gap;
                let (_, area) = ui.allocate_space(vec2(width, height));
                let stop = GridNav::roving_stop(ui, &ids, self.selected, true);
                for (i, (card, id)) in cards.iter().zip(&ids).enumerate() {
                    let (r, c) = (i / columns, i % columns);
                    let rect = Rect::from_min_size(
                        area.min
                            + vec2(
                                c as f32 * (card_size.x + gap),
                                r as f32 * (card_size.y + gap),
                            ),
                        card_size,
                    );
                    let on = self.selected == Some(i);
                    let line = size_line(card.width_mm, card.height_mm, units);
                    let mut element = Interactive::with_id(
                        *id,
                        egui::accesskit::Role::Button,
                        card.title.as_str(),
                    )
                    .surface(SurfaceKind::Raised)
                    .corner(Corner::Control)
                    .selected(on);
                    if i != stop {
                        element = element.sense(Sense::CLICK);
                    }
                    let response = element.interact_at(ui, rect).finish(ui);
                    if response.gained_focus() {
                        ui.scroll_to_rect(rect, None);
                    }
                    if ui.is_rect_visible(rect) {
                        if on {
                            ui.painter().rect_stroke(
                                rect,
                                Corner::Control.radius(&tokens),
                                Stroke::new(1.5, accent),
                                StrokeKind::Inside,
                            );
                        }
                        let center = pos2(rect.center().x, rect.top() + nd.card_icon_center);
                        paint_thumb(
                            ui.painter(),
                            Rect::from_center_size(center, Vec2::splat(nd.page_box)),
                            card,
                            nd.page_icon,
                            outline,
                            accent,
                        );
                        for (text, format, max_rows, y) in [
                            (card.title.clone(), &title_format, 2, nd.card_title_center),
                            (line.clone(), &line_format, 1, nd.card_line_center),
                        ] {
                            let mut job =
                                egui::text::LayoutJob::single_section(text, format.clone());
                            job.wrap.max_width = card_size.x - 2.0 * inset;
                            job.wrap.max_rows = max_rows;
                            job.halign = Align::Center;
                            let galley = ui.painter().layout_job(job);
                            let top = rect.top() + y - galley.size().y / 2.0;
                            ui.painter()
                                .galley(pos2(rect.center().x, top), galley, format.color);
                        }
                    }
                    let response = response.on_hover_text(format!("{} · {line}", card.title));
                    if response.clicked() || response.gained_focus() {
                        clicked = Some(i);
                    }
                    double |= response.double_clicked();
                }
            });
        if let Some(i) = clicked
            && let Some(card) = cards.get(i)
        {
            if self.selected != Some(i) {
                self.pick(i, card);
            }
            self.create_now |= double;
        }
    }

    fn details(&mut self, ui: &mut Ui) {
        let tokens = Tokens::of(ui);
        let numbers = LocaleNumbers::of_ctx(ui.ctx());
        let choice = &mut self.choice;
        let before = (choice.width_mm, choice.height_mm, choice.mat);
        SectionLabel::new("Preset Details").show(ui);
        ui.add_space(Gap::Row.points(&tokens));
        let edit = TextField::new("new-document-name", "Name", &choice.name)
            .char_limit(255)
            .show(ui);
        if let Some(name) = edit.event.as_ref().and_then(EditEvent::value) {
            choice.name.clone_from(name);
        }

        let range = 0.01..=choice.units.display_value(MAX_SIDE_MM);
        for (label, side) in [
            ("Width", &mut choice.width_mm),
            ("Height", &mut choice.height_mm),
        ] {
            caption(ui, label);
            let edit = NumberField::new(
                ("new-document-side", label),
                label,
                choice.units.display_value(*side),
                &numbers,
            )
            .range(range.clone())
            .unit(Unit::new(choice.units.suffix()))
            .show(ui);
            if let Some(value) = edit.event.and_then(|e| e.value().copied())
                && value.is_finite()
            {
                *side = choice.units.document_value(value);
            }
        }

        caption(ui, "Units");
        let _ = Select::new("new-document-units", "Units", &mut choice.units)
            .option(Units::Millimetres, Units::Millimetres.label())
            .option(Units::Inches, Units::Inches.label())
            .show(ui);

        caption(ui, "Orientation");
        let mut orientation = choice.orientation();
        let turned = Segmented::new("new-document-orientation", "Orientation", &mut orientation)
            .icon_item(Orientation::Portrait, Icon::OrientationPortrait, "Portrait")
            .icon_item(
                Orientation::Landscape,
                Icon::OrientationLandscape,
                "Landscape",
            )
            .show(ui);
        if turned.changed() {
            choice.set_orientation(orientation);
        }

        caption(ui, "Cutting Mat");
        let mut mat_select = Select::new("new-document-mat", "Cutting Mat", &mut choice.mat);
        for (mat, label) in crate::MAT_CHOICES {
            mat_select = mat_select.option(*mat, *label);
        }
        let _ = mat_select.show(ui);

        caption(ui, "Machine");
        let mut model_select = Select::new("new-document-machine", "Machine", &mut choice.model);
        for (model, label) in crate::MODEL_CHOICES {
            model_select = model_select.option(*model, *label);
        }
        let _ = model_select.show(ui);

        caption(ui, "Kerf");
        let edit = NumberField::new("new-document-kerf", "Kerf", choice.kerf_mm, &numbers)
            .range(0.0..=MAX_KERF_MM)
            .unit(Unit::new("{n} mm"))
            .show(ui);
        if let Some(value) = edit.event.and_then(|e| e.value().copied())
            && value.is_finite()
        {
            choice.kerf_mm = value.clamp(0.0, MAX_KERF_MM);
        }

        if !choice.is_valid() {
            ui.add_space(Gap::Row.points(&tokens));
            ui.add(
                Text::new("Enter a name, sides up to 10 m and a kerf of 0–2 mm.")
                    .role(TypeRole::Caption)
                    .content(Content::Muted),
            );
        }
        // Editing a size or mat by hand leaves the card it came from.
        if (choice.width_mm, choice.height_mm, choice.mat) != before
            && self.selected.is_some()
            && !turned.changed()
        {
            self.selected = None;
        }
    }
}

fn caption(ui: &mut Ui, text: &str) {
    ui.add(
        Text::new(text)
            .role(TypeRole::Caption)
            .content(Content::Muted),
    );
}

/// "300 × 300 mm" or "12 × 12 in".
pub fn size_line(width_mm: f64, height_mm: f64, units: Units) -> String {
    let fmt = |v: f64| {
        let v = units.display_value(v);
        if (v - v.round()).abs() < 0.005 {
            format!("{v:.0}")
        } else {
            format!("{v:.2}").trim_end_matches('0').to_string()
        }
    };
    let unit = match units {
        Units::Millimetres => "mm",
        Units::Inches => "in",
    };
    format!("{} × {} {unit}", fmt(width_mm), fmt(height_mm))
}

/// The page outline at the card's aspect (longer side `longer` points) plus the kind's mark.
fn paint_thumb(
    painter: &egui::Painter,
    rect: Rect,
    card: &Card,
    longer: f32,
    color: Color32,
    accent: Color32,
) {
    let (w, h) = (card.width_mm as f32, card.height_mm as f32);
    let scale = longer / w.max(h).max(f32::EPSILON);
    let size = vec2((w * scale).max(4.0), (h * scale).max(4.0));
    let page = Rect::from_center_size(rect.center(), size);
    let thin = Stroke::new(1.0, color);
    painter.rect_stroke(page, 1.0, Stroke::new(1.2, color), StrokeKind::Inside);
    let lerp = |fx: f32, fy: f32| {
        pos2(
            page.left() + page.width() * fx,
            page.top() + page.height() * fy,
        )
    };
    match card.thumb {
        Thumb::Blank => {}
        Thumb::Mat => {
            for f in [0.25, 0.5, 0.75] {
                painter.line_segment([lerp(f, 0.0), lerp(f, 1.0)], Stroke::new(0.5, color));
                painter.line_segment([lerp(0.0, f), lerp(1.0, f)], Stroke::new(0.5, color));
            }
        }
        Thumb::Vinyl => {
            painter.line_segment([lerp(0.6, 1.0), lerp(1.0, 0.6)], Stroke::new(1.2, accent));
            painter.line_segment([lerp(0.6, 1.0), lerp(0.75, 0.75)], thin);
            painter.line_segment([lerp(1.0, 0.6), lerp(0.75, 0.75)], thin);
        }
        Thumb::Transfer => {
            painter.arrow(
                lerp(0.45, 0.5),
                vec2(-page.width() * 0.3, 0.0),
                Stroke::new(1.2, accent),
            );
            painter.arrow(
                lerp(0.55, 0.5),
                vec2(page.width() * 0.3, 0.0),
                Stroke::new(1.2, accent),
            );
        }
        Thumb::Card => {
            let (top, bottom) = (lerp(0.5, 0.0), lerp(0.5, 1.0));
            painter.extend(egui::Shape::dashed_line(
                &[top, bottom],
                Stroke::new(1.0, accent),
                3.0,
                2.0,
            ));
        }
        Thumb::Tag => {
            painter.circle_stroke(
                lerp(0.5, 0.15),
                page.width().min(page.height()) * 0.1,
                Stroke::new(1.2, accent),
            );
        }
        Thumb::PrintCut => {
            let s = page.width().min(page.height()) * 0.15;
            let mark = lerp(0.1, 0.1);
            painter.rect_filled(Rect::from_min_size(mark, Vec2::splat(s)), 0.0, accent);
            for (fx, fy, dx, dy) in [(0.9, 0.1, -1.0, 1.0), (0.1, 0.9, 1.0, -1.0)] {
                let corner = lerp(fx, fy);
                painter.line_segment(
                    [corner, corner + vec2(dx * s, 0.0)],
                    Stroke::new(1.2, accent),
                );
                painter.line_segment(
                    [corner, corner + vec2(0.0, dy * s)],
                    Stroke::new(1.2, accent),
                );
            }
        }
        Thumb::Stencil => {
            let window = Rect::from_min_max(lerp(0.3, 0.35), lerp(0.7, 0.65));
            painter.rect_stroke(window, 2.0, Stroke::new(1.2, accent), StrokeKind::Inside);
        }
        Thumb::Laser => {
            let c = page.center();
            let r = page.width().min(page.height()) * 0.3;
            painter.circle_stroke(c, r * 0.6, Stroke::new(1.2, accent));
            painter.line_segment([c - vec2(r, 0.0), c + vec2(r, 0.0)], thin);
            painter.line_segment([c - vec2(0.0, r), c + vec2(0.0, r)], thin);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_are_valid_boards_with_unique_ids() {
        let mut ids = std::collections::HashSet::new();
        for preset in CATEGORIES
            .iter()
            .flat_map(|c| c.presets)
            .chain(RECENT_DEFAULTS)
        {
            assert!(Choice::side_ok(preset.width_mm), "{}", preset.id);
            assert!(Choice::side_ok(preset.height_mm), "{}", preset.id);
            if let Some(kerf) = preset.kerf_mm {
                assert!((0.0..=MAX_KERF_MM).contains(&kerf), "{}", preset.id);
            }
            if !RECENT_DEFAULTS.iter().any(|d| d.id == preset.id) {
                assert!(ids.insert(preset.id), "duplicate id {}", preset.id);
            }
        }
        assert!(CATEGORIES.iter().all(|c| !c.presets.is_empty()));
    }

    #[test]
    fn mat_presets_match_the_machine_mats() {
        let mats = CATEGORIES.iter().find(|c| c.id == "mats").unwrap();
        for preset in mats.presets {
            let (down, right) = preset.mat.extent_mm();
            assert!((preset.width_mm - right).abs() < 1e-9, "{}", preset.id);
            assert!((preset.height_mm - down).abs() < 1e-9, "{}", preset.id);
        }
        // Every real mat has a card.
        for mat in Mat::ALL.iter().filter(|m| **m != Mat::None) {
            assert!(mats.presets.iter().any(|p| p.mat == *mat), "{mat:?}");
        }
    }

    #[test]
    fn picking_a_card_fills_details_and_keeps_the_name() {
        let mut state = NewDocState::default();
        state.choice.name = "Poster".into();
        state.open(5, 0.0, Mat::None, Model::Cameo4);
        let cards = state.cards(&[]);
        let card = cards.first().unwrap();
        state.pick(0, card);
        assert_eq!(state.selected, Some(0));
        assert_eq!(state.choice.name, "Poster");
        assert_eq!(state.choice.width_mm, card.width_mm);
        assert_eq!(state.choice.kerf_mm, 0.15);
        assert_eq!(state.choice.model, Model::Cameo4);
    }

    #[test]
    fn orientation_swaps_sides_but_not_squares() {
        let mut choice = Choice {
            width_mm: 100.0,
            height_mm: 150.0,
            ..Choice::default()
        };
        assert_eq!(choice.orientation(), Orientation::Portrait);
        choice.set_orientation(Orientation::Landscape);
        assert_eq!((choice.width_mm, choice.height_mm), (150.0, 100.0));
        choice.set_orientation(Orientation::Landscape);
        assert_eq!((choice.width_mm, choice.height_mm), (150.0, 100.0));
        choice.width_mm = 100.0;
        choice.height_mm = 100.0;
        choice.set_orientation(Orientation::Landscape);
        assert_eq!((choice.width_mm, choice.height_mm), (100.0, 100.0));
    }

    #[test]
    fn invalid_details_block_create() {
        let ok = Choice::default();
        assert!(ok.is_valid());
        for bad in [
            Choice {
                width_mm: f64::NAN,
                ..ok.clone()
            },
            Choice {
                height_mm: 0.0,
                ..ok.clone()
            },
            Choice {
                width_mm: MAX_SIDE_MM * 2.0,
                ..ok.clone()
            },
            Choice {
                kerf_mm: -1.0,
                ..ok.clone()
            },
            Choice {
                name: "  ".into(),
                ..ok.clone()
            },
        ] {
            assert!(!bad.is_valid(), "{bad:?}");
        }
    }

    #[test]
    fn commands_reset_the_board_and_set_kerf() {
        let choice = Choice {
            width_mm: 300.0,
            height_mm: 200.0,
            kerf_mm: 0.1,
            ..Choice::default()
        };
        let mut engine = stencilcraft_engine::Engine::default();
        for command in choice.commands() {
            engine.run(command).unwrap();
        }
        assert_eq!(engine.doc.board.width_mm, 300.0);
        assert_eq!(engine.doc.board.height_mm, 200.0);
        assert_eq!(engine.doc.kerf.width_mm, 0.1);
    }

    #[test]
    fn recent_lists_newest_first_without_duplicates_and_skips_bad_entries() {
        let mut recent = Vec::new();
        let doc = |w: f64| RecentDoc {
            name: format!("d{w}"),
            width_mm: w,
            height_mm: 10.0,
            mat: Mat::None,
        };
        for w in [1.0, 2.0, 1.0] {
            push_recent(&mut recent, doc(w), 2);
        }
        assert_eq!(recent, vec![doc(1.0), doc(2.0)]);
        recent.push(doc(f64::INFINITY));
        let state = NewDocState::default();
        let cards = state.cards(&recent);
        assert_eq!(cards.len(), 2 + RECENT_DEFAULTS.len());
        assert_eq!(cards[0].title, "d1");
    }

    #[test]
    fn size_line_formats_units() {
        assert_eq!(size_line(304.8, 304.8, Units::Inches), "12 × 12 in");
        assert_eq!(size_line(215.9, 279.4, Units::Inches), "8.5 × 11 in");
        assert_eq!(size_line(210.0, 297.0, Units::Millimetres), "210 × 297 mm");
    }
}
