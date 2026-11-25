//! The StencilCraft desktop app: window bootstrap, Craft-suite chrome (`MenuBar`, `OptionsBar`,
//! `ToolRail`, `Workspace`, `DockArea`, `Panel`, `StatusBar`, `Rulers`, `BezierOverlay`,
//! `PenDraft`), file dialogs, and machine cut preparation.
//!
//! All user-visible document mutations route through `stencilcraft_engine::Command`; canvas
//! painting and geometry conversions live in `stencilcraft_ui_egui`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use craft_behavior::{IconId, Selection};
use craft_draw::ui::pen::{
    BezierHit, BezierOverlay, BezierTool, EditPath, EditSubpath, HandlesShown, PathEdit,
    PathSelection, PenDraft,
};
#[cfg(not(target_arch = "wasm32"))]
use craft_settings::{Location, Settings, SettingsStore};
use craft_theme::{ControlSize, TypeRole, Variant};
use craft_ui::bars::{AlignAction, Bar, BarKind, BarTool, StatusMessage};
use craft_ui::buttons::{Button, IconButton, ToggleButton};
use craft_ui::choice::Checkbox;
use craft_ui::contract::{
    CollectionEvent, ColumnSpec, Entry, Props as _, Show as _, ShowCollection as _,
    ShowContainer as _, ToggleColumn,
};
use craft_ui::dialog::Dialog;
use craft_ui::display::{Divider, Shade, Text};
use craft_ui::list::{List, ListRow, RowInk, ToggleCell};
use craft_ui::patterns::panels::Inspector;
use craft_ui::pickers::{ReferencePoint, Select};
use craft_ui::primitives::IconSpec;
use craft_ui::sections::{DialogRole, FooterAction, FooterBar, FooterButton, Panel, Section};
use craft_ui::text::{NumberField, TextField};
use craft_ui::value_drag::{Slider, SliderScale};
use panels::LocaleNumbers;
use std::sync::Arc;
use tool_rail::{LastUsed, RailTool, ToolSlot};

use craft_behavior::EditEvent;
use craft_behavior::dock::SplitSize;
use craft_behavior::dock::{Column, DockEdge, Group, Layout, Node, Permissions, Workspace};
use craft_behavior::flow::Axis;
use craft_ui::contract::{Bind, BodyCx, Tracked};
use craft_ui::docking::{DockConfig, DockPanels, GroupAt, PanelSpec, PanelSpecs};
use craft_ui::patterns::dock::DockArea;
use craft_ui::shell::{Title, TitleBar};
use craft_window::policy::WindowKind;
use eframe::egui;
use serde::{Deserialize, Serialize};
use stencilcraft_doc::CutStyle;
use stencilcraft_engine::document::{DocumentCommand, TransformRequest};
use stencilcraft_engine::{Alignment, BooleanOperation, Command, DistributionAxis};

/// CLI/agent control channel: JSON lines on a TCP socket, one session per
/// connection (the same session core the CLI's `mcp` mode uses).
mod actions;
mod arc_draw;
mod click_draw;
mod colour_panels;
mod control;
mod distort_tool;
mod effects;
mod file_menu;
mod framework;
mod freehand;
mod graphic_styles;

/// The menu bar (Silhouette Studio order plus Illustrator-grade items; docs/menu-map.md).
mod menus;

/// File › New: the New Document dialog (preset cards and details).
mod library_panel;
mod machine_menu;
mod misc_panels;
mod new_doc;
mod object_menu;

/// Silhouette-style Transform, Replicate, Offset, Modify and Point Editing panels.
mod page_setup;
mod panels;
mod pixscan;
mod print_marks;
mod send_modes;
mod text_tool;
mod tool_rail;
mod trace_panel;

const APP_ICON_PNG: &[u8] =
    include_bytes!("../../../assets/app-icon/hicolor/256x256/apps/ai.storyteller.stencilcraft.png");

/// Persistent user preferences stored via `craft-settings`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UserPrefs {
    pub dark_theme: bool,
    pub show_rulers: bool,
    pub show_grid: bool,
    /// View › Guides › Show Guides.
    pub show_guides: bool,
    /// View › Smart Guides: draw the alignment lines snapping found while dragging.
    pub smart_guides: bool,
    pub print_cut_marks: bool,
    /// Print & Cut: registration mark size and inset, and bleed.
    pub marks: print_marks::MarkSettings,
    /// View › Show Cut Border (on, as in Silhouette).
    pub show_cut_border: bool,
    /// View › Show Print Border.
    pub show_print_border: bool,
    /// View › Show Dimensions: the selection's width and height beside it.
    pub show_dimensions: bool,
    /// View › Outline View: hairlines only.
    pub outline_view: bool,
    /// Machine › Mat Preview: draw the cutting mat behind the page.
    pub mat_preview: bool,
    /// Panels › Starter Mode: only the everyday panels.
    pub starter_mode: bool,
    /// Send › My materials: saved cut settings.
    pub custom_materials: Vec<send_modes::CustomMaterial>,
    /// The cutter model and material last used, restored at start (Silhouette remembers
    /// them).
    pub machine_model: Option<String>,
    pub media_id: Option<u16>,
    /// The cutting mat last used (`Mat::id`).
    pub mat_id: Option<String>,
    /// Text Style › Recently used font families, newest first.
    pub recent_fonts: Vec<String>,
    /// Return to the Select tool after drawing a shape (else the tool stays, as by default).
    pub select_after_drawing: bool,
    /// Page Setup › Smart Snap only within media: off the page only the grid and guides snap.
    pub snap_within_media: bool,
    /// Panels › Graphic Styles: saved line/fill/operation sets.
    pub graphic_styles: Vec<graphic_styles::GraphicStyle>,
    /// Panels › Actions: recorded edit sequences.
    pub actions: Vec<actions::SavedAction>,
    /// Preferences › Units: what lengths are shown and typed in.
    pub length_unit: stencilcraft_ui_egui::LengthUnit,
    pub rail_columns: u8,
    pub link_aspect: bool,
    pub recent_files: Vec<String>,
    /// Sizes created from New Document, newest first (its Recent tab).
    pub recent_new: Vec<new_doc::RecentDoc>,
    /// File › New: presets the person saved (Save as Preset).
    pub saved_new: Vec<new_doc::RecentDoc>,
}

impl Default for UserPrefs {
    fn default() -> Self {
        Self {
            dark_theme: true,
            show_rulers: true,
            show_grid: true,
            show_guides: true,
            smart_guides: true,
            print_cut_marks: false,
            marks: print_marks::MarkSettings::default(),
            show_cut_border: true,
            show_print_border: false,
            show_dimensions: true,
            outline_view: false,
            mat_preview: true,
            starter_mode: false,
            custom_materials: Vec::new(),
            machine_model: None,
            media_id: None,
            mat_id: None,
            recent_fonts: Vec::new(),
            select_after_drawing: false,
            snap_within_media: false,
            graphic_styles: Vec::new(),
            actions: Vec::new(),
            length_unit: Default::default(),
            rail_columns: 1,
            link_aspect: false,
            recent_files: Vec::new(),
            recent_new: Vec::new(),
            saved_new: Vec::new(),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Settings for UserPrefs {
    const APP_ID: &'static str = "stencilcraft";
    const SCHEMA_VERSION: u32 = 1;
}

/// An arrow key pressed alone or with Shift: the nudge in screen pixels (Silhouette moves about
/// 2 px per press, 16 px with Shift).
fn arrow_nudge(ctx: &egui::Context) -> Option<(f64, f64)> {
    ctx.input_mut(|input| {
        for (key, dx, dy) in [
            (egui::Key::ArrowLeft, -1.0, 0.0),
            (egui::Key::ArrowRight, 1.0, 0.0),
            (egui::Key::ArrowUp, 0.0, -1.0),
            (egui::Key::ArrowDown, 0.0, 1.0),
        ] {
            for (mods, step) in [(egui::Modifiers::SHIFT, 16.0), (egui::Modifiers::NONE, 2.0)] {
                if consume_exact(input, mods, key) {
                    return Some((dx * step, dy * step));
                }
            }
        }
        None
    })
}

/// Takes this pass's Escape: from egui's input while one of the app's own widgets has focus,
/// else the window's Cancel, which the runtime routes Escape to.
pub(crate) fn take_escape(ctx: &egui::Context) -> bool {
    let egui_escape =
        ctx.input_mut(|input| consume_exact(input, egui::Modifiers::NONE, egui::Key::Escape));
    egui_escape || framework::take_window_cancel(ctx)
}

/// Consume `key` only when it was pressed with exactly `modifiers`. egui's `consume_key`
/// ignores extra Shift and Alt, so Shift+G would pick the Polygon tool and Cmd+Shift+L would
/// run Cmd+L; shortcuts here want the combination as written.
pub(crate) fn consume_exact(
    input: &mut egui::InputState,
    modifiers: egui::Modifiers,
    key: egui::Key,
) -> bool {
    let pressed = input.events.iter().any(|event| {
        matches!(
            event,
            egui::Event::Key { key: k, pressed: true, modifiers: m, .. }
                if *k == key && m.matches_exact(modifiers)
        )
    });
    pressed && input.consume_key(modifiers, key)
}

/// Modifier keys that shape a drag-drawn box, read at release as in Silhouette Studio.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DragMods {
    /// Shift: equal width and height (square, circle, regular polygon).
    pub square: bool,
    /// Alt: the press point is the centre of the shape.
    pub from_centre: bool,
}

/// The one anchor a node drag moved, snapped as the node overlay snapped it before: an open end
/// dragged within `join_reach` of another open end lands on it (it joins on release), otherwise the
/// anchor snaps by the document's snap settings unless `off` (Ctrl/Cmd, a suspended snap). Its
/// handles move with it.
pub fn snap_moved_anchor(
    edited: &mut [EditSubpath],
    change: &PathEdit,
    doc: &stencilcraft_doc::Document,
    exclude: &[u64],
    join_reach: f64,
    off: bool,
    within_media: bool,
) {
    let PathEdit::MoveAnchors { anchors, .. } = change else {
        return;
    };
    let [moved] = anchors.as_slice() else {
        return;
    };
    let Some(at) = edited
        .get(moved.subpath)
        .and_then(|s| s.knots.get(moved.knot))
        .map(|k| k.anchor)
    else {
        return;
    };
    let join = stencilcraft_ui_egui::open_path_ends(edited)
        .contains(moved)
        .then(|| stencilcraft_ui_egui::open_end_near(edited, *moved, at, join_reach))
        .flatten()
        .and_then(|end| edited.get(end.subpath)?.knots.get(end.knot))
        .map(|k| k.anchor);
    let snapped = match join {
        Some(target) => target,
        None if off => return,
        None => snap_point_in(doc, at, exclude, within_media),
    };
    if let Some(knot) = edited
        .get_mut(moved.subpath)
        .and_then(|s| s.knots.get_mut(moved.knot))
    {
        let shift = snapped - knot.anchor;
        knot.anchor = snapped;
        knot.in_ctrl += shift;
        knot.out_ctrl += shift;
    }
}

/// The affine that maps the selection box's start `bounds` onto `state`'s quad (scale and
/// rotation; the box allows no skew or distortion). `None` for an empty or non-finite box.
pub fn box_affine(
    bounds: stencilcraft_geom::Rect,
    state: &craft_behavior::geometry::quad::BoxState,
) -> Option<craft_behavior::kurbo::Affine> {
    let (w, h) = (bounds.width(), bounds.height());
    if !(w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0) {
        return None;
    }
    let [q0, q1, _, q3] = state.quad;
    let ux = (q1 - q0) / w;
    let uy = (q3 - q0) / h;
    let (a, b, c, d) = (ux.x, ux.y, uy.x, uy.y);
    let e = q0.x - a * bounds.x0 - c * bounds.y0;
    let f = q0.y - b * bounds.x0 - d * bounds.y0;
    let coeffs = [a, b, c, d, e, f];
    coeffs
        .iter()
        .all(|v| v.is_finite())
        .then(|| craft_behavior::kurbo::Affine::new(coeffs))
}

/// Paints `rows` (a label and a value each) beside the pointer at `at` for this pass: the value
/// readout of a canvas gesture.
pub fn pointer_readout(ui: &egui::Ui, at: egui::Pos2, rows: &[(&str, String)]) {
    let env = craft_ui::runtime::env::Env::of(ui);
    let rows: Vec<(craft_ui::runtime::env::Label, String)> = rows
        .iter()
        .map(|(label, value)| {
            (
                craft_ui::runtime::env::Label::from((*label).to_owned()),
                value.clone(),
            )
        })
        .collect();
    craft_ui::contract::ghost::readout(ui.ctx(), &env.view(), at, &rows);
}

/// The first guide of `before` that `after` moves, turns or removes, as a drag preview.
pub fn guide_change(
    before: &[craft_ui::canvas::Guide],
    after: &[craft_ui::canvas::Guide],
) -> Option<stencilcraft_ui_egui::GuidePreview> {
    before
        .iter()
        .find_map(|old| match after.iter().find(|g| g.key == old.key) {
            None => Some(stencilcraft_ui_egui::GuidePreview {
                key: old.key,
                along: old.along,
                at: None,
            }),
            Some(new) if new.along != old.along || new.position != old.position => {
                Some(stencilcraft_ui_egui::GuidePreview {
                    key: new.key,
                    along: new.along,
                    at: Some(new.position),
                })
            }
            Some(_) => None,
        })
}

/// The guide a drag in progress changes, as [`stencilcraft_ui_egui::canvas_guides`] previews
/// it: a moved, turned or removed document guide, else one being dragged out of a ruler (a key
/// `before` does not hold).
pub fn guide_drag(
    before: &[craft_ui::canvas::Guide],
    proposed: &[craft_ui::canvas::Guide],
) -> Option<stencilcraft_ui_egui::GuidePreview> {
    guide_change(before, proposed).or_else(|| {
        proposed
            .iter()
            .find(|g| !before.iter().any(|b| b.key == g.key))
            .map(|g| stencilcraft_ui_egui::GuidePreview {
                key: g.key,
                along: g.along,
                at: Some(g.position),
            })
    })
}

/// How near (screen points) the pointer must be to a selection box handle or a guide to be on it.
const CONTROL_REACH_PX: f32 = 10.0;
/// How far outside a selection box corner its rotate band reaches, in screen points.
const ROTATE_BAND_PX: f32 = 24.0;

/// Whether `at` (screen) is on the selection box of `bounds` (millimetres): one of its eight
/// handles, or the rotate band just outside a corner. The app's canvas widget leaves those
/// presses to the framework's transform box.
fn over_selection_box_part(
    view: &stencilcraft_ui_egui::View,
    bounds: stencilcraft_geom::Rect,
    at: egui::Pos2,
) -> bool {
    let corners = [
        (bounds.x0, bounds.y0),
        (bounds.x1, bounds.y0),
        (bounds.x1, bounds.y1),
        (bounds.x0, bounds.y1),
    ]
    .map(|(x, y)| view.world_to_screen(stencilcraft_geom::Point::new(x, y)));
    let mids = [(0, 1), (1, 2), (2, 3), (3, 0)].map(|(a, b)| corners[a].lerp(corners[b], 0.5));
    if corners
        .iter()
        .chain(&mids)
        .any(|p| p.distance(at) <= CONTROL_REACH_PX)
    {
        return true;
    }
    let screen = egui::Rect::from_points(&corners);
    !screen.contains(at) && corners.iter().any(|c| c.distance(at) <= ROTATE_BAND_PX)
}

/// Whether `at` (screen) is on one of `guides`.
fn over_guide_line(
    view: &stencilcraft_ui_egui::View,
    guides: &[stencilcraft_doc::Guide],
    at: egui::Pos2,
) -> bool {
    guides.iter().any(|guide| {
        let (a, b) = match guide.axis {
            stencilcraft_doc::GuideAxis::Vertical => (
                stencilcraft_geom::Point::new(guide.position_mm, 0.0),
                stencilcraft_geom::Point::new(guide.position_mm, 1.0),
            ),
            stencilcraft_doc::GuideAxis::Horizontal => (
                stencilcraft_geom::Point::new(0.0, guide.position_mm),
                stencilcraft_geom::Point::new(1.0, guide.position_mm),
            ),
        };
        let (a, b) = (view.world_to_screen(a), view.world_to_screen(b));
        let dir = b - a;
        let len = dir.length();
        len > f32::EPSILON
            && ((at - a).x * dir.y - (at - a).y * dir.x).abs() / len <= CONTROL_REACH_PX / 2.0
    })
}

/// `p` snapped by the document's snap settings, ignoring shapes `exclude` (a dragged node
/// must not snap to its own shape's bounds). `p` itself when nothing is in reach.
pub fn snap_point_in(
    doc: &stencilcraft_doc::Document,
    p: stencilcraft_geom::Point,
    exclude: &[u64],
    within_media: bool,
) -> stencilcraft_geom::Point {
    let snap = &doc.snap;
    if !(snap.grid || snap.guides || snap.objects || snap.points) {
        return p;
    }
    let query = stencilcraft_engine::document::SnapQuery {
        x: p.x,
        y: p.y,
        width: 0.0,
        height: 0.0,
        exclude: exclude.to_vec(),
        tolerance_mm: None,
        within_media,
    };
    match stencilcraft_engine::document::snap(doc, &query) {
        Ok(r) if r.x.is_finite() && r.y.is_finite() => stencilcraft_geom::Point::new(r.x, r.y),
        _ => p,
    }
}

impl From<egui::Modifiers> for DragMods {
    fn from(modifiers: egui::Modifiers) -> Self {
        Self {
            square: modifiers.shift,
            from_centre: modifiers.alt,
        }
    }
}

/// Where the Line tool's line ends: Shift (`mods.square`) snaps its angle to the nearest 45°
/// step, keeping its length, as Silhouette Studio does.
pub fn line_end(
    start: stencilcraft_geom::Point,
    end: stencilcraft_geom::Point,
    mods: DragMods,
) -> stencilcraft_geom::Point {
    let run = end - start;
    if !mods.square || run.hypot() == 0.0 {
        return end;
    }
    let step = std::f64::consts::FRAC_PI_4;
    let angle = (run.y.atan2(run.x) / step).round() * step;
    start + stencilcraft_geom::Vec2::from_angle(angle) * run.hypot()
}

/// The box a shape tool draws from a press at `start` dragged to `end`.
pub fn drag_box(
    start: stencilcraft_geom::Point,
    end: stencilcraft_geom::Point,
    mods: DragMods,
) -> stencilcraft_geom::Rect {
    let (mut dx, mut dy) = (end.x - start.x, end.y - start.y);
    if mods.square {
        let side = dx.abs().max(dy.abs());
        dx = side.copysign(dx);
        dy = side.copysign(dy);
    }
    if mods.from_centre {
        stencilcraft_geom::Rect::new(
            start.x - dx.abs(),
            start.y - dy.abs(),
            start.x + dx.abs(),
            start.y + dy.abs(),
        )
    } else {
        stencilcraft_geom::Rect::from_points(
            start,
            stencilcraft_geom::Point::new(start.x + dx, start.y + dy),
        )
    }
}

/// Interactive canvas tools in the left `ToolRail`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ActiveTool {
    #[default]
    Select,
    DirectSelect,
    ConvertPoint,
    BoxSelect,
    LassoSelect,
    Pen,
    Line,
    Polyline,
    Curve,
    Arc,
    Freehand,
    SmoothFreehand,
    Knife,
    Eraser,
    Rect,
    RoundedRect,
    Ellipse,
    Polygon,
    Star,
    Text,
    TraceImage,
    Hand,
    Zoom,
}

impl ActiveTool {
    pub fn icon(self) -> IconId {
        IconId::new(self.icon_name())
    }

    /// The tool's icon in the pin's icon registry.
    pub fn icon_name(self) -> &'static str {
        match self {
            Self::Select => "pointer",
            Self::DirectSelect => "direct_select",
            Self::ConvertPoint => "convert_point",
            Self::BoxSelect => "marquee_rect",
            Self::LassoSelect => "lasso",
            Self::Pen => "pen_tool",
            Self::Line => "slash",
            Self::Polyline => "route_elbow",
            Self::Curve => "spline",
            Self::Arc => "route_curved",
            Self::Freehand => "pencil",
            Self::SmoothFreehand => "brush",
            Self::Knife => "slice_knife",
            Self::Eraser => "eraser",
            Self::Rect => "square",
            Self::RoundedRect => "frame_placeholder",
            Self::Ellipse => "circle",
            Self::Polygon => "pentagon",
            Self::Star => "star",
            Self::Text => "text",
            Self::TraceImage => "image",
            Self::Hand => "hand",
            Self::Zoom => "zoom_in",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::DirectSelect => "Direct Select",
            Self::ConvertPoint => "Convert Point",
            Self::BoxSelect => "Box Select",
            Self::LassoSelect => "Lasso Select",
            Self::Pen => "Pen",
            Self::Line => "Line",
            Self::Polyline => "Polyline",
            Self::Curve => "Curve",
            Self::Arc => "Arc",
            Self::Freehand => "Freehand",
            Self::SmoothFreehand => "Smooth Freehand",
            Self::Knife => "Knife",
            Self::Eraser => "Eraser",
            Self::Rect => "Rectangle",
            Self::RoundedRect => "Rounded Rectangle",
            Self::Ellipse => "Ellipse",
            Self::Polygon => "Polygon",
            Self::Star => "Star",
            Self::Text => "Text",
            Self::TraceImage => "Trace Image",
            Self::Hand => "Hand",
            Self::Zoom => "Zoom",
        }
    }

    /// The click-to-place tools (Silhouette's Polyline and Curved Shape tools).
    pub fn click_kind(self) -> Option<click_draw::ClickKind> {
        match self {
            Self::Polyline => Some(click_draw::ClickKind::Polyline),
            Self::Curve => Some(click_draw::ClickKind::Curve),
            _ => None,
        }
    }

    pub fn is_shape_creator(self) -> bool {
        matches!(
            self,
            Self::Line
                | Self::Rect
                | Self::RoundedRect
                | Self::Ellipse
                | Self::Polygon
                | Self::Star
        )
    }
}

fn build_tool_slots() -> Vec<ToolSlot<ActiveTool>> {
    // Each tool's rail icon is its `ActiveTool::icon`, as before.
    let tool = |t: ActiveTool, label: &str| RailTool::new(t, t.icon(), label);
    vec![
        ToolSlot::new(
            0,
            vec![
                tool(ActiveTool::Select, "Select").shortcut("V"),
                tool(ActiveTool::DirectSelect, "Direct Select (Nodes)").shortcut("A"),
                tool(ActiveTool::ConvertPoint, "Convert Anchor Point").shortcut("X"),
            ],
        ),
        ToolSlot::new(
            1,
            vec![
                tool(ActiveTool::BoxSelect, "Box Select").shortcut("M"),
                tool(ActiveTool::LassoSelect, "Lasso Select").shortcut("L"),
            ],
        ),
        ToolSlot::new(
            2,
            vec![
                tool(ActiveTool::Pen, "Pen (Draw Path)").shortcut("P"),
                tool(ActiveTool::Line, "Line").shortcut("\\"),
                tool(ActiveTool::Polyline, "Polyline"),
                tool(ActiveTool::Curve, "Curve"),
                tool(ActiveTool::Arc, "Arc"),
            ],
        ),
        ToolSlot::new(
            3,
            vec![
                tool(ActiveTool::Rect, "Rectangle").shortcut("R"),
                tool(ActiveTool::RoundedRect, "Rounded Rectangle").shortcut("U"),
                tool(ActiveTool::Ellipse, "Ellipse").shortcut("O"),
                tool(ActiveTool::Polygon, "Polygon").shortcut("G"),
                tool(ActiveTool::Star, "Star").shortcut("S"),
            ],
        ),
        ToolSlot::new(
            7,
            vec![
                tool(ActiveTool::Freehand, "Freehand").shortcut("N"),
                tool(ActiveTool::SmoothFreehand, "Smooth Freehand"),
            ],
        ),
        ToolSlot::new(8, vec![tool(ActiveTool::Eraser, "Eraser").shortcut("E")]),
        ToolSlot::new(9, vec![tool(ActiveTool::Knife, "Knife").shortcut("K")]),
        ToolSlot::new(
            4,
            vec![tool(ActiveTool::Text, "Text Lettering").shortcut("T")],
        ),
        ToolSlot::new(
            5,
            vec![tool(ActiveTool::TraceImage, "Trace Image Silhouette").shortcut("I")],
        ),
        ToolSlot::new(
            6,
            vec![
                tool(ActiveTool::Hand, "Hand (Pan)").shortcut("H"),
                tool(ActiveTool::Zoom, "Zoom").shortcut("Z"),
            ],
        ),
    ]
}

/// Menu items in the integrated window title bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopMenuAction {
    NewDoc,
    NewFromTemplate,
    OpenDoc,
    OpenRecent(usize),
    SaveDoc,
    SaveAsDoc,
    ImportSvg,
    ImportImage,
    ExportSvg,
    ExportPrintCutSvg,
    ExportDxf,
    ExportGcode,
    ExportTiledSvg,
    Undo,
    Redo,
    Duplicate,
    Cut,
    Copy,
    Paste(stencilcraft_engine::document::PasteMode),
    Arrange(stencilcraft_engine::document::ArrangeOrder),
    Group,
    Ungroup,
    Delete,
    SelectAll,
    Deselect,
    Pathfinder(BooleanOperation),
    Align(Alignment),
    Distribute(DistributionAxis),
    /// Rotate the selection by whole degrees (positive is clockwise on screen).
    RotateBy(i16),
    /// Mirror the selection in place: `Horizontal` swaps left and right.
    Flip(DistributionAxis),
    /// Align the selection to the last-selected shape, which stays still.
    AlignToKey(Alignment),
    TransformAgain,
    CenterToPage,
    SetStyle(CutStyle),
    PlaceText,
    EditText,
    /// Type › Create Outlines: selected text becomes plain paths.
    TextToOutlines,
    /// Type › Ungroup to Letters: each selected text splits into one text per letter.
    TextUngroupLetters,
    /// Select › Next Object Above/Below: step the selection through the stacking order.
    SelectNextObject {
        above: bool,
    },
    SelectInverse,
    SelectSameStyle,
    SelectSameLayer,
    ShowDesign,
    ShowSend,
    ShowPanel(WorkspacePanel),
    DirectSelectTool,
    DryRun,
    Quit,
    /// A planned item with no command yet; its menu row is disabled.
    NotYet,
    /// Replicate, offset, path and lock/hide actions (`panels.rs`).
    Edit(panels::EditAction),
    FitView,
    CenterView,
    ActualSize,
    ZoomIn,
    ZoomOut,
    /// View › Zoom › Selection: fit the selection's bounds.
    ZoomSelection,
    /// View › Zoom presets: 100 is one screen point per millimetre, as the status bar shows.
    ZoomPercent(u16),
    /// View › Rotate Page: clockwise quarter turns of the view (0–3).
    RotateView(u8),
    ToggleRulers,
    ToggleGrid,
    ToggleSnapGrid,
    ToggleSmartSnapping,
    ToggleSnapPoint,
    ToggleSmartGuides,
    ToggleShowGuides,
    ToggleLockGuides,
    MakeGuides,
    ReleaseGuides,
    ClearGuides,
    ToggleKerfPreview,
    /// File › Import › DXF/PDF/AI-EPS.
    ImportVector(file_menu::ImportKind),
    /// File › Export › PDF/PNG/JPEG.
    Export(file_menu::ExportKind),
    SaveSelectionAs,
    /// Machine › Connect / Rescan, Test Cut and Cut (run from the Send tab).
    Machine(machine_menu::MachineAction),
    /// Machine › Mat Preview and Mirror on Mat.
    ToggleMatPreview,
    /// Object › Distort › Free Distort (1 × 1) and Effect › Lattice (a grid).
    BeginDistort {
        rows: u32,
        columns: u32,
    },
    /// Machine › Send Mode.
    SetSendMode(send_modes::SendMode),
    /// Machine › Cancel: stop a send before its next job.
    CancelSend,
    /// Panels › Starter Mode.
    ToggleStarterMode,
    ToggleMirror,
    /// Machine › Load Type: on a mat (`true`) or without one.
    LoadOnMat(bool),
    /// Type › Type on a Path / Curve Text.
    TypeOnPath,
    CurveText,
    /// File › Print: the page as a PDF in the system viewer.
    Print,
    /// File › Close Tab: back to a new, empty document (asking before discarding changes).
    CloseDoc,
    /// Object › Arrange › Send to Layer.
    SendToLayer(usize),
    /// Object › Attach / Detach.
    ToggleAttach,
    /// Object › Flatten.
    Flatten,
    /// Object › Clipping Mask › Make / Release.
    ClipMake,
    ClipRelease,
    /// An Effect menu item: the Effects panel on that effect.
    OpenEffect(effects::EffectMenu),
    /// Effect › Apply Last Effect.
    ApplyLastEffect,
    ToggleOutlineView,
    ToggleCutBorder,
    TogglePrintBorder,
    ToggleDimensions,
    TogglePrintCutMarks,
    ResetWorkspace,
    SetTheme(stencilcraft_ui_egui::ColorTheme),
    HelpManual,
    HelpCheatsheet,
    HelpAbout,
}

fn apply_edit<T: Clone>(
    value: &mut T,
    before: &mut Option<T>,
    events: &craft_behavior::EditEvents<T>,
) -> bool {
    let mut changed = false;
    for event in events.iter() {
        match event {
            EditEvent::Begin { .. } => *before = Some(value.clone()),
            EditEvent::Update { value: next, .. } => {
                *value = next.clone();
                changed = true;
            }
            EditEvent::Set { after, .. } | EditEvent::Commit { after, .. } => {
                *value = after.clone();
                *before = None;
                changed = true;
            }
            EditEvent::Cancel { reason, .. } => {
                // The old `External` (the app changed the value) is the pin's document-moved reasons.
                if !reason.document_moved()
                    && let Some(original) = before.take()
                {
                    *value = original;
                    changed = true;
                }
                *before = None;
            }
            _ => {}
        }
    }
    changed
}

/// The eye column of the Layers panel's lists.
const EYE_COLUMN: craft_behavior::ColumnKey = craft_behavior::ColumnKey::new("eye");
/// The lock column of the Layers panel's lists.
const LOCK_COLUMN: craft_behavior::ColumnKey = craft_behavior::ColumnKey::new("lock");
/// The name column of the Layers panel's lists.
const NAME_COLUMN: craft_behavior::ColumnKey = craft_behavior::ColumnKey::new("name");

/// A Layers panel list's columns: the eye and the lock toggles, then the name (the old rows' order).
fn toggle_columns() -> Vec<ColumnSpec> {
    vec![
        ColumnSpec::toggle(EYE_COLUMN, "Visibility", ToggleColumn::default()),
        ColumnSpec::toggle(LOCK_COLUMN, "Lock", ToggleColumn::default()),
        ColumnSpec::new(NAME_COLUMN, "Name", craft_behavior::ColumnWidth::fill(1.0)),
    ]
}

/// The toggle cells a list committed or set this pass: item, column and the new state (the old rows'
/// `RowEvent::Toggle` Commit and Set).
fn toggle_commits<K: Clone>(
    events: &[CollectionEvent<K, craft_ui::list::RowEvent>],
) -> Vec<(K, craft_behavior::ColumnKey, bool)> {
    events
        .iter()
        .filter_map(|event| match event {
            CollectionEvent::Toggle(
                EditEvent::Commit { after, .. } | EditEvent::Set { after, .. },
            ) => Some((after.key.clone(), after.column.clone(), after.on)),
            _ => None,
        })
        .collect()
}

/// A Layers panel footer button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LayerFooter {
    Add,
    Delete,
    /// Move the layer at the first index to the second.
    Move(usize, usize),
}

/// The value a number field's events propose this pass (Update, Commit or Set), as the old field's
/// `event.value()`; `None` for an empty or non-finite one.
fn live_number(events: &craft_behavior::EditEvents<Option<f64>>) -> Option<f64> {
    events.value().copied().flatten().filter(|v| v.is_finite())
}

/// The value a number field committed or set this pass; `None` for an empty or non-finite one.
fn committed_number(events: &craft_behavior::EditEvents<Option<f64>>) -> Option<f64> {
    events
        .committed()
        .copied()
        .flatten()
        .filter(|v| v.is_finite())
}

/// Display label for a media preset index (parallel to `MEDIA_PRESETS`).
fn media_label(index: usize) -> String {
    match stencilcraft_machine::MEDIA_PRESETS.get(index) {
        Some(m) => format!("{} (ID {})", m.name, m.id),
        None => "Custom (ID 300)".to_string(),
    }
}

/// Mat chooser entries: (device mat, label).
pub const MAT_CHOICES: &[(stencilcraft_machine::Mat, &str)] = &[
    (stencilcraft_machine::Mat::None, "no mat"),
    (stencilcraft_machine::Mat::Cameo12x12, "12×12"),
    (stencilcraft_machine::Mat::Cameo12x24, "12×24"),
    (stencilcraft_machine::Mat::Portrait8x12, "8×12 portrait"),
    (stencilcraft_machine::Mat::CameoPlus15x15, "15×15 plus"),
    (stencilcraft_machine::Mat::CameoPro24x24, "24×24 Pro"),
    (stencilcraft_machine::Mat::Portrait8_5x12, "8.5×12 portrait"),
];

fn mat_supported(
    caps: Option<&stencilcraft_machine::Capabilities>,
    mat: stencilcraft_machine::Mat,
) -> bool {
    mat == stencilcraft_machine::Mat::None || caps.is_some_and(|c| c.mats.contains(&mat))
}

pub fn mat_label(mat: stencilcraft_machine::Mat) -> &'static str {
    MAT_CHOICES
        .iter()
        .find(|(m, _)| *m == mat)
        .map(|(_, label)| *label)
        .unwrap_or("no mat")
}

/// Machine chooser entries: (device model, label). Most specific first.
pub const MODEL_CHOICES: &[(stencilcraft_machine::Model, &str)] = &[
    (stencilcraft_machine::Model::Cameo4Pro, "Cameo 4 Pro"),
    (stencilcraft_machine::Model::Cameo4Plus, "Cameo 4 Plus"),
    (stencilcraft_machine::Model::Cameo4, "Cameo 4"),
    (stencilcraft_machine::Model::Cameo5, "Cameo 5"),
    (stencilcraft_machine::Model::Cameo5Plus, "Cameo 5 Plus"),
    (stencilcraft_machine::Model::CameoProMk2, "Cameo Pro MK-II"),
    (stencilcraft_machine::Model::Cameo3, "Cameo 3"),
    (stencilcraft_machine::Model::Portrait4, "Portrait 4"),
    (stencilcraft_machine::Model::Portrait3, "Portrait 3"),
];

pub fn model_label(model: stencilcraft_machine::Model) -> &'static str {
    MODEL_CHOICES
        .iter()
        .find(|(m, _)| *m == model)
        .map(|(_, label)| *label)
        .unwrap_or("Cameo 4 Pro")
}

/// The canvas backdrop for the currently selected mat.
pub fn mat_backdrop(mat: stencilcraft_machine::Mat) -> stencilcraft_ui_egui::MatBackdrop {
    let (down_in, right_in) = mat.extent_inch();
    stencilcraft_ui_egui::MatBackdrop {
        width_mm: right_in * 25.4,
        height_mm: down_in * 25.4,
        label: mat_label(mat).to_string(),
    }
}

/// Flatten every visible shape into machine paths (document mm coordinates:
/// x right, y down), closing closed polylines.
fn machine_paths(doc: &stencilcraft_doc::Document) -> Vec<Vec<(f64, f64)>> {
    let mut paths = Vec::new();
    for layer in &doc.layers {
        if !layer.visible {
            continue;
        }
        for shape in &layer.shapes {
            for pl in stencilcraft_geom::path_to_polylines(
                &shape.path,
                stencilcraft_geom::DEFAULT_FLATTEN_STEPS,
            ) {
                if pl.points.len() < 2 {
                    continue;
                }
                let mut path: Vec<(f64, f64)> = pl.points.iter().map(|p| (p.x, p.y)).collect();
                if pl.closed
                    && let Some(first) = pl.points.first()
                {
                    path.push((first.x, first.y));
                }
                paths.push(path);
            }
        }
    }
    paths
}

fn main() -> Result<(), String> {
    let mut control_port: Option<u16> = None;
    let mut open_path: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--control" => {
                let port = args.next().ok_or("--control needs a port".to_string())?;
                control_port = Some(
                    port.parse()
                        .map_err(|_| "--control needs a numeric port".to_string())?,
                );
            }
            "-h" | "--help" => {
                println!("stencilcraft — desktop app");
                println!(
                    "usage: stencilcraft [--control PORT] [FILE.{}]",
                    stencilcraft_doc::FORMAT_EXTENSION
                );
                return Ok(());
            }
            other => open_path = Some(other.to_string()),
        }
    }
    let decision = craft_window::policy::decide_for(
        craft_window::Prefix::new("STENCILCRAFT"),
        craft_window::policy::Backend::from_env(),
        craft_window::policy::TitleBarPref::Integrated,
    );

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1120.0, 760.0])
        .with_title("StencilCraft");

    if let Ok(rgba) = craft_raster_codecs::common::decode_rgba(
        APP_ICON_PNG,
        craft_raster_codecs::common::DecodeLimits::default(),
    ) {
        let (w, h) = rgba.dimensions();
        viewport = viewport.with_icon(egui::IconData {
            rgba: rgba.into_raw(),
            width: w,
            height: h,
        });
    }

    let viewport = craft_window::policy::apply(viewport, &decision.frame);

    let mut app = StencilApp::new(open_path.as_deref());
    let install_frame = decision;
    app.control_port = control_port;
    if open_path.is_none() {
        app.open_new_document(0);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "stencilcraft",
        options,
        Box::new(move |cc| {
            framework::install(&cc.egui_ctx, install_frame)?;
            Ok(Box::new(app.with_egui(cc)))
        }),
    )
    .map_err(|e| format!("stencilcraft: {e}"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WorkspacePanel {
    Canvas,
    Tools,
    Inspector,
    Layers,
    Transform,
    Replicate,
    Offset,
    Modify,
    Points,
    Trace,
    TextStyle,
    PageSetup,
    Effects,
    Contour,
    Library,
    Glyphs,
    FillColor,
    LineStyle,
    SelectByColor,
    History,
    Preferences,
    Selections,
    Distort,
    GraphicStyles,
    Actions,
    PixScan,
    PrintCut,
}

struct WorkspaceContent<'a> {
    app: &'a mut StencilApp,
}

/// What the dock reads of each panel: its tab label, the canvas as the document group, and
/// every body the app's own egui code.
#[derive(Clone, Copy, Debug, Default, PartialEq, craft_ui::contract::PlainData)]
struct WorkspaceSpecs;

impl PanelSpecs<WorkspacePanel> for WorkspaceSpecs {
    fn spec(&self, panel: &WorkspacePanel) -> PanelSpec {
        let spec = PanelSpec::new(panel_label(*panel)).host_body();
        if *panel == WorkspacePanel::Canvas {
            spec.canvas().window_kind(WindowKind::Document)
        } else {
            spec
        }
    }

    fn permissions(&self, panel: &WorkspacePanel) -> Permissions {
        panel_permissions(panel)
    }
}

fn panel_permissions(panel: &WorkspacePanel) -> Permissions {
    if *panel == WorkspacePanel::Canvas {
        Permissions::PROTECTED
    } else {
        Permissions::default()
    }
}

impl DockPanels<WorkspacePanel> for WorkspaceContent<'_> {
    /// The canvas group is the app's canvas, drawn in the group's room with no tab strip.
    fn group_view(
        &mut self,
        panels: &[WorkspacePanel],
        _active: usize,
        ui: &mut egui::Ui,
        at: GroupAt,
        _cx: &mut BodyCx<'_>,
    ) -> Option<f32> {
        if !panels.contains(&WorkspacePanel::Canvas) {
            return None;
        }
        match at {
            GroupAt::Measure(_) => Some(0.0),
            GroupAt::Show(rect) => {
                framework::app_style(ui);
                self.app.canvas_body(ui);
                Some(rect.height())
            }
        }
    }

    fn body(&mut self, panel: &WorkspacePanel, ui: &mut egui::Ui, _cx: &mut BodyCx<'_>) {
        match panel {
            WorkspacePanel::Canvas => self.app.canvas_body(ui),
            WorkspacePanel::Tools => self.app.tools_panel(ui),
            WorkspacePanel::Inspector => self.app.inspector_panel(ui),
            WorkspacePanel::Layers => self.app.layers_panel(ui),
            WorkspacePanel::Transform => self.app.transform_panel(ui),
            WorkspacePanel::Replicate => self.app.replicate_panel(ui),
            WorkspacePanel::Offset => self.app.offset_panel(ui),
            WorkspacePanel::Modify => self.app.modify_panel(ui),
            WorkspacePanel::Points => self.app.points_panel(ui),
            WorkspacePanel::Trace => self.app.trace_panel(ui),
            WorkspacePanel::TextStyle => self.app.text_style_panel(ui),
            WorkspacePanel::PageSetup => self.app.page_setup_panel(ui),
            WorkspacePanel::Effects => self.app.effects_panel(ui),
            WorkspacePanel::Contour => self.app.contour_panel(ui),
            WorkspacePanel::Library => self.app.library_panel(ui),
            WorkspacePanel::Glyphs => self.app.glyphs_panel(ui),
            WorkspacePanel::FillColor => self.app.fill_colour_panel(ui),
            WorkspacePanel::LineStyle => self.app.line_style_panel(ui),
            WorkspacePanel::SelectByColor => self.app.select_by_colour_panel(ui),
            WorkspacePanel::History => self.app.history_panel(ui),
            WorkspacePanel::Preferences => self.app.preferences_panel(ui),
            WorkspacePanel::Selections => self.app.selections_panel(ui),
            WorkspacePanel::Distort => self.app.distort_panel(ui),
            WorkspacePanel::GraphicStyles => self.app.graphic_styles_panel(ui),
            WorkspacePanel::Actions => self.app.actions_panel(ui),
            WorkspacePanel::PixScan => self.app.pixscan_panel(ui),
            WorkspacePanel::PrintCut => self.app.print_cut_panel(ui),
        }
    }
}

/// A panel's tab label.
fn panel_label(panel: WorkspacePanel) -> &'static str {
    match panel {
        WorkspacePanel::Canvas => "Canvas",
        WorkspacePanel::Tools => "Tools",
        WorkspacePanel::Inspector => "Properties",
        WorkspacePanel::Layers => "Layers",
        WorkspacePanel::Transform => "Transform",
        WorkspacePanel::Replicate => "Replicate",
        WorkspacePanel::Offset => "Offset",
        WorkspacePanel::Modify => "Modify",
        WorkspacePanel::Points => "Point Editing",
        WorkspacePanel::Trace => "Trace",
        WorkspacePanel::TextStyle => "Text Style",
        WorkspacePanel::PageSetup => "Page Setup",
        WorkspacePanel::Effects => "Effects",
        WorkspacePanel::Contour => "Contour",
        WorkspacePanel::Library => "Library",
        WorkspacePanel::Glyphs => "Glyphs",
        WorkspacePanel::FillColor => "Fill Color",
        WorkspacePanel::LineStyle => "Line Style",
        WorkspacePanel::SelectByColor => "Select by Color",
        WorkspacePanel::History => "History",
        WorkspacePanel::Preferences => "Preferences",
        WorkspacePanel::Selections => "Selections",
        WorkspacePanel::Distort => "Distort",
        WorkspacePanel::GraphicStyles => "Graphic Styles",
        WorkspacePanel::Actions => "Actions",
        WorkspacePanel::PixScan => "PixScan",
        WorkspacePanel::PrintCut => "Print & Cut",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanvasViewCommand {
    Fit,
    Center,
    /// Fit the selection's bounds (does nothing without a selection).
    Selection,
    /// Zoom to a percentage about the canvas centre.
    ZoomPercent(u16),
    /// Turn the view to this many clockwise quarter turns about the canvas centre.
    Rotate(u8),
}

pub struct StencilApp {
    /// The panel arrangement the dock shows and edits.
    pub workspace: Arc<Workspace<WorkspacePanel>>,
    /// The dock's panel specs; one value across frames, so the dock reads them once.
    panel_specs: Tracked<WorkspaceSpecs>,
    pub engine: stencilcraft_engine::Engine,
    pub view: stencilcraft_ui_egui::View,
    pub pending_view_command: Option<CanvasViewCommand>,
    /// Apply status-bar zoom changes at the canvas center once its bounds are known.
    pub pending_zoom: Option<f32>,
    pub kerf_preview: bool,
    pub kerf_preview_cache: Option<KerfPreviewCache>,
    pub kerf_draft: Option<f64>,
    pub pressure_before: Option<f64>,
    pub speed_before: Option<f64>,
    pub zoom_before: Option<f64>,
    /// A ruler guide being dragged on the canvas: its key and where it is now.
    pub guide_preview: Option<stencilcraft_ui_egui::GuidePreview>,
    /// The selection's bounds when a move drag began, so snapping works from the start.
    pub drag_origin: Option<stencilcraft_geom::Rect>,
    /// Smart guides: the lines the last snap of a move drag found, in millimetres.
    pub snap_lines: Vec<(stencilcraft_geom::Point, stencilcraft_geom::Point)>,
    /// Holding Ctrl (Cmd on macOS) suspends snapping for this frame.
    pub snap_suspended: bool,
    /// This pass the app's canvas widget only hovers: the pointer is over a framework canvas
    /// control's part (the node overlay, the selection box, a guide).
    pub canvas_hover_only: bool,
    pub status: String,
    pub open_path: Option<String>,
    pub send_mode: bool,
    pub theme: stencilcraft_ui_egui::ColorTheme,
    pub view_initialized: bool,
    /// Board size the canvas was last framed for, millimetres.
    pub last_board: Option<(f64, f64)>,
    pub recent_files: Vec<String>,
    pub mat: stencilcraft_machine::Mat,
    pub selected_shape: Option<u64>,
    pub selected_shapes: Vec<u64>,
    pub selected_layer: Option<usize>,
    /// File › Quit was chosen; the next frame closes the window.
    pub quit_requested: bool,
    /// File › New's dialog.
    pub new_doc: new_doc::NewDocState,
    saved_document: stencilcraft_doc::Document,
    pending_new_document: Option<new_doc::Choice>,
    /// Name of an unsaved document (New Document's Name), shown in the title bar.
    pub doc_title: String,
    /// Font family and size for new text and the Text Style panel.
    pub text_family: String,
    pub text_size: f64,
    pub text_fonts: Vec<stencilcraft_engine::text::FontFamily>,
    /// The rest of the Text Style panel's settings.
    pub text_style: text_tool::TextStyle,
    /// Text being typed on the canvas.
    pub text_session: Option<text_tool::TextSession>,
    /// A panel to bring forward once the dock has drawn (the canvas draws inside the dock,
    /// while its layout cannot change).
    pub pending_panel: Option<WorkspacePanel>,
    /// The Effects panel and its live preview.
    pub effects: effects::EffectsState,
    pub effect_preview: Option<effects::EffectPreview>,
    /// Recovery snapshots and the design library.
    pub stores: library_panel::FileStores,
    /// The colour panels' hex field, and a line style picked there (applied after drawing).
    pub colour_hex_draft: String,
    /// Line Style › Perforate: cut and gap lengths (mm).
    pub perforate: (f64, f64),
    /// Select by Color: the colour row whose recolour palette is open.
    pub recolor_row: Option<usize>,
    /// A Machine-menu hardware action waiting for the Send tab.
    pub machine_action: Option<machine_menu::MachineAction>,
    /// Select › Save Selection: named selections for this session.
    pub saved_selections: Vec<misc_panels::SavedSelection>,
    /// Graphic Styles panel: the name field.
    pub style_name_draft: String,
    /// Actions panel: the name field.
    pub action_name_draft: String,
    /// Free Distort / Lattice in progress.
    pub distort: Option<distort_tool::DistortDraft>,
    /// PixScan: corners picked on a photo of the page.
    pub pixscan: pixscan::PixScanState,
    pub selection_name_draft: String,
    /// The mat before Load Type › Without Mat / Roll, for On Mat.
    pub mat_before_unload: Option<stencilcraft_machine::Mat>,
    pub pending_style: Option<CutStyle>,
    pub pending_perforate: bool,
    pub canvas_drag_shape: Option<u64>,
    pub canvas_selection_start: Option<egui::Pos2>,
    pub canvas_selection_additive: bool,
    /// The selection before a Shift marquee started, so the marquee can shrink back.
    pub canvas_selection_base: Vec<u64>,
    /// Moves applied while dragging the selection, folded into one undo step on release.
    pub drag_steps: usize,
    pub canvas_selection_points: Vec<egui::Pos2>,
    pub canvas_panning: bool,
    pub last_drag_delta: egui::Vec2,
    pub send: SendTab,
    pub active_tool: ActiveTool,
    pub rail_last_used: LastUsed<ActiveTool>,
    pub path_selection: PathSelection,
    pub bezier_gesture_base: Option<(u64, Vec<EditSubpath>)>,
    pub bezier_live_path: Option<(u64, stencilcraft_geom::BezPath)>,
    pub handles_shown: HandlesShown,
    pub pen_draft: Option<EditSubpath>,
    /// The Pen tool on the canvas's tool host; it keeps its run between frames.
    pub pen_tool: craft_draw::ui::pen::PenTool,
    /// Set by Cmd+Z while a pen path is being drawn: the pen drops its last anchor this frame
    /// instead of the document undoing its last command.
    pub pen_undo_requested: bool,
    /// The Polyline/Curve tool's points while placing them.
    pub click_draft: Option<click_draw::ClickDraft>,
    /// Uploaded textures of placed images, keyed by image id with the pixel data they show.
    pub image_textures: std::collections::HashMap<u64, (usize, egui::TextureHandle)>,
    /// A placed image picked on the canvas (images select on their own, as in Silhouette).
    pub selected_image: Option<u64>,
    /// An image being dragged: its id and the press point.
    pub image_drag: Option<(u64, egui::Pos2)>,
    /// The Trace panel's live preview.
    pub trace_preview: Option<trace_panel::TracePreview>,
    /// Where a trace-area drag started (millimetres).
    pub trace_area_drag: Option<stencilcraft_geom::Point>,
    /// A drag on the trace box: what it grabs, the box before and the press point.
    pub trace_area_grab: Option<(trace_panel::AreaGrab, [f64; 4], stencilcraft_geom::Point)>,
    /// The selection box part under the pointer, which picks how a drag of it scales.
    pub box_part: Option<craft_behavior::geometry::quad::BoxPart>,
    /// The transform a selection-box drag would apply, previewed until release.
    pub box_preview: Option<craft_behavior::kurbo::Affine>,
    /// The Arc tool after its first drag: centre and radius are set, the sweep follows the pointer.
    pub arc_draft: Option<arc_draw::ArcDraft>,
    /// The Freehand/Smooth Freehand stroke while the pointer is down.
    pub freehand_stroke: Option<freehand::Stroke>,
    pub shape_drag_start: Option<stencilcraft_geom::Point>,
    pub shape_drag_current: Option<stencilcraft_geom::Point>,
    pub ruler_origin: stencilcraft_geom::Point,
    pub locale_numbers: LocaleNumbers,
    pub prefs: UserPrefs,
    #[cfg(not(target_arch = "wasm32"))]
    pub settings_store: Option<SettingsStore<UserPrefs>>,
    /// Port requested with `--control`; the socket starts once eframe hands us the
    /// paint context, so requests can wake the window.
    pub control_port: Option<u16>,
    /// Queued control requests, handled on the UI thread against `engine`.
    pub control_jobs: Option<control::Jobs>,
    /// Settings of the Transform, Replicate, Offset, Modify and Point Editing panels.
    pub panels: panels::PanelState,
}

#[derive(Clone, Debug)]
pub struct KerfPreviewCache {
    pub source: stencilcraft_doc::Document,
    pub kerf_width_mm: f64,
    pub preview: Result<stencilcraft_doc::Document, String>,
}

type HardwareResult =
    std::sync::Arc<std::sync::Mutex<Option<Result<stencilcraft_engine::Effect, String>>>>;

#[derive(Debug, Clone)]
pub struct SendTab {
    pub media_index: usize,
    pub model: stencilcraft_machine::Model,
    pub pressure_override: u8,
    pub speed_override: u8,
    pub bbox_only: bool,
    pub set_blade_depth: bool,
    pub blade_depth: u8,
    pub draw: bool,
    pub tool: Option<stencilcraft_machine::ToolKind>,
    pub holder: u8,
    pub passes: u8,
    pub mirror: bool,
    pub summary: Option<Vec<String>>,
    pub devices: Vec<(String, String, String)>,
    pub device: String,
    pub connection_mode: u8,
    pub bluetooth_address: String,
    pub bluetooth_channel: u8,
    pub hardware_result: HardwareResult,
    pub busy: bool,
    /// Machine › Cancel: stops a multi-job send before its next job.
    pub cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Machine › Send Mode: one set of settings, or a row per line colour, fill colour or
    /// layer.
    pub mode: send_modes::SendMode,
    pub rows: Vec<send_modes::SendRow>,
    /// Advanced: path order (`roller`, `speed`, `none`), interior contours first, closed
    /// overcut (mm), weld on cut and roller track enhancing.
    pub order: String,
    /// My materials: the chosen saved material and the name field for saving one.
    pub custom_material: Option<usize>,
    pub custom_name: String,
    pub interior_first: bool,
    pub overcut_mm: f64,
    pub weld: bool,
    pub track_enhancing: bool,
}

impl Default for SendTab {
    fn default() -> Self {
        SendTab {
            media_index: machine_index_of_media(102),
            model: stencilcraft_machine::Model::Cameo4Pro,
            pressure_override: 10,
            speed_override: 5,
            bbox_only: false,
            set_blade_depth: false,
            blade_depth: 1,
            draw: false,
            tool: None,
            holder: 1,
            passes: 1,
            mirror: false,
            summary: None,
            devices: Vec::new(),
            device: String::new(),
            connection_mode: 0,
            bluetooth_address: String::new(),
            bluetooth_channel: 1,
            hardware_result: Default::default(),
            busy: false,
            cancel: Default::default(),
            mode: send_modes::SendMode::Standard,
            rows: Vec::new(),
            order: "roller".to_string(),
            custom_material: None,
            custom_name: String::new(),
            interior_first: true,
            overcut_mm: 0.0,
            weld: false,
            track_enhancing: false,
        }
    }
}

pub fn default_workspace() -> Arc<Workspace<WorkspacePanel>> {
    workspace_with(FULL_PANELS.to_vec())
}

/// Silhouette's right-hand panel strip, in full.
const FULL_PANELS: &[WorkspacePanel] = &[
    WorkspacePanel::Transform,
    WorkspacePanel::Replicate,
    WorkspacePanel::Offset,
    WorkspacePanel::Modify,
    WorkspacePanel::Points,
    WorkspacePanel::Trace,
    WorkspacePanel::TextStyle,
    WorkspacePanel::PageSetup,
    WorkspacePanel::Effects,
    WorkspacePanel::Contour,
    WorkspacePanel::Library,
    WorkspacePanel::Glyphs,
    WorkspacePanel::FillColor,
    WorkspacePanel::LineStyle,
    WorkspacePanel::SelectByColor,
    WorkspacePanel::History,
    WorkspacePanel::Preferences,
    WorkspacePanel::Selections,
    WorkspacePanel::Distort,
    WorkspacePanel::GraphicStyles,
    WorkspacePanel::Actions,
    WorkspacePanel::PixScan,
    WorkspacePanel::PrintCut,
];

/// Starter Mode's few panels, as in Silhouette (Page Setup, Fill, Line Style, Trace,
/// Transform, Replicate, Text Style).
const STARTER_PANELS: &[WorkspacePanel] = &[
    WorkspacePanel::PageSetup,
    WorkspacePanel::FillColor,
    WorkspacePanel::LineStyle,
    WorkspacePanel::Trace,
    WorkspacePanel::Transform,
    WorkspacePanel::Replicate,
    WorkspacePanel::TextStyle,
];

pub fn starter_workspace() -> Arc<Workspace<WorkspacePanel>> {
    workspace_with(STARTER_PANELS.to_vec())
}

fn workspace_with(strip: Vec<WorkspacePanel>) -> Arc<Workspace<WorkspacePanel>> {
    let root = Node::split(
        Axis::X,
        SplitSize::Ratio(0.78),
        Node::tabs(vec![WorkspacePanel::Canvas], 0),
        Node::dock(
            DockEdge::End,
            vec![
                Column::new(vec![
                    Group::new(vec![WorkspacePanel::Inspector, WorkspacePanel::Layers], 0),
                    // Silhouette's right-hand panel strip.
                    Group::new(strip, 0),
                ])
                .with_width(300.0),
            ],
        ),
    );
    Arc::new(Workspace::new(Layout::new(Some(root), Vec::new())))
}

fn machine_index_of_media(id: u16) -> usize {
    stencilcraft_machine::MEDIA_PRESETS
        .iter()
        .position(|m| m.id == id)
        .unwrap_or(0)
}

impl StencilApp {
    pub fn new(open_path: Option<&str>) -> Self {
        let mut engine = stencilcraft_engine::Engine::default();
        let mut status = String::from("ready");
        let open_path = open_path.and_then(|path| match stencilcraft_doc::load_from(path) {
            Ok(doc) => {
                engine = stencilcraft_engine::Engine::new(doc);
                status = format!("opened {path}");
                Some(path.to_string())
            }
            Err(e) => {
                status = format!("could not open: {e}");
                None
            }
        });
        let prefs = UserPrefs::default();
        let theme = if prefs.dark_theme {
            stencilcraft_ui_egui::ColorTheme::Dark
        } else {
            stencilcraft_ui_egui::ColorTheme::Light
        };
        let saved_document = engine.doc.clone();
        Self {
            workspace: default_workspace(),
            panel_specs: Tracked::new(WorkspaceSpecs),
            engine,
            view: stencilcraft_ui_egui::View::default(),
            pending_view_command: None,
            pending_zoom: None,
            kerf_preview: false,
            kerf_preview_cache: None,
            kerf_draft: None,
            pressure_before: None,
            speed_before: None,
            zoom_before: None,
            guide_preview: None,
            drag_origin: None,
            snap_lines: Vec::new(),
            snap_suspended: false,
            canvas_hover_only: false,
            status,
            open_path: open_path.clone(),
            send_mode: false,
            theme,
            view_initialized: false,
            last_board: None,
            recent_files: open_path.iter().cloned().collect(),
            mat: stencilcraft_machine::Mat::CameoPro24x24,
            selected_shape: None,
            selected_shapes: Vec::new(),
            selected_layer: Some(0),
            quit_requested: false,
            new_doc: new_doc::NewDocState::default(),
            saved_document,
            pending_new_document: None,
            doc_title: "Untitled".to_string(),
            text_family: String::new(),
            text_size: text_tool::DEFAULT_TEXT_SIZE_PT,
            text_fonts: Vec::new(),
            text_style: text_tool::TextStyle::default(),
            text_session: None,
            pending_panel: None,
            effects: effects::EffectsState::default(),
            effect_preview: None,
            stores: library_panel::FileStores::default(),
            colour_hex_draft: String::new(),
            perforate: (3.0, 1.0),
            recolor_row: None,
            machine_action: None,
            saved_selections: Vec::new(),
            style_name_draft: String::new(),
            action_name_draft: String::new(),
            distort: None,
            pixscan: pixscan::PixScanState::default(),
            selection_name_draft: String::new(),
            mat_before_unload: None,
            pending_style: None,
            pending_perforate: false,
            canvas_drag_shape: None,
            canvas_selection_start: None,
            canvas_selection_additive: false,
            canvas_selection_base: Vec::new(),
            drag_steps: 0,
            canvas_selection_points: Vec::new(),
            canvas_panning: false,
            last_drag_delta: egui::Vec2::ZERO,
            send: SendTab::default(),
            active_tool: ActiveTool::Select,
            rail_last_used: LastUsed::default(),
            path_selection: PathSelection::default(),
            bezier_gesture_base: None,
            bezier_live_path: None,
            handles_shown: HandlesShown::Selected,
            pen_draft: None,
            pen_tool: craft_draw::ui::pen::PenTool::new(),
            pen_undo_requested: false,
            click_draft: None,
            image_textures: std::collections::HashMap::new(),
            selected_image: None,
            image_drag: None,
            trace_preview: None,
            trace_area_drag: None,
            trace_area_grab: None,
            box_part: None,
            box_preview: None,
            arc_draft: None,
            freehand_stroke: None,
            shape_drag_start: None,
            shape_drag_current: None,
            ruler_origin: stencilcraft_geom::Point::ZERO,
            locale_numbers: LocaleNumbers::from_tag("en-US"),
            prefs,
            #[cfg(not(target_arch = "wasm32"))]
            settings_store: None,
            control_port: None,
            control_jobs: None,
            panels: panels::PanelState::default(),
        }
    }

    /// Run queued `--control` requests against the document on screen. The socket
    /// hands work to the UI thread, so an agent edits what the user sees (and the
    /// user can undo it) instead of a private copy of the file.
    fn drain_control_requests(&mut self, ctx: &egui::Context) {
        let Some(jobs) = self.control_jobs.as_ref() else {
            return;
        };
        let mut pending = Vec::new();
        while let Some(job) = jobs.try_recv() {
            pending.push(job);
        }
        for job in pending {
            if let Some(path) = screenshot_request(&job.line) {
                request_screenshot(ctx, path, job);
                continue;
            }
            if let Some((id, ids)) = select_request(&job.line) {
                // Selection is view state, not a document command: the window owns it.
                self.set_selection(ids);
                let reply = serde_json::json!({"id": id, "ok": true, "result": {"selected": self.selected_shapes}});
                let _ = job.reply.send(reply.to_string());
                continue;
            }
            if let Some((id, kerf_preview, theme)) = view_request(&job.line) {
                if let Some(on) = kerf_preview {
                    self.kerf_preview = on;
                }
                if let Some(theme) = theme {
                    self.theme = theme;
                }
                let dark = self.theme == stencilcraft_ui_egui::ColorTheme::Dark;
                let reply = serde_json::json!({"id": id, "ok": true, "result": {
                    "kerf_preview": self.kerf_preview,
                    "theme": if dark { "dark" } else { "light" },
                }});
                let _ = job.reply.send(reply.to_string());
                continue;
            }
            // The session owns the engine while it handles one request, then hands
            // it back: the document, undo history and counters all survive.
            let engine = std::mem::take(&mut self.engine);
            let mut session = stencilcraft_mcp::Session::with_engine(engine);
            let reply = match session.handle_stdio_line(&job.line) {
                Ok(Some(response)) => response,
                Ok(None) => String::new(),
                Err(e) => {
                    self.status = format!("control: {e}");
                    r#"{"id":null,"ok":false,"error":"internal control error"}"#.to_string()
                }
            };
            self.engine = session.into_engine();
            self.retain_valid_selection();
            self.kerf_preview_cache = None;
            if let Some((method, path)) = document_path_request(&job.line, &reply) {
                // An agent opened or saved the visible document: the title, recent files and
                // framing follow it as they do for File › Open and Save.
                if method == "doc.open" {
                    self.set_selection([]);
                    self.view_initialized = false;
                    self.last_board = None;
                }
                self.open_path = Some(path.clone());
                self.saved_document = self.engine.doc.clone();
                self.record_recent_file(path);
            }
            let _ = job.reply.send(reply);
        }
        if let Some(jobs) = self.control_jobs.as_ref() {
            jobs.publish(&self.engine.doc);
        }
    }

    pub fn with_egui(mut self, cc: &eframe::CreationContext) -> Self {
        if let Some(port) = self.control_port {
            let ctx = cc.egui_ctx.clone();
            self.control_jobs = control::serve(port, move || ctx.request_repaint());
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (store, _report) = SettingsStore::<UserPrefs>::open(Location::Auto);
            self.prefs = store.get().clone();
            self.stores = library_panel::FileStores::open();
            self.restore_machine_prefs();
            // The New Document dialog shown at start was opened before the preferences loaded.
            if self.new_doc.open {
                self.new_doc.choice.model = self.send.model;
                self.new_doc.choice.mat = self.mat;
            }
            if self.prefs.starter_mode {
                self.workspace = starter_workspace();
            }
            if !self.stores.recovered.is_empty() {
                self.pending_panel = Some(WorkspacePanel::Library);
                self.status = "recovered designs from an earlier session are in the Library panel"
                    .to_string();
            }
            self.settings_store = Some(store);
            if !self.prefs.recent_files.is_empty() {
                self.recent_files = self.prefs.recent_files.clone();
            }
        }
        if self.recent_files.is_empty() {
            self.recent_files = cc
                .storage
                .and_then(|storage| eframe::get_value(storage, "recent_files"))
                .unwrap_or_default();
        }
        if let Some(path) = self.open_path.clone() {
            self.record_recent_file(path);
        }
        self.theme = if self.prefs.dark_theme {
            stencilcraft_ui_egui::ColorTheme::Dark
        } else {
            stencilcraft_ui_egui::ColorTheme::Light
        };
        self
    }

    pub fn save_prefs(&mut self) {
        self.prefs.recent_files = self.recent_files.clone();
        self.prefs.dark_theme = self.theme == stencilcraft_ui_egui::ColorTheme::Dark;
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(store) = self.settings_store.as_mut() {
            let p = self.prefs.clone();
            store.update(|val| *val = p);
            let _ = store.flush();
        }
    }

    /// Use the cutter model and material saved in the preferences.
    pub fn restore_machine_prefs(&mut self) {
        if let Some(model) = self
            .prefs
            .machine_model
            .as_deref()
            .and_then(stencilcraft_machine::Model::by_id)
        {
            self.send.model = model;
        }
        if let Some(mat) = self
            .prefs
            .mat_id
            .as_deref()
            .and_then(stencilcraft_machine::Mat::parse)
        {
            self.mat = mat;
        }
        if let Some(media) = self
            .prefs
            .media_id
            .and_then(stencilcraft_machine::Media::by_id)
        {
            self.send.media_index = machine_index_of_media(media.id);
            self.send.pressure_override = media.pressure.unwrap_or(10);
            self.send.speed_override = media.speed.unwrap_or(5);
        }
    }

    /// Save the cutter model and material when they change.
    pub fn remember_machine_prefs(&mut self) {
        let model = Some(self.send.model.id().to_string());
        let media = stencilcraft_machine::MEDIA_PRESETS
            .get(self.send.media_index)
            .map(|m| m.id);
        let mat = Some(self.mat.id().to_string());
        if self.prefs.machine_model != model
            || self.prefs.media_id != media
            || self.prefs.mat_id != mat
        {
            self.prefs.machine_model = model;
            self.prefs.media_id = media;
            self.prefs.mat_id = mat;
            self.save_prefs();
        }
    }

    pub fn reset_workspace(&mut self) {
        self.workspace = if self.prefs.starter_mode {
            starter_workspace()
        } else {
            default_workspace()
        };
        self.status = "panel layout restored".to_string();
    }

    /// Whether every guide is locked (and there is at least one): View › Guides › Lock Guides.
    pub fn guides_locked(&self) -> bool {
        let guides = &self.engine.doc.guides;
        !guides.is_empty() && guides.iter().all(|g| g.locked)
    }

    /// Change snapping settings through `snap.set`, leaving the fields `edit` doesn't set.
    fn run_snap_set(&mut self, edit: impl FnOnce(&mut SnapToggles)) {
        let mut t = SnapToggles::default();
        edit(&mut t);
        self.run_command(Command::Document(DocumentCommand::SnapSet {
            grid: t.grid,
            guides: None,
            objects: t.objects,
            spacing: t.spacing,
            points: t.points,
            tolerance_mm: None,
        }));
    }

    pub fn run_command(&mut self, command: Command) {
        match self.engine.run(command) {
            Ok(_effect) => self.status = "done".to_string(),
            Err(e) => self.status = e.to_string(),
        }
    }

    pub fn save_to(&mut self, path: String) {
        match stencilcraft_doc::save_to(&self.engine.doc, &path) {
            Ok(()) => {
                self.open_path = Some(path.clone());
                self.saved_document = self.engine.doc.clone();
                self.discard_own_snapshot();
                self.record_recent_file(path);
                self.status = "saved".to_string();
            }
            Err(e) => self.status = e.to_string(),
        }
    }

    pub fn record_recent_file(&mut self, path: String) {
        let path = std::fs::canonicalize(&path)
            .unwrap_or_else(|_| std::path::PathBuf::from(path))
            .to_string_lossy()
            .into_owned();
        self.recent_files.retain(|recent| recent != &path);
        self.recent_files.insert(0, path);
        self.recent_files.truncate(8);
        self.save_prefs();
    }

    /// Records the board size the view is framed for. Returns true when the size is new,
    /// which the canvas turns into a `Fit` so a resized canvas is framed instead of
    /// leaving the artwork wherever the old sheet happened to place it.
    pub fn note_board_size(&mut self, width_mm: f64, height_mm: f64) -> bool {
        let board = (width_mm, height_mm);
        if self.last_board == Some(board) {
            return false;
        }
        self.last_board = Some(board);
        true
    }

    pub fn open_document(&mut self, path: &str) {
        match stencilcraft_doc::load_from(path) {
            Ok(doc) => {
                self.engine = stencilcraft_engine::Engine::new(doc);
                self.saved_document = self.engine.doc.clone();
                self.set_selection([]);
                self.open_path = Some(path.to_string());
                self.view_initialized = false;
                self.last_board = None;
                self.kerf_preview_cache = None;
                self.record_recent_file(path.to_string());
                self.status = format!("opened {path}");
            }
            Err(error) => self.status = format!("open: {error}"),
        }
    }

    pub fn update_kerf_preview_cache(&mut self, doc: &stencilcraft_doc::Document) -> bool {
        let unchanged_geometry = self.kerf_preview_cache.as_ref().is_some_and(|cache| {
            cache.kerf_width_mm == doc.kerf.width_mm
                && cache.source.layers.len() == doc.layers.len()
                && cache
                    .source
                    .layers
                    .iter()
                    .zip(&doc.layers)
                    .all(|(old, new)| {
                        old.shapes.len() == new.shapes.len()
                            && old.shapes.iter().zip(&new.shapes).all(|(old, new)| {
                                old.id == new.id && old.style == new.style && old.path == new.path
                            })
                    })
        });

        if unchanged_geometry {
            if let Some(cache) = self.kerf_preview_cache.as_mut()
                && let Ok(preview) = &mut cache.preview
            {
                preview.board = doc.board.clone();
                preview.kerf = doc.kerf.clone();
                preview.next_shape_id = doc.next_shape_id;
                for (preview_layer, source_layer) in preview.layers.iter_mut().zip(&doc.layers) {
                    preview_layer.name.clone_from(&source_layer.name);
                    preview_layer.visible = source_layer.visible;
                }
            }
            return false;
        }
        let preview = stencilcraft_kerf::apply(doc, 4.0).map_err(|error| error.to_string());
        self.kerf_preview_cache = Some(KerfPreviewCache {
            source: doc.clone(),
            kerf_width_mm: doc.kerf.width_mm,
            preview,
        });
        true
    }

    /// The outermost group holding shape `id`, if it is grouped.
    fn outer_group(&self, id: u64) -> Option<u64> {
        self.engine
            .doc
            .group_chain(stencilcraft_doc::GroupChild::Shape(id))
            .first()
            .copied()
    }

    /// `id` and, when it is grouped, every other shape of its outermost group: what a click on
    /// a grouped shape selects in Silhouette.
    fn with_group(&self, id: u64) -> Vec<u64> {
        match self.outer_group(id) {
            Some(group) => self.engine.doc.group_shapes(group),
            None => vec![id],
        }
    }

    /// Select `ids`, widened to whole groups (a group is selected as one object).
    pub fn set_selection(&mut self, ids: impl IntoIterator<Item = u64>) {
        let ids: Vec<u64> = ids.into_iter().flat_map(|id| self.with_group(id)).collect();
        let mut selected = Vec::new();
        for id in ids {
            let selectable = self
                .engine
                .doc
                .get_shape(id)
                .is_some_and(stencilcraft_ui_egui::is_selectable);
            if selectable && !selected.contains(&id) {
                selected.push(id);
            }
        }
        if !selected.is_empty() {
            // Shapes and a placed image are not picked together.
            self.selected_image = None;
        }
        self.selected_shape = selected.last().copied();
        self.selected_shapes = selected;
    }

    pub fn select_latest_shape(&mut self) {
        if let Some(id) = self.engine.doc.next_shape_id.checked_sub(1) {
            self.set_selection([id]);
        }
    }

    pub fn retain_valid_selection(&mut self) {
        let ids = self.selected_shapes.clone();
        self.set_selection(ids);
    }

    pub fn select_all_shapes(&mut self) {
        let ids = self
            .engine
            .doc
            .shapes()
            .map(|(_, shape)| shape.id)
            .collect::<Vec<_>>();
        self.set_selection(ids);
    }

    /// One screen pixel in millimetres at the current zoom.
    fn screen_px_mm(&self) -> f64 {
        1.0 / f64::from(self.view.zoom.max(f32::EPSILON))
    }

    /// Duplicate the selection beside itself, 40 screen pixels to the right, as Silhouette does
    /// (the copy is easy to see and grab); the copies become the selection.
    pub fn duplicate_selection(&mut self) {
        let ids = self.selected_shapes.clone();
        let dx_mm = 40.0 * self.screen_px_mm();
        self.select_added(Command::ShapesDuplicate {
            ids,
            dx_mm,
            dy_mm: 0.0,
        });
    }

    /// Run a command that adds shapes and select what it added.
    fn select_added(&mut self, command: Command) {
        match self.engine.run(command) {
            Ok(stencilcraft_engine::Effect::ShapesAdded { ids }) => {
                self.path_selection = PathSelection::default();
                self.set_selection(ids);
                self.status = "done".to_string();
            }
            Ok(_) => self.status = "done".to_string(),
            Err(error) => self.status = error.to_string(),
        }
    }

    /// The desktop backend turns Cmd/Ctrl+X, C and V into clipboard events rather than key
    /// presses, and sends Paste only when the system clipboard holds text. Copy and Cut put the
    /// selection there as SVG (so it also pastes into other apps, and Paste always arrives);
    /// Paste pastes the in-app clipboard, in place when Shift is held. Returns whether a
    /// clipboard event was handled.
    fn handle_clipboard_events(&mut self, ctx: &egui::Context) -> bool {
        if ctx.text_edit_focused() || self.send_mode {
            return false;
        }
        let (cut, copy, paste) = ctx.input_mut(|input| {
            let mut seen = (false, false, false);
            input.events.retain(|event| match event {
                egui::Event::Cut => {
                    seen.0 = true;
                    false
                }
                egui::Event::Copy => {
                    seen.1 = true;
                    false
                }
                egui::Event::Paste(_) => {
                    seen.2 = true;
                    false
                }
                _ => true,
            });
            seen
        });
        if (cut || copy) && !self.selected_shapes.is_empty() {
            if let Some(svg) = self.selection_svg() {
                ctx.copy_text(svg);
            }
            if cut {
                self.clipboard_cut();
            } else {
                self.clipboard_copy();
            }
        }
        if paste {
            let mode = if ctx.input(|input| input.modifiers.shift) {
                stencilcraft_engine::document::PasteMode::InPlace
            } else {
                stencilcraft_engine::document::PasteMode::Offset
            };
            self.clipboard_paste(mode);
        }
        cut || copy || paste
    }

    /// The selected shapes as an SVG document, for the system clipboard.
    fn selection_svg(&self) -> Option<String> {
        let mut doc = self.engine.doc.clone();
        for layer in &mut doc.layers {
            layer
                .shapes
                .retain(|shape| self.selected_shapes.contains(&shape.id));
        }
        let bytes = stencilcraft_import::write_document(&doc, "svg", 300.0).ok()?;
        String::from_utf8(bytes).ok()
    }

    /// Group the selection (Silhouette's Group, Cmd+G): its loose shapes and the outermost
    /// groups it holds become one group, which then selects as one object.
    pub fn group_selection(&mut self) {
        let mut ids = Vec::new();
        let mut groups = Vec::new();
        for &id in &self.selected_shapes {
            match self.outer_group(id) {
                Some(group) if !groups.contains(&group) => groups.push(group),
                Some(_) => {}
                None => ids.push(id),
            }
        }
        if ids.len() + groups.len() < 2 {
            self.status = "select two or more objects to group".to_string();
            return;
        }
        self.run_command(Command::Document(DocumentCommand::GroupCreate {
            ids,
            groups,
            name: String::new(),
        }));
        self.status = "grouped".to_string();
    }

    /// Ungroup (Cmd+Shift+G): dissolve the selected outermost groups so their parts select on
    /// their own. As in Silhouette, ungrouping a compound path releases it into its contours.
    pub fn ungroup_selection(&mut self) {
        let mut groups = Vec::new();
        let mut loose = Vec::new();
        for &id in &self.selected_shapes {
            match self.outer_group(id) {
                Some(group) if !groups.contains(&group) => groups.push(group),
                Some(_) => {}
                None => loose.push(id),
            }
        }
        let keep = self.selected_shapes.clone();
        for group in groups {
            self.run_command(Command::Document(DocumentCommand::GroupUngroup {
                id: group,
            }));
        }
        let compound: Vec<u64> = loose
            .into_iter()
            .filter(|id| {
                self.engine.doc.get_shape(*id).is_some_and(|shape| {
                    stencilcraft_ui_egui::bezpath_to_edit_subpaths(&shape.path).len() > 1
                })
            })
            .collect();
        let mut selection = keep;
        if !compound.is_empty() {
            match self
                .engine
                .run(Command::ShapesReleaseCompound { ids: compound })
            {
                Ok(stencilcraft_engine::Effect::ShapesAdded { ids }) => selection.extend(ids),
                Ok(_) => {}
                Err(error) => self.status = error.to_string(),
            }
        }
        // The parts stay selected, now as separate objects.
        self.set_selection(selection);
        self.status = "ungrouped".to_string();
    }

    pub fn clipboard_copy(&mut self) {
        if self.selected_shapes.is_empty() {
            return;
        }
        self.run_command(Command::Document(DocumentCommand::ClipboardCopy {
            ids: self.selected_shapes.clone(),
        }));
        self.status = "copied".to_string();
    }

    pub fn clipboard_cut(&mut self) {
        if self.selected_shapes.is_empty() {
            return;
        }
        self.run_command(Command::Document(DocumentCommand::ClipboardCut {
            ids: self.selected_shapes.clone(),
        }));
        self.retain_valid_selection();
    }

    /// Paste: Silhouette puts the copy 80 screen pixels down and to the right; Paste in Front
    /// puts it exactly on top of the original, Paste in Place at the same spot on top.
    /// Selected shapes that are live text.
    pub fn selected_text_ids(&self) -> impl Iterator<Item = u64> + '_ {
        self.selected_shapes.iter().copied().filter(|id| {
            self.engine
                .doc
                .get_shape(*id)
                .is_some_and(|shape| shape.text.is_some())
        })
    }

    /// Select the next selectable shape above (or below) the selection in stacking order,
    /// across layers; at the end of the stack the selection stays.
    pub fn select_next_object(&mut self, above: bool) {
        let order: Vec<u64> = self
            .engine
            .doc
            .layers
            .iter()
            .filter(|layer| layer.visible && !layer.locked)
            .flat_map(|layer| layer.shapes.iter())
            .filter(|shape| stencilcraft_ui_egui::is_selectable(shape))
            .map(|shape| shape.id)
            .collect();
        let positions = order
            .iter()
            .enumerate()
            .filter(|(_, id)| self.selected_shapes.contains(id))
            .map(|(i, _)| i);
        let next = if above {
            positions.max().and_then(|i| order.get(i.saturating_add(1)))
        } else {
            positions
                .min()
                .and_then(|i| i.checked_sub(1))
                .and_then(|i| order.get(i))
        };
        if let Some(&id) = next {
            self.set_selection([id]);
        }
    }

    pub fn clipboard_paste(&mut self, mode: stencilcraft_engine::document::PasteMode) {
        let offset_mm = 80.0 * self.screen_px_mm();
        self.select_added(Command::Document(DocumentCommand::ClipboardPaste {
            mode,
            offset_mm,
        }));
    }

    /// Arrow keys move the selection by screen pixels, so the step suits the zoom (Silhouette:
    /// about 2 px, 16 px with Shift). One undo step per press.
    pub fn nudge_selection(&mut self, dx_px: f64, dy_px: f64) {
        if self.selected_shapes.is_empty() {
            return;
        }
        let px = self.screen_px_mm();
        self.run_command(Command::ShapesTransform {
            ids: self.selected_shapes.clone(),
            dx_mm: dx_px * px,
            dy_mm: dy_px * px,
            scale_x: 1.0,
            scale_y: 1.0,
            degrees: 0.0,
        });
    }

    /// Move the dragged selection so its bounds sit `total_screen` from where the drag
    /// began, snapped by `snap.point` (grid, guides, objects, spacing, points as the
    /// document's snap settings say). The snap's lines become the smart guides.
    /// `p` snapped to the grid, guides, objects and points the document's snap settings
    /// turn on (drawing snaps like moving); the smart-guide lines are kept for painting.
    /// Resizing the selection box snaps its moving edges (Silhouette's snapping applies to
    /// resizes too). Only plain scales snap; rotations pass through. A proportional (corner)
    /// resize keeps its proportions: the axis that snapped sets the scale for both.
    pub fn snap_box_scale(
        &mut self,
        bounds: stencilcraft_geom::Rect,
        affine: craft_behavior::kurbo::Affine,
    ) -> craft_behavior::kurbo::Affine {
        use craft_behavior::kurbo::Affine;
        let [sx, b, c, sy, _, _] = affine.as_coeffs();
        if b.abs() > 1e-9 || c.abs() > 1e-9 || !(sx > 0.0 && sy > 0.0) {
            return affine;
        }
        let moved = affine.transform_rect_bbox(bounds);
        let proportional = (sx - sy).abs() < 1e-9 && (sx - 1.0).abs() > 1e-12;
        // The moving edge on each axis: the one that went further.
        let x_moves = (moved.x0 - bounds.x0).abs() > 1e-9 || (moved.x1 - bounds.x1).abs() > 1e-9;
        let y_moves = (moved.y0 - bounds.y0).abs() > 1e-9 || (moved.y1 - bounds.y1).abs() > 1e-9;
        let left = (moved.x0 - bounds.x0).abs() > (moved.x1 - bounds.x1).abs();
        let top = (moved.y0 - bounds.y0).abs() > (moved.y1 - bounds.y1).abs();
        let (mx, my) = (
            if left { moved.x0 } else { moved.x1 },
            if top { moved.y0 } else { moved.y1 },
        );
        let snapped = self.snap_world_point(stencilcraft_geom::Point::new(mx, my));
        let mut out = moved;
        if x_moves {
            if left {
                out.x0 = snapped.x
            } else {
                out.x1 = snapped.x
            }
        }
        if y_moves {
            if top {
                out.y0 = snapped.y
            } else {
                out.y1 = snapped.y
            }
        }
        if proportional && out != moved {
            // The axis that snapped (x first) sets one scale for both.
            let scale = if (snapped.x - mx).abs() > 1e-12 {
                out.width() / bounds.width()
            } else {
                out.height() / bounds.height()
            };
            let (w, h) = (bounds.width() * scale, bounds.height() * scale);
            out = if left {
                out.with_origin((moved.x1 - w, out.y0))
                    .with_size((w, out.height()))
            } else {
                out.with_size((w, out.height()))
            };
            out = if top {
                out.with_origin((out.x0, moved.y1 - h))
                    .with_size((out.width(), h))
            } else {
                out.with_size((out.width(), h))
            };
        }
        if !(out.width() > 1e-9 && out.height() > 1e-9) {
            return affine;
        }
        let (nsx, nsy) = (out.width() / bounds.width(), out.height() / bounds.height());
        let candidate = Affine::new([
            nsx,
            0.0,
            0.0,
            nsy,
            out.x0 - bounds.x0 * nsx,
            out.y0 - bounds.y0 * nsy,
        ]);
        if candidate.as_coeffs().iter().all(|v| v.is_finite()) {
            candidate
        } else {
            affine
        }
    }

    pub fn snap_world_point(&mut self, p: stencilcraft_geom::Point) -> stencilcraft_geom::Point {
        let snap = &self.engine.doc.snap;
        if self.snap_suspended || !(snap.grid || snap.guides || snap.objects || snap.points) {
            self.snap_lines.clear();
            return p;
        }
        let query = stencilcraft_engine::document::SnapQuery {
            x: p.x,
            y: p.y,
            width: 0.0,
            height: 0.0,
            exclude: Vec::new(),
            tolerance_mm: None,
            within_media: self.prefs.snap_within_media,
        };
        match self
            .engine
            .run(Command::Document(DocumentCommand::SnapPoint(query)))
        {
            Ok(stencilcraft_engine::Effect::Document(
                stencilcraft_engine::document::DocumentEffect::Snap(r),
            )) if r.x.is_finite() && r.y.is_finite() => {
                self.snap_lines = r
                    .lines
                    .iter()
                    .map(|l| {
                        (
                            stencilcraft_geom::Point::new(l.from[0], l.from[1]),
                            stencilcraft_geom::Point::new(l.to[0], l.to[1]),
                        )
                    })
                    .collect();
                stencilcraft_geom::Point::new(r.x, r.y)
            }
            _ => p,
        }
    }

    pub fn drag_selection_to(&mut self, id: u64, total_screen: egui::Vec2) {
        let Some(origin) = self.drag_origin else {
            return;
        };
        let d = self.view.screen_delta_to_world(total_screen);
        let (mut x, mut y) = (origin.x0 + d.x, origin.y0 + d.y);
        self.snap_lines.clear();
        let snap = &self.engine.doc.snap;
        if !self.snap_suspended
            && (snap.grid || snap.guides || snap.objects || snap.spacing || snap.points)
        {
            let query = stencilcraft_engine::document::SnapQuery {
                x,
                y,
                width: origin.width(),
                height: origin.height(),
                exclude: self.selected_shapes.clone(),
                tolerance_mm: None,
                within_media: self.prefs.snap_within_media,
            };
            if let Ok(stencilcraft_engine::Effect::Document(
                stencilcraft_engine::document::DocumentEffect::Snap(r),
            )) = self
                .engine
                .run(Command::Document(DocumentCommand::SnapPoint(query)))
            {
                (x, y) = (r.x, r.y);
                self.snap_lines = r
                    .lines
                    .iter()
                    .map(|l| {
                        (
                            stencilcraft_geom::Point::new(l.from[0], l.from[1]),
                            stencilcraft_geom::Point::new(l.to[0], l.to[1]),
                        )
                    })
                    .collect();
            }
        }
        let Some(now) =
            stencilcraft_ui_egui::selected_world_bounds(&self.engine.doc, &self.selected_shapes)
        else {
            return;
        };
        let (dx_mm, dy_mm) = (x - now.x0, y - now.y0);
        if dx_mm.abs() < 1e-9 && dy_mm.abs() < 1e-9 {
            return;
        }
        let command = if self.selected_shapes.len() > 1 {
            Command::ShapesTransform {
                ids: self.selected_shapes.clone(),
                dx_mm,
                dy_mm,
                scale_x: 1.0,
                scale_y: 1.0,
                degrees: 0.0,
            }
        } else {
            Command::ShapeMove { id, dx_mm, dy_mm }
        };
        match self.engine.run(command) {
            Ok(_) => self.drag_steps = self.drag_steps.saturating_add(1),
            Err(error) => self.status = error.to_string(),
        }
    }

    /// A drag moves the selection a little every frame; on release those moves become one
    /// undo step: undo them and apply the total move once.
    pub fn fold_drag_into_one_step(&mut self) {
        let steps = std::mem::take(&mut self.drag_steps);
        if steps < 2 {
            return;
        }
        let Some(end) =
            stencilcraft_ui_egui::selected_world_bounds(&self.engine.doc, &self.selected_shapes)
        else {
            return;
        };
        let mut undone = 0;
        while undone < steps && self.engine.undo() {
            undone += 1;
        }
        let Some(start) =
            stencilcraft_ui_egui::selected_world_bounds(&self.engine.doc, &self.selected_shapes)
        else {
            return;
        };
        let (dx_mm, dy_mm) = (end.x0 - start.x0, end.y0 - start.y0);
        let ids = self.selected_shapes.clone();
        let command = match ids.as_slice() {
            [id] => Command::ShapeMove {
                id: *id,
                dx_mm,
                dy_mm,
            },
            _ => Command::ShapesTransform {
                ids,
                dx_mm,
                dy_mm,
                scale_x: 1.0,
                scale_y: 1.0,
                degrees: 0.0,
            },
        };
        self.run_command(command);
    }

    /// Turn a canvas guide edit into `guide.*` commands: a drag shows as a preview and
    /// commits once; a guide dragged out of a ruler is added; one dropped off the page
    /// (or deleted) is removed. Keys are guide index + 1 ([`stencilcraft_ui_egui::canvas_guides`]).
    ///
    /// `before` is the document's guides as the canvas shows them with no drag
    /// ([`stencilcraft_ui_egui::canvas_guides`] with no preview); `events` the guides control's edit
    /// of the whole list, which this pass changes at most one guide of.
    pub fn apply_guide_edit(
        &mut self,
        before: &[craft_ui::canvas::Guide],
        events: &craft_behavior::EditEvents<Vec<craft_ui::canvas::Guide>>,
    ) {
        if events.is_empty() {
            return;
        }
        if events.cancel_reason().is_some() {
            self.guide_preview = None;
            return;
        }
        // A drag's updates only preview; the document changes once, on its Commit (or a Set).
        // (`EditEvents::landed` carries an Update's value too, which added a guide per update.)
        let Some(after) = events.committed() else {
            if let Some(proposed) = events.value() {
                self.guide_preview = guide_drag(before, proposed);
            }
            return;
        };
        self.guide_preview = None;
        // A moved, turned or removed guide, else one dragged out of a ruler (a key not shown before).
        let added = after
            .iter()
            .find(|g| !before.iter().any(|b| b.key == g.key));
        let (index, along, position) = match (guide_change(before, after), added) {
            (Some(change), _) => (
                before.iter().position(|g| g.key == change.key),
                change.along,
                change.at,
            ),
            (None, Some(guide)) => (None, guide.along, Some(guide.position)),
            (None, None) => return,
        };
        {
            {
                // A horizontal guide runs along x.
                let axis = if along == craft_behavior::Axis::X {
                    stencilcraft_doc::GuideAxis::Horizontal
                } else {
                    stencilcraft_doc::GuideAxis::Vertical
                };
                let command = match (index, position) {
                    (None, Some(position_mm)) => DocumentCommand::GuideAdd { axis, position_mm },
                    (None, None) => return,
                    (Some(index), None) => DocumentCommand::GuideRemove { index },
                    (Some(index), Some(position_mm)) => {
                        if self.engine.doc.guides.get(index).map(|g| g.axis) != Some(axis) {
                            // shortcut: an Alt-turned guide is re-added on its new axis;
                            // two undo steps instead of one.
                            self.run_command(Command::Document(DocumentCommand::GuideRemove {
                                index,
                            }));
                            DocumentCommand::GuideAdd { axis, position_mm }
                        } else {
                            DocumentCommand::GuideMove { index, position_mm }
                        }
                    }
                };
                self.run_command(Command::Document(command));
            }
        }
    }

    pub fn delete_selection(&mut self) {
        if self.selected_shapes.is_empty() {
            if let Some(id) = self.selected_image.take() {
                self.run_command(Command::ImageRemove { id });
            }
            return;
        }
        let ids = self.selected_shapes.clone();
        match self.engine.run(Command::ShapesRemove { ids }) {
            Ok(_) => {
                self.set_selection([]);
                self.status = "deleted".to_string();
            }
            Err(error) => self.status = error.to_string(),
        }
    }

    /// Remove points from a shape, their neighbours joining with a connecting line (Silhouette's
    /// Delete Point); a shape left with no points is removed.
    pub fn remove_path_points(&mut self, id: u64, knots: &[craft_draw::ui::pen::KnotRef]) {
        let Some(shape) = self.engine.doc.get_shape(id) else {
            return;
        };
        let mut subs = stencilcraft_ui_egui::bezpath_to_edit_subpaths(&shape.path);
        stencilcraft_ui_egui::delete_path_knots(&mut subs, knots);
        let path = stencilcraft_ui_egui::edit_subpaths_to_bezpath(&subs);
        if path.elements().is_empty() {
            self.run_command(Command::ShapeRemove { id });
            self.path_selection = PathSelection::default();
        } else {
            self.run_command(Command::ShapeSetPath { id, path });
            self.path_selection.anchors.clear();
        }
    }

    /// The right-click menu of the Select tool, in Silhouette's order: undo/redo, clipboard,
    /// flip, edit points/weld/offset, group, arrange. With nothing selected it offers undo,
    /// paste and Select All.
    fn shape_context_menu(&mut self, ui: &mut egui::Ui) {
        use stencilcraft_engine::document::{ArrangeOrder, PasteMode};
        let count = self.selected_shapes.len();
        let any = count > 0;
        let grouped = self
            .selected_shapes
            .iter()
            .any(|id| self.outer_group(*id).is_some());
        let mut chosen = None;
        let mut item = |ui: &mut egui::Ui, label: &str, enabled: bool, action: TopMenuAction| {
            if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                chosen = Some(action);
                ui.close();
            }
        };
        item(ui, "Undo", self.engine.can_undo(), TopMenuAction::Undo);
        item(ui, "Redo", self.engine.can_redo(), TopMenuAction::Redo);
        ui.separator();
        if any {
            item(ui, "Cut", true, TopMenuAction::Cut);
            item(ui, "Copy", true, TopMenuAction::Copy);
        }
        item(ui, "Paste", true, TopMenuAction::Paste(PasteMode::Offset));
        item(
            ui,
            "Paste in Front",
            true,
            TopMenuAction::Paste(PasteMode::InFront),
        );
        item(
            ui,
            "Paste in Back",
            true,
            TopMenuAction::Paste(PasteMode::InBack),
        );
        if !any {
            item(ui, "Select All", true, TopMenuAction::SelectAll);
            if let Some(action) = chosen {
                self.handle_menu_action(action);
            }
            return;
        }
        item(ui, "Duplicate", true, TopMenuAction::Duplicate);
        item(ui, "Delete", true, TopMenuAction::Delete);
        ui.separator();
        item(
            ui,
            "Flip Horizontally",
            true,
            TopMenuAction::Flip(DistributionAxis::Horizontal),
        );
        item(
            ui,
            "Flip Vertically",
            true,
            TopMenuAction::Flip(DistributionAxis::Vertical),
        );
        ui.separator();
        let mut edit_points = false;
        if ui
            .add_enabled(count == 1 && !grouped, egui::Button::new("Edit Points"))
            .clicked()
        {
            edit_points = true;
            ui.close();
        }
        item(
            ui,
            "Weld",
            count >= 2,
            TopMenuAction::Pathfinder(BooleanOperation::Union),
        );
        let mut offset = false;
        if ui.button("Offset").clicked() {
            offset = true;
            ui.close();
        }
        ui.separator();
        item(ui, "Group", count >= 2, TopMenuAction::Group);
        item(ui, "Ungroup", grouped || count >= 1, TopMenuAction::Ungroup);
        ui.separator();
        item(
            ui,
            "Send to Back",
            true,
            TopMenuAction::Arrange(ArrangeOrder::Back),
        );
        item(
            ui,
            "Bring to Front",
            true,
            TopMenuAction::Arrange(ArrangeOrder::Front),
        );
        item(
            ui,
            "Bring Forward",
            true,
            TopMenuAction::Arrange(ArrangeOrder::Forward),
        );
        item(
            ui,
            "Send Backward",
            true,
            TopMenuAction::Arrange(ArrangeOrder::Backward),
        );
        if let Some(action) = chosen {
            self.handle_menu_action(action);
        }
        if edit_points {
            self.path_selection = PathSelection::default();
            self.active_tool = ActiveTool::DirectSelect;
        }
        if offset {
            self.handle_edit_action(panels::EditAction::Offset { internal: false });
        }
    }

    /// The right-click menu in point editing (Silhouette's Delete Point / Break Path and the
    /// Point Editing panel's actions), acting on the point or line under the click.
    fn point_context_menu(&mut self, ui: &mut egui::Ui) {
        let has_point = !self.path_selection.anchors.is_empty();
        let has_line = has_point || self.path_selection.segment.is_some();
        let items: [(&str, bool, panels::EditAction); 6] = [
            ("Delete Point", has_point, panels::EditAction::DeleteNodes),
            ("Break Path", has_point, panels::EditAction::Break),
            (
                "Corner",
                has_point,
                panels::EditAction::NodeType(stencilcraft_engine::NodeType::Corner),
            ),
            (
                "Smooth",
                has_point,
                panels::EditAction::NodeType(stencilcraft_engine::NodeType::Smooth),
            ),
            (
                "Make Flat",
                has_line,
                panels::EditAction::Segment(stencilcraft_engine::SegmentType::Line),
            ),
            (
                "Make Curve",
                has_line,
                panels::EditAction::Segment(stencilcraft_engine::SegmentType::Curve),
            ),
        ];
        for (label, enabled, action) in items {
            if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                self.handle_edit_action(action);
                ui.close();
            }
        }
    }

    /// Finish the Polyline/Curve draft as a cut path (needs two points; fewer are dropped).
    pub fn commit_click_draft(&mut self) {
        if let Some(draft) = self.click_draft.take()
            && let Some(path) = draft.finish()
        {
            self.run_command(Command::ShapeAddPath {
                path,
                style: CutStyle::Cut,
            });
            self.select_latest_shape();
            self.status = match draft.kind {
                click_draw::ClickKind::Polyline => "created polyline",
                click_draw::ClickKind::Curve => "created curve",
            }
            .to_string();
        }
    }

    /// Finish a freehand stroke as a cut path. `px_mm` is one screen pixel in millimetres, so
    /// the simplification and the close radius feel the same at every zoom.
    pub fn commit_freehand(&mut self, stroke: &freehand::Stroke, smooth: bool, px_mm: f64) {
        if let Some(path) = stroke.finish(smooth, 0.75 * px_mm, 8.0 * px_mm) {
            self.run_command(Command::ShapeAddPath {
                path,
                style: CutStyle::Cut,
            });
            self.select_latest_shape();
            self.status = if smooth {
                "created smooth freehand path"
            } else {
                "created freehand path"
            }
            .to_string();
        }
    }

    /// The Knife: cut the selected shapes along `from` → `to` with the options bar's mode and gap.
    /// Like Silhouette it needs a selection; the pieces become the new selection.
    pub fn knife_cut(&mut self, from: stencilcraft_geom::Point, to: stencilcraft_geom::Point) {
        if self.selected_shapes.is_empty() {
            self.status = "select the shapes to cut first".to_string();
            return;
        }
        let mode = if self.panels.knife_solid {
            stencilcraft_engine::cut_tools::CutMode::Solid
        } else {
            stencilcraft_engine::cut_tools::CutMode::Outline
        };
        let gap_mm = if self.panels.knife_gap_mm.is_finite() {
            self.panels.knife_gap_mm.clamp(0.0, 100.0)
        } else {
            0.0
        };
        self.run_cut(Command::ShapesKnife {
            ids: self.selected_shapes.clone(),
            from: [from.x, from.y],
            to: [to.x, to.y],
            mode,
            gap_mm,
        });
    }

    /// The Eraser: erase along `points` from the selected shapes, or from every visible,
    /// unlocked shape when nothing is selected.
    pub fn erase_stroke(&mut self, points: &[stencilcraft_geom::Point]) {
        let ids = if self.selected_shapes.is_empty() {
            self.engine
                .doc
                .layers
                .iter()
                .filter(|layer| layer.visible && !layer.locked && !layer.template)
                .flat_map(|layer| layer.shapes.iter())
                .filter(|shape| !shape.hidden && !shape.locked)
                .map(|shape| shape.id)
                .collect()
        } else {
            self.selected_shapes.clone()
        };
        if ids.is_empty() || points.is_empty() {
            return;
        }
        let size_mm = if self.panels.eraser_size_mm.is_finite() {
            self.panels.eraser_size_mm.clamp(0.01, 500.0)
        } else {
            5.0
        };
        self.run_cut(Command::ShapesErase {
            ids,
            points: points.iter().map(|p| [p.x, p.y]).collect(),
            size_mm,
            brush: if self.panels.eraser_round {
                stencilcraft_engine::cut_tools::BrushShape::Round
            } else {
                stencilcraft_engine::cut_tools::BrushShape::Square
            },
            mode: if self.panels.eraser_solid {
                stencilcraft_engine::cut_tools::CutMode::Solid
            } else {
                stencilcraft_engine::cut_tools::CutMode::Outline
            },
        });
    }

    fn run_cut(&mut self, command: Command) {
        match self.engine.run(command) {
            Ok(stencilcraft_engine::Effect::ShapesAdded { ids }) => {
                self.path_selection = PathSelection::default();
                self.set_selection(ids);
                self.status = "cut".to_string();
            }
            Ok(_) => self.status = "done".to_string(),
            Err(error) => self.status = error.to_string(),
        }
        self.retain_valid_selection();
    }

    /// Place the arc being swept as an open cut path; the Arc tool stays active.
    pub fn commit_arc_draft(&mut self) {
        if let Some(path) = self.arc_draft.take().and_then(|draft| draft.path()) {
            self.run_command(Command::ShapeAddPath {
                path,
                style: CutStyle::Cut,
            });
            self.select_latest_shape();
            self.status = "created arc".to_string();
        }
    }

    /// Enter/Esc end a Polyline/Curve draft and keep it; Backspace, Delete and Cmd+Z take back
    /// its last point. Returns whether a key was used.
    fn handle_click_draft_keys(&mut self, ctx: &egui::Context) -> bool {
        if self.arc_draft.is_some() && !ctx.text_edit_focused() {
            // Enter places the arc as swept so far; Esc or Backspace drops it.
            let (place, back) = ctx.input_mut(|input| {
                (
                    consume_exact(input, egui::Modifiers::NONE, egui::Key::Enter),
                    consume_exact(input, egui::Modifiers::NONE, egui::Key::Backspace),
                )
            });
            let drop = take_escape(ctx) | back;
            if place {
                self.commit_arc_draft();
            } else if drop {
                self.arc_draft = None;
            }
            if place || drop {
                return true;
            }
        }
        if self.click_draft.is_none() || ctx.text_edit_focused() {
            return false;
        }
        let escape = take_escape(ctx);
        let (end, back) = ctx.input_mut(|input| {
            let end = consume_exact(input, egui::Modifiers::NONE, egui::Key::Enter) | escape;
            let back = consume_exact(input, egui::Modifiers::NONE, egui::Key::Backspace)
                | consume_exact(input, egui::Modifiers::NONE, egui::Key::Delete)
                | consume_exact(input, egui::Modifiers::COMMAND, egui::Key::Z);
            (end, back)
        });
        if end {
            self.commit_click_draft();
        } else if back
            && let Some(draft) = self.click_draft.as_mut()
            && !draft.undo()
        {
            self.click_draft = None;
        }
        end || back
    }

    /// A pen path is being drawn: Delete, Backspace, Esc and undo belong to the pen.
    fn pen_drawing(&self) -> bool {
        self.active_tool == ActiveTool::Pen
            && self
                .pen_draft
                .as_ref()
                .is_some_and(|draft| !draft.knots.is_empty())
    }

    /// Points are selected in point editing: Delete and Backspace remove those points (the
    /// neighbours rejoin) and Esc deselects them, instead of acting on whole shapes.
    fn editing_points(&self) -> bool {
        matches!(
            self.active_tool,
            ActiveTool::DirectSelect | ActiveTool::ConvertPoint
        ) && !self.path_selection.anchors.is_empty()
    }

    pub fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        stencilcraft_ui_egui::set_length_unit(self.prefs.length_unit);
        self.snap_suspended = ctx.input(|input| input.modifiers.command);
        // Typing on the canvas takes every plain key; Cmd shortcuts other than undo still work.
        self.handle_text_session_keys(ctx);
        if self.handle_distort_keys(ctx)
            || self.handle_click_draft_keys(ctx)
            || self.handle_clipboard_events(ctx)
        {
            return;
        }
        // Buttons and other controls can retain keyboard focus after a click. Only
        // text edits should consume Delete as text rather than removing shapes.
        // The pen draft and the point overlay read their own Delete keys later this frame.
        if !ctx.text_edit_focused()
            && !self.pen_drawing()
            && !self.editing_points()
            && ctx.input_mut(|input| {
                consume_exact(input, egui::Modifiers::NONE, egui::Key::Delete)
                    || consume_exact(input, egui::Modifiers::NONE, egui::Key::Backspace)
            })
        {
            self.delete_selection();
            return;
        }
        // Only a focused text field keeps the keys: canvas controls (the selection box, a
        // button just clicked) hold keyboard focus too, and must not swallow Cmd+Z and friends.
        if self.send_mode || ctx.text_edit_focused() {
            return;
        }
        // Shifted shortcuts first (harmless now that keys match exactly).
        let cmd_shift = egui::Modifiers::COMMAND | egui::Modifiers::SHIFT;
        let cmd_alt = egui::Modifiers::COMMAND | egui::Modifiers::ALT;
        // Object and Path menu keys (menus.rs), modified variants before their plain twins.
        let edit_keys = [
            (cmd_alt, egui::Key::Num2, panels::EditAction::UnlockAll),
            (cmd_alt, egui::Key::Num3, panels::EditAction::ShowAll),
            (
                cmd_alt,
                egui::Key::J,
                panels::EditAction::Average(stencilcraft_engine::NodeAxis::Both),
            ),
            (cmd_shift, egui::Key::E, panels::EditAction::ReleaseCompound),
            (cmd_shift, egui::Key::J, panels::EditAction::Join),
            (
                egui::Modifiers::COMMAND,
                egui::Key::Num2,
                panels::EditAction::Lock,
            ),
            (
                egui::Modifiers::COMMAND,
                egui::Key::Num3,
                panels::EditAction::Hide,
            ),
            (
                egui::Modifiers::COMMAND,
                egui::Key::E,
                panels::EditAction::MakeCompound,
            ),
            (
                egui::Modifiers::COMMAND,
                egui::Key::J,
                panels::EditAction::Simplify,
            ),
        ];
        // View menu keys (menus.rs), modified variants before their plain twins.
        let view_keys = [
            (cmd_alt, egui::Key::Num0, TopMenuAction::ZoomSelection),
            (cmd_alt, egui::Key::Num5, TopMenuAction::ReleaseGuides),
            (
                cmd_alt,
                egui::Key::Semicolon,
                TopMenuAction::ToggleLockGuides,
            ),
            (cmd_alt, egui::Key::Quote, TopMenuAction::ToggleSnapPoint),
            (cmd_shift, egui::Key::Quote, TopMenuAction::ToggleSnapGrid),
            (
                egui::Modifiers::COMMAND,
                egui::Key::Num5,
                TopMenuAction::MakeGuides,
            ),
            (
                egui::Modifiers::COMMAND,
                egui::Key::Semicolon,
                TopMenuAction::ToggleShowGuides,
            ),
            (
                egui::Modifiers::COMMAND,
                egui::Key::U,
                TopMenuAction::ToggleSmartGuides,
            ),
        ];
        if let Some(action) = view_keys.into_iter().find_map(|(modifiers, key, action)| {
            ctx.input_mut(|input| consume_exact(input, modifiers, key))
                .then_some(action)
        }) {
            self.handle_menu_action(action);
        } else if let Some(action) = edit_keys.into_iter().find_map(|(modifiers, key, action)| {
            ctx.input_mut(|input| consume_exact(input, modifiers, key))
                .then_some(action)
        }) {
            self.handle_edit_action(action);
        } else if ctx.input_mut(|input| consume_exact(input, cmd_shift, egui::Key::A)) {
            self.handle_menu_action(TopMenuAction::Deselect);
        } else if ctx.input_mut(|input| consume_exact(input, cmd_shift, egui::Key::D)) {
            self.handle_menu_action(TopMenuAction::TransformAgain);
        } else if ctx.input_mut(|input| consume_exact(input, cmd_shift, egui::Key::W)) {
            self.handle_menu_action(TopMenuAction::Pathfinder(BooleanOperation::Union));
        } else if ctx
            .input_mut(|input| consume_exact(input, egui::Modifiers::COMMAND, egui::Key::L))
        {
            self.handle_menu_action(TopMenuAction::ShowSend);
        } else if ctx
            .input_mut(|input| consume_exact(input, egui::Modifiers::COMMAND, egui::Key::A))
        {
            self.select_all_shapes();
        } else if ctx.input_mut(|input| consume_exact(input, egui::Modifiers::NONE, egui::Key::F))
            || ctx
                .input_mut(|input| consume_exact(input, egui::Modifiers::COMMAND, egui::Key::Num0))
        {
            self.pending_view_command = Some(CanvasViewCommand::Fit);
        } else if ctx.input_mut(|input| consume_exact(input, egui::Modifiers::NONE, egui::Key::C)) {
            self.pending_view_command = Some(CanvasViewCommand::Center);
        } else if ctx
            .input_mut(|input| consume_exact(input, egui::Modifiers::COMMAND, egui::Key::Num1))
        {
            self.view.zoom = 1.0;
        } else if ctx.input_mut(|input| {
            consume_exact(
                input,
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::G,
            )
        }) {
            self.ungroup_selection();
        } else if ctx
            .input_mut(|input| consume_exact(input, egui::Modifiers::COMMAND, egui::Key::G))
        {
            self.group_selection();
        } else if ctx.input_mut(|input| {
            consume_exact(
                input,
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::CloseBracket,
            )
        }) {
            self.handle_menu_action(TopMenuAction::Arrange(
                stencilcraft_engine::document::ArrangeOrder::Front,
            ));
        } else if ctx.input_mut(|input| {
            consume_exact(
                input,
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::OpenBracket,
            )
        }) {
            self.handle_menu_action(TopMenuAction::Arrange(
                stencilcraft_engine::document::ArrangeOrder::Back,
            ));
        } else if ctx.input_mut(|input| {
            consume_exact(input, egui::Modifiers::COMMAND, egui::Key::CloseBracket)
        }) {
            self.handle_menu_action(TopMenuAction::Arrange(
                stencilcraft_engine::document::ArrangeOrder::Forward,
            ));
        } else if ctx.input_mut(|input| {
            consume_exact(input, egui::Modifiers::COMMAND, egui::Key::OpenBracket)
        }) {
            self.handle_menu_action(TopMenuAction::Arrange(
                stencilcraft_engine::document::ArrangeOrder::Backward,
            ));
        } else if ctx.input_mut(|input| {
            consume_exact(
                input,
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::V,
            )
        }) {
            self.clipboard_paste(stencilcraft_engine::document::PasteMode::InPlace);
        } else if ctx
            .input_mut(|input| consume_exact(input, egui::Modifiers::COMMAND, egui::Key::X))
        {
            self.clipboard_cut();
        } else if ctx
            .input_mut(|input| consume_exact(input, egui::Modifiers::COMMAND, egui::Key::C))
        {
            self.clipboard_copy();
        } else if ctx
            .input_mut(|input| consume_exact(input, egui::Modifiers::COMMAND, egui::Key::V))
        {
            self.clipboard_paste(stencilcraft_engine::document::PasteMode::Offset);
        } else if ctx
            .input_mut(|input| consume_exact(input, egui::Modifiers::COMMAND, egui::Key::F))
        {
            self.clipboard_paste(stencilcraft_engine::document::PasteMode::InFront);
        } else if ctx
            .input_mut(|input| consume_exact(input, egui::Modifiers::COMMAND, egui::Key::B))
        {
            self.clipboard_paste(stencilcraft_engine::document::PasteMode::InBack);
        } else if ctx.input_mut(|input| {
            consume_exact(input, egui::Modifiers::COMMAND, egui::Key::T)
                || consume_exact(input, cmd_alt, egui::Key::T)
        }) {
            self.handle_menu_action(TopMenuAction::ShowPanel(WorkspacePanel::TextStyle));
        } else if ctx
            .input_mut(|input| consume_exact(input, egui::Modifiers::COMMAND, egui::Key::K))
        {
            self.handle_menu_action(TopMenuAction::ShowPanel(WorkspacePanel::Preferences));
        } else if ctx
            .input_mut(|input| consume_exact(input, egui::Modifiers::COMMAND, egui::Key::P))
        {
            self.handle_menu_action(TopMenuAction::Print);
        } else if ctx
            .input_mut(|input| consume_exact(input, egui::Modifiers::COMMAND, egui::Key::W))
        {
            self.handle_menu_action(TopMenuAction::CloseDoc);
        } else if ctx.input_mut(|input| consume_exact(input, cmd_shift, egui::Key::P)) {
            self.handle_menu_action(TopMenuAction::TogglePrintBorder);
        } else if ctx.input_mut(|input| consume_exact(input, cmd_shift, egui::Key::L)) {
            self.handle_menu_action(TopMenuAction::ToggleCutBorder);
        } else if let Some((dx, dy)) = (!self.editing_points()).then(|| arrow_nudge(ctx)).flatten()
        {
            self.nudge_selection(dx, dy);
        } else if let Some(side) = ctx.input_mut(|input| {
            [
                (egui::Key::ArrowLeft, panels::ScreenSide::Left),
                (egui::Key::ArrowRight, panels::ScreenSide::Right),
                (egui::Key::ArrowUp, panels::ScreenSide::Above),
                (egui::Key::ArrowDown, panels::ScreenSide::Below),
            ]
            .into_iter()
            .find(|(key, _)| consume_exact(input, egui::Modifiers::COMMAND, *key))
            .map(|(_, side)| side)
        }) {
            // Silhouette: Ctrl+Arrow duplicates the selection to that side.
            self.handle_edit_action(panels::EditAction::Duplicate(side));
        } else if ctx
            .input_mut(|input| consume_exact(input, egui::Modifiers::COMMAND, egui::Key::D))
        {
            self.duplicate_selection();
        } else if ctx.input_mut(|input| {
            consume_exact(input, cmd_shift, egui::Key::Z)
                || consume_exact(input, egui::Modifiers::COMMAND, egui::Key::Y)
        }) {
            let _ = self.engine.redo();
            self.retain_valid_selection();
        } else if ctx
            .input_mut(|input| consume_exact(input, egui::Modifiers::COMMAND, egui::Key::Z))
        {
            if self.pen_drawing() {
                self.pen_undo_requested = true;
            } else {
                let _ = self.engine.undo();
                self.retain_valid_selection();
            }
        } else if ctx.input_mut(|input| {
            consume_exact(
                input,
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::R,
            )
        }) {
            self.prefs.show_rulers = !self.prefs.show_rulers;
            self.save_prefs();
        } else if ctx
            .input_mut(|input| consume_exact(input, egui::Modifiers::COMMAND, egui::Key::Quote))
        {
            self.prefs.show_grid = !self.prefs.show_grid;
            self.save_prefs();
        } else if !self.pen_drawing() && !self.editing_points() && take_escape(ctx) {
            self.set_selection([]);
            self.path_selection = PathSelection::default();
            self.pen_draft = None;
            self.shape_drag_start = None;
        } else if ctx.input_mut(|input| consume_exact(input, egui::Modifiers::NONE, egui::Key::V)) {
            self.active_tool = ActiveTool::Select;
        } else if ctx.input_mut(|input| consume_exact(input, egui::Modifiers::NONE, egui::Key::A)) {
            self.active_tool = ActiveTool::DirectSelect;
        } else if ctx.input_mut(|input| consume_exact(input, egui::Modifiers::NONE, egui::Key::X)) {
            self.active_tool = ActiveTool::ConvertPoint;
        } else if ctx.input_mut(|input| consume_exact(input, egui::Modifiers::NONE, egui::Key::M)) {
            self.active_tool = ActiveTool::BoxSelect;
        } else if ctx.input_mut(|input| consume_exact(input, egui::Modifiers::NONE, egui::Key::L)) {
            self.active_tool = ActiveTool::LassoSelect;
        } else if ctx.input_mut(|input| consume_exact(input, egui::Modifiers::NONE, egui::Key::P)) {
            self.active_tool = ActiveTool::Pen;
        } else if ctx
            .input_mut(|input| consume_exact(input, egui::Modifiers::NONE, egui::Key::Backslash))
        {
            self.active_tool = ActiveTool::Line;
        } else if ctx.input_mut(|input| consume_exact(input, egui::Modifiers::NONE, egui::Key::N)) {
            self.active_tool = ActiveTool::Freehand;
        } else if ctx.input_mut(|input| consume_exact(input, egui::Modifiers::NONE, egui::Key::E)) {
            self.active_tool = ActiveTool::Eraser;
        } else if ctx.input_mut(|input| consume_exact(input, egui::Modifiers::NONE, egui::Key::K)) {
            self.active_tool = ActiveTool::Knife;
        } else if ctx.input_mut(|input| consume_exact(input, egui::Modifiers::NONE, egui::Key::R)) {
            self.active_tool = ActiveTool::Rect;
        } else if ctx.input_mut(|input| consume_exact(input, egui::Modifiers::NONE, egui::Key::U)) {
            self.active_tool = ActiveTool::RoundedRect;
        } else if ctx.input_mut(|input| consume_exact(input, egui::Modifiers::NONE, egui::Key::O)) {
            self.active_tool = ActiveTool::Ellipse;
        } else if ctx.input_mut(|input| consume_exact(input, egui::Modifiers::NONE, egui::Key::G)) {
            self.active_tool = ActiveTool::Polygon;
        } else if ctx.input_mut(|input| consume_exact(input, egui::Modifiers::NONE, egui::Key::S)) {
            self.active_tool = ActiveTool::Star;
        } else if ctx.input_mut(|input| consume_exact(input, egui::Modifiers::NONE, egui::Key::T)) {
            self.active_tool = ActiveTool::Text;
        } else if ctx.input_mut(|input| consume_exact(input, egui::Modifiers::NONE, egui::Key::H)) {
            self.active_tool = ActiveTool::Hand;
        } else if ctx.input_mut(|input| consume_exact(input, egui::Modifiers::NONE, egui::Key::Z)) {
            self.active_tool = ActiveTool::Zoom;
        }
    }

    pub fn run_pathfinder(&mut self, operation: BooleanOperation) {
        let ids = self.selected_shapes.clone();
        match self.engine.run(Command::ShapePathfinder { ids, operation }) {
            Ok(stencilcraft_engine::Effect::ShapesAdded { ids }) => {
                self.set_selection(ids);
                self.status = "done".to_string();
            }
            Ok(_) => {}
            Err(error) => self.status = error.to_string(),
        }
    }

    pub fn add_imported_shapes(&mut self, imported: stencilcraft_doc::Document, label: &str) {
        let shapes = imported
            .layers
            .into_iter()
            .flat_map(|layer| {
                layer
                    .shapes
                    .into_iter()
                    .map(|shape| (shape.path, shape.style))
            })
            .collect::<Vec<_>>();
        let count = shapes.len();
        if count == 0 {
            self.status = format!("no cuttable paths found in {label}");
            return;
        }
        match self.engine.run(Command::ShapeAddPaths { shapes }) {
            Ok(_) => {
                self.select_latest_shape();
                self.status = format!("imported {count} paths from {label}");
            }
            Err(error) => self.status = format!("import failed: {error}"),
        }
    }

    pub fn import_svg(&mut self, path: std::path::PathBuf) {
        let name = path.to_string_lossy();
        let result =
            stencilcraft_import::import_file(&name, &stencilcraft_import::ImportOptions::default())
                .map_err(|error| error.to_string());
        match result {
            Ok(document) => self.add_imported_shapes(document, &name),
            Err(error) => self.status = error,
        }
    }

    /// Place a PNG/JPEG on the page as Silhouette Studio does: at 300 dpi, at the page's top
    /// left, selected, ready to trace with the Trace panel.
    pub fn import_image(&mut self, path: std::path::PathBuf) {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "image".to_string());
        let placed = std::fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(|bytes| stencilcraft_raster::decode(&bytes).map_err(|e| e.to_string()))
            .and_then(|pixels| {
                stencilcraft_raster::place(&pixels, &name, 300.0).map_err(|e| e.to_string())
            });
        match placed {
            Ok(image) => self.place_image(image),
            Err(error) => self.status = format!("{}: {error}", path.display()),
        }
    }

    /// Move a placed image by `by` millimetres in one undoable step.
    pub fn move_image(&mut self, id: u64, by: stencilcraft_geom::Vec2) {
        let Some(image) = self.engine.doc.get_image(id) else {
            return;
        };
        if by.hypot() < 1e-9 {
            return;
        }
        let (x, y, width, height) = (
            image.x_mm + by.x,
            image.y_mm + by.y,
            image.width_mm,
            image.height_mm,
        );
        self.run_command(Command::ImageSetBounds {
            id,
            x,
            y,
            width,
            height,
        });
    }

    pub fn place_image(&mut self, image: stencilcraft_doc::RasterImage) {
        let before: std::collections::BTreeSet<u64> =
            self.engine.doc.images.iter().map(|i| i.id).collect();
        self.run_command(Command::ImagePlace { image });
        self.selected_image = self
            .engine
            .doc
            .images
            .iter()
            .map(|i| i.id)
            .find(|id| !before.contains(id));
        if self.selected_image.is_some() {
            self.set_selection([]);
            self.status = "placed image (trace it from the Trace panel)".to_string();
        }
    }

    /// Upload or refresh the textures of placed images; drop those of removed ones.
    fn sync_image_textures(&mut self, ctx: &egui::Context) {
        let live: std::collections::BTreeSet<u64> =
            self.engine.doc.images.iter().map(|i| i.id).collect();
        self.image_textures.retain(|id, _| live.contains(id));
        for image in &self.engine.doc.images {
            let key = std::sync::Arc::as_ptr(&image.png_base64).cast::<u8>() as usize;
            if self
                .image_textures
                .get(&image.id)
                .is_some_and(|(cached, _)| *cached == key)
            {
                continue;
            }
            let Ok(pixels) = stencilcraft_raster::image_pixels(image) else {
                continue;
            };
            let (w, h) = pixels.dimensions();
            let (Ok(w), Ok(h)) = (usize::try_from(w), usize::try_from(h)) else {
                continue;
            };
            let colour = egui::ColorImage::from_rgba_unmultiplied([w, h], &pixels.into_raw());
            let texture = ctx.load_texture(
                format!("placed-image-{}", image.id),
                colour,
                egui::TextureOptions::LINEAR,
            );
            self.image_textures.insert(image.id, (key, texture));
        }
    }

    /// Open a new text dialog, or edit the selected live text shape.
    /// Opens File › New on `tab` (0 Recent, 1.. the preset categories).
    pub fn open_new_document(&mut self, tab: usize) {
        let kerf = self.engine.doc.kerf.width_mm;
        self.new_doc.open(tab, kerf, self.mat, self.send.model);
        self.new_doc.saved.clone_from(&self.prefs.saved_new);
    }

    pub fn new_document_dialog(&mut self, parent: &mut egui::Ui) {
        let created = self.new_doc.show(parent, &self.prefs.recent_new);
        if std::mem::take(&mut self.new_doc.save_requested) {
            let preset = self.new_doc.choice.recent();
            self.prefs.saved_new.retain(|p| p.name != preset.name);
            self.prefs.saved_new.insert(0, preset);
            self.prefs.saved_new.truncate(new_doc::MAX_SAVED);
            self.new_doc.saved.clone_from(&self.prefs.saved_new);
            self.save_prefs();
        }
        if let Some(name) = self.new_doc.delete_saved.take() {
            self.prefs.saved_new.retain(|p| p.name != name);
            self.new_doc.saved.clone_from(&self.prefs.saved_new);
            self.new_doc.selected = None;
            self.save_prefs();
        }
        if let Some(choice) = created {
            self.request_new_document(choice);
        }
        if self.pending_new_document.is_some() {
            let mut open = true;
            let shown = Dialog::new("Discard unsaved changes?")
                .id_salt("discard-before-new")
                .opened(Bind::bound(&mut open))
                .buttons([
                    FooterButton::new("Discard changes", DialogRole::Accept),
                    FooterButton::new("Cancel", DialogRole::Cancel),
                ])
                .show(parent, |ui, _| {
                    let _ = Text::new(
                        "Creating a new document will discard changes. Cancel to save your work first.",
                    )
                    .shade(Shade::Muted)
                    .show(ui);
                });
            if shown.output.picked == Some(0) {
                self.confirm_new_document();
            } else if !open || shown.output.closed.is_some() {
                self.pending_new_document = None;
            }
        }
    }

    fn request_new_document(&mut self, choice: new_doc::Choice) {
        if self.engine.doc != self.saved_document {
            self.pending_new_document = Some(choice);
        } else {
            self.create_document(&choice);
        }
    }

    fn confirm_new_document(&mut self) {
        if let Some(choice) = self.pending_new_document.take() {
            self.create_document(&choice);
        }
    }

    /// Create in New Document: a fresh board and kerf through the engine, then the mat,
    /// machine and title the details chose.
    pub fn create_document(&mut self, choice: &new_doc::Choice) {
        for command in choice.commands() {
            if let Err(e) = self.engine.run(command) {
                self.status = e.to_string();
                return;
            }
        }
        self.saved_document = self.engine.doc.clone();
        if choice.starter == new_doc::Starter::PrintMarks {
            self.prefs.print_cut_marks = true;
        }
        self.mat = choice.mat;
        self.send.model = choice.model;
        self.doc_title = choice.name.trim().to_string();
        self.open_path = None;
        self.set_selection([]);
        self.view_initialized = false;
        self.last_board = None;
        new_doc::push_recent(
            &mut self.prefs.recent_new,
            choice.recent(),
            new_doc::MAX_RECENT,
        );
        self.save_prefs();
        self.status = format!(
            "new document {}",
            new_doc::size_line(choice.width_mm, choice.height_mm, choice.units)
        );
    }

    pub fn commit_pen_draft(&mut self) {
        if let Some(draft) = self.pen_draft.take() {
            let path = stencilcraft_ui_egui::edit_subpaths_to_bezpath(&[draft]);
            if !path.elements().is_empty() {
                self.run_command(Command::ShapeAddPath {
                    path,
                    style: CutStyle::Cut,
                });
                self.select_latest_shape();
                self.status = "created cut path with pen tool".to_string();
            }
        }
    }

    pub fn commit_dragged_shape(
        &mut self,
        start: stencilcraft_geom::Point,
        end: stencilcraft_geom::Point,
        mods: DragMods,
    ) {
        if let Some(command) = self.dragged_shape_command(start, end, mods) {
            self.run_command(command);
            self.select_latest_shape();
            // Silhouette's "action after tool use" preference.
            if self.prefs.select_after_drawing {
                self.active_tool = ActiveTool::Select;
            }
        }
    }

    fn dragged_shape_command(
        &self,
        start: stencilcraft_geom::Point,
        end: stencilcraft_geom::Point,
        mods: DragMods,
    ) -> Option<Command> {
        if self.active_tool == ActiveTool::Line {
            let end = line_end(start, end, mods);
            if (end - start).hypot() < 1.0 {
                return None;
            }
            let mut path = stencilcraft_geom::BezPath::new();
            path.move_to(start);
            path.line_to(end);
            return Some(Command::ShapeAddPath {
                path,
                style: CutStyle::Cut,
            });
        }
        let drag = drag_box(start, end, mods);
        let (x, y, w, h) = (drag.x0, drag.y0, drag.width(), drag.height());
        if w < 1.0 && h < 1.0 {
            return None;
        }
        let centre = drag.center();
        match self.active_tool {
            ActiveTool::Rect => Some(Command::ShapeAddRect {
                x,
                y,
                width: w.max(2.0),
                height: h.max(2.0),
            }),
            ActiveTool::RoundedRect => Some(Command::ShapeAddRoundedRect {
                x,
                y,
                width: w.max(2.0),
                height: h.max(2.0),
                radius: self.panels.corner_radius(w.max(2.0), h.max(2.0)),
            }),
            ActiveTool::Ellipse => Some(Command::ShapeAddEllipse {
                x,
                y,
                width: w.max(2.0),
                height: h.max(2.0),
            }),
            ActiveTool::Polygon => Some(Command::ShapeAddPolygon {
                cx: centre.x,
                cy: centre.y,
                radius: (w.max(h) / 2.0).max(2.0),
                sides: self.panels.polygon_sides(),
                rotation: 0.0,
            }),
            ActiveTool::Star => Some(Command::ShapeAddStar {
                cx: centre.x,
                cy: centre.y,
                radius: (w.max(h) / 2.0).max(3.0),
                inner_radius: (w.max(h) / 2.0).max(3.0) * self.panels.star_inner_ratio(),
                points: self.panels.star_points(),
                rotation: 0.0,
            }),
            _ => None,
        }
    }

    fn build_menu_bar(&self) -> Vec<menus::MenuItem<TopMenuAction>> {
        menus::build(self)
    }

    pub fn handle_menu_action(&mut self, action: TopMenuAction) {
        match action {
            TopMenuAction::NewDoc => self.open_new_document(0),
            TopMenuAction::NewFromTemplate => self.open_new_document(2),
            TopMenuAction::OpenDoc => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("StencilCraft", &[stencilcraft_doc::FORMAT_EXTENSION])
                    .pick_file()
                {
                    self.open_document(&path.to_string_lossy());
                }
            }
            TopMenuAction::OpenRecent(idx) => {
                if let Some(path) = self.recent_files.get(idx).cloned() {
                    self.open_document(&path);
                }
            }
            TopMenuAction::SaveDoc => {
                let pick = match &self.open_path {
                    Some(p) => Some(std::path::PathBuf::from(p)),
                    None => rfd::FileDialog::new()
                        .add_filter("StencilCraft", &[stencilcraft_doc::FORMAT_EXTENSION])
                        .set_file_name(format!(
                            "{}.{}",
                            self.doc_title,
                            stencilcraft_doc::FORMAT_EXTENSION
                        ))
                        .save_file(),
                };
                if let Some(path) = pick {
                    self.save_to(path.to_string_lossy().into_owned());
                }
            }
            TopMenuAction::SaveAsDoc => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("StencilCraft", &[stencilcraft_doc::FORMAT_EXTENSION])
                    .set_file_name(self.open_path.clone().unwrap_or_else(|| {
                        format!("untitled.{}", stencilcraft_doc::FORMAT_EXTENSION)
                    }))
                    .save_file()
                {
                    self.save_to(path.to_string_lossy().into_owned());
                }
            }
            TopMenuAction::ImportSvg => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("SVG", &["svg", "svgz"])
                    .pick_file()
                {
                    self.import_svg(path);
                }
            }
            TopMenuAction::ImportImage => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("PNG or JPEG", &["png", "jpg", "jpeg"])
                    .pick_file()
                {
                    self.import_image(path);
                }
            }
            TopMenuAction::ExportSvg => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("SVG", &["svg"])
                    .set_file_name("export.svg")
                    .save_file()
                {
                    match stencilcraft_import::write_document(&self.engine.doc, "svg", 300.0) {
                        Ok(svg) => {
                            if let Err(e) = std::fs::write(&path, svg) {
                                self.status = format!("export failed: {e}");
                            } else {
                                self.status = format!("exported SVG to {}", path.display());
                            }
                        }
                        Err(e) => self.status = format!("SVG export error: {e}"),
                    }
                }
            }
            TopMenuAction::ExportPrintCutSvg => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("SVG", &["svg"])
                    .set_file_name("print_and_cut.svg")
                    .save_file()
                {
                    match self.print_sheet_svg() {
                        Ok(svg) => {
                            if let Err(e) = std::fs::write(&path, svg) {
                                self.status = format!("export failed: {e}");
                            } else {
                                self.status =
                                    format!("exported Print & Cut SVG to {}", path.display());
                            }
                        }
                        Err(e) => self.status = format!("Print & Cut SVG export error: {e}"),
                    }
                }
            }
            TopMenuAction::ExportDxf => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("DXF", &["dxf"])
                    .set_file_name("export.dxf")
                    .save_file()
                {
                    match stencilcraft_import::write_document(&self.engine.doc, "dxf", 300.0) {
                        Ok(dxf) => {
                            if let Err(e) = std::fs::write(&path, dxf) {
                                self.status = format!("export failed: {e}");
                            } else {
                                self.status = format!("exported DXF to {}", path.display());
                            }
                        }
                        Err(e) => self.status = format!("DXF export error: {e}"),
                    }
                }
            }
            TopMenuAction::ExportGcode => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("G-code", &["gcode", "nc"])
                    .set_file_name("export.gcode")
                    .save_file()
                {
                    match stencilcraft_gcode::export(
                        &self.engine.doc,
                        &stencilcraft_gcode::GcodeSettings::default(),
                    ) {
                        Ok(gcode) => {
                            if let Err(e) = std::fs::write(&path, gcode) {
                                self.status = format!("export failed: {e}");
                            } else {
                                self.status = format!("exported G-code to {}", path.display());
                            }
                        }
                        Err(e) => self.status = format!("G-code export error: {e}"),
                    }
                }
            }
            TopMenuAction::ExportTiledSvg => {
                if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                    // Sheets carry registration marks: tiled jobs are assembled by hand.
                    let options = stencilcraft_tiling::SheetOptions {
                        sheet_w: 300.0,
                        sheet_h: 300.0,
                        overlap_mm: 10.0,
                        marks: true,
                    };
                    match stencilcraft_tiling::tile_document_with(&self.engine.doc, &options) {
                        Ok(sheets) => {
                            let mut saved = 0;
                            for (i, sheet_doc) in sheets.iter().enumerate() {
                                let out_file = dir.join(format!("sheet_{:02}.svg", i + 1));
                                if let Ok(svg) =
                                    stencilcraft_import::write_document(sheet_doc, "svg", 300.0)
                                    && std::fs::write(&out_file, svg).is_ok()
                                {
                                    saved += 1;
                                }
                            }
                            // One page showing every sheet, so the job can be checked
                            // without opening the sheets one by one.
                            let preview_path = dir.join("preview.svg");
                            let preview = stencilcraft_svg::export_tile_preview(
                                &sheets,
                                &stencilcraft_svg::TilePreviewOptions::default(),
                            )
                            .and_then(|svg| {
                                std::fs::write(&preview_path, svg).map_err(|e| {
                                    stencilcraft_svg::SvgError::Write(format!(
                                        "{}: {e}",
                                        preview_path.display()
                                    ))
                                })
                            });
                            self.status = match preview {
                                Ok(()) => format!(
                                    "exported {saved} tiled sheets and preview.svg to {}",
                                    dir.display()
                                ),
                                Err(e) => format!(
                                    "exported {saved} tiled sheets to {} (preview failed: {e})",
                                    dir.display()
                                ),
                            };
                        }
                        Err(e) => self.status = format!("tiling error: {e}"),
                    }
                }
            }
            TopMenuAction::Undo => {
                let _ = self.engine.undo();
                self.retain_valid_selection();
            }
            TopMenuAction::Redo => {
                let _ = self.engine.redo();
                self.retain_valid_selection();
            }
            TopMenuAction::Duplicate => self.duplicate_selection(),
            TopMenuAction::Group => self.group_selection(),
            TopMenuAction::Ungroup => self.ungroup_selection(),
            TopMenuAction::Cut => self.clipboard_cut(),
            TopMenuAction::Copy => self.clipboard_copy(),
            TopMenuAction::Paste(mode) => self.clipboard_paste(mode),
            TopMenuAction::Arrange(order) => {
                self.run_command(Command::Document(DocumentCommand::ShapesArrange {
                    ids: self.selected_shapes.clone(),
                    order,
                }))
            }
            TopMenuAction::Delete => self.delete_selection(),
            TopMenuAction::SelectAll => self.select_all_shapes(),
            TopMenuAction::Deselect => {
                self.set_selection([]);
                self.path_selection = PathSelection::default();
            }
            TopMenuAction::Pathfinder(op) => self.run_pathfinder(op),
            TopMenuAction::Align(alignment) => {
                self.run_command(Command::ShapesAlign {
                    ids: self.selected_shapes.clone(),
                    alignment,
                    to_board: false,
                });
            }
            TopMenuAction::Distribute(axis) => {
                self.run_command(Command::ShapesDistribute {
                    ids: self.selected_shapes.clone(),
                    axis,
                });
            }
            TopMenuAction::RotateBy(degrees) => {
                self.run_command(Command::Document(DocumentCommand::TransformEach(
                    TransformRequest {
                        ids: self.selected_shapes.clone(),
                        degrees: f64::from(degrees),
                        ..TransformRequest::default()
                    },
                )));
            }
            TopMenuAction::Flip(axis) => {
                let (scale_x, scale_y) = match axis {
                    DistributionAxis::Horizontal => (-1.0, 1.0),
                    DistributionAxis::Vertical => (1.0, -1.0),
                };
                self.run_command(Command::Document(DocumentCommand::TransformEach(
                    TransformRequest {
                        ids: self.selected_shapes.clone(),
                        scale_x,
                        scale_y,
                        ..TransformRequest::default()
                    },
                )));
            }
            TopMenuAction::AlignToKey(alignment) => {
                if let Some(key) = self.selected_shape {
                    self.run_command(Command::Document(DocumentCommand::ShapesAlignToKey {
                        ids: self.selected_shapes.clone(),
                        key,
                        alignment,
                    }));
                }
            }
            TopMenuAction::TransformAgain => {
                self.run_command(Command::Document(DocumentCommand::TransformAgain {
                    ids: self.selected_shapes.clone(),
                }));
            }
            TopMenuAction::CenterToPage => {
                // One move of the whole selection, so one shape works and it is one undo step.
                if let Some(bounds) = stencilcraft_ui_egui::selected_world_bounds(
                    &self.engine.doc,
                    &self.selected_shapes,
                ) {
                    let board = &self.engine.doc.board;
                    let target = stencilcraft_geom::Point::new(board.width_mm, board.height_mm)
                        .midpoint(stencilcraft_geom::Point::ZERO);
                    let shift = target - bounds.center();
                    self.run_command(Command::ShapesTransform {
                        ids: self.selected_shapes.clone(),
                        dx_mm: shift.x,
                        dy_mm: shift.y,
                        scale_x: 1.0,
                        scale_y: 1.0,
                        degrees: 0.0,
                    });
                }
            }
            TopMenuAction::SetStyle(style) => self.run_command(Command::ShapesSetStyle {
                ids: self.selected_shapes.clone(),
                style,
            }),
            TopMenuAction::PlaceText => {
                self.active_tool = ActiveTool::Text;
                self.status = "click the page where the text starts".to_string();
            }
            TopMenuAction::EditText => self.edit_selected_text(),
            TopMenuAction::TextToOutlines => {
                let ids: Vec<u64> = self.selected_text_ids().collect();
                if !ids.is_empty() {
                    self.select_added(Command::TextToOutlines { ids });
                }
            }
            TopMenuAction::TextUngroupLetters => {
                let ids: Vec<u64> = self.selected_text_ids().collect();
                let mut letters = Vec::new();
                for id in ids {
                    match self.engine.run(Command::TextUngroup {
                        id,
                        to: stencilcraft_engine::text::UngroupTo::Letters,
                    }) {
                        Ok(stencilcraft_engine::Effect::ShapesAdded { ids }) => letters.extend(ids),
                        Ok(_) => {}
                        Err(e) => self.status = e.to_string(),
                    }
                }
                if !letters.is_empty() {
                    self.set_selection(letters);
                    self.status = "ungrouped to letters".to_string();
                }
            }
            TopMenuAction::SelectNextObject { above } => self.select_next_object(above),
            TopMenuAction::SelectInverse => {
                self.handle_edit_action(panels::EditAction::SelectInverse);
            }
            TopMenuAction::SelectSameStyle => {
                let styles: Vec<CutStyle> = self
                    .selected_shapes
                    .iter()
                    .filter_map(|id| self.engine.doc.get_shape(*id).map(|s| s.style))
                    .collect();
                let same: Vec<u64> = self
                    .engine
                    .doc
                    .shapes()
                    .filter(|(_, shape)| styles.contains(&shape.style))
                    .map(|(_, shape)| shape.id)
                    .collect();
                self.set_selection(same);
            }
            TopMenuAction::SelectSameLayer => {
                let same: Vec<u64> = self
                    .engine
                    .doc
                    .layers
                    .iter()
                    .filter(|layer| {
                        layer
                            .shapes
                            .iter()
                            .any(|shape| self.selected_shapes.contains(&shape.id))
                    })
                    .flat_map(|layer| layer.shapes.iter().map(|shape| shape.id))
                    .collect();
                self.set_selection(same);
            }
            TopMenuAction::ShowDesign => self.send_mode = false,
            TopMenuAction::ShowSend => self.send_mode = true,
            TopMenuAction::ShowPanel(panel) => {
                let action = if self.workspace.layout().contains(&panel) {
                    craft_behavior::dock::Action::activate(panel)
                } else {
                    craft_behavior::dock::Action::open(panel, None)
                };
                if let Err(error) =
                    Arc::make_mut(&mut self.workspace).apply(action, false, panel_permissions)
                {
                    self.status = format!("could not show panel: {error}");
                }
            }
            TopMenuAction::DirectSelectTool => self.active_tool = ActiveTool::DirectSelect,
            TopMenuAction::DryRun => {
                self.send_mode = true;
                self.send.summary = Some(self.encode_send_summary());
            }
            TopMenuAction::Quit => {
                self.quit_requested = true;
            }
            TopMenuAction::NotYet => self.status = menus::NOT_YET.to_lowercase(),
            TopMenuAction::Edit(action) => self.handle_edit_action(action),
            TopMenuAction::FitView => self.pending_view_command = Some(CanvasViewCommand::Fit),
            TopMenuAction::CenterView => {
                self.pending_view_command = Some(CanvasViewCommand::Center)
            }
            TopMenuAction::ActualSize => {
                self.pending_view_command = Some(CanvasViewCommand::ZoomPercent(100))
            }
            TopMenuAction::ZoomSelection => {
                if self.selected_shapes.is_empty() {
                    self.status = "zoom to selection: select shapes first".to_string();
                } else {
                    self.pending_view_command = Some(CanvasViewCommand::Selection);
                }
            }
            TopMenuAction::ZoomPercent(percent) => {
                self.pending_view_command = Some(CanvasViewCommand::ZoomPercent(percent))
            }
            TopMenuAction::RotateView(turns) => {
                self.pending_view_command = Some(CanvasViewCommand::Rotate(turns))
            }
            TopMenuAction::ToggleSnapGrid => {
                let on = !self.engine.doc.snap.grid;
                self.run_snap_set(|s| s.grid = Some(on));
            }
            TopMenuAction::ToggleSmartSnapping => {
                // Silhouette's Smart Snapping: other objects' edges, centres and spacing.
                let on = !self.engine.doc.snap.objects;
                self.run_snap_set(|s| {
                    s.objects = Some(on);
                    s.spacing = Some(on);
                });
            }
            TopMenuAction::ToggleSnapPoint => {
                let on = !self.engine.doc.snap.points;
                self.run_snap_set(|s| s.points = Some(on));
            }
            TopMenuAction::ToggleSmartGuides => {
                self.prefs.smart_guides = !self.prefs.smart_guides;
                self.save_prefs();
            }
            TopMenuAction::ToggleShowGuides => {
                self.prefs.show_guides = !self.prefs.show_guides;
                self.save_prefs();
            }
            TopMenuAction::ToggleLockGuides => {
                let locked = !self.guides_locked();
                self.run_command(Command::Document(DocumentCommand::GuideLockAll { locked }));
            }
            TopMenuAction::MakeGuides => {
                let ids = self.selected_shapes.clone();
                self.run_command(Command::Document(DocumentCommand::GuideMake { ids }));
                self.retain_valid_selection();
            }
            TopMenuAction::ReleaseGuides => {
                if let Ok(stencilcraft_engine::Effect::ShapesAdded { ids }) = self
                    .engine
                    .run(Command::Document(DocumentCommand::GuideRelease {}))
                {
                    self.status = format!("released {} guides", ids.len());
                    self.set_selection(ids);
                }
            }
            TopMenuAction::ClearGuides => {
                self.run_command(Command::Document(DocumentCommand::GuideClear {}))
            }
            TopMenuAction::ZoomIn => self.view.zoom = (self.view.zoom * 1.25).clamp(0.05, 400.0),
            TopMenuAction::ZoomOut => self.view.zoom = (self.view.zoom / 1.25).clamp(0.05, 400.0),
            TopMenuAction::ToggleRulers => {
                self.prefs.show_rulers = !self.prefs.show_rulers;
                self.save_prefs();
            }
            TopMenuAction::ToggleGrid => {
                self.prefs.show_grid = !self.prefs.show_grid;
                self.save_prefs();
            }
            TopMenuAction::ToggleKerfPreview => self.kerf_preview = !self.kerf_preview,
            TopMenuAction::OpenEffect(menu) => self.open_effect(menu),
            TopMenuAction::SendToLayer(index) => self.send_to_layer(index),
            TopMenuAction::ImportVector(kind) => self.import_vector_dialog(kind),
            TopMenuAction::Export(kind) => self.export_dialog(kind),
            TopMenuAction::SaveSelectionAs => self.save_selection_dialog(),
            TopMenuAction::Print => self.print_via_pdf(),
            TopMenuAction::TypeOnPath => self.type_on_path(),
            TopMenuAction::Machine(action) => self.queue_machine_action(action),
            TopMenuAction::ToggleMatPreview => {
                self.prefs.mat_preview = !self.prefs.mat_preview;
                self.save_prefs();
            }
            TopMenuAction::ToggleMirror => self.send.mirror = !self.send.mirror,
            TopMenuAction::BeginDistort { rows, columns } => self.begin_distort(rows, columns),
            TopMenuAction::CancelSend => {
                self.send
                    .cancel
                    .store(true, std::sync::atomic::Ordering::SeqCst);
                self.status = "the send stops before its next job".to_string();
            }
            TopMenuAction::SetSendMode(mode) => {
                self.send.mode = mode;
                self.send_mode = true;
            }
            TopMenuAction::ToggleStarterMode => {
                self.prefs.starter_mode = !self.prefs.starter_mode;
                self.save_prefs();
                self.reset_workspace();
            }
            TopMenuAction::LoadOnMat(on) => {
                if on {
                    if self.mat == stencilcraft_machine::Mat::None {
                        self.mat = self
                            .mat_before_unload
                            .take()
                            .unwrap_or(stencilcraft_machine::Mat::Cameo12x12);
                    }
                } else if self.mat != stencilcraft_machine::Mat::None {
                    self.mat_before_unload = Some(self.mat);
                    self.mat = stencilcraft_machine::Mat::None;
                }
            }
            TopMenuAction::CurveText => self.curve_text(),
            TopMenuAction::CloseDoc => self.request_new_document(new_doc::Choice::default()),
            TopMenuAction::ToggleAttach => self.toggle_attach(),
            TopMenuAction::Flatten => self.flatten_selection(),
            TopMenuAction::ClipMake => self.make_clipping_mask(),
            TopMenuAction::ClipRelease => self.release_clipping_mask(),
            TopMenuAction::ApplyLastEffect => self.apply_last_effect(),
            TopMenuAction::ToggleOutlineView => {
                self.prefs.outline_view = !self.prefs.outline_view;
                self.save_prefs();
            }
            TopMenuAction::ToggleCutBorder => {
                self.prefs.show_cut_border = !self.prefs.show_cut_border;
                self.save_prefs();
            }
            TopMenuAction::TogglePrintBorder => {
                self.prefs.show_print_border = !self.prefs.show_print_border;
                self.save_prefs();
            }
            TopMenuAction::ToggleDimensions => {
                self.prefs.show_dimensions = !self.prefs.show_dimensions;
                self.save_prefs();
            }
            TopMenuAction::TogglePrintCutMarks => {
                self.prefs.print_cut_marks = !self.prefs.print_cut_marks;
                self.save_prefs();
            }
            TopMenuAction::ResetWorkspace => self.reset_workspace(),
            TopMenuAction::SetTheme(theme) => {
                self.theme = theme;
                self.save_prefs();
            }
            TopMenuAction::HelpManual
            | TopMenuAction::HelpCheatsheet
            | TopMenuAction::HelpAbout => {
                self.status = "StencilCraft: clean-room Rust cut preparation".to_string();
            }
        }
    }

    pub fn top_panel(&mut self, parent: &mut egui::Ui) {
        let doc_name = self
            .open_path
            .as_deref()
            .and_then(|p| std::path::Path::new(p).file_name()?.to_str())
            .map(str::to_string)
            .unwrap_or_else(|| {
                format!("{}.{}", self.doc_title, stencilcraft_doc::FORMAT_EXTENSION)
            });
        let title_text = format!("StencilCraft — {doc_name}");

        let menus = self.build_menu_bar();
        let mut chosen_action = None;
        let mut undo_clicked = false;
        let mut redo_clicked = false;
        let mut switch_theme_clicked = false;

        let can_undo = self.engine.can_undo();
        let can_redo = self.engine.can_redo();
        let (theme_icon, next_theme) = match self.theme {
            stencilcraft_ui_egui::ColorTheme::Dark => {
                (IconId::new("sun"), stencilcraft_ui_egui::ColorTheme::Light)
            }
            stencilcraft_ui_egui::ColorTheme::Light => {
                (IconId::new("moon"), stencilcraft_ui_egui::ColorTheme::Dark)
            }
        };

        egui::Panel::top("top-chrome")
            .frame(egui::Frame::NONE)
            .show_separator_line(false)
            .show(parent, |ui| {
                let resp = TitleBar::new()
                    .id_salt("stencilcraft-title-bar")
                    .show_titled(
                        ui,
                        Title::new(title_text),
                        |ui, _| {
                            if IconButton::new(IconId::new("home"))
                                .label("Home / Fit View")
                                .show(ui)
                                .clicked()
                            {
                                chosen_action = Some(TopMenuAction::FitView);
                            }
                            chosen_action = chosen_action.or(menus::show_bar(ui, &menus));
                        },
                        |ui, _| {
                            if IconButton::new(theme_icon.clone())
                                .label("Switch Theme")
                                .show(ui)
                                .clicked()
                            {
                                switch_theme_clicked = true;
                            }
                            Divider::vertical().show(ui);
                            if IconButton::new(IconId::new("undo"))
                                .label("Undo")
                                .enabled(can_undo)
                                .show(ui)
                                .clicked()
                            {
                                undo_clicked = true;
                            }
                            if IconButton::new(IconId::new("redo"))
                                .label("Redo")
                                .enabled(can_redo)
                                .show(ui)
                                .clicked()
                            {
                                redo_clicked = true;
                            }
                        },
                    );

                if resp.close_requested {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });

        if undo_clicked {
            let _ = self.engine.undo();
            self.retain_valid_selection();
        }
        if redo_clicked {
            let _ = self.engine.redo();
            self.retain_valid_selection();
        }
        if switch_theme_clicked {
            self.theme = next_theme;
            self.save_prefs();
        }
        if let Some(action) = chosen_action {
            self.handle_menu_action(action);
        }
    }

    pub fn options_bar(&mut self, parent: &mut egui::Ui) {
        let active_tool = self.active_tool;
        let count = self.selected_shapes.len();
        let selected_shape_id = self.selected_shape;
        let selection_bounds =
            stencilcraft_ui_egui::selected_world_bounds(&self.engine.doc, &self.selected_shapes)
                .map(|b| [b.x0, b.y0, b.width(), b.height()]);
        let mut pivot = self.panels.pivot;
        let mut link_aspect = self.prefs.link_aspect;
        let mut bound_edit = None;
        let kerf_val_init = self.kerf_draft.unwrap_or(self.engine.doc.kerf.width_mm);
        let object_count = self.engine.doc.shape_count();

        let mut preview_kerf = self.kerf_preview;
        let mut show_rulers = self.prefs.show_rulers;
        let mut show_grid = self.prefs.show_grid;
        let mut send_mode = self.send_mode;
        let mut new_kerf = None;
        let mut do_unite = false;
        let mut do_minus = false;
        let mut knife_solid = self.panels.knife_solid;
        let mut knife_gap = self.panels.knife_gap_mm;
        let mut eraser_solid = self.panels.eraser_solid;
        let mut eraser_round = self.panels.eraser_round;
        let mut eraser_size = self.panels.eraser_size_mm;
        let mut shape_options = (
            self.panels.polygon_sides,
            self.panels.star_points,
            self.panels.star_inner_pct,
            self.panels.corner_radius_mm,
        );

        egui::Panel::top("options-bar")
            .frame(egui::Frame::NONE)
            .show_separator_line(false)
            .show(parent, |ui| {
                // The old bar's leading, body and trailing cells, in reading order in the bar's body.
                let _ = Bar::new("Tool options", BarKind::Options)
                    .id_salt("stencilcraft-options")
                    .show(ui, |ui, _| {
                        {
                            let _ = BarTool::new(active_tool.icon(), active_tool.label()).show(ui);
                        }
                        {
                            // The drawing tool's own settings come first, as in Silhouette Studio,
                            // whether or not the shape just drawn is selected.
                            let field = |ui: &mut egui::Ui,
                                         id: &str,
                                         label: &str,
                                         value: &mut f64,
                                         range: std::ops::RangeInclusive<f64>,
                                         unit: &str| {
                                let _ = Text::new(label.to_owned()).show(ui);
                                let (shown, range, unit, per) =
                                    panels::in_display(*value, range, unit);
                                let edit =
                                    NumberField::new(label.to_owned(), Bind::given(Some(shown)))
                                        .id_salt(id.to_owned())
                                        .range(range)
                                        .max_decimals(3)
                                        .unit(panels::unit_text(unit))
                                        .show(ui);
                                if let Some(v) = live_number(&edit.events) {
                                    *value = v / per;
                                }
                            };
                            let (sides, points, inner, corner) = &mut shape_options;
                            let shown = match active_tool {
                                ActiveTool::Polygon => {
                                    field(ui, "opt-sides", "Sides", sides, 3.0..=64.0, "{n}");
                                    true
                                }
                                ActiveTool::Star => {
                                    field(ui, "opt-points", "Points", points, 3.0..=64.0, "{n}");
                                    field(ui, "opt-inner", "Inner", inner, 5.0..=95.0, "{n} %");
                                    true
                                }
                                ActiveTool::RoundedRect => {
                                    field(
                                        ui,
                                        "opt-corner",
                                        "Corner",
                                        corner,
                                        0.0..=500.0,
                                        "{n} mm",
                                    );
                                    true
                                }
                                ActiveTool::Knife => {
                                    let _ =
                                        ToggleButton::new("Solid", Bind::bound(&mut knife_solid))
                                            .id_salt("opt-knife-solid")
                                            .size(ControlSize::Sm)
                                            .show(ui);
                                    field(
                                        ui,
                                        "opt-knife-gap",
                                        "Gap",
                                        &mut knife_gap,
                                        0.0..=100.0,
                                        "{n} mm",
                                    );
                                    true
                                }
                                ActiveTool::Eraser => {
                                    let _ =
                                        ToggleButton::new("Solid", Bind::bound(&mut eraser_solid))
                                            .id_salt("opt-eraser-solid")
                                            .size(ControlSize::Sm)
                                            .show(ui);
                                    let _ =
                                        ToggleButton::new("Round", Bind::bound(&mut eraser_round))
                                            .id_salt("opt-eraser-round")
                                            .size(ControlSize::Sm)
                                            .show(ui);
                                    field(
                                        ui,
                                        "opt-eraser-size",
                                        "Size",
                                        &mut eraser_size,
                                        0.1..=500.0,
                                        "{n} mm",
                                    );
                                    true
                                }
                                _ => false,
                            };
                            if shown {
                                Divider::vertical().show(ui);
                            }
                            if count == 0 {
                                let _ = Text::new("No selection").show(ui);
                                Divider::vertical().show(ui);
                                let mut kerf_val = kerf_val_init;
                                let _ = Text::new("Kerf:").show(ui);
                                let (shown, range, unit, per) =
                                    panels::in_display(kerf_val, 0.0..=2.0, "{n} mm");
                                let edit = NumberField::new("Kerf", Bind::given(Some(shown)))
                                    .id_salt("opt-kerf")
                                    .range(range)
                                    .max_decimals(4)
                                    .unit(panels::unit_text(unit))
                                    .show(ui);
                                if let Some(val) = live_number(&edit.events) {
                                    kerf_val = val / per;
                                    new_kerf = Some(kerf_val);
                                }
                                let _ = ToggleButton::new(
                                    "Preview Kerf",
                                    Bind::bound(&mut preview_kerf),
                                )
                                .id_salt("opt-preview-kerf")
                                .size(ControlSize::Sm)
                                .show(ui);
                            } else {
                                let title = match (count, selected_shape_id) {
                                    (1, Some(id)) => format!("Shape #{id}"),
                                    _ => format!("{count} shapes"),
                                };
                                let _ = Text::new(title).show(ui);
                                if let Some(b) = selection_bounds {
                                    Divider::vertical().show(ui);
                                    let mut picked = Some(pivot);
                                    let _ = ReferencePoint::new(
                                        "Reference point",
                                        Bind::bound(&mut picked),
                                    )
                                    .id_salt("opt-ref-point")
                                    .compact()
                                    .show(ui);
                                    if let Some(anchor) = picked {
                                        pivot = anchor;
                                    }
                                    let (u, v) = pivot.uv();
                                    let shown = [b[0] + u * b[2], b[1] + v * b[3], b[2], b[3]];
                                    for (i, name) in ["X", "Y", "W", "H"].into_iter().enumerate() {
                                        if i == 3 {
                                            let _ = craft_ui::text::link_toggle(
                                                "Keep ratio",
                                                Bind::bound(&mut link_aspect),
                                            )
                                            .id_salt("opt-aspect-link")
                                            .size(ControlSize::Sm)
                                            .show(ui);
                                        }
                                        let _ = Text::new(format!("{name}:")).show(ui);
                                        let min = if i < 2 { -10_000.0 } else { 0.01 };
                                        let (value, range, unit, per) =
                                            panels::in_display(shown[i], min..=10_000.0, "{n} mm");
                                        let edit = NumberField::new(name, Bind::given(Some(value)))
                                            .id_salt(("opt-bound", selected_shape_id, i))
                                            .range(range)
                                            .max_decimals(3)
                                            .unit(panels::unit_text(unit))
                                            .show(ui);
                                        if let Some(value) = committed_number(&edit.events) {
                                            bound_edit = Some((b, i, value / per));
                                        }
                                    }
                                }
                            }
                            if count >= 2 {
                                Divider::vertical().show(ui);
                                if Button::new("Unite")
                                    .id_salt("opt-unite")
                                    .size(ControlSize::Sm)
                                    .show(ui)
                                    .clicked()
                                {
                                    do_unite = true;
                                }
                                if Button::new("Minus Front")
                                    .id_salt("opt-minus-front")
                                    .size(ControlSize::Sm)
                                    .show(ui)
                                    .clicked()
                                {
                                    do_minus = true;
                                }
                            }
                        }
                        {
                            Divider::vertical().show(ui);
                            let _ = ToggleButton::new("Rulers", Bind::bound(&mut show_rulers))
                                .id_salt("opt-rulers")
                                .icon(IconId::new("ruler"))
                                .size(ControlSize::Sm)
                                .show(ui);
                            let _ = ToggleButton::new("Grid", Bind::bound(&mut show_grid))
                                .id_salt("opt-grid")
                                .icon(IconId::new("grid"))
                                .size(ControlSize::Sm)
                                .show(ui);
                            Divider::vertical().show(ui);
                            let _ = Text::new(format!("{object_count} objects")).show(ui);
                            let (mode_label, variant) = if send_mode {
                                ("← Design", Variant::SECONDARY)
                            } else {
                                ("Make →", Variant::PRIMARY)
                            };
                            if Button::new(mode_label)
                                .id_salt("opt-send-mode")
                                .variant(variant)
                                .size(ControlSize::Sm)
                                .show(ui)
                                .clicked()
                            {
                                send_mode = !send_mode;
                            }
                        }
                    });
            });

        self.panels.pivot = pivot;
        if link_aspect != self.prefs.link_aspect {
            self.prefs.link_aspect = link_aspect;
            self.save_prefs();
        }
        if let Some((from, field, value)) = bound_edit {
            let to = panels::edit_bounds(from, pivot, field, value, link_aspect);
            if let Some(command) = panels::bounds_command(self.selected_shapes.clone(), from, to) {
                self.run_command(command);
            }
        }
        self.kerf_preview = preview_kerf;
        self.prefs.show_rulers = show_rulers;
        self.prefs.show_grid = show_grid;
        self.send_mode = send_mode;
        (
            self.panels.polygon_sides,
            self.panels.star_points,
            self.panels.star_inner_pct,
            self.panels.corner_radius_mm,
        ) = shape_options;
        self.panels.knife_solid = knife_solid;
        self.panels.knife_gap_mm = knife_gap;
        self.panels.eraser_solid = eraser_solid;
        self.panels.eraser_round = eraser_round;
        self.panels.eraser_size_mm = eraser_size;
        if let Some(k) = new_kerf {
            self.run_command(Command::KerfSet { width_mm: k });
        }
        if do_unite {
            self.run_pathfinder(BooleanOperation::Union);
        }
        if do_minus {
            self.run_pathfinder(BooleanOperation::Difference);
        }
    }

    pub fn tools_panel(&mut self, ui: &mut egui::Ui) {
        let has_selection = !self.selected_shapes.is_empty();
        let mut lasso = self.active_tool == ActiveTool::LassoSelect;
        let was_lasso = lasso;
        let mut clicked = None;
        let _ = Panel::new("Tools")
            .id_salt("stencilcraft-tools-panel")
            .show(ui, |ui, _| {
                let _ = Section::fixed("Select")
                    .id_salt("tools-select")
                    .show(ui, |ui, _| {
                        ui.horizontal_wrapped(|ui| {
                            let _ = ToggleButton::new("Lasso select", Bind::bound(&mut lasso))
                                .id_salt("tools-lasso")
                                .icon(IconId::new("lasso"))
                                .size(ControlSize::Sm)
                                .show(ui);
                            for (label, icon, enabled, tool) in [
                                ("Deselect", "close", has_selection, ToolsAction::Deselect),
                                ("Duplicate", "copy", has_selection, ToolsAction::Duplicate),
                                ("Delete", "trash", has_selection, ToolsAction::Delete),
                            ] {
                                if Button::new(label)
                                    .id_salt(("tools", label))
                                    .icon(IconId::new(icon))
                                    .enabled(enabled)
                                    .size(ControlSize::Sm)
                                    .show(ui)
                                    .clicked()
                                {
                                    clicked = Some(tool);
                                }
                            }
                        });
                    });
                let _ = Section::fixed("Add")
                    .id_salt("tools-add")
                    .show(ui, |ui, _| {
                        ui.horizontal_wrapped(|ui| {
                            for (label, icon, tool) in [
                                ("Rectangle", "rectangle", ToolsAction::Rectangle),
                                ("Image", "image", ToolsAction::Image),
                                ("Text", "text", ToolsAction::Text),
                            ] {
                                if Button::new(label)
                                    .id_salt(("tools-add", label))
                                    .icon(IconId::new(icon))
                                    .size(ControlSize::Sm)
                                    .show(ui)
                                    .clicked()
                                {
                                    clicked = Some(tool);
                                }
                            }
                        });
                    });
            });
        if lasso != was_lasso {
            self.active_tool = if lasso {
                ActiveTool::LassoSelect
            } else {
                ActiveTool::Select
            };
        }
        match clicked {
            Some(ToolsAction::Deselect) => self.set_selection([]),
            Some(ToolsAction::Duplicate) => self.duplicate_selection(),
            Some(ToolsAction::Delete) => self.delete_selection(),
            Some(ToolsAction::Rectangle) => {
                self.run_command(Command::ShapeAddRect {
                    x: 20.0,
                    y: 20.0,
                    width: 40.0,
                    height: 30.0,
                });
                self.select_latest_shape();
            }
            Some(ToolsAction::Image) => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("PNG or JPEG", &["png", "jpg", "jpeg"])
                    .pick_file()
                {
                    self.import_image(path);
                }
            }
            Some(ToolsAction::Text) => self.active_tool = ActiveTool::Text,
            None => {}
        }
    }

    pub fn layers_panel(&mut self, ui: &mut egui::Ui) {
        /// Tags layer rows' keys so they never equal an object row's.
        const LAYER_ROW: &str = "layer";
        let mut add_layer = false;
        let mut delete_layer = false;
        let mut selected_idx = self.selected_layer;
        let mut toggles = Vec::new();

        let can_delete = self.engine.doc.layers.len() > 1;
        let layer_count = self.engine.doc.layers.len();
        let layer_data: Vec<(usize, String, bool, bool)> = self
            .engine
            .doc
            .layers
            .iter()
            .enumerate()
            .map(|(i, l)| (i, l.name.clone(), l.visible, l.locked))
            .collect();
        let selected_layer = self.selected_layer.and_then(|i| {
            self.engine
                .doc
                .layers
                .get(i)
                .map(|l| (i, l.name.clone(), l.template))
        });
        let mut lock_toggles: Vec<(usize, bool)> = Vec::new();
        let mut move_layer: Option<(usize, usize)> = None;
        let footer_selected = self.selected_layer;
        let mut rename: Option<(usize, String)> = None;
        let mut template: Option<(usize, bool)> = None;
        // Every shape with its eye (shown) and lock toggles: hidden and locked shapes are left
        // off the canvas's picking, so this list is where they come back.
        let shape_rows: Vec<(u64, String, bool, bool)> = self
            .engine
            .doc
            .shapes()
            .map(|(_, s)| {
                let kind = match s.style {
                    CutStyle::Cut => "Cut",
                    CutStyle::Engrave => "Engrave",
                };
                (s.id, format!("#{} {kind}", s.id), !s.hidden, s.locked)
            })
            .collect();
        let mut shape_toggles: Vec<(u64, usize, bool)> = Vec::new();
        let mut clicked_shape = None;

        // The footer's buttons, in the old order; the move buttons only with a selected layer.
        let mut footer_actions = vec![
            (
                FooterAction::new(IconId::new("plus"), "Add Layer"),
                LayerFooter::Add,
            ),
            (
                FooterAction::new(IconId::new("trash"), "Delete Layer").enabled(can_delete),
                LayerFooter::Delete,
            ),
        ];
        if let Some(i) = footer_selected {
            footer_actions.push((
                FooterAction::new(IconId::new("chevron_up"), "Move Layer Up").enabled(i > 0),
                LayerFooter::Move(i, i.saturating_sub(1)),
            ));
            footer_actions.push((
                FooterAction::new(IconId::new("chevron_down"), "Move Layer Down")
                    .enabled(i + 1 < layer_count),
                LayerFooter::Move(i, i + 1),
            ));
        }
        let footer_picks: Vec<LayerFooter> = footer_actions.iter().map(|(_, pick)| *pick).collect();
        let footer_buttons: Vec<FooterAction> = footer_actions
            .into_iter()
            .map(|(action, _)| action)
            .collect();
        let mut footer_picked = None;
        let selected_shapes = self.selected_shapes.clone();

        let _ = Panel::new("Layers")
            .id_salt("stencilcraft-layers-panel")
            .footer(|ui, _| {
                let out = FooterBar::actions(footer_buttons.clone())
                    .label("Layer actions")
                    .show_footer(ui);
                if out.picked.is_some() {
                    footer_picked = out.picked;
                }
            })
            .show(ui, |ui, _| {
                // The layer rows: an eye and a lock column, then the name. A click selects the layer.
                let mut layer_selection = selected_idx.map(Selection::single).unwrap_or_default();
                let layer_source =
                    craft_behavior::keyed(&layer_data, |row: &(usize, String, bool, bool)| row.0)
                        .with_toggled(
                            |row, column| if *column == EYE_COLUMN { row.2 } else { row.3 },
                        );
                let out = List::new("Layers")
                    .id_salt((LAYER_ROW, "list"))
                    // Grows to its rows inside the scrolling panel, as before the port.
                    .natural_height(true)
                    .selection(Bind::bound(&mut layer_selection))
                    .columns(toggle_columns())
                    .show_items(ui, &layer_source, &mut |key, icx| {
                        let (name, visible, locked) = layer_data
                            .get(*key)
                            .map(|row| (row.1.clone(), row.2, row.3))
                            .unwrap_or_default();
                        ListRow::new(name)
                            .cells(icx.cells())
                            .toggle(ToggleCell::eye(EYE_COLUMN, visible))
                            .toggle(ToggleCell::lock(LOCK_COLUMN, locked))
                    });
                for (key, column, on) in toggle_commits(&out.events) {
                    if column == EYE_COLUMN {
                        toggles.push((key, on));
                    } else {
                        lock_toggles.push((key, on));
                    }
                }
                if let Some(i) = layer_selection
                    .lead
                    .or_else(|| layer_selection.items.first().copied())
                {
                    selected_idx = Some(i);
                }
                // The selected layer's name and whether it is a guide (shown, never cut).
                if let Some((i, name, is_template)) = &selected_layer {
                    // Given, not bound: the name changes through the engine on Commit, as before.
                    let edit = TextField::new("Layer name", Bind::given(name.clone()))
                        .id_salt(("layer-name", *i))
                        .limit(80)
                        .show(ui);
                    if let Some(value) = edit.events.committed()
                        && !value.trim().is_empty()
                        && value != name
                    {
                        rename = Some((*i, value.trim().to_string()));
                    }
                    let mut guide = *is_template;
                    let _ = Checkbox::new(Bind::bound(&mut guide))
                        .label("Guide layer (shown, never cut)")
                        .id_salt(("layer-guide", *i))
                        .show(ui);
                    if guide != *is_template {
                        template = Some((*i, guide));
                    }
                }
                let _ = Section::new("Objects")
                    .id_salt("layers-objects")
                    .show(ui, |ui, _| {
                        let mut shape_selection = Selection::new(
                            selected_shapes.clone(),
                            selected_shapes.first().copied(),
                            selected_shapes.first().copied(),
                        );
                        let before = shape_selection.clone();
                        let source = craft_behavior::keyed(
                            &shape_rows,
                            |row: &(u64, String, bool, bool)| row.0,
                        )
                        .with_toggled(|row, column| {
                            if *column == EYE_COLUMN { row.2 } else { row.3 }
                        });
                        let out = List::new("Objects")
                            .id_salt("layers-objects-list")
                            // Grows to its rows inside the scrolling panel, as before the port.
                            .natural_height(true)
                            .selection(Bind::bound(&mut shape_selection))
                            .columns(toggle_columns())
                            .show_items(ui, &source, &mut |key, icx| {
                                let (label, shown, locked) = shape_rows
                                    .iter()
                                    .find(|row| row.0 == *key)
                                    .map(|row| (row.1.clone(), row.2, row.3))
                                    .unwrap_or_default();
                                let row = ListRow::new(label)
                                    .icon(IconSpec::svg(IconId::new("spline")))
                                    .cells(icx.cells())
                                    .toggle(ToggleCell::eye(EYE_COLUMN, shown))
                                    .toggle(ToggleCell::lock(LOCK_COLUMN, locked));
                                if shown { row } else { row.ink(RowInk::Dimmed) }
                            });
                        for (key, column, on) in toggle_commits(&out.events) {
                            shape_toggles.push((key, usize::from(column != EYE_COLUMN), on));
                        }
                        if shape_selection != before
                            && let Some(id) = shape_selection
                                .lead
                                .or_else(|| shape_selection.items.first().copied())
                        {
                            clicked_shape = Some(id);
                        }
                    });
            });
        match footer_picked.and_then(|i| footer_picks.get(i).copied()) {
            Some(LayerFooter::Add) => add_layer = true,
            Some(LayerFooter::Delete) => delete_layer = true,
            Some(LayerFooter::Move(from, to)) => move_layer = Some((from, to)),
            None => {}
        }
        for (id, column, on) in shape_toggles {
            self.run_command(if column == 0 {
                Command::ShapesSetHidden {
                    ids: vec![id],
                    hidden: !on,
                }
            } else {
                Command::ShapesSetLocked {
                    ids: vec![id],
                    locked: on,
                }
            });
            self.retain_valid_selection();
        }
        if let Some(id) = clicked_shape {
            self.set_selection([id]);
        }

        self.selected_layer = selected_idx;
        if add_layer {
            self.run_command(Command::LayerAdd {
                name: format!("Layer {}", self.engine.doc.layers.len() + 1),
                kind: CutStyle::Cut,
            });
            self.selected_layer = Some(self.engine.doc.layers.len() - 1);
        }
        if delete_layer && let Some(idx) = self.selected_layer {
            self.run_command(Command::LayerRemove { index: idx });
            self.selected_layer = Some(idx.saturating_sub(1));
        }
        for (idx, vis) in toggles {
            self.run_command(Command::LayerSetVisible {
                index: idx,
                visible: vis,
            });
        }
        for (index, locked) in lock_toggles {
            self.run_command(Command::Document(DocumentCommand::LayerSetLocked {
                index,
                locked,
            }));
            self.retain_valid_selection();
        }
        if let Some((index, name)) = rename {
            self.run_command(Command::Document(DocumentCommand::LayerRename {
                index,
                name,
            }));
        }
        if let Some((index, on)) = template {
            self.run_command(Command::Document(DocumentCommand::LayerSetTemplate {
                index,
                template: on,
            }));
        }
        if let Some((from, to)) = move_layer {
            self.run_command(Command::Document(DocumentCommand::LayerMove { from, to }));
            self.selected_layer = Some(to);
        }
    }

    pub fn inspector_panel(&mut self, ui: &mut egui::Ui) {
        // The header: icon, title and kind, as the old `InspectorHeader`s.
        let (icon, title, kind, target): (Option<&str>, String, Option<&str>, u64) =
            if self.send_mode {
                (Some("scissors"), "Make".to_owned(), Some("USB cutter"), 0)
            } else {
                match self.selected_shapes.as_slice() {
                    [] => (None, "Document".to_owned(), None, 1),
                    [id] => (
                        Some("spline"),
                        format!("Shape #{id}"),
                        Some("Path"),
                        2 + *id,
                    ),
                    ids => (None, format!("{} objects", ids.len()), None, 1),
                }
            };
        let mut inspector = Inspector::new("Properties")
            .target(("stencilcraft-inspector", target))
            .header(title);
        if let Some(icon) = icon {
            inspector = inspector.icon(IconSpec::svg(IconId::new(icon)));
        }
        if let Some(kind) = kind {
            inspector = inspector.kind(kind);
        }
        let _ = inspector.show(ui, |ui, _| {
            // The Send settings are long; they scroll when the dock is short.
            egui::ScrollArea::vertical()
                .id_salt("stencilcraft-inspector-scroll")
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    if self.send_mode {
                        let _ =
                            Text::new("Review your material and machine settings before cutting.")
                                .role(TypeRole::Caption)
                                .shade(Shade::Muted)
                                .show(ui);
                        self.send_tab(ui);
                        if Button::new("← Back to canvas")
                            .id_salt("send-back")
                            .show(ui)
                            .clicked()
                        {
                            self.send_mode = false;
                        }
                    } else {
                        self.inspector(ui);
                    }
                });
        });
    }

    pub fn inspector(&mut self, ui: &mut egui::Ui) {
        let mat_entries: Vec<Entry<stencilcraft_machine::Mat>> = MAT_CHOICES
            .iter()
            .map(|(mat, label)| Entry::new(*mat, *label))
            .collect();
        panels::labelled(ui, "Cutting mat", |ui| {
            let _ = Select::new("Cutting mat", Bind::bound(&mut self.mat), &mat_entries)
                .id_salt("design-mat")
                .show(ui);
        });

        if let Some(id) = (self.selected_shapes.len() == 1)
            .then_some(self.selected_shape)
            .flatten()
        {
            if let Some(bounds) = self
                .engine
                .doc
                .get_shape(id)
                .and_then(stencilcraft_doc::Shape::bounds)
            {
                let mut values = [bounds.x0, bounds.y0, bounds.width(), bounds.height()];
                let mut committed = false;
                let mut action = None;
                let _ = Section::new("Transform")
                    .id_salt("inspector-transform")
                    .show(ui, |ui, _| {
                        // X and Y, then W and H, two to a row, each named by its glyph as before.
                        let names = ["X", "Y", "W", "H"];
                        let shown = values;
                        for (a, b) in [(0, 1), (2, 3)] {
                            ui.horizontal(|ui| {
                                for index in [a, b] {
                                    let _ = Text::new(names[index]).role(TypeRole::Label).show(ui);
                                    let min = if index < 2 { -10_000.0 } else { 0.01 };
                                    let (value, range, unit, per) =
                                        panels::in_display(shown[index], min..=10_000.0, "{n} mm");
                                    let edit =
                                        NumberField::new(names[index], Bind::given(Some(value)))
                                            .id_salt(("bound", id, index))
                                            .range(range)
                                            .max_decimals(3)
                                            .unit(panels::unit_text(unit))
                                            .show(ui);
                                    if let Some(v) = committed_number(&edit.events) {
                                        values[index] = v / per;
                                        committed = true;
                                    }
                                }
                            });
                        }
                        ui.horizontal_wrapped(|ui| {
                            for (label, op) in [
                                ("−15°", ShapeAction::Rotate(-15.0)),
                                ("+15°", ShapeAction::Rotate(15.0)),
                                ("−10%", ShapeAction::Scale(0.9)),
                                ("+10%", ShapeAction::Scale(1.1)),
                            ] {
                                if Button::new(label)
                                    .id_salt(("inspector-shape", label))
                                    .size(ControlSize::Sm)
                                    .show(ui)
                                    .clicked()
                                {
                                    action = Some(op);
                                }
                            }
                        });
                        let _ = Text::new("Drag an object on the mat to reposition it.")
                            .role(TypeRole::Caption)
                            .shade(Shade::Muted)
                            .show(ui);
                    });
                if committed {
                    let [x, y, width, height] = values;
                    self.run_command(Command::ShapeSetBounds {
                        id,
                        x,
                        y,
                        width,
                        height,
                    });
                }
                match action {
                    Some(ShapeAction::Rotate(degrees)) => {
                        self.run_command(Command::ShapeRotate { id, degrees });
                    }
                    Some(ShapeAction::Scale(s)) => {
                        self.run_command(Command::ShapeScale {
                            id,
                            scale_x: s,
                            scale_y: s,
                        });
                    }
                    None => {}
                }
            } else if self.engine.doc.get_shape(id).is_none() {
                self.set_selection([]);
            }
        }

        let mut add_layer = false;
        let mut visibility_changes: Vec<(usize, bool)> = Vec::new();
        let layers: Vec<(String, bool)> = self
            .engine
            .doc
            .layers
            .iter()
            .map(|layer| {
                (
                    format!("{}  ·  {}", layer.name, layer.shapes.len()),
                    layer.visible,
                )
            })
            .collect();
        let _ = Section::new("Layers")
            .id_salt("inspector-layers")
            .show(ui, |ui, _| {
                // The old header's Add Layer control, at the top of the body.
                if IconButton::new(IconId::new("plus"))
                    .label("Add Layer")
                    .id_salt("inspector-add-layer")
                    .show(ui)
                    .clicked()
                {
                    add_layer = true;
                }
                for (i, (label, visible)) in layers.iter().enumerate() {
                    let mut visible_now = *visible;
                    let _ = Checkbox::new(Bind::bound(&mut visible_now))
                        .label(label.clone())
                        .id_salt(("layer-visibility", i))
                        .show(ui);
                    if visible_now != *visible {
                        visibility_changes.push((i, visible_now));
                    }
                }
            });
        if add_layer {
            self.run_command(Command::LayerAdd {
                name: format!("Layer {}", self.engine.doc.layers.len() + 1),
                kind: stencilcraft_doc::CutStyle::Cut,
            });
        }
        // Visibility is document state: it goes through the engine so it undoes.
        for (index, visible) in visibility_changes {
            self.run_command(Command::LayerSetVisible { index, visible });
        }

        let shape_rows: Vec<(u64, String)> = self
            .engine
            .doc
            .shapes()
            .map(|(_, shape)| (shape.id, format!("Shape #{} · {:?}", shape.id, shape.style)))
            .collect();
        // The objects list: the list owns the click selection (Shift and Cmd/Ctrl extend it, as
        // the old Shift-click did); the app takes the list's selection when it changes.
        let selected = self.selected_shapes.clone();
        let mut picked_selection = None;
        let _ = Section::new("Objects")
            .id_salt("inspector-objects")
            .show(ui, |ui, _| {
                let mut selection = Selection::new(
                    selected.clone(),
                    selected.first().copied(),
                    selected.first().copied(),
                );
                let before = selection.clone();
                let source = craft_behavior::keyed(&shape_rows, |row: &(u64, String)| row.0);
                let _ = List::new("Objects")
                    .id_salt("inspector-objects-list")
                    // Grows to its rows inside the scrolling panel, as before the port.
                    .natural_height(true)
                    .selection(Bind::bound(&mut selection))
                    .show_items(ui, &source, &mut |key, _| {
                        let label = shape_rows
                            .iter()
                            .find(|row| row.0 == *key)
                            .map(|row| row.1.clone())
                            .unwrap_or_default();
                        ListRow::new(label).icon(IconSpec::svg(IconId::new("spline")))
                    });
                if selection != before {
                    picked_selection = Some(selection.items.clone());
                }
            });
        if let Some(ids) = picked_selection {
            self.set_selection(ids);
        }

        if self.selected_shapes.len() >= 2 {
            let mut operation = None;
            let _ = Section::new("Pathfinder")
                .id_salt("inspector-pathfinder")
                .show(ui, |ui, _| {
                    ui.horizontal_wrapped(|ui| {
                        for (label, op) in [
                            ("Unite", BooleanOperation::Union),
                            ("Minus Front", BooleanOperation::Difference),
                            ("Intersect", BooleanOperation::Intersect),
                            ("Exclude", BooleanOperation::Xor),
                            ("Divide", BooleanOperation::Divide),
                            ("Trim", BooleanOperation::Trim),
                            ("Merge", BooleanOperation::Merge),
                            ("Crop", BooleanOperation::Crop),
                            ("Outline", BooleanOperation::Outline),
                            ("Minus Back", BooleanOperation::MinusBack),
                        ] {
                            if Button::new(label)
                                .id_salt(("inspector-pathfinder", label))
                                .size(ControlSize::Sm)
                                .show(ui)
                                .clicked()
                            {
                                operation = Some(op);
                            }
                        }
                    });
                });
            if let Some(op) = operation {
                self.run_pathfinder(op);
            }
            let mut picked = None;
            let _ = Section::new("Align")
                .id_salt("inspector-align")
                .show(ui, |ui, _| {
                    if let Some(action) = panels::align_bar(ui, "inspector-align-bar", |action| {
                        align_command(action).is_some()
                    }) {
                        picked = Some(action);
                    }
                });
            match picked.and_then(align_command) {
                Some(AlignCommand::Align(alignment)) => {
                    self.run_command(Command::ShapesAlign {
                        ids: self.selected_shapes.clone(),
                        alignment,
                        to_board: false,
                    });
                }
                Some(AlignCommand::Distribute(axis)) => {
                    self.run_command(Command::ShapesDistribute {
                        ids: self.selected_shapes.clone(),
                        axis,
                    });
                }
                None => {}
            }
            ui.horizontal(|ui| {
                if Button::new("−15°")
                    .id_salt("inspector-rotate-ccw")
                    .show(ui)
                    .clicked()
                {
                    self.run_command(Command::ShapesTransform {
                        ids: self.selected_shapes.clone(),
                        dx_mm: 0.0,
                        dy_mm: 0.0,
                        scale_x: 1.0,
                        scale_y: 1.0,
                        degrees: -15.0,
                    });
                }
                if Button::new("+15°")
                    .id_salt("inspector-rotate-cw")
                    .show(ui)
                    .clicked()
                {
                    self.run_command(Command::ShapesTransform {
                        ids: self.selected_shapes.clone(),
                        dx_mm: 0.0,
                        dy_mm: 0.0,
                        scale_x: 1.0,
                        scale_y: 1.0,
                        degrees: 15.0,
                    });
                }
            });
        }

        let kerf_width = self.kerf_draft.unwrap_or(self.engine.doc.kerf.width_mm);
        let mut kerf_events = craft_behavior::EditEvents::new();
        let mut kerf_preview = self.kerf_preview;
        let _ = Section::new("Cut settings")
            .id_salt("inspector-cut")
            .show(ui, |ui, _| {
                let _ = Text::new(format!(
                    "Kerf  {}",
                    stencilcraft_ui_egui::length_unit().format(kerf_width, 2)
                ))
                .role(TypeRole::Label)
                .shade(Shade::Muted)
                .show(ui);
                kerf_events = Slider::new(Bind::given(kerf_width), 0.0..=1.0)
                    .label("Kerf")
                    .id_salt("kerf")
                    .show(ui)
                    .events;
                let _ = Checkbox::new(Bind::bound(&mut kerf_preview))
                    .label("Preview kerf compensation")
                    .id_salt("inspector-kerf-preview")
                    .show(ui);
            });
        self.kerf_preview = kerf_preview;
        for event in kerf_events.iter() {
            match event {
                EditEvent::Begin { .. } => self.kerf_draft = Some(kerf_width),
                EditEvent::Update { value, .. } => self.kerf_draft = Some(*value),
                EditEvent::Commit { before, after, .. } => {
                    self.kerf_draft = None;
                    // The old Commit's `changed`: the value differs from the one the drag began with.
                    let changed = !matches!(before, craft_behavior::Mixed::Same(b) if b == after);
                    if changed {
                        self.run_command(Command::KerfSet { width_mm: *after });
                    }
                }
                EditEvent::Set { after, .. } => {
                    self.run_command(Command::KerfSet { width_mm: *after })
                }
                EditEvent::Cancel { .. } => self.kerf_draft = None,
                _ => {}
            }
        }
    }

    fn start_hardware(
        &mut self,
        command: stencilcraft_engine::HardwareCommand,
        ctx: egui::Context,
    ) {
        let doc = self.engine.doc.clone();
        self.start_hardware_on(doc, command, ctx);
    }

    /// Run cut jobs one after another in the background (a by-colour send has one per row).
    fn start_hardware_jobs(
        &mut self,
        jobs: Vec<(
            stencilcraft_doc::Document,
            stencilcraft_engine::HardwareCommand,
        )>,
        ctx: egui::Context,
    ) {
        if self.send.busy || jobs.is_empty() {
            return;
        }
        self.send.busy = true;
        let result = self.send.hardware_result.clone();
        let cancel = std::sync::Arc::clone(&self.send.cancel);
        cancel.store(false, std::sync::atomic::Ordering::SeqCst);
        let spawned = std::thread::Builder::new()
            .name("cutter-jobs".into())
            .spawn(move || {
                let mut outcome = Err("no jobs".to_string());
                for (doc, command) in jobs {
                    if cancel.load(std::sync::atomic::Ordering::SeqCst) {
                        outcome = Err("cancelled before the next job".to_string());
                        break;
                    }
                    outcome = stencilcraft_engine::Engine::new(doc)
                        .run(Command::Hardware(command))
                        .map_err(|e| e.to_string());
                    if outcome.is_err() {
                        break;
                    }
                }
                if let Ok(mut slot) = result.lock() {
                    *slot = Some(outcome);
                }
                ctx.request_repaint();
            });
        if let Err(error) = spawned {
            self.send.busy = false;
            self.send.summary = Some(vec![error.to_string()]);
        }
    }

    /// Run a hardware command on `doc` in the background (the test cut sends its own design).
    fn start_hardware_on(
        &mut self,
        doc: stencilcraft_doc::Document,
        command: stencilcraft_engine::HardwareCommand,
        ctx: egui::Context,
    ) {
        if self.send.busy {
            return;
        }
        self.send.busy = true;
        let result = self.send.hardware_result.clone();
        let spawned = std::thread::Builder::new()
            .name("cutter-usb".into())
            .spawn(move || {
                let outcome = stencilcraft_engine::Engine::new(doc)
                    .run(Command::Hardware(command))
                    .map_err(|e| e.to_string());
                if let Ok(mut slot) = result.lock() {
                    *slot = Some(outcome);
                }
                ctx.request_repaint();
            });
        if let Err(error) = spawned {
            self.send.busy = false;
            self.send.summary = Some(vec![error.to_string()]);
        }
    }

    /// The Send tab's cut (or validation, when `send` is false) with its current settings.
    pub fn cut_command(&self, send: bool) -> stencilcraft_engine::HardwareCommand {
        stencilcraft_engine::HardwareCommand::Cut {
            model: self.send.model.id().into(),
            location: self.send.device.clone(),
            mat: self.mat,
            force: u32::from(self.send.pressure_override),
            speed: u32::from(self.send.speed_override),
            depth: self.send.set_blade_depth.then_some(self.send.blade_depth),
            tool: self.send.tool,
            draw: self.send.draw,
            holder: self.send.holder,
            passes: self.send.passes,
            mirror: self.send.mirror,
            bluetooth: (self.send.connection_mode != 0).then(|| {
                stencilcraft_engine::hardware::BluetoothTarget {
                    address: if self.send.connection_mode == 1 {
                        self.send.device.clone()
                    } else {
                        self.send.bluetooth_address.trim().to_owned()
                    },
                    channel: self.send.bluetooth_channel,
                    ble: self.send.connection_mode == 1,
                }
            }),
            send,
            order: Some(self.send.order.clone()),
            interior_first: self.send.interior_first,
            overcut_mm: self.send.overcut_mm,
            weld: self.send.weld,
            track_enhancing: self.send.track_enhancing,
        }
    }

    pub fn send_tab(&mut self, ui: &mut egui::Ui) {
        self.run_machine_action(ui.ctx());
        self.remember_machine_prefs();
        let result = self
            .send
            .hardware_result
            .lock()
            .ok()
            .and_then(|mut slot| slot.take());
        let has_new_result = result.is_some();
        if let Some(result) = result {
            self.send.busy = false;
            match result {
                Ok(stencilcraft_engine::Effect::Hardware(value)) => {
                    if let Some(devices) = value
                        .as_array()
                        .or_else(|| value.get("bluetooth_devices").and_then(|d| d.as_array()))
                    {
                        self.send.devices = devices
                            .iter()
                            .filter_map(|d| {
                                Some((
                                    d["location"].as_str()?.to_owned(),
                                    d["model"].as_str()?.to_owned(),
                                    d["name"].as_str()?.to_owned(),
                                ))
                            })
                            .collect();
                        self.send.device.clear();
                        let unsupported: Vec<_> = devices.iter().filter(|d| d["model"].is_null()).map(|d| format!("{}: direct cutting unsupported; export SVG for the manufacturer software.", d["product"].as_str().unwrap_or("Unknown USB cutter"))).collect();
                        self.send.summary = Some(vec![format!(
                            "{} recognized cutters. Select your cutter below.",
                            self.send.devices.len()
                        )]);
                        if let Some(summary) = &mut self.send.summary {
                            summary.extend(unsupported);
                        }
                    } else {
                        let bytes = value["bytes"].as_u64().unwrap_or_default();
                        self.send.summary = Some(if value["sent"].as_bool() == Some(true) {
                            vec![
                                format!("Sent {bytes} bytes to the cutter."),
                                "Sending finished; physical cutting may still be in progress."
                                    .into(),
                            ]
                        } else {
                            vec![
                                format!(
                                    "Validation passed: {bytes} bytes prepared. Nothing has been sent."
                                ),
                                "When ready, use the Send cut button.".into(),
                            ]
                        });
                    }
                }
                Err(error) => self.send.summary = Some(vec![error]),
                _ => {}
            }
        }
        if self.send.busy {
            let _ =
                Text::new("Communicating with cutter… Use the machine's controls to stop motion.")
                    .show(ui);
            return;
        }
        if let Some(summary) = &self.send.summary {
            let response = ui
                .group(|ui| {
                    let _ = Text::new("Cutter result")
                        .role(TypeRole::BodyStrong)
                        .show(ui);
                    egui::ScrollArea::vertical()
                        .id_salt("cutter-result")
                        .max_height(160.0)
                        .show(ui, |ui| {
                            for line in summary {
                                let _ = Text::new(line.clone()).role(TypeRole::Mono).show(ui);
                            }
                        });
                })
                .response;
            if has_new_result {
                response.scroll_to_me(Some(egui::Align::TOP));
            }
        }
        #[cfg(all(feature = "bluetooth", target_os = "linux"))]
        {
            let previous = self.send.connection_mode;
            let connections = [
                Entry::new(0, "USB"),
                Entry::new(1, "Bluetooth LE"),
                Entry::new(2, "Bluetooth Classic"),
            ];
            let _ = Select::new(
                "Connection",
                Bind::bound(&mut self.send.connection_mode),
                &connections,
            )
            .id_salt("cutter-connection")
            .show(ui);
            if previous != self.send.connection_mode {
                self.send.device.clear();
                self.send.devices.clear();
                self.send.summary = None;
            }
        }
        if self.send.connection_mode == 2 {
            ui.label("Pair the Classic Bluetooth device in system settings; enter its Classic address and RFCOMM channel.");
            ui.horizontal(|ui| {
                ui.label("Bluetooth address");
                ui.text_edit_singleline(&mut self.send.bluetooth_address);
            });
            ui.horizontal(|ui| {
                ui.label("RFCOMM channel");
                ui.add(egui::DragValue::new(&mut self.send.bluetooth_channel).range(1..=30));
            });
        } else {
            let (label, command) = if self.send.connection_mode == 1 {
                (
                    "Find paired Bluetooth cutters",
                    stencilcraft_engine::HardwareCommand::BluetoothDevices,
                )
            } else {
                (
                    "Find USB cutters",
                    stencilcraft_engine::HardwareCommand::Devices,
                )
            };
            if Button::new(label).id_salt("send-find").show(ui).clicked() {
                self.start_hardware(command, ui.ctx().clone());
            }
            let devices: Vec<Entry<String>> = self
                .send
                .devices
                .iter()
                .map(|(location, _, name)| {
                    Entry::new(location.clone(), format!("{name} ({location})"))
                })
                .collect();
            let _ = Select::new(
                "Connected cutter",
                Bind::bound(&mut self.send.device),
                &devices,
            )
            .id_salt("cutter-device")
            .show(ui);
            if let Some((_, model, _)) = self.send.devices.iter().find(|d| d.0 == self.send.device)
                && let Some(model) = stencilcraft_machine::Model::by_id(model)
            {
                self.send.model = model;
            }
        }

        let _ = Text::new("Model").show(ui);
        if self.send.connection_mode == 2 || self.send.device.is_empty() {
            let models: Vec<Entry<stencilcraft_machine::Model>> = MODEL_CHOICES
                .iter()
                .map(|(model, label)| Entry::new(*model, *label))
                .collect();
            let _ = Select::new("Model", Bind::bound(&mut self.send.model), &models)
                .id_salt("send-model")
                .show(ui);
        } else {
            let _ = Text::new(model_label(self.send.model)).show(ui);
        }
        let caps = stencilcraft_machine::DeviceRegistry::builtin().by_id(self.send.model.id());
        if !mat_supported(caps, self.mat) {
            self.mat = stencilcraft_machine::Mat::None;
            self.send.summary = Some(vec![
                "The previous mat is unavailable for this model. Select your loaded mat.".into(),
            ]);
        }
        let mats: Vec<Entry<stencilcraft_machine::Mat>> = MAT_CHOICES
            .iter()
            .filter(|(mat, _)| mat_supported(caps, *mat))
            .map(|(mat, label)| Entry::new(*mat, *label))
            .collect();
        let _ = Select::new("Loaded mat", Bind::bound(&mut self.mat), &mats)
            .id_salt("send-mat")
            .show(ui);
        if self.mat == stencilcraft_machine::Mat::None {
            let _ = Text::new(
                "Matless: backed materials only. Pop-out cardstock is not supported yet.",
            )
            .show(ui);
            let _ = Text::new("Material size (mm)").show(ui);
            let (mut width, mut height) = (
                self.engine.doc.board.width_mm,
                self.engine.doc.board.height_mm,
            );
            ui.horizontal(|ui| {
                ui.label("Width");
                let changed_w = ui
                    .add(
                        egui::DragValue::new(&mut width)
                            .range(1.0..=100000.0)
                            .speed(1.0),
                    )
                    .changed();
                ui.label("Length");
                let changed_h = ui
                    .add(
                        egui::DragValue::new(&mut height)
                            .range(1.0..=100000.0)
                            .speed(1.0),
                    )
                    .changed();
                if changed_w || changed_h {
                    self.run_command(Command::DocumentSetBoard {
                        width_mm: width,
                        height_mm: height,
                    });
                }
            });
            let _ = Text::new("Align the sheet with the left guide, adjust and lock the right roller so both grip the material, then load it. The page edge is shown on the canvas; roller margins are not yet shown.").show(ui);
        }
        let _ = Text::new("Material").show(ui);
        let media: Vec<Entry<usize>> = (0..stencilcraft_machine::MEDIA_PRESETS.len())
            .map(|i| Entry::new(i, media_label(i)))
            .collect();
        let media_edit = Select::new("Material", Bind::bound(&mut self.send.media_index), &media)
            .id_salt("send-material")
            .show(ui);
        if let Some(&index) = media_edit.events.committed()
            && let Some(media) = stencilcraft_machine::MEDIA_PRESETS.get(index)
        {
            self.send.pressure_override = media.pressure.unwrap_or(10);
            self.send.speed_override = media.speed.unwrap_or(5);
        }
        ui.add_space(4.0);
        let _ = Text::new(format!("Pressure  {}", self.send.pressure_override)).show(ui);
        let mut pressure = f64::from(self.send.pressure_override);
        let edit = Slider::new(Bind::given(pressure), 1.0..=33.0)
            .label("Pressure")
            .id_salt("send-pressure")
            .step(1.0)
            .show(ui);
        if apply_edit(&mut pressure, &mut self.pressure_before, &edit.events) {
            self.send.pressure_override = pressure as u8;
        }
        let _ = Text::new(format!("Speed  {}", self.send.speed_override)).show(ui);
        let mut speed = f64::from(self.send.speed_override);
        let edit = Slider::new(Bind::given(speed), 1.0..=30.0)
            .label("Speed")
            .id_salt("send-speed")
            .step(1.0)
            .show(ui);
        if apply_edit(&mut speed, &mut self.speed_before, &edit.events) {
            self.send.speed_override = speed as u8;
        }
        let _ = Text::new("Action").show(ui);
        let actions = [Entry::new(false, "Cut"), Entry::new(true, "Draw with pen")];
        let _ = Select::new("Action", Bind::bound(&mut self.send.draw), &actions)
            .id_salt("send-action")
            .show(ui);
        if self.send.draw
            && self
                .send
                .tool
                .is_some_and(|t| t != stencilcraft_machine::ToolKind::Pen)
        {
            self.send.tool = None;
        }
        if !self.send.draw && self.send.tool == Some(stencilcraft_machine::ToolKind::Pen) {
            self.send.tool = None;
        }
        if self
            .send
            .tool
            .is_some_and(|t| caps.is_none_or(|c| !c.tools.contains(&t)))
        {
            self.send.tool = None;
        }
        let mut tools = vec![Entry::new(None, "Loaded tool (unspecified)")];
        if let Some(caps) = caps {
            for (tool, label) in [
                (stencilcraft_machine::ToolKind::AutoBlade, "AutoBlade"),
                (
                    stencilcraft_machine::ToolKind::RatchetBlade,
                    "Ratchet blade",
                ),
                (
                    stencilcraft_machine::ToolKind::DeepCutBlade,
                    "Deep-cut blade",
                ),
                (stencilcraft_machine::ToolKind::Pen, "Pen"),
            ] {
                if caps.tools.contains(&tool)
                    && ((tool == stencilcraft_machine::ToolKind::Pen) == self.send.draw)
                {
                    tools.push(Entry::new(Some(tool), label));
                }
            }
        }
        let _ = Select::new("Tool", Bind::bound(&mut self.send.tool), &tools)
            .id_salt("send-tool")
            .show(ui);
        ui.horizontal(|ui| {
            ui.label("Tool holder");
            let max = caps.and_then(|c| c.tool_holders).unwrap_or(1).max(1);
            self.send.holder = self.send.holder.clamp(1, max);
            ui.add(egui::DragValue::new(&mut self.send.holder).range(1..=max));
            ui.label("Passes");
            let max_passes = if caps.is_some_and(|c| c.multi_pass) {
                10
            } else {
                1
            };
            self.send.passes = self.send.passes.clamp(1, max_passes);
            ui.add(egui::DragValue::new(&mut self.send.passes).range(1..=max_passes));
        });
        let _ = Checkbox::new(Bind::bound(&mut self.send.mirror))
            .label("Mirror on material")
            .id_salt("send-mirror")
            .show(ui);
        if self.send.draw
            || self
                .send
                .tool
                .is_some_and(|t| t != stencilcraft_machine::ToolKind::AutoBlade)
            || self.send.holder != 1
        {
            self.send.set_blade_depth = false;
        }
        if !self.send.draw
            && self.send.holder == 1
            && self
                .send
                .tool
                .is_none_or(|t| t == stencilcraft_machine::ToolKind::AutoBlade)
        {
            let _ = Checkbox::new(Bind::bound(&mut self.send.set_blade_depth))
                .label("Set AutoBlade depth")
                .id_salt("send-blade-depth")
                .show(ui);
        }
        if self.send.set_blade_depth {
            ui.horizontal(|ui| {
                ui.label("AutoBlade depth");
                ui.add(egui::DragValue::new(&mut self.send.blade_depth).range(0..=10));
            });
        }
        let _ = Checkbox::new(Bind::bound(&mut self.send.bbox_only))
            .label("Bounding box preview")
            .id_salt("send-bbox")
            .show(ui);
        self.send_mode_ui(ui);
        Divider::horizontal().show(ui);
        let selected_connection = if self.send.connection_mode == 2 {
            !self.send.bluetooth_address.trim().is_empty()
        } else {
            !self.send.device.is_empty()
        };
        if selected_connection && !self.send.bbox_only {
            let _ = Text::new(
                "Load the material and selected tool. The action applies to all visible paths.",
            )
            .show(ui);
            let (validate_label, send_label) = if self.send.connection_mode != 0 {
                ("Validate Bluetooth cut", "Send cut via Bluetooth")
            } else {
                ("Validate USB cut", "Send cut to selected USB cutter")
            };
            for (label, send) in [(validate_label, false), (send_label, true)] {
                if Button::new(label)
                    .id_salt(("send-cut", send))
                    .show(ui)
                    .clicked()
                {
                    let jobs = self.send_jobs(send);
                    self.start_hardware_jobs(jobs, ui.ctx().clone());
                }
            }
        }

        if Button::new("Encode dry run")
            .id_salt("send-dry-run")
            .show(ui)
            .clicked()
        {
            self.send.summary = Some(self.encode_send_summary());
            if let Some(summary) = &mut self.send.summary {
                summary.push("Legacy preview: action, tool, holder, passes and mirror are not reflected. Use Validate USB cut for the selected settings.".into());
            }
        }
        self.send_advanced_ui(ui);
        self.custom_materials_ui(ui);
    }

    /// Encode the visible flattened design through the machine crate and
    /// build the summary lines shown in the Send tab (always a dry run).
    pub fn encode_send_summary(&self) -> Vec<String> {
        let mut out = Vec::new();
        let paths = machine_paths(&self.engine.doc);
        if paths.is_empty() {
            out.push("no visible shapes to cut".to_string());
            return out;
        }
        let media = stencilcraft_machine::MEDIA_PRESETS
            .get(self.send.media_index)
            .copied()
            .unwrap_or(stencilcraft_machine::Media {
                id: 300,
                name: "Custom",
                pressure: None,
                speed: None,
                depth: None,
            });
        let mut settings = stencilcraft_machine::CutSettings::from_media(&media);
        settings.pressure = Some(self.send.pressure_override);
        settings.speed = Some(self.send.speed_override);
        settings.mat = self.mat;
        let program = stencilcraft_machine::encode_cut(
            self.send.model,
            &settings,
            &paths,
            &stencilcraft_machine::PlotOptions::default(),
            self.send.bbox_only,
        );
        out.push(format!(
            "{} commands, {} bytes for {}",
            program.commands.len(),
            program.to_stream().len(),
            model_label(self.send.model),
        ));
        if let Some((sd, sr, ed, er)) = program.bbox_mm {
            out.push(format!(
                "cut area {:.0} x {:.0} mm ({} mat)",
                er - sr,
                ed - sd,
                mat_label(self.mat),
            ));
        }
        out.push("program:".to_string());
        for command in program.commands.iter().take(80) {
            out.push(command.to_ascii());
        }
        if program.commands.len() > 80 {
            out.push(format!("… {} more", program.commands.len() - 80));
        }
        out
    }

    pub fn canvas_body(&mut self, ui: &mut egui::Ui) {
        use craft_behavior::geometry::quad::{BoxAllow, BoxPart, BoxRules, BoxState, ScaleRule};
        use craft_draw::ui::pen::PenEvent;
        use craft_ui::canvas::{Canvas, CanvasScrollbars, RulerText, Rulers, ToolHost, tool_pass};
        use craft_ui::canvas_controls::Guides;
        use craft_ui::contract::ShowContainer as _;
        use craft_ui::runtime::env::UnitKind;
        let mut doc = self.engine.doc.clone();
        if let Some(width_mm) = self.kerf_draft {
            doc.kerf.width_mm = width_mm;
        }
        let backdrop = mat_backdrop(self.mat);
        let full_rect = ui.available_rect_before_wrap();
        // The view lies inside the rulers' strips while they show (the canvas lays them out).
        let clip = if self.prefs.show_rulers {
            let (top, left, _) = Rulers::strips(ui, full_rect);
            egui::Rect::from_min_max(egui::pos2(left.max.x, top.max.y), full_rect.max)
        } else {
            full_rect
        };

        let mut view_command = self.pending_view_command.take();
        // A new board size (document setup, New, an agent's `document.set-board`) leaves
        // the old zoom and pan pointing at where the previous sheet was: re-frame it.
        if self.note_board_size(doc.board.width_mm, doc.board.height_mm) && self.view_initialized {
            view_command = Some(CanvasViewCommand::Fit);
        }
        let extent_width = backdrop.width_mm.max(doc.board.width_mm);
        let extent_height = backdrop.height_mm.max(doc.board.height_mm);
        if !self.view_initialized {
            self.view
                .fit_extent(extent_width, extent_height, clip, 24.0);
            self.view_initialized = true;
        }
        if let Some(command) = view_command {
            let selection = stencilcraft_ui_egui::selected_world_bounds(
                &self.engine.doc,
                &self.selected_shapes,
            );
            apply_canvas_view_command(
                &mut self.view,
                command,
                extent_width,
                extent_height,
                selection,
                clip,
            );
        }
        if let Some(zoom) = self.pending_zoom.take() {
            self.view.zoom_at(zoom / self.view.zoom, clip.center());
        }

        let view = self.view;
        let canvas_view = self.view.to_canvas_view(clip);
        let length_unit = stencilcraft_ui_egui::length_unit();
        let unit_kind = match length_unit {
            stencilcraft_ui_egui::LengthUnit::Millimetres => UnitKind::Millimetres,
            stencilcraft_ui_egui::LengthUnit::Centimetres => UnitKind::Centimetres,
            stencilcraft_ui_egui::LengthUnit::Inches => UnitKind::Inches,
        };
        // The canvas publishes the units to its rulers, guides, node overlay and selection box.
        let mm_units = RulerText::new(
            craft_behavior::RulerUnits::new(
                length_unit.per_mm(),
                craft_behavior::Ticks::decimal(false),
            ),
            unit_kind,
        );
        let guides_before = stencilcraft_ui_egui::canvas_guides(&doc.guides, None);
        let guides_shown = stencilcraft_ui_egui::canvas_guides(&doc.guides, self.guide_preview);
        let scroll_document = stencilcraft_ui_egui::scroll_extent(extent_width, extent_height);
        let canvas_id = ui.scope_id().with("stencil-canvas");
        let mut origin = self.ruler_origin;
        let mut canvas = Canvas::new("Canvas")
            .id(canvas_id)
            .view(canvas_view)
            .rulers(mm_units, mm_units);
        if self.prefs.show_rulers {
            canvas = canvas.with_rulers(Some(Rulers::new("Rulers", Bind::bound(&mut origin))));
        }
        let shown = canvas.show_at(ui, full_rect, |ui, body| {
            // The measuring run has no tool pass: the app's canvas runs once, in the real run.
            let pass = tool_pass(&body.env)?;
            // The node overlay's points, handles and lines take their own presses: the app's
            // canvas widget below them is an egui widget, which the pin gives every press over
            // it, so it only hovers there (and for as long as a press that began there lasts).
            let over_node_part = matches!(
                self.active_tool,
                ActiveTool::DirectSelect | ActiveTool::ConvertPoint
            ) && {
                let paths: Vec<EditPath> = self
                    .selected_shapes
                    .iter()
                    .filter_map(|&id| {
                        let shape = self.engine.doc.get_shape(id)?;
                        let subpaths = stencilcraft_ui_egui::bezpath_to_edit_subpaths(&shape.path);
                        Some(EditPath::new(id, subpaths))
                    })
                    .collect();
                let at = ui.input(|input| {
                    input
                        .pointer
                        .press_origin()
                        .filter(|_| input.pointer.any_down())
                        .or(input.pointer.hover_pos())
                });
                at.and_then(|at| {
                    BezierOverlay::hit_test(
                        &paths,
                        &self.path_selection,
                        self.handles_shown,
                        &canvas_view,
                        at,
                        8.0,
                    )
                })
                .is_some()
            };
            // The same for the selection box's handles and rotate band, and the guides.
            let press_or_hover = ui.input(|input| {
                input
                    .pointer
                    .press_origin()
                    .filter(|_| input.pointer.any_down())
                    .or(input.pointer.hover_pos())
            });
            let over_box_part = self.active_tool == ActiveTool::Select
                && press_or_hover.is_some_and(|at| {
                    stencilcraft_ui_egui::selected_world_bounds(
                        &self.engine.doc,
                        &self.selected_shapes,
                    )
                    .is_some_and(|bounds| over_selection_box_part(&view, bounds, at))
                });
            let over_guide = self.prefs.show_guides
                && press_or_hover.is_some_and(|at| over_guide_line(&view, &doc.guides, at));
            let over_node_part = over_node_part || over_box_part || over_guide;
            let sense = if over_node_part {
                egui::Sense::hover()
            } else {
                egui::Sense::click_and_drag()
            };
            self.canvas_hover_only = over_node_part;
            let response = ui.allocate_rect(clip, sense);
            framework::host_canvas(ui.ctx(), response.id);
            let preview_updated = self.kerf_preview && self.update_kerf_preview_cache(&doc);
            if preview_updated
                && let Some(Err(error)) =
                    self.kerf_preview_cache.as_ref().map(|cache| &cache.preview)
            {
                self.status = format!("kerf preview: {error}");
            }
            self.sync_image_textures(ui.ctx());
            let paint_doc = if self.kerf_preview {
                self.kerf_preview_cache
                    .as_ref()
                    .and_then(|cache| cache.preview.as_ref().ok())
                    .unwrap_or(&doc)
            } else {
                &doc
            };
            let view = self.view;
            let painter = ui.painter_at(clip);
            let selection = stencilcraft_ui_egui::SelectionState {
                selected_shapes: self.selected_shapes.clone(),
                overlay: self
                    .canvas_selection_start
                    .zip(ui.ctx().pointer_interact_pos())
                    .map(|(start, end)| {
                        if self.active_tool == ActiveTool::LassoSelect {
                            let mut points = self.canvas_selection_points.clone();
                            if points.last().is_none_or(|last| *last != end) {
                                points.push(end);
                            }
                            stencilcraft_ui_egui::SelectionOverlay::Lasso(points)
                        } else {
                            stencilcraft_ui_egui::SelectionOverlay::Box(egui::Rect::from_two_pos(
                                start, end,
                            ))
                        }
                    }),
            };
            let textures: std::collections::HashMap<u64, egui::TextureId> = self
                .image_textures
                .iter()
                .map(|(id, (_, texture))| (*id, texture.id()))
                .collect();
            stencilcraft_ui_egui::paint_document(
                &painter,
                clip,
                paint_doc,
                &view,
                stencilcraft_ui_egui::CanvasLayers {
                    backdrop: self.prefs.mat_preview.then_some(&backdrop),
                    image_textures: &textures,
                },
                stencilcraft_ui_egui::CanvasOptions {
                    kerf_preview: self.kerf_preview,
                    show_grid: self.prefs.show_grid,
                    print_cut_marks: self.prefs.print_cut_marks,
                    mark_geometry: self.prefs.marks.geometry(),
                    show_selection_bounds: true,
                    theme: self.theme,
                    outline_view: self.prefs.outline_view,
                    cut_border: self.prefs.show_cut_border,
                    print_border: self.prefs.show_print_border,
                    show_dimensions: self.prefs.show_dimensions,
                },
                &selection,
            );
            // The picked image: its outline, moved with a drag in progress.
            if let Some(image) = self
                .selected_image
                .and_then(|id| self.engine.doc.get_image(id))
            {
                let shift = match self.image_drag {
                    Some((id, from)) if id == image.id => ui
                        .ctx()
                        .pointer_interact_pos()
                        .map(|now| self.view.screen_delta_to_world(now - from))
                        .unwrap_or_default(),
                    _ => stencilcraft_geom::Vec2::ZERO,
                };
                let corners = stencilcraft_ui_egui::image_corners(image).map(|p| p + shift);
                let mut outline = stencilcraft_geom::BezPath::new();
                for (i, corner) in corners.into_iter().enumerate() {
                    if i == 0 {
                        outline.move_to(corner);
                    } else {
                        outline.line_to(corner);
                    }
                }
                outline.close_path();
                stencilcraft_ui_egui::paint_preview_paths(
                    &painter,
                    &view,
                    &[outline],
                    stencilcraft_ui_egui::Palette::for_theme(self.theme).selection,
                );
            }
            // The Offset panel's live preview, painted only while that panel is on screen.
            if std::mem::take(&mut self.panels.offset_preview_shown)
                && let Some(Ok(paths)) = self.panels.offset_preview.as_ref().map(|p| &p.result)
            {
                let color = stencilcraft_ui_egui::Palette::for_theme(self.theme).kerf_line;
                stencilcraft_ui_egui::paint_preview_paths(&painter, &view, paths, color);
            }

            // Scroll bars over the mat/board extent (plus a margin): drag one to bring the mat
            // back after scrolling or zooming it out of view.
            let bars =
                CanvasScrollbars::new("Scroll bars", Bind::given(canvas_view), scroll_document)
                    .show_at(ui, clip);
            if let Some(scrolled) = bars.view.value() {
                self.view.from_canvas_view(scrolled);
                ui.ctx().request_repaint();
            }
            let mut guides_claimed = bars.over;

            if self.prefs.show_guides {
                let guides = Guides::new("Guides", canvas_id, Bind::given(guides_shown.clone()))
                    .interactive(true)
                    .show_at(ui, body.rect);
                self.apply_guide_edit(&guides_before, &guides.edit.events);
                // A press on a guide never reaches the canvas below it.
                guides_claimed |= guides.edit.response.hovered()
                    || guides.edit.response.dragged()
                    || !guides.edit.events.is_empty();
            }

            let mut overlay_claimed = guides_claimed;
            overlay_claimed |= self.trace_canvas(ui, &response, &painter);
            overlay_claimed |= self.text_canvas(ui, &response, &painter);
            overlay_claimed |= self.distort_canvas(ui, &response, &painter);
            overlay_claimed |= self.pixscan_canvas(ui, &response, &painter);
            self.paint_effect_preview(&painter);

            // Interactive BezierOverlay for node editing
            if matches!(
                self.active_tool,
                ActiveTool::DirectSelect | ActiveTool::ConvertPoint
            ) {
                let b_tool = if self.active_tool == ActiveTool::ConvertPoint {
                    BezierTool::Convert
                } else {
                    BezierTool::Direct
                };
                let edit_paths: Vec<EditPath> = self
                    .selected_shapes
                    .iter()
                    .filter_map(|&id| {
                        let shape = self.engine.doc.get_shape(id)?;
                        let subpaths = stencilcraft_ui_egui::bezpath_to_edit_subpaths(&shape.path);
                        Some(EditPath::new(id, subpaths))
                    })
                    .collect();

                // Within 8 px an open end is "on" another (as for shape hit tests).
                let join_reach = 8.0 / f64::from(view.zoom.max(f32::EPSILON));
                // Dragged points snap like everything else (grid, guides, other shapes); Ctrl (Cmd)
                // suspends it.
                let snap_off = self.snap_suspended || ui.input(|input| input.modifiers.command);
                let b_resp = BezierOverlay::new(
                    "Paths",
                    Bind::given(edit_paths.clone()),
                    &self.path_selection,
                )
                .tool(b_tool)
                .handles(self.handles_shown)
                .show_at(ui, body.rect);

                // The overlay's parts take their presses before the canvas below them. The
                // app's canvas widget sees the same press, and egui may start its drag a pass
                // before the overlay reports one: a press that began on a point, handle or line
                // is the overlay's for as long as it lasts (it used to start a marquee there and
                // clear the selection).
                let press_on_part = ui
                    .input(|input| input.pointer.press_origin())
                    .and_then(|at| {
                        BezierOverlay::hit_test(
                            &edit_paths,
                            &self.path_selection,
                            self.handles_shown,
                            &canvas_view,
                            at,
                            8.0,
                        )
                    })
                    .is_some();
                overlay_claimed =
                    b_resp.hover.is_some() || !b_resp.edit.events.is_empty() || press_on_part;
                if let Some(sel) = b_resp.select.clone() {
                    if let Some(pid) = sel.path {
                        self.selected_shape = Some(pid);
                        self.selected_shapes = vec![pid];
                    }
                    self.path_selection = sel;
                }
                // The drag's updates preview the path; it reshapes once, on Commit (or a key's
                // Set): one undo step. (`landed` carries an Update's value too.)
                let landed = b_resp.edit.events.committed().is_some();
                let cancelled = b_resp.edit.events.cancel_reason().is_some();
                if cancelled {
                    self.bezier_gesture_base = None;
                    self.bezier_live_path = None;
                } else if let Some(value) = b_resp.change.as_ref() {
                    match landed {
                        false if b_resp.edit.events.is_empty() => {}
                        false => {
                            let pid = value.path();
                            if self.bezier_gesture_base.is_none()
                                && let Some(orig) = self.engine.doc.get_shape(pid)
                            {
                                let base =
                                    stencilcraft_ui_egui::bezpath_to_edit_subpaths(&orig.path);
                                self.bezier_gesture_base = Some((pid, base));
                            }
                            if let Some((base_id, ref base)) = self.bezier_gesture_base
                                && base_id == pid
                            {
                                let mut edited = stencilcraft_ui_egui::apply_path_edit(base, value);
                                snap_moved_anchor(
                                    &mut edited,
                                    value,
                                    &self.engine.doc,
                                    &self.selected_shapes,
                                    join_reach,
                                    snap_off,
                                    self.prefs.snap_within_media,
                                );
                                self.bezier_live_path = Some((
                                    pid,
                                    stencilcraft_ui_egui::edit_subpaths_to_bezpath(&edited),
                                ));
                            }
                        }
                        true => {
                            let pid = value.path();
                            let base = self
                                .bezier_gesture_base
                                .take()
                                .filter(|(id, _)| *id == pid)
                                .map(|(_, b)| b)
                                .or_else(|| {
                                    self.engine.doc.get_shape(pid).map(|s| {
                                        stencilcraft_ui_egui::bezpath_to_edit_subpaths(&s.path)
                                    })
                                });
                            self.bezier_live_path = None;
                            if let Some(base) = base {
                                let mut edited =
                                    stencilcraft_ui_egui::apply_path_edit(&base, value);
                                snap_moved_anchor(
                                    &mut edited,
                                    value,
                                    &self.engine.doc,
                                    &self.selected_shapes,
                                    join_reach,
                                    snap_off,
                                    self.prefs.snap_within_media,
                                );
                                // An open end dropped on another open end rejoins the path.
                                if let PathEdit::MoveAnchors { anchors, .. } = value
                                    && let [moved] = anchors.as_slice()
                                    && let Some(at) = edited
                                        .get(moved.subpath)
                                        .and_then(|s| s.knots.get(moved.knot))
                                        .map(|k| k.anchor)
                                    && let Some(target) = stencilcraft_ui_egui::open_end_near(
                                        &edited, *moved, at, join_reach,
                                    )
                                    && stencilcraft_ui_egui::join_path_ends(
                                        &mut edited,
                                        *moved,
                                        target,
                                    )
                                {
                                    self.path_selection = PathSelection::default();
                                    self.status = "joined path ends".to_string();
                                }
                                let path = stencilcraft_ui_egui::edit_subpaths_to_bezpath(&edited);
                                if !path.elements().is_empty() {
                                    self.run_command(Command::ShapeSetPath { id: pid, path });
                                }
                            }
                        }
                    }
                }
                if let Some((pid, seg, t)) = b_resp.insert
                    && let Some(s) = self.engine.doc.get_shape(pid)
                {
                    let mut subs = stencilcraft_ui_egui::bezpath_to_edit_subpaths(&s.path);
                    if let Some(new_knot) =
                        stencilcraft_ui_egui::insert_path_knot(&mut subs, seg, t)
                    {
                        let path = stencilcraft_ui_egui::edit_subpaths_to_bezpath(&subs);
                        self.run_command(Command::ShapeSetPath { id: pid, path });
                        self.path_selection = PathSelection::new(Some(pid), vec![new_knot], None);
                    }
                }
                // Silhouette: in point editing a click on a line where no point is adds one there.
                if self.active_tool == ActiveTool::DirectSelect
                    && response.clicked()
                    && !ui.input(|input| input.modifiers.shift)
                    && let Some(pointer) = response.interact_pointer_pos()
                    && let Some(BezierHit::Segment {
                        path: pid,
                        segment,
                        t,
                        ..
                    }) = BezierOverlay::hit_test(
                        &edit_paths,
                        &self.path_selection,
                        self.handles_shown,
                        &canvas_view,
                        pointer,
                        6.0,
                    )
                    && let Some(shape) = self.engine.doc.get_shape(pid)
                {
                    let mut subs = stencilcraft_ui_egui::bezpath_to_edit_subpaths(&shape.path);
                    if let Some(new_knot) =
                        stencilcraft_ui_egui::insert_path_knot(&mut subs, segment, t)
                    {
                        let path = stencilcraft_ui_egui::edit_subpaths_to_bezpath(&subs);
                        self.run_command(Command::ShapeSetPath { id: pid, path });
                        self.path_selection = PathSelection::new(Some(pid), vec![new_knot], None);
                        self.status = "added a point".to_string();
                    }
                }
                // A right-click on a point or line selects it for the point menu below.
                if let Some((hit, _)) = &b_resp.context {
                    self.path_selection = match *hit {
                        BezierHit::Anchor { path, knot, .. }
                        | BezierHit::Handle { path, knot, .. } => {
                            PathSelection::new(Some(path), vec![knot], None)
                        }
                        BezierHit::Segment { path, segment, .. } => {
                            PathSelection::new(Some(path), Vec::new(), Some(segment))
                        }
                        _ => self.path_selection.clone(),
                    };
                }
                response.context_menu(|ui| self.point_context_menu(ui));
                if let Some(delete) = b_resp.delete {
                    self.remove_path_points(delete.path, &delete.anchors);
                }
            }

            // The Pen tool: the framework's pen draft, run on the canvas's tool host.
            if self.active_tool == ActiveTool::Pen {
                if std::mem::take(&mut self.pen_undo_requested) {
                    let _ = self.pen_tool.remove_last();
                }
                self.pen_tool
                    .draft(self.pen_draft.as_ref().map(PenDraft::Own));
                let pen_resp = ToolHost::new().run(ui, &pass, &mut self.pen_tool);
                overlay_claimed = overlay_claimed || pen_resp.gesture.is_some();
                for event in self.pen_tool.take_events() {
                    match event {
                        PenEvent::Edit(EditEvent::Update { value, .. }) => {
                            self.pen_draft = Some(value);
                        }
                        PenEvent::Edit(
                            EditEvent::Commit { after, .. } | EditEvent::Set { after, .. },
                        ) => {
                            self.pen_draft = Some(after);
                            self.commit_pen_draft();
                        }
                        PenEvent::Edit(EditEvent::Cancel { .. }) => {
                            self.pen_draft = None;
                        }
                        // The pen ended a draft without a commit: commit what the app holds, as before.
                        PenEvent::Finished(_) if self.pen_draft.is_some() => {
                            self.commit_pen_draft();
                        }
                        _ => {}
                    }
                }
            }

            // The selection box: corner handles scale in proportion and edge handles stretch (as in
            // Silhouette Studio), the band just outside a corner rotates (Shift snaps 15°). Presses
            // inside stay with the move-drag below, which snaps. One `shapes.affine` on release.
            if self.active_tool == ActiveTool::Select
                && !overlay_claimed
                && self.canvas_drag_shape.is_none()
                && self.canvas_selection_start.is_none()
                && let Some(bounds) = stencilcraft_ui_egui::selected_world_bounds(
                    &self.engine.doc,
                    &self.selected_shapes,
                )
                && bounds.width() > 0.0
                && bounds.height() > 0.0
            {
                let state = BoxState::from_rect(bounds);
                let rules = BoxRules::default()
                    .with_scale(if matches!(self.box_part, Some(BoxPart::Corner(_))) {
                        ScaleRule::Proportional
                    } else {
                        ScaleRule::Free
                    })
                    .with_allow(BoxAllow::new(true, false, false).with_warp(false));
                let r = craft_draw::ui::TransformBox::new(
                    "Selection box",
                    Bind::given(state),
                    bounds,
                    0.0,
                )
                .rules(rules)
                .rotate_zone(craft_draw::ui::RotateZone::Band)
                .inside_moves(false)
                .reference_point(false)
                .show_at(ui, body.rect);
                overlay_claimed = overlay_claimed || r.hovered.is_some() || r.held.is_some();
                // Updates preview; the shapes change once, on Commit or Set (`landed` carries an
                // Update's value too, which would apply the box per drag update).
                if let Some(after) = r.edit.events.committed() {
                    self.box_preview = None;
                    self.snap_lines.clear();
                    if let Some(affine) =
                        box_affine(bounds, after).map(|a| self.snap_box_scale(bounds, a))
                        && affine != craft_behavior::kurbo::Affine::IDENTITY
                    {
                        self.run_command(Command::ShapesAffine {
                            ids: self.selected_shapes.clone(),
                            matrix: affine.as_coeffs(),
                        });
                    }
                } else if let Some(value) = r.edit.value() {
                    self.box_preview =
                        box_affine(bounds, value).map(|a| self.snap_box_scale(bounds, a));
                } else if !r.edit.events.is_empty() {
                    self.box_preview = None;
                }
                if self.box_preview.is_none() {
                    self.box_part = r.hovered.or(r.picked);
                }
                if let Some(affine) = self.box_preview {
                    let paths: Vec<_> = self
                        .selected_shapes
                        .iter()
                        .filter_map(|id| self.engine.doc.get_shape(*id))
                        .map(|shape| affine * shape.path.clone())
                        .collect();
                    stencilcraft_ui_egui::paint_preview_paths(
                        &painter,
                        &view,
                        &paths,
                        stencilcraft_ui_egui::Palette::for_theme(self.theme).kerf_line,
                    );
                }
            } else {
                self.box_preview = None;
            }
            Some((response, overlay_claimed))
        });
        self.ruler_origin = origin;
        let Some(Some((response, overlay_claimed))) = shown.inner else {
            return;
        };
        let painter = ui.painter_at(clip);

        // Polyline and Curve: click to place points (Silhouette Studio's tools). Switching to
        // another tool keeps what was placed, like the pen.
        if self
            .click_draft
            .as_ref()
            .is_some_and(|draft| self.active_tool.click_kind() != Some(draft.kind))
        {
            self.commit_click_draft();
        }
        if let Some(kind) = self.active_tool.click_kind() {
            let constrain = ui.input(|input| input.modifiers.shift);
            // A click within 8 px of a point is on it, as for shape hit tests.
            let snap = 8.0 / f64::from(view.zoom.max(f32::EPSILON));
            if !overlay_claimed
                && response.clicked()
                && let Some(pointer) = response.interact_pointer_pos()
            {
                let at = self.view.screen_to_world(pointer.to_vec2());
                // The first click of a double-click placed the point; the second ends the line.
                let finish = response.double_clicked()
                    || self
                        .click_draft
                        .get_or_insert_with(|| click_draw::ClickDraft::new(kind))
                        .click(at, snap, constrain)
                        == click_draw::ClickResult::Closed;
                if finish {
                    self.commit_click_draft();
                }
            }
            if let Some(draft) = &self.click_draft {
                let cursor = response
                    .hover_pos()
                    .map(|pointer| self.view.screen_to_world(pointer.to_vec2()));
                stencilcraft_ui_egui::paint_preview_paths(
                    &painter,
                    &view,
                    &[draft.preview(cursor, constrain)],
                    stencilcraft_ui_egui::Palette::for_theme(self.theme).kerf_line,
                );
                ui.ctx().request_repaint();
            }
        }

        // Arc: drag centre → start sets the radius, then the pointer sweeps the arc and a
        // click places it (Silhouette Studio's Arc tool).
        if self.active_tool != ActiveTool::Arc {
            self.arc_draft = None;
        } else {
            let snap = ui.input(|input| input.modifiers.shift);
            let preview_colour = stencilcraft_ui_egui::Palette::for_theme(self.theme).kerf_line;
            if let Some(mut draft) = self.arc_draft {
                if let Some(pointer) = response.hover_pos() {
                    draft.track(self.view.screen_to_world(pointer.to_vec2()), snap);
                    pointer_readout(
                        ui,
                        pointer,
                        &[
                            ("R", panels::len(draft.radius)),
                            ("∠", format!("{:.0}°", draft.sweep_degrees())),
                        ],
                    );
                }
                self.arc_draft = Some(draft);
                let end = draft.end();
                let mut guides = stencilcraft_geom::BezPath::new();
                guides.move_to(draft.start());
                guides.line_to(draft.centre);
                guides.line_to(end);
                let mut paths = vec![guides];
                paths.extend(draft.path());
                stencilcraft_ui_egui::paint_preview_paths(&painter, &view, &paths, preview_colour);
                ui.ctx().request_repaint();
                if !overlay_claimed && response.clicked() {
                    self.commit_arc_draft();
                }
            } else if !overlay_claimed {
                if response.drag_started()
                    && let Some(press) = ui
                        .input(|input| input.pointer.press_origin())
                        .or(response.interact_pointer_pos())
                {
                    let world_pt =
                        self.snap_world_point(self.view.screen_to_world(press.to_vec2()));
                    self.shape_drag_start = Some(world_pt);
                    self.shape_drag_current = Some(world_pt);
                }
                if response.dragged()
                    && let (Some(start), Some(pointer)) =
                        (self.shape_drag_start, response.interact_pointer_pos())
                {
                    let world_pt =
                        self.snap_world_point(self.view.screen_to_world(pointer.to_vec2()));
                    self.shape_drag_current = Some(world_pt);
                    let mut radius = stencilcraft_geom::BezPath::new();
                    radius.move_to(start);
                    radius.line_to(world_pt);
                    stencilcraft_ui_egui::paint_preview_paths(
                        &painter,
                        &view,
                        &[radius],
                        preview_colour,
                    );
                    pointer_readout(
                        ui,
                        pointer,
                        &[("R", panels::len((world_pt - start).hypot()))],
                    );
                }
                if response.drag_stopped()
                    && let (Some(centre), Some(start)) =
                        (self.shape_drag_start.take(), self.shape_drag_current.take())
                {
                    self.arc_draft = arc_draw::ArcDraft::new(centre, start);
                    ui.ctx().request_repaint();
                }
            }
        }

        // Freehand / Smooth Freehand: the stroke follows the pointer until release.
        if matches!(
            self.active_tool,
            ActiveTool::Freehand | ActiveTool::SmoothFreehand
        ) {
            let px_mm = 1.0 / f64::from(view.zoom.max(f32::EPSILON));
            if !overlay_claimed && response.drag_started() {
                // Start at the press, not where the drag threshold was crossed.
                let mut stroke = freehand::Stroke::default();
                if let Some(press) = ui.input(|input| input.pointer.press_origin()) {
                    stroke.push(self.view.screen_to_world(press.to_vec2()), 0.0);
                }
                self.freehand_stroke = Some(stroke);
            }
            if let (Some(stroke), Some(pointer)) = (
                self.freehand_stroke.as_mut(),
                response.interact_pointer_pos(),
            ) && response.dragged()
            {
                stroke.push(self.view.screen_to_world(pointer.to_vec2()), px_mm);
            }
            if let Some(stroke) = &self.freehand_stroke {
                stencilcraft_ui_egui::paint_preview_paths(
                    &painter,
                    &view,
                    &[click_draw::polyline(&stroke.points, false)],
                    stencilcraft_ui_egui::Palette::for_theme(self.theme).kerf_line,
                );
            }
            if response.drag_stopped()
                && let Some(mut stroke) = self.freehand_stroke.take()
            {
                // End where the button came up, so a loop can meet its start.
                if let Some(pointer) = ui.input(|input| input.pointer.latest_pos()) {
                    stroke.push(self.view.screen_to_world(pointer.to_vec2()), 0.0);
                }
                let smooth = self.active_tool == ActiveTool::SmoothFreehand;
                self.commit_freehand(&stroke, smooth, px_mm);
                ui.ctx().request_repaint();
            }
        } else if self.active_tool != ActiveTool::Eraser {
            self.freehand_stroke = None;
        }

        // Zoom: a click zooms in around the pointer, Alt+click zooms out.
        if self.active_tool == ActiveTool::Zoom
            && !overlay_claimed
            && response.clicked()
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let factor = if ui.input(|input| input.modifiers.alt) {
                1.0 / 1.5
            } else {
                1.5
            };
            self.view.zoom_at(factor, pointer);
        }

        // Knife: drag the cut line across the selection (Shift snaps to 45°).
        if self.active_tool == ActiveTool::Knife && !overlay_claimed {
            let mods = DragMods::from(ui.input(|input| input.modifiers));
            if response.drag_started()
                && let Some(press) = ui.input(|input| input.pointer.press_origin())
            {
                let world_pt = self.snap_world_point(self.view.screen_to_world(press.to_vec2()));
                self.shape_drag_start = Some(world_pt);
                self.shape_drag_current = Some(world_pt);
            }
            if response.dragged()
                && let Some(pointer) = response.interact_pointer_pos()
            {
                let world_pt = self.snap_world_point(self.view.screen_to_world(pointer.to_vec2()));
                self.shape_drag_current = Some(world_pt);
            }
            if let (Some(start), Some(current)) = (self.shape_drag_start, self.shape_drag_current) {
                let end = line_end(start, current, mods);
                let mut cut = stencilcraft_geom::BezPath::new();
                cut.move_to(start);
                cut.line_to(end);
                stencilcraft_ui_egui::paint_preview_paths(
                    &painter,
                    &view,
                    &[cut],
                    stencilcraft_ui_egui::Palette::for_theme(self.theme).kerf_line,
                );
                if let Some(pointer) = response.interact_pointer_pos() {
                    pointer_readout(ui, pointer, &[("L", panels::len((end - start).hypot()))]);
                }
            }
            if response.drag_stopped()
                && let (Some(start), Some(current)) =
                    (self.shape_drag_start.take(), self.shape_drag_current.take())
            {
                self.knife_cut(start, line_end(start, current, mods));
                ui.ctx().request_repaint();
            }
        }

        // Eraser: the stroke follows the pointer; the brush is shown under it.
        if self.active_tool == ActiveTool::Eraser {
            let px_mm = 1.0 / f64::from(view.zoom.max(f32::EPSILON));
            if !overlay_claimed && response.drag_started() {
                let mut stroke = freehand::Stroke::default();
                if let Some(press) = ui.input(|input| input.pointer.press_origin()) {
                    stroke.push(self.view.screen_to_world(press.to_vec2()), 0.0);
                }
                self.freehand_stroke = Some(stroke);
            }
            if let (Some(stroke), Some(pointer)) = (
                self.freehand_stroke.as_mut(),
                response.interact_pointer_pos(),
            ) && response.dragged()
            {
                stroke.push(self.view.screen_to_world(pointer.to_vec2()), px_mm);
            }
            let brush = if self.panels.eraser_round {
                stencilcraft_engine::cut_tools::BrushShape::Round
            } else {
                stencilcraft_engine::cut_tools::BrushShape::Square
            };
            let size = if self.panels.eraser_size_mm.is_finite() {
                self.panels.eraser_size_mm.clamp(0.01, 500.0)
            } else {
                5.0
            };
            let mut preview = Vec::new();
            if let Some(stroke) = &self.freehand_stroke {
                preview.push(stencilcraft_engine::cut_tools::brush_region(
                    &stroke.points,
                    size,
                    brush,
                ));
            } else if let Some(pointer) = response.hover_pos() {
                let at = self.view.screen_to_world(pointer.to_vec2());
                preview.push(stencilcraft_engine::cut_tools::brush_region(
                    &[at],
                    size,
                    brush,
                ));
                ui.ctx().request_repaint();
            }
            stencilcraft_ui_egui::paint_preview_paths(
                &painter,
                &view,
                &preview,
                stencilcraft_ui_egui::Palette::for_theme(self.theme).kerf_line,
            );
            if response.drag_stopped()
                && let Some(mut stroke) = self.freehand_stroke.take()
            {
                if let Some(pointer) = ui.input(|input| input.pointer.latest_pos()) {
                    stroke.push(self.view.screen_to_world(pointer.to_vec2()), px_mm);
                }
                self.erase_stroke(&stroke.points);
                ui.ctx().request_repaint();
            }
        }

        // Live shape creation via drag
        if self.active_tool.is_shape_creator() && !overlay_claimed {
            if response.drag_started()
                && let Some(press) = ui
                    .input(|input| input.pointer.press_origin())
                    .or(response.interact_pointer_pos())
            {
                let world_pt = self.snap_world_point(self.view.screen_to_world(press.to_vec2()));
                self.shape_drag_start = Some(world_pt);
                self.shape_drag_current = Some(world_pt);
            }
            let mods = DragMods::from(ui.input(|input| input.modifiers));
            if response.dragged()
                && let Some(pointer) = response.interact_pointer_pos()
            {
                let world_pt = self.snap_world_point(self.view.screen_to_world(pointer.to_vec2()));
                self.shape_drag_current = Some(world_pt);
                if let Some(start) = self.shape_drag_start {
                    if self.active_tool == ActiveTool::Line {
                        let run = line_end(start, world_pt, mods) - start;
                        // Screen y grows downward; report the angle the way it looks.
                        // `+ 0.0` turns -0 into 0 so a level line reads 0°, not -0°.
                        let angle = (-run.y).atan2(run.x).to_degrees() + 0.0;
                        pointer_readout(
                            ui,
                            pointer,
                            &[
                                ("L", panels::len(run.hypot())),
                                ("∠", format!("{angle:.0}°")),
                            ],
                        );
                    } else {
                        let drag = drag_box(start, world_pt, mods);
                        let (w, h) = (drag.width(), drag.height());
                        pointer_readout(
                            ui,
                            pointer,
                            &[("W", panels::len(w)), ("H", panels::len(h))],
                        );
                    }
                }
            }
            if let (Some(start), Some(end)) = (self.shape_drag_start, self.shape_drag_current)
                && let Some(command) = self.dragged_shape_command(start, end, mods)
            {
                let mut preview =
                    stencilcraft_engine::Engine::new(stencilcraft_doc::Document::default());
                if preview.run(command).is_ok() {
                    let paths: Vec<_> = preview
                        .doc
                        .layers
                        .iter()
                        .flat_map(|layer| layer.shapes.iter().map(|shape| shape.path.clone()))
                        .collect();
                    stencilcraft_ui_egui::paint_preview_paths(
                        &painter,
                        &view,
                        &paths,
                        stencilcraft_ui_egui::Palette::for_theme(self.theme).kerf_line,
                    );
                }
            }
            if response.drag_stopped()
                && let (Some(start), Some(end)) =
                    (self.shape_drag_start.take(), self.shape_drag_current.take())
            {
                self.commit_dragged_shape(start, end, mods);
                ui.ctx().request_repaint();
            }
        }

        let (space_down, middle_down, primary_down, pointer_delta) = ui.ctx().input(|input| {
            (
                input.key_down(egui::Key::Space),
                input.pointer.middle_down(),
                input.pointer.primary_down(),
                input.pointer.delta(),
            )
        });
        let pan_requested = middle_down
            || (space_down && primary_down)
            || (self.active_tool == ActiveTool::Hand && primary_down);
        if pan_requested && response.hovered() {
            self.canvas_panning = true;
            self.canvas_drag_shape = None;
        } else if !pan_requested {
            self.canvas_panning = false;
        }

        if self.canvas_panning {
            if pointer_delta != egui::Vec2::ZERO {
                self.view.pan_mm -= pointer_delta / self.view.zoom;
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if !overlay_claimed
            && !self.active_tool.is_shape_creator()
            && self.active_tool.click_kind().is_none()
            && !matches!(
                self.active_tool,
                ActiveTool::Arc
                    | ActiveTool::Freehand
                    | ActiveTool::SmoothFreehand
                    | ActiveTool::Knife
                    | ActiveTool::Eraser
                    | ActiveTool::Zoom
            )
        {
            if response.drag_started()
                && let Some(pointer) = response.interact_pointer_pos()
            {
                let hit = stencilcraft_ui_egui::hit_test_shape(&doc, &view, pointer, 8.0);
                self.canvas_selection_additive = ui.ctx().input(|input| input.modifiers.shift);
                let lasso = self.active_tool == ActiveTool::LassoSelect;
                self.canvas_selection_base = if self.canvas_selection_additive {
                    self.selected_shapes.clone()
                } else {
                    Vec::new()
                };
                self.canvas_drag_shape = if lasso || self.canvas_selection_additive {
                    None
                } else {
                    hit.filter(|id| self.selected_shapes.contains(id))
                };
                if lasso {
                    self.canvas_selection_start = Some(pointer);
                    self.canvas_selection_points.clear();
                    self.canvas_selection_points.push(pointer);
                    if !self.canvas_selection_additive {
                        self.set_selection([]);
                    }
                } else if let Some(id) = hit
                    && !self.canvas_selection_additive
                {
                    if !self.selected_shapes.contains(&id) {
                        self.set_selection([id]);
                        self.canvas_drag_shape = Some(id);
                    }
                } else if !self.canvas_selection_additive
                    && let press = ui
                        .input(|input| input.pointer.press_origin())
                        .unwrap_or(pointer)
                    && let Some(image) = stencilcraft_ui_egui::hit_test_image(&doc, &view, press)
                {
                    // A press on a placed image (and no shape) drags the image. The drag starts
                    // only once the pointer has moved, so test where the button went down.
                    self.set_selection([]);
                    self.selected_image = Some(image);
                    self.image_drag = Some((image, press));
                } else {
                    self.canvas_selection_start = Some(pointer);
                    if !self.canvas_selection_additive {
                        self.set_selection([]);
                        self.selected_image = None;
                    }
                }
                self.last_drag_delta = egui::Vec2::ZERO;
            } else if response.clicked()
                && let Some(pointer) = response.interact_pointer_pos()
            {
                let hit = stencilcraft_ui_egui::hit_test_shape(&doc, &view, pointer, 8.0);
                if ui.ctx().input(|input| input.modifiers.shift) {
                    let mut ids = self.selected_shapes.clone();
                    if let Some(id) = hit {
                        // Toggle the shape's whole group, which is selected as one object.
                        let members = self.with_group(id);
                        if ids.contains(&id) {
                            ids.retain(|selected| !members.contains(selected));
                        } else {
                            ids.extend(members);
                        }
                    }
                    self.set_selection(ids);
                } else {
                    self.set_selection(hit);
                    // No shape there: a placed image under the click is picked instead.
                    self.selected_image = if hit.is_none() {
                        stencilcraft_ui_egui::hit_test_image(&doc, &view, pointer)
                    } else {
                        None
                    };
                }
            }
            // Silhouette's point editing: double-click a shape to edit its points, double-click
            // empty canvas to leave.
            // Over the selection box's or the overlay's parts the canvas widget only hovers, so a
            // double click there is read from the input (the parts take drags, not double clicks).
            let double_click_at = if response.double_clicked() {
                response.interact_pointer_pos()
            } else if self.canvas_hover_only {
                ui.input(|input| {
                    input
                        .pointer
                        .button_double_clicked(egui::PointerButton::Primary)
                        .then(|| input.pointer.interact_pos())
                        .flatten()
                })
                .filter(|at| clip.contains(*at))
            } else {
                None
            };
            if let Some(pointer) = double_click_at {
                let hit = stencilcraft_ui_egui::hit_test_shape(&doc, &view, pointer, 8.0);
                let text_hit = hit.filter(|id| {
                    self.engine
                        .doc
                        .get_shape(*id)
                        .is_some_and(|shape| shape.text.is_some())
                });
                match (self.active_tool, hit) {
                    // Double-clicking text edits it, as in Silhouette.
                    (ActiveTool::Select, Some(_)) if text_hit.is_some() => {
                        if let Some(id) = text_hit {
                            let p = self.view.screen_to_world(pointer.to_vec2());
                            self.begin_text_edit(id, None);
                            self.place_text_caret(p);
                        }
                    }
                    (ActiveTool::Select, Some(id)) => {
                        self.set_selection([id]);
                        self.path_selection = PathSelection::default();
                        self.active_tool = ActiveTool::DirectSelect;
                        self.status = "editing points: double-click empty canvas to finish".into();
                    }
                    (ActiveTool::DirectSelect, None) => {
                        self.path_selection = PathSelection::default();
                        self.active_tool = ActiveTool::Select;
                    }
                    _ => {}
                }
            }

            if response.drag_started() {
                self.drag_steps = 0;
                self.drag_origin = self.canvas_drag_shape.and_then(|_| {
                    stencilcraft_ui_egui::selected_world_bounds(
                        &self.engine.doc,
                        &self.selected_shapes,
                    )
                });
                self.snap_lines.clear();
            }
            if response.dragged() && !self.canvas_panning {
                if let Some(id) = self.canvas_drag_shape {
                    let total = response.total_drag_delta().unwrap_or_default();
                    self.drag_selection_to(id, total);
                } else if self.canvas_selection_start.is_none() && self.image_drag.is_none() {
                    self.view.pan_mm -= response.drag_delta() / self.view.zoom;
                }
                // Live position callout while a selection is dragged.
                if self.canvas_drag_shape.is_some()
                    && let Some(b) = stencilcraft_ui_egui::selected_world_bounds(
                        &self.engine.doc,
                        &self.selected_shapes,
                    )
                    && let Some(pointer) = response.interact_pointer_pos()
                {
                    pointer_readout(
                        ui,
                        pointer,
                        &[
                            ("X", panels::len(b.x0)),
                            ("Y", panels::len(b.y0)),
                            ("W", panels::len(b.width())),
                            ("H", panels::len(b.height())),
                        ],
                    );
                }
            }
            if response.dragged()
                && let Some(start) = self.canvas_selection_start
                && let Some(end) = response.interact_pointer_pos()
            {
                let found = if self.active_tool == ActiveTool::LassoSelect {
                    if self
                        .canvas_selection_points
                        .last()
                        .is_none_or(|last| last.distance(end) >= 3.0)
                    {
                        self.canvas_selection_points.push(end);
                    }
                    stencilcraft_ui_egui::select_shapes_in_lasso(
                        &doc,
                        &view,
                        &self.canvas_selection_points,
                        stencilcraft_ui_egui::SelectionMode::Intersect,
                    )
                } else {
                    // Silhouette's default: shapes the box touches, whichever way it is drawn.
                    let rect = egui::Rect::from_two_pos(start, end);
                    stencilcraft_ui_egui::select_shapes_in_rect(
                        &doc,
                        &view,
                        rect,
                        stencilcraft_ui_egui::SelectionMode::Intersect,
                    )
                };
                // From the selection before the drag, so sweeping back drops shapes again.
                let mut ids = self.canvas_selection_base.clone();
                ids.extend(
                    found
                        .into_iter()
                        .filter(|id| !self.canvas_selection_base.contains(id)),
                );
                self.set_selection(ids);
            }
            // Silhouette's right-click menu: a right-click on a shape selects it (and its group)
            // first, then the menu acts on the selection.
            if response.secondary_clicked()
                && let Some(pointer) = response.interact_pointer_pos()
                && let Some(id) = stencilcraft_ui_egui::hit_test_shape(&doc, &view, pointer, 8.0)
                && !self.selected_shapes.contains(&id)
            {
                self.set_selection([id]);
            }
            response.context_menu(|ui| self.shape_context_menu(ui));
            if response.drag_stopped() {
                if self.canvas_drag_shape.is_some() {
                    self.fold_drag_into_one_step();
                }
                if let Some((id, from)) = self.image_drag.take()
                    && let Some(to) = response.interact_pointer_pos()
                {
                    self.move_image(id, self.view.screen_delta_to_world(to - from));
                }
                self.canvas_drag_shape = None;
                self.canvas_selection_base.clear();
                self.canvas_selection_start = None;
                self.canvas_selection_points.clear();
                self.last_drag_delta = egui::Vec2::ZERO;
                self.drag_origin = None;
                self.snap_lines.clear();
            }
        }
        if response.drag_stopped() {
            self.snap_lines.clear();
        }
        if self.prefs.smart_guides && !self.snap_lines.is_empty() {
            // The smart guide colour of the canvas controls: the theme's `data.magenta`.
            let env = craft_ui::runtime::env::Env::of(ui);
            let style = craft_ui::style::StyleCx::of_env(ui.ctx(), &env);
            let color = style.color(style.tokens.color.data.magenta).egui();
            stencilcraft_ui_egui::paint_snap_lines(&painter, &self.view, &self.snap_lines, color);
        }

        // Zoom: scroll wheel around the pointer, only over the visible canvas (the panels
        // beside it scroll their own content).
        let over_canvas = response.hovered()
            && ui
                .ctx()
                .pointer_hover_pos()
                .is_some_and(|p| clip.contains(p));
        if over_canvas {
            let scroll = ui.ctx().input(|input| input.smooth_scroll_delta.y);
            if scroll != 0.0 {
                let pointer = response.hover_pos().unwrap_or(clip.center());
                self.view.zoom_at((scroll / 400.0).exp(), pointer);
            }
        }
    }

    pub fn status_bar(&mut self, parent: &mut egui::Ui) {
        let unit = stencilcraft_ui_egui::length_unit();
        let info = format!(
            "{:.*} × {:.*} {} | kerf {} | {} shapes",
            unit.decimals(),
            unit.from_mm(self.engine.doc.board.width_mm),
            unit.decimals(),
            unit.from_mm(self.engine.doc.board.height_mm),
            unit.symbol(),
            unit.format(self.engine.doc.kerf.width_mm, 2),
            self.engine.doc.shape_count()
        );
        let status_msg = self.status.clone();
        let mut zoom_value = f64::from((self.view.zoom * 100.0).round().clamp(5.0, 40000.0));

        egui::Panel::bottom("status")
            .frame(egui::Frame::NONE)
            .show_separator_line(false)
            .show(parent, |ui| {
                // The old bar's leading and trailing cells, in reading order in the bar's body.
                let _ = Bar::new("Status", BarKind::Status)
                    .id_salt("stencilcraft-status")
                    .show(ui, |ui, _| {
                        if Button::new("Fit")
                            .id_salt("status-fit")
                            .size(ControlSize::Sm)
                            .tooltip("Fit the mat and board (F)")
                            .show(ui)
                            .clicked()
                        {
                            self.pending_view_command = Some(CanvasViewCommand::Fit);
                        }
                        if Button::new("Center")
                            .id_salt("status-center")
                            .size(ControlSize::Sm)
                            .tooltip("Center the mat and board (C)")
                            .show(ui)
                            .clicked()
                        {
                            self.pending_view_command = Some(CanvasViewCommand::Center);
                        }
                        if IconButton::new(IconId::new("minus"))
                            .label("Zoom out")
                            .id_salt("status-zoom-out")
                            .show(ui)
                            .clicked()
                        {
                            self.pending_zoom = Some((self.view.zoom / 1.2).clamp(0.05, 400.0));
                        }
                        let mut shown_zoom = zoom_value;
                        let edit = Slider::new(Bind::bound(&mut shown_zoom), 5.0..=40000.0)
                            .label("Zoom")
                            .id_salt("canvas-zoom")
                            .scale(SliderScale::Log)
                            .step(1.0)
                            .show(ui);
                        if apply_edit(&mut zoom_value, &mut self.zoom_before, &edit.events) {
                            self.pending_zoom =
                                Some((zoom_value as f32 / 100.0).clamp(0.05, 400.0));
                        }
                        let _ = Text::new(format!("{zoom_value:.0}%")).show(ui);
                        if IconButton::new(IconId::new("plus"))
                            .label("Zoom in")
                            .id_salt("status-zoom-in")
                            .show(ui)
                            .clicked()
                        {
                            self.pending_zoom = Some((self.view.zoom * 1.2).clamp(0.05, 400.0));
                        }
                        Divider::vertical().show(ui);
                        let _ = Text::new(info.clone()).shade(Shade::Muted).show(ui);
                        let _ = StatusMessage::new(status_msg.clone()).show(ui);
                        Divider::vertical().show(ui);
                        let _ = Text::new(format!("Tool: {}", self.active_tool.label()))
                            .tooltip("Space+drag or middle-drag to pan · wheel to zoom")
                            .show(ui);
                    });
            });
    }
}

/// The snapping switches the View menu flips (`snap.set` fields).
#[derive(Default)]
struct SnapToggles {
    grid: Option<bool>,
    objects: Option<bool>,
    spacing: Option<bool>,
    points: Option<bool>,
}

/// Apply a view command: `width_mm` × `height_mm` is the extent Fit and Center frame,
/// `selection` the selection's bounds for Zoom to Selection.
pub fn apply_canvas_view_command(
    view: &mut stencilcraft_ui_egui::View,
    command: CanvasViewCommand,
    width_mm: f64,
    height_mm: f64,
    selection: Option<stencilcraft_geom::Rect>,
    viewport: egui::Rect,
) {
    match command {
        CanvasViewCommand::Fit => {
            view.fit_extent(width_mm, height_mm, viewport, 24.0);
        }
        CanvasViewCommand::Center => {
            view.center_extent(width_mm, height_mm, viewport);
        }
        CanvasViewCommand::Selection => {
            if let Some(rect) = selection {
                view.fit_rect(rect, viewport, 24.0);
            }
        }
        CanvasViewCommand::ZoomPercent(percent) => {
            let zoom = f32::from(percent) / 100.0;
            view.zoom_at(zoom / view.zoom, viewport.center());
        }
        CanvasViewCommand::Rotate(turns) => view.set_rotation(turns, viewport),
    }
}

/// A button of the Tools panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ToolsAction {
    Deselect,
    Duplicate,
    Delete,
    Rectangle,
    Image,
    Text,
}

/// A quick transform button in the inspector's Transform section.
#[derive(Clone, Copy, Debug, PartialEq)]
enum ShapeAction {
    Rotate(f64),
    Scale(f64),
}

/// The engine command an [`AlignBar`] pick runs.
#[derive(Clone, Copy, Debug, PartialEq)]
enum AlignCommand {
    Align(Alignment),
    Distribute(DistributionAxis),
}

/// The engine command behind `action`; `None` for the edge-distribute variants the engine does not
/// have (the bar shows them disabled).
fn align_command(action: AlignAction) -> Option<AlignCommand> {
    Some(match action {
        AlignAction::AlignLeftEdges => AlignCommand::Align(Alignment::Left),
        AlignAction::AlignHorizontalCenters => AlignCommand::Align(Alignment::HorizontalCenter),
        AlignAction::AlignRightEdges => AlignCommand::Align(Alignment::Right),
        AlignAction::AlignTopEdges => AlignCommand::Align(Alignment::Top),
        AlignAction::AlignVerticalCenters => AlignCommand::Align(Alignment::VerticalCenter),
        AlignAction::AlignBottomEdges => AlignCommand::Align(Alignment::Bottom),
        AlignAction::DistributeHorizontally => {
            AlignCommand::Distribute(DistributionAxis::Horizontal)
        }
        AlignAction::DistributeVertically => AlignCommand::Distribute(DistributionAxis::Vertical),
        _ => return None,
    })
}

/// Legacy control calls are accepted without IDs; JSON-RPC calls must be valid requests.
fn window_control_request(value: &serde_json::Value) -> bool {
    if value.get("jsonrpc").is_none() {
        return true;
    }
    craft_jsonrpc::request(value, stencilcraft_mcp::rpc_limits()).is_some_and(|request| {
        request.id.is_some() && request.params.is_none_or(serde_json::Value::is_object)
    })
}

/// The output path of an `app.screenshot` control request (`{"method":"app.screenshot",
/// "params":{"path":…}}`), the one control method the window answers itself: the MCP session
/// is headless and has no frame to capture.
fn screenshot_request(line: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if !window_control_request(&value) || value.get("method")?.as_str()? != "app.screenshot" {
        return None;
    }
    Some(value.get("params")?.get("path")?.as_str()?.to_string())
}

/// The method and path of a `doc.open` or `doc.save` request (legacy envelope or the MCP
/// `doc_open`/`doc_save` tools) that `reply` answered successfully.
fn document_path_request(line: &str, reply: &str) -> Option<(&'static str, String)> {
    let request: serde_json::Value = serde_json::from_str(line).ok()?;
    let reply: serde_json::Value = serde_json::from_str(reply).ok()?;
    let (method, params) = match request.get("method")?.as_str()? {
        "doc.open" => ("doc.open", request.get("params")?),
        "doc.save" => ("doc.save", request.get("params")?),
        "tools/call" => {
            let params = request.get("params")?;
            let method = match params.get("name")?.as_str()? {
                "doc_open" => "doc.open",
                "doc_save" => "doc.save",
                _ => return None,
            };
            (method, params.get("arguments")?)
        }
        _ => return None,
    };
    let failed = reply.get("ok") == Some(&serde_json::Value::Bool(false))
        || reply.get("error").is_some()
        || reply.pointer("/result/isError") == Some(&serde_json::Value::Bool(true));
    if failed {
        return None;
    }
    Some((method, params.get("path")?.as_str()?.to_string()))
}

/// The request id and shape ids of an `app.select` control request
/// (`{"method":"app.select","params":{"ids":[1,2]}}`); at most 10 000 ids are read.
fn select_request(line: &str) -> Option<(serde_json::Value, Vec<u64>)> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if !window_control_request(&value) || value.get("method")?.as_str()? != "app.select" {
        return None;
    }
    let ids = value
        .get("params")?
        .get("ids")?
        .as_array()?
        .iter()
        .take(10_000)
        .filter_map(serde_json::Value::as_u64)
        .collect();
    Some((value.get("id").cloned().unwrap_or_default(), ids))
}

/// The request id, kerf-preview switch and colour theme of an `app.view` control request
/// (`{"method":"app.view","params":{"kerf_preview":true,"theme":"light"}}`); each setting is
/// optional and an unrecognised theme name leaves the theme alone.
fn view_request(
    line: &str,
) -> Option<(
    serde_json::Value,
    Option<bool>,
    Option<stencilcraft_ui_egui::ColorTheme>,
)> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if !window_control_request(&value) || value.get("method")?.as_str()? != "app.view" {
        return None;
    }
    let params = value.get("params");
    let kerf_preview = params
        .and_then(|p| p.get("kerf_preview"))
        .and_then(serde_json::Value::as_bool);
    let theme = match params.and_then(|p| p.get("theme")).and_then(|t| t.as_str()) {
        Some("dark") => Some(stencilcraft_ui_egui::ColorTheme::Dark),
        Some("light") => Some(stencilcraft_ui_egui::ColorTheme::Light),
        _ => None,
    };
    Some((
        value.get("id").cloned().unwrap_or_default(),
        kerf_preview,
        theme,
    ))
}

/// Capture the next painted frame to a PNG at `path`, then answer `job`.
fn request_screenshot(ctx: &egui::Context, path: String, job: control::Job) {
    let id = serde_json::from_str::<serde_json::Value>(&job.line)
        .ok()
        .and_then(|v| v.get("id").cloned())
        .unwrap_or(serde_json::Value::Null);
    let callback = egui::ScreenshotCallback::new(move |image| {
        let [w, h] = image.size;
        let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
        let saved = u32::try_from(w)
            .ok()
            .zip(u32::try_from(h).ok())
            .and_then(|(w, h)| craft_raster_codecs::common::Pixels::from_raw(w, h, bytes))
            .ok_or_else(|| "screenshot: malformed image".to_string())
            .and_then(|img| {
                craft_raster_codecs::common::encode_for_path(&path, &img)
                    .map_err(|e| format!("screenshot: {path}: {e}"))
                    .and_then(|bytes| {
                        std::fs::write(&path, bytes).map_err(|e| format!("screenshot: {path}: {e}"))
                    })
            });
        let reply = match saved {
            Ok(()) => {
                serde_json::json!({"id": id, "ok": true, "result": {"path": path, "size": [w, h]}})
            }
            Err(e) => serde_json::json!({"id": id, "ok": false, "error": e}),
        };
        let _ = job.reply.send(reply.to_string());
    });
    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(callback));
    ctx.request_repaint();
}

impl eframe::App for StencilApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "recent_files", &self.recent_files);
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, _raw_input: &mut egui::RawInput) {
        framework::before_input(ctx, self.theme);
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        framework::close_path(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        framework::before_ui(ui.ctx(), frame);
        self.drain_control_requests(ui.ctx());
        self.handle_keyboard_shortcuts(ui.ctx());
        self.autosave_tick();
        if std::mem::take(&mut self.quit_requested) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }

        self.top_panel(ui);
        self.options_bar(ui);
        self.status_bar(ui);

        let slots = build_tool_slots();
        let rail_out = tool_rail::show(
            ui,
            "stencilcraft-tool-rail",
            usize::from(self.prefs.rail_columns),
            "Toggle tool rail columns",
            &slots,
            &self.active_tool,
            &mut self.rail_last_used,
        );
        if let Some(tool) = rail_out.select {
            self.active_tool = tool;
        }
        if rail_out.toggle_columns {
            self.prefs.rail_columns = if self.prefs.rail_columns == 1 { 2 } else { 1 };
            self.save_prefs();
        }

        let mut workspace = Arc::clone(&self.workspace);
        let specs = std::mem::take(&mut self.panel_specs);
        let mut content = WorkspaceContent { app: self };
        let output = DockArea::new(
            "StencilCraft workspace",
            Bind::bound(&mut workspace),
            &specs,
            &mut content,
            DockConfig::default(),
        )
        .show(ui);
        if !output.requested.is_empty()
            && let Err(error) =
                Arc::make_mut(&mut workspace).apply_all(output.requested, false, panel_permissions)
        {
            content.app.status = format!("workspace layout rejected: {error}");
        }
        content.app.workspace = workspace;
        content.app.panel_specs = specs;
        if let Some(panel) = content.app.pending_panel.take() {
            content
                .app
                .handle_menu_action(TopMenuAction::ShowPanel(panel));
        }
        content.app.retain_valid_selection();
        content.app.new_document_dialog(ui);
        ui.ctx().request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use craft_behavior::dock::{Action, DockPlacement};

    #[test]
    fn mats_follow_machine_capabilities() {
        use stencilcraft_machine::{DeviceRegistry, Mat};
        let registry = DeviceRegistry::builtin();
        let cameo = registry.by_id("cameo-4");
        let pro = registry.by_id("cameo-4-pro");
        assert!(mat_supported(cameo, Mat::Cameo12x12));
        assert!(!mat_supported(cameo, Mat::CameoPro24x24));
        assert!(mat_supported(pro, Mat::CameoPro24x24));
        assert!(mat_supported(cameo, Mat::None));
        assert!(!mat_supported(None, Mat::Cameo12x12));
    }

    #[test]
    fn window_control_methods_parse_and_ignore_everything_else() {
        assert_eq!(
            screenshot_request(
                r#"{"id":1,"method":"app.screenshot","params":{"path":"/tmp/a.png"}}"#
            ),
            Some("/tmp/a.png".to_string())
        );
        assert_eq!(
            screenshot_request(r#"{"id":1,"method":"doc.summary"}"#),
            None
        );
        assert_eq!(screenshot_request("not json"), None);
        assert_eq!(
            screenshot_request(
                r#"{"jsonrpc":"2.0","method":"app.screenshot","params":{"path":"/tmp/a.png"}}"#
            ),
            None
        );
        assert_eq!(
            select_request(r#"{"jsonrpc":"2.0","method":"app.select","params":{"ids":[1]}}"#),
            None
        );
        assert!(
            view_request(r#"{"jsonrpc":"2.0","method":"app.view","params":{"theme":"dark"}}"#)
                .is_none()
        );
        assert_eq!(
            screenshot_request(
                r#"{"jsonrpc":"2.0","id":false,"method":"app.screenshot","params":{"path":"/tmp/a.png"}}"#
            ),
            None
        );
        assert_eq!(
            screenshot_request(
                r#"{"jsonrpc":"2.0","id":1,"method":"app.screenshot","params":{"path":"/tmp/a.png"}}"#
            ),
            Some("/tmp/a.png".to_string())
        );
        let (id, ids) =
            select_request(r#"{"id":7,"method":"app.select","params":{"ids":[1,"x",3]}}"#).unwrap();
        assert_eq!((id, ids), (serde_json::json!(7), vec![1, 3]));
        assert_eq!(
            select_request(r#"{"method":"app.select","params":{}}"#),
            None
        );
        let (id, kerf, theme) = view_request(
            r#"{"id":2,"method":"app.view","params":{"kerf_preview":true,"theme":"light"}}"#,
        )
        .unwrap();
        assert_eq!((id, kerf), (serde_json::json!(2), Some(true)));
        assert!(theme == Some(stencilcraft_ui_egui::ColorTheme::Light));
        let (_, kerf, theme) =
            view_request(r#"{"method":"app.view","params":{"theme":"pink"}}"#).unwrap();
        assert!(kerf.is_none() && theme.is_none());
        assert!(view_request(r#"{"method":"app.select","params":{}}"#).is_none());
        let open = r#"{"id":1,"method":"doc.open","params":{"path":"a.stencilcraft"}}"#;
        assert_eq!(
            document_path_request(open, r#"{"id":1,"ok":true,"result":{}}"#),
            Some(("doc.open", "a.stencilcraft".to_string()))
        );
        assert_eq!(
            document_path_request(open, r#"{"id":1,"ok":false,"error":"x"}"#),
            None
        );
        let tool = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"doc_save","arguments":{"path":"b.stencilcraft"}}}"#;
        assert_eq!(
            document_path_request(
                tool,
                r#"{"jsonrpc":"2.0","id":2,"result":{"isError":false}}"#
            ),
            Some(("doc.save", "b.stencilcraft".to_string()))
        );
        assert_eq!(
            document_path_request(
                tool,
                r#"{"jsonrpc":"2.0","id":2,"result":{"isError":true}}"#
            ),
            None
        );
    }

    #[test]
    fn reset_panels_restores_the_default_docking_positions() {
        let mut app = StencilApp::new(None);
        let default = default_workspace();
        Arc::make_mut(&mut app.workspace)
            .apply_all(
                [Action::move_beside(
                    WorkspacePanel::Layers,
                    WorkspacePanel::Inspector,
                    DockPlacement::group(true, None),
                )],
                false,
                panel_permissions,
            )
            .unwrap();
        assert_ne!(app.workspace, default);

        app.reset_workspace();

        assert_eq!(app.workspace, default);
        assert_eq!(app.status, "panel layout restored");
        assert!(app.workspace.validate().is_ok());
    }

    #[test]
    fn command_d_duplicates_the_selection_and_selects_the_copies() {
        let ctx = egui::Context::default();
        crate::framework::install_headless(&ctx).unwrap();
        let mut app = StencilApp::new(None);
        app.run_command(Command::ShapeAddRect {
            x: 3.0,
            y: 4.0,
            width: 12.0,
            height: 8.0,
        });
        app.set_selection([1]);
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::D,
                    physical_key: Some(egui::Key::D),
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                }],
                ..Default::default()
            },
            |ui| app.handle_keyboard_shortcuts(ui.ctx()),
        );
        output.textures_delta.clear();
        assert_eq!(app.engine.doc.shape_count(), 2);
        assert_eq!(app.selected_shapes, vec![2]);
        // Silhouette places a duplicate 40 screen pixels to the right.
        let step = 40.0 / f64::from(app.view.zoom);
        let original = app.engine.doc.get_shape(1).unwrap().bounds().unwrap();
        let copy = app.engine.doc.get_shape(2).unwrap().bounds().unwrap();
        assert!(
            (copy.x0 - original.x0 - step).abs() < 1e-6,
            "{original:?} {copy:?}"
        );
        assert!((copy.y0 - original.y0).abs() < 1e-6);
    }

    #[test]
    fn flip_rotate_again_and_key_align_menu_actions() {
        let mut app = StencilApp::new(None);
        for x in [0.0, 30.0] {
            app.run_command(Command::ShapeAddRect {
                x,
                y: 0.0,
                width: 10.0,
                height: 20.0,
            });
        }
        let bounds = |app: &StencilApp, id| app.engine.doc.get_shape(id).unwrap().bounds().unwrap();
        app.set_selection([1]);
        let before = bounds(&app, 1);
        app.handle_menu_action(TopMenuAction::Flip(DistributionAxis::Horizontal));
        assert_eq!(bounds(&app, 1), before, "flipped in place");
        app.handle_menu_action(TopMenuAction::RotateBy(90));
        app.handle_menu_action(TopMenuAction::TransformAgain);
        // Two quarter turns: back to 10 wide, 20 tall.
        assert!((bounds(&app, 1).width() - 10.0).abs() < 1e-9);
        // Key = last selected (shape 1) stays; shape 2 moves to its left edge.
        app.set_selection([2, 1]);
        let key = bounds(&app, 1);
        app.handle_menu_action(TopMenuAction::AlignToKey(Alignment::Left));
        assert_eq!(bounds(&app, 1), key);
        assert!((bounds(&app, 2).x0 - key.x0).abs() < 1e-9);
    }

    #[test]
    fn shifted_shortcuts_are_not_taken_by_their_unshifted_twins() {
        let ctx = egui::Context::default();
        crate::framework::install_headless(&ctx).unwrap();
        let mut app = StencilApp::new(None);
        app.run_command(Command::ShapeAddRect {
            x: 3.0,
            y: 4.0,
            width: 12.0,
            height: 8.0,
        });
        assert!(app.engine.undo());
        let mut press = |key, modifiers| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events: vec![egui::Event::Key {
                        key,
                        physical_key: Some(key),
                        pressed: true,
                        repeat: false,
                        modifiers,
                    }],
                    ..Default::default()
                },
                |ui| app.handle_keyboard_shortcuts(ui.ctx()),
            );
            output.textures_delta.clear();
        };
        let cmd_shift = egui::Modifiers::COMMAND | egui::Modifiers::SHIFT;
        press(egui::Key::Z, cmd_shift);
        press(egui::Key::A, egui::Modifiers::COMMAND);
        press(egui::Key::A, cmd_shift);
        assert_eq!(app.engine.doc.shape_count(), 1, "Cmd+Shift+Z redoes");
        assert!(app.selected_shapes.is_empty(), "Cmd+Shift+A deselects");
    }

    #[test]
    fn command_a_selects_every_shape_on_the_canvas() {
        let ctx = egui::Context::default();
        crate::framework::install_headless(&ctx).unwrap();
        let mut app = StencilApp::new(None);
        for x in [3.0, 23.0, 43.0] {
            app.run_command(Command::ShapeAddRect {
                x,
                y: 4.0,
                width: 12.0,
                height: 8.0,
            });
        }
        app.set_selection([2]);
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::A,
                    physical_key: Some(egui::Key::A),
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                }],
                ..Default::default()
            },
            |ui| app.handle_keyboard_shortcuts(ui.ctx()),
        );
        output.textures_delta.clear();
        assert_eq!(app.selected_shapes, vec![1, 2, 3]);
        assert_eq!(app.selected_shape, Some(3));
    }

    #[test]
    fn f_and_c_queue_fit_and_center_without_command_modifiers() {
        for (key, expected) in [
            (egui::Key::F, CanvasViewCommand::Fit),
            (egui::Key::C, CanvasViewCommand::Center),
        ] {
            let ctx = egui::Context::default();
            crate::framework::install_headless(&ctx).unwrap();
            let mut app = StencilApp::new(None);
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events: vec![egui::Event::Key {
                        key,
                        physical_key: Some(key),
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                    ..Default::default()
                },
                |ui| app.handle_keyboard_shortcuts(ui.ctx()),
            );
            output.textures_delta.clear();
            assert_eq!(app.pending_view_command, Some(expected));

            let mut view = stencilcraft_ui_egui::View {
                zoom: 2.5,
                pan_mm: egui::vec2(700.0, -300.0),
                ..Default::default()
            };
            let viewport =
                egui::Rect::from_min_size(egui::pos2(100.0, 60.0), egui::vec2(800.0, 500.0));
            apply_canvas_view_command(&mut view, expected, 600.0, 300.0, None, viewport);
            let center = view.world_to_screen(stencilcraft_geom::Point::new(300.0, 150.0));
            assert!((center - viewport.center()).length() < 1e-4);
            match expected {
                CanvasViewCommand::Fit => assert!(view.zoom < 2.5),
                _ => assert_eq!(view.zoom, 2.5),
            }
        }
    }

    #[test]
    fn resizing_the_canvas_refits_the_view() {
        let ctx = egui::Context::default();
        crate::framework::install_headless(&ctx).unwrap();
        let mut app = StencilApp::new(None);
        frame(&ctx, &mut app, vec![], |app, ui| app.canvas_body(ui));
        assert!(app.view_initialized);
        // A second frame on the same board keeps what the user set up.
        let steady_zoom = app.view.zoom;
        frame(&ctx, &mut app, vec![], |app, ui| app.canvas_body(ui));
        assert_eq!(app.view.zoom, steady_zoom);

        app.engine
            .run(Command::DocumentSetBoard {
                width_mm: 900.0,
                height_mm: 600.0,
            })
            .expect("set board");
        // A zoom that only made sense for the old 100 × 150 mm sheet.
        app.view.zoom = 6.0;
        frame(&ctx, &mut app, vec![], |app, ui| app.canvas_body(ui));
        assert_eq!(app.last_board, Some((900.0, 600.0)));
        assert!(
            app.view.zoom < 6.0,
            "a resized canvas must refit, zoom stayed {}",
            app.view.zoom
        );
        // Framed, not merely zoomed out: the far corner lands on screen.
        let corner = app
            .view
            .world_to_screen(stencilcraft_geom::Point::new(900.0, 600.0));
        assert!(
            corner.x > 0.0 && corner.y > 0.0,
            "board corner off screen: {corner:?}"
        );
    }

    #[test]
    fn wheel_zoom_keeps_the_point_under_the_pointer() {
        let ctx = egui::Context::default();
        crate::framework::install_headless(&ctx).unwrap();
        let mut app = StencilApp::new(None);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
        let pointer = egui::pos2(300.0, 250.0);
        let frame = |app: &mut StencilApp, events: Vec<egui::Event>, t: f64| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    time: Some(t),
                    events,
                    ..Default::default()
                },
                |ui| app.canvas_body(ui),
            );
            output.textures_delta.clear();
        };
        for i in 0..3 {
            frame(
                &mut app,
                vec![egui::Event::PointerMoved(pointer)],
                f64::from(i) * 0.016,
            );
        }
        let before = app.view.screen_to_world(pointer.to_vec2());
        let zoom_before = app.view.zoom;
        let wheel = egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, 120.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Unknown,
        };
        frame(&mut app, vec![wheel], 0.064);
        for i in 0..40 {
            frame(&mut app, vec![], 0.08 + f64::from(i) * 0.016);
        }
        let after = app.view.screen_to_world(pointer.to_vec2());
        assert!(
            app.view.zoom > zoom_before * 1.05,
            "zoomed in: {zoom_before} -> {}",
            app.view.zoom
        );
        assert!(
            (after.x - before.x).abs() < 0.05 && (after.y - before.y).abs() < 0.05,
            "point under the pointer moved: {before:?} -> {after:?}"
        );
    }

    #[test]
    fn zoom_buttons_zoom_about_the_canvas_centre() {
        let ctx = egui::Context::default();
        crate::framework::install_headless(&ctx).unwrap();
        let mut app = StencilApp::new(None);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
        let frame = |app: &mut StencilApp| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    ..Default::default()
                },
                |ui| app.canvas_body(ui),
            );
            output.textures_delta.clear();
        };
        frame(&mut app);
        let centre = |app: &StencilApp| app.view.screen_to_world(screen.center().to_vec2());
        let start = centre(&app);
        let start_zoom = app.view.zoom;
        // The anchor is the canvas centre (inside the rulers), a few px from the screen
        // centre, so the world point there may move by those px at the new zoom only.
        for factor in [1.2_f32; 5].into_iter().chain([1.0 / 1.2; 5]) {
            app.pending_zoom = Some(app.view.zoom * factor);
            frame(&mut app);
            let at = centre(&app);
            let slack = 30.0 / f64::from(app.view.zoom);
            assert!(
                (at.x - start.x).abs() < slack && (at.y - start.y).abs() < slack,
                "view centre drifted: {start:?} -> {at:?} at zoom {}",
                app.view.zoom
            );
        }
        let end = centre(&app);
        assert!((app.view.zoom - start_zoom).abs() < 1e-3);
        assert!((end.x - start.x).abs() < 0.01 && (end.y - start.y).abs() < 0.01);
    }

    #[test]
    fn canvas_view_shortcuts_are_ignored_while_text_input_is_wanted() {
        let ctx = egui::Context::default();
        crate::framework::install_headless(&ctx).unwrap();
        let mut app = StencilApp::new(None);
        let mut text = String::new();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.text_edit_singleline(&mut text).request_focus();
        });
        output.textures_delta.clear();
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::F,
                    physical_key: Some(egui::Key::F),
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ui| {
                let response = ui.text_edit_singleline(&mut text);
                assert!(response.has_focus());
                app.handle_keyboard_shortcuts(ui.ctx());
            },
        );
        output.textures_delta.clear();
        assert_eq!(app.pending_view_command, None);
    }

    #[test]
    fn kerf_preview_cache_reuses_and_invalidates_by_document_content() {
        let mut app = StencilApp::new(None);
        assert!(app.update_kerf_preview_cache(&app.engine.doc.clone()));
        let first = app.kerf_preview_cache.as_ref().unwrap().preview.clone();
        let mut metadata_only_change = app.engine.doc.clone();
        metadata_only_change.board.width_mm += 10.0;
        metadata_only_change.layers[0].visible = false;
        assert!(!app.update_kerf_preview_cache(&metadata_only_change));
        let cached_preview = app
            .kerf_preview_cache
            .as_ref()
            .unwrap()
            .preview
            .as_ref()
            .unwrap();
        assert_eq!(cached_preview.board, metadata_only_change.board);
        assert!(!cached_preview.layers[0].visible);
        assert_eq!(
            cached_preview.layers[0].shapes,
            first.as_ref().unwrap().layers[0].shapes
        );
        assert!(!app.update_kerf_preview_cache(&app.engine.doc.clone()));

        app.run_command(Command::ShapeAddRect {
            x: 3.0,
            y: 4.0,
            width: 12.0,
            height: 8.0,
        });
        assert!(app.update_kerf_preview_cache(&app.engine.doc.clone()));
        let preview = app
            .kerf_preview_cache
            .as_ref()
            .unwrap()
            .preview
            .as_ref()
            .unwrap();
        assert_eq!(preview.shape_count(), 1);

        app.run_command(Command::KerfSet { width_mm: 2.0 });
        assert!(app.update_kerf_preview_cache(&app.engine.doc.clone()));
        let compensated = app
            .kerf_preview_cache
            .as_ref()
            .unwrap()
            .preview
            .as_ref()
            .unwrap();
        assert_ne!(
            compensated.get_shape(1).unwrap().path,
            app.engine.doc.get_shape(1).unwrap().path
        );
    }

    #[test]
    fn new_document_waits_for_confirmation_when_document_changed() {
        let mut app = StencilApp::new(None);
        app.run_command(Command::KerfSet { width_mm: 0.4 });
        let changed = app.engine.doc.clone();
        let choice = new_doc::Choice {
            width_mm: 300.0,
            ..Default::default()
        };
        app.request_new_document(choice.clone());
        assert_eq!(app.engine.doc, changed);
        assert!(app.pending_new_document.is_some());
        app.pending_new_document = None; // Cancel preserves the original document.
        assert_eq!(app.engine.doc, changed);
        app.request_new_document(choice);
        app.confirm_new_document();
        assert_eq!(app.engine.doc.board.width_mm, 300.0);
        assert_eq!(app.engine.doc, app.saved_document);
    }

    #[test]
    fn recent_files_are_unique_most_recent_first_and_bounded() {
        let mut app = StencilApp::new(None);
        for index in 0..10 {
            app.record_recent_file(format!("/documents/design-{index}.stencilcraft"));
        }
        app.record_recent_file("/documents/design-5.stencilcraft".to_string());
        assert_eq!(app.recent_files.len(), 8);
        assert_eq!(
            app.recent_files.first().unwrap(),
            "/documents/design-5.stencilcraft"
        );

        assert_eq!(
            app.recent_files
                .iter()
                .filter(|path| path.as_str() == "/documents/design-5.stencilcraft")
                .count(),
            1
        );
    }

    #[test]
    fn recent_file_opens_document_and_updates_recent_order() {
        let mut app = StencilApp::new(None);
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/starter.stencilcraft")
            .to_string_lossy()
            .into_owned();
        app.record_recent_file(path.clone());
        app.open_document(&path);
        let expected = stencilcraft_doc::load_from(&path).unwrap();
        assert_eq!(app.engine.doc, expected);
        assert_eq!(app.open_path.as_deref(), Some(path.as_str()));
        let recent_path = std::fs::canonicalize(path)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert_eq!(app.recent_files.first(), Some(&recent_path));
    }

    #[test]
    fn drag_box_follows_shift_and_alt_like_silhouette_studio() {
        use stencilcraft_geom::{Point, Rect};
        let start = Point::new(10.0, 10.0);
        let mods = |square, from_centre| DragMods {
            square,
            from_centre,
        };
        // Plain: the box between press and release, in any drag direction.
        assert_eq!(
            drag_box(start, Point::new(40.0, 20.0), mods(false, false)),
            Rect::new(10.0, 10.0, 40.0, 20.0)
        );
        assert_eq!(
            drag_box(start, Point::new(0.0, 0.0), mods(false, false)),
            Rect::new(0.0, 0.0, 10.0, 10.0)
        );
        // Shift: square, on the side the pointer is, sized by the longer drag.
        assert_eq!(
            drag_box(start, Point::new(40.0, 20.0), mods(true, false)),
            Rect::new(10.0, 10.0, 40.0, 40.0)
        );
        assert_eq!(
            drag_box(start, Point::new(-20.0, 15.0), mods(true, false)),
            Rect::new(-20.0, 10.0, 10.0, 40.0)
        );
        // Alt: the press point is the centre.
        assert_eq!(
            drag_box(start, Point::new(40.0, 20.0), mods(false, true)),
            Rect::new(-20.0, 0.0, 40.0, 20.0)
        );
        // Both: a square centred on the press point.
        assert_eq!(
            drag_box(start, Point::new(15.0, 30.0), mods(true, true)),
            Rect::new(-10.0, -10.0, 30.0, 30.0)
        );
    }

    #[test]
    fn line_tool_draws_an_open_two_point_line_and_shift_snaps_to_45_degrees() {
        use stencilcraft_geom::Point;
        let shift = DragMods {
            square: true,
            from_centre: false,
        };
        // Snapping keeps the length and picks the nearest 45° step.
        let snapped = line_end(Point::new(0.0, 0.0), Point::new(10.0, 1.0), shift);
        assert!(
            (snapped - Point::new(10.0499, 0.0)).hypot() < 1e-3,
            "{snapped:?}"
        );
        let diagonal = line_end(Point::new(0.0, 0.0), Point::new(10.0, 9.0), shift);
        assert!((diagonal.x - diagonal.y).abs() < 1e-9, "{diagonal:?}");
        assert_eq!(
            line_end(
                Point::new(0.0, 0.0),
                Point::new(10.0, 1.0),
                DragMods::default()
            ),
            Point::new(10.0, 1.0)
        );

        let mut app = StencilApp::new(None);
        app.active_tool = ActiveTool::Line;
        app.commit_dragged_shape(Point::new(5.0, 5.0), Point::new(45.0, 7.0), shift);
        let shape = app.engine.doc.get_shape(1).expect("line created");
        let elements = shape.path.elements();
        assert_eq!(
            elements.len(),
            2,
            "open: move + line, no close: {elements:?}"
        );
        assert_eq!(elements[0].end_point(), Some(Point::new(5.0, 5.0)));
        let end = elements[1].end_point().expect("line end");
        assert!(
            (end.y - 5.0).abs() < 1e-9,
            "Shift kept it horizontal: {end:?}"
        );
        assert_eq!(app.selected_shapes, vec![1], "the new line is selected");
        assert_eq!(app.active_tool, ActiveTool::Line, "the tool stays active");
        // A click without a drag draws nothing.
        app.commit_dragged_shape(Point::new(5.0, 5.0), Point::new(5.2, 5.2), shift);
        assert_eq!(app.engine.doc.shape_count(), 1);
    }

    #[test]
    fn shape_tool_options_shape_new_polygons_stars_and_rounded_rects() {
        use stencilcraft_geom::{Point, Shape};
        let knots = |app: &StencilApp, id| {
            let shape = app.engine.doc.get_shape(id).expect("shape");
            stencilcraft_ui_egui::bezpath_to_edit_subpaths(&shape.path)[0]
                .knots
                .len()
        };
        let mut app = StencilApp::new(None);
        // Silhouette's regular polygon starts with five sides.
        app.active_tool = ActiveTool::Polygon;
        app.commit_dragged_shape(
            Point::new(0.0, 0.0),
            Point::new(40.0, 40.0),
            DragMods::default(),
        );
        assert_eq!(knots(&app, 1), 5);
        app.panels.polygon_sides = 8.0;
        app.commit_dragged_shape(
            Point::new(0.0, 0.0),
            Point::new(40.0, 40.0),
            DragMods::default(),
        );
        assert_eq!(knots(&app, 2), 8);

        app.active_tool = ActiveTool::Star;
        app.panels.star_points = 6.0;
        app.panels.star_inner_pct = 30.0;
        app.commit_dragged_shape(
            Point::new(0.0, 0.0),
            Point::new(40.0, 40.0),
            DragMods::default(),
        );
        assert_eq!(knots(&app, 3), 12, "six points, six inner corners");

        // Out-of-range or non-finite settings are clamped, never trusted.
        app.panels.polygon_sides = f64::NAN;
        assert_eq!(app.panels.polygon_sides(), 3);
        app.panels.polygon_sides = 1e9;
        assert_eq!(app.panels.polygon_sides(), 64);
        app.panels.star_inner_pct = -4.0;
        assert!((app.panels.star_inner_ratio() - 0.05).abs() < 1e-12);

        // The corner radius is capped at half the shorter side.
        app.active_tool = ActiveTool::RoundedRect;
        app.panels.corner_radius_mm = 50.0;
        assert_eq!(app.panels.corner_radius(30.0, 10.0), 5.0);
        app.commit_dragged_shape(
            Point::new(0.0, 0.0),
            Point::new(30.0, 10.0),
            DragMods::default(),
        );
        let pill = app
            .engine
            .doc
            .get_shape(4)
            .expect("rounded rect")
            .path
            .bounding_box();
        assert!((pill.width() - 30.0).abs() < 1e-6 && (pill.height() - 10.0).abs() < 1e-6);
    }

    #[test]
    fn shift_dragged_ellipse_is_a_circle_and_alt_centres_it() {
        use stencilcraft_geom::{Point, Shape};
        let mut app = StencilApp::new(None);
        app.active_tool = ActiveTool::Ellipse;
        let both = DragMods {
            square: true,
            from_centre: true,
        };
        app.commit_dragged_shape(Point::new(50.0, 50.0), Point::new(70.0, 60.0), both);
        let shape = app.engine.doc.get_shape(1).expect("ellipse created");
        let bounds = shape.path.bounding_box();
        assert!((bounds.width() - 40.0).abs() < 1e-6, "{bounds:?}");
        assert!((bounds.height() - 40.0).abs() < 1e-6, "{bounds:?}");
        assert!(
            (bounds.center() - Point::new(50.0, 50.0)).hypot() < 1e-6,
            "{bounds:?}"
        );
    }

    #[test]
    fn delete_and_backspace_remove_selected_shapes() {
        for (key, send_mode) in [
            (egui::Key::Delete, false),
            (egui::Key::Backspace, false),
            (egui::Key::Delete, true),
            (egui::Key::Backspace, true),
        ] {
            let ctx = egui::Context::default();
            crate::framework::install_headless(&ctx).unwrap();
            let mut app = StencilApp::new(None);
            app.run_command(Command::ShapeAddRect {
                x: 3.0,
                y: 4.0,
                width: 12.0,
                height: 8.0,
            });
            app.set_selection([1]);
            app.send_mode = send_mode;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events: vec![egui::Event::Key {
                        key,
                        physical_key: Some(key),
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                    ..Default::default()
                },
                |ui| app.handle_keyboard_shortcuts(ui.ctx()),
            );
            output.textures_delta.clear();
            assert_eq!(app.engine.doc.shape_count(), 0);
            assert!(app.selected_shapes.is_empty());
        }
    }

    fn press_key(app: &mut StencilApp, key: egui::Key, modifiers: egui::Modifiers) {
        let ctx = egui::Context::default();
        crate::framework::install_headless(&ctx).unwrap();
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed: true,
                    repeat: false,
                    modifiers,
                }],
                ..Default::default()
            },
            |ui| app.handle_keyboard_shortcuts(ui.ctx()),
        );
        output.textures_delta.clear();
    }

    #[test]
    fn delete_in_point_editing_leaves_the_shape_to_the_point_overlay() {
        // Regression: Delete/Backspace with points selected removed the whole shape.
        for key in [egui::Key::Delete, egui::Key::Backspace] {
            let mut app = StencilApp::new(None);
            app.run_command(Command::ShapeAddRect {
                x: 3.0,
                y: 4.0,
                width: 12.0,
                height: 8.0,
            });
            app.set_selection([1]);
            app.active_tool = ActiveTool::DirectSelect;
            app.path_selection =
                PathSelection::new(Some(1), vec![craft_draw::ui::pen::KnotRef::new(0, 0)], None);
            press_key(&mut app, key, egui::Modifiers::NONE);
            assert_eq!(app.engine.doc.shape_count(), 1, "{key:?} kept the shape");
            press_key(&mut app, egui::Key::Escape, egui::Modifiers::NONE);
            assert_eq!(app.selected_shapes, vec![1], "Esc left the shape selected");
        }
        // With no point selected, Delete still removes the selected shape.
        let mut app = StencilApp::new(None);
        app.run_command(Command::ShapeAddRect {
            x: 3.0,
            y: 4.0,
            width: 12.0,
            height: 8.0,
        });
        app.set_selection([1]);
        app.active_tool = ActiveTool::DirectSelect;
        press_key(&mut app, egui::Key::Delete, egui::Modifiers::NONE);
        assert_eq!(app.engine.doc.shape_count(), 0);
    }

    #[test]
    fn polyline_draft_keys_take_back_points_and_enter_keeps_the_path() {
        use click_draw::{ClickDraft, ClickKind};
        use stencilcraft_geom::Point;
        let mut app = StencilApp::new(None);
        app.active_tool = ActiveTool::Polyline;
        let mut draft = ClickDraft::new(ClickKind::Polyline);
        for (x, y) in [(0.0, 0.0), (20.0, 0.0), (20.0, 20.0)] {
            draft.click(Point::new(x, y), 1.0, false);
        }
        app.click_draft = Some(draft);
        press_key(&mut app, egui::Key::Backspace, egui::Modifiers::NONE);
        assert_eq!(app.click_draft.as_ref().map(|d| d.points.len()), Some(2));
        press_key(&mut app, egui::Key::Z, egui::Modifiers::COMMAND);
        assert_eq!(app.click_draft.as_ref().map(|d| d.points.len()), Some(1));
        assert_eq!(
            app.engine.doc.shape_count(),
            0,
            "undo stayed inside the draft"
        );
        // Down to nothing: the draft goes away and no shape is made.
        press_key(&mut app, egui::Key::Delete, egui::Modifiers::NONE);
        assert!(app.click_draft.is_none());

        let mut draft = ClickDraft::new(ClickKind::Curve);
        for (x, y) in [(0.0, 0.0), (20.0, 10.0), (40.0, 0.0)] {
            draft.click(Point::new(x, y), 1.0, false);
        }
        app.active_tool = ActiveTool::Curve;
        app.click_draft = Some(draft);
        press_key(&mut app, egui::Key::Escape, egui::Modifiers::NONE);
        assert!(app.click_draft.is_none());
        assert_eq!(
            app.engine.doc.shape_count(),
            1,
            "Esc keeps the curve, like the pen"
        );
        assert_eq!(app.selected_shapes, vec![1]);
    }

    #[test]
    fn arc_tool_enter_places_the_swept_arc_and_esc_drops_it() {
        use stencilcraft_geom::{Point, Shape};
        let mut app = StencilApp::new(None);
        app.active_tool = ActiveTool::Arc;
        let mut draft =
            arc_draw::ArcDraft::new(Point::new(50.0, 50.0), Point::new(70.0, 50.0)).expect("arc");
        draft.track(Point::new(50.0, 70.0), false);
        app.arc_draft = Some(draft);
        press_key(&mut app, egui::Key::Enter, egui::Modifiers::NONE);
        assert!(app.arc_draft.is_none());
        let shape = app.engine.doc.get_shape(1).expect("quarter arc placed");
        let bounds = shape.path.bounding_box();
        assert!((bounds.width() - 20.0).abs() < 0.01 && (bounds.height() - 20.0).abs() < 0.01);
        assert_eq!(app.active_tool, ActiveTool::Arc, "the tool stays active");

        app.arc_draft = Some(draft);
        press_key(&mut app, egui::Key::Escape, egui::Modifiers::NONE);
        assert!(app.arc_draft.is_none());
        assert_eq!(
            app.engine.doc.shape_count(),
            1,
            "Esc drops the unplaced arc"
        );
    }

    #[test]
    fn freehand_strokes_become_cut_paths_and_close_on_their_start() {
        use stencilcraft_geom::Point;
        let mut app = StencilApp::new(None);
        app.active_tool = ActiveTool::Freehand;
        let mut stroke = freehand::Stroke::default();
        for i in 0..=40 {
            let angle = f64::from(i) / 40.0 * std::f64::consts::TAU;
            stroke.push(
                Point::new(50.0 + 20.0 * angle.cos(), 50.0 + 20.0 * angle.sin()),
                0.1,
            );
        }
        app.commit_freehand(&stroke, false, 0.1);
        app.commit_freehand(&stroke, true, 0.1);
        for id in [1, 2] {
            let shape = app.engine.doc.get_shape(id).expect("freehand path");
            assert!(
                shape
                    .path
                    .elements()
                    .last()
                    .is_some_and(|e| e.end_point().is_none()),
                "a stroke ending on its start is closed"
            );
        }
        assert_eq!(app.selected_shapes, vec![2]);
        // A dot (one sample) makes nothing.
        let mut dot = freehand::Stroke::default();
        dot.push(Point::new(1.0, 1.0), 0.1);
        app.commit_freehand(&dot, false, 0.1);
        assert_eq!(app.engine.doc.shape_count(), 2);
    }

    #[test]
    fn delete_point_joins_neighbours_without_moving_them() {
        use stencilcraft_geom::Shape;
        let mut app = StencilApp::new(None);
        app.run_command(Command::ShapeAddRect {
            x: 0.0,
            y: 0.0,
            width: 20.0,
            height: 10.0,
        });
        app.set_selection([1]);
        app.active_tool = ActiveTool::DirectSelect;
        let before = stencilcraft_ui_egui::bezpath_to_edit_subpaths(
            &app.engine.doc.get_shape(1).expect("rect").path,
        );
        app.path_selection =
            PathSelection::new(Some(1), vec![craft_draw::ui::pen::KnotRef::new(0, 0)], None);
        app.handle_edit_action(panels::EditAction::DeleteNodes);
        let shape = app.engine.doc.get_shape(1).expect("triangle left");
        let after = stencilcraft_ui_egui::bezpath_to_edit_subpaths(&shape.path);
        assert_eq!(after[0].knots.len(), before[0].knots.len() - 1);
        assert_eq!(
            &after[0].knots[..],
            &before[0].knots[1..],
            "the rest are untouched"
        );
        assert!(shape.path.bounding_box().area() > 0.0);
    }

    #[test]
    fn knife_needs_a_selection_and_eraser_defaults_to_every_shape() {
        use stencilcraft_geom::Point;
        let mut app = StencilApp::new(None);
        app.run_command(Command::ShapeAddRect {
            x: 0.0,
            y: 0.0,
            width: 20.0,
            height: 10.0,
        });
        app.set_selection([]);
        app.knife_cut(Point::new(10.0, -5.0), Point::new(10.0, 15.0));
        assert_eq!(app.engine.doc.shape_count(), 1, "no selection, no cut");

        app.set_selection([1]);
        app.panels.knife_solid = true;
        app.knife_cut(Point::new(10.0, -5.0), Point::new(10.0, 15.0));
        assert_eq!(app.engine.doc.shape_count(), 2, "Solid: two closed halves");
        assert_eq!(app.selected_shapes.len(), 2, "the pieces are selected");

        // Nothing selected: a solid square brush across both halves trims each.
        app.set_selection([]);
        app.panels.eraser_round = false;
        app.panels.eraser_size_mm = 2.0;
        app.erase_stroke(&[Point::new(-5.0, 5.0), Point::new(25.0, 5.0)]);
        assert_eq!(app.engine.doc.shape_count(), 4, "each half split in two");
    }

    #[test]
    fn shortcuts_need_their_exact_modifiers() {
        // Regression: Shift+G picked the Polygon tool and Cmd+Shift+L opened Send (Cmd+L).
        let mut app = StencilApp::new(None);
        press_key(&mut app, egui::Key::G, egui::Modifiers::SHIFT);
        assert_eq!(app.active_tool, ActiveTool::Select, "Shift+G is not G");
        press_key(&mut app, egui::Key::G, egui::Modifiers::ALT);
        assert_eq!(app.active_tool, ActiveTool::Select, "Alt+G is not G");
        press_key(&mut app, egui::Key::G, egui::Modifiers::NONE);
        assert_eq!(app.active_tool, ActiveTool::Polygon);
        press_key(
            &mut app,
            egui::Key::L,
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        );
        assert!(!app.send_mode, "Cmd+Shift+L is not Cmd+L");
        press_key(&mut app, egui::Key::L, egui::Modifiers::COMMAND);
        assert!(app.send_mode);
    }

    #[test]
    fn folding_a_snapped_drag_keeps_earlier_history() {
        // The fold undoes exactly the drag's moves: shapes created before it survive.
        let mut app = StencilApp::new(None);
        for x in [10.0, 60.0, 110.0] {
            app.run_command(Command::ShapeAddRect {
                x,
                y: 10.0,
                width: 20.0,
                height: 10.0,
            });
        }
        app.set_selection([1]);
        app.drag_origin =
            stencilcraft_ui_egui::selected_world_bounds(&app.engine.doc, &app.selected_shapes);
        for step in 1..=8 {
            app.drag_selection_to(1, egui::vec2(9.0 * step as f32, -5.0 * step as f32));
        }
        app.fold_drag_into_one_step();
        assert_eq!(app.engine.doc.shape_count(), 3);
        assert!(app.engine.undo(), "undo the drag");
        assert_eq!(app.engine.doc.shape_count(), 3, "only the move was undone");
    }

    #[test]
    fn dragging_a_shape_is_one_undo_step() {
        // Regression: every frame of a drag was its own undo step.
        let mut app = StencilApp::new(None);
        app.run_command(Command::ShapeAddRect {
            x: 10.0,
            y: 10.0,
            width: 20.0,
            height: 10.0,
        });
        app.set_selection([1]);
        app.engine.doc.snap = Default::default();
        app.drag_origin =
            stencilcraft_ui_egui::selected_world_bounds(&app.engine.doc, &app.selected_shapes);
        let start = app.drag_origin.expect("bounds");
        for step in 1..=5 {
            app.drag_selection_to(1, egui::vec2(10.0 * step as f32, 0.0));
        }
        assert!(app.drag_steps >= 2, "several moves were applied live");
        app.fold_drag_into_one_step();
        let moved =
            stencilcraft_ui_egui::selected_world_bounds(&app.engine.doc, &[1]).expect("moved");
        assert!(moved.x0 > start.x0 + 1.0);
        assert!(app.engine.undo());
        let back =
            stencilcraft_ui_egui::selected_world_bounds(&app.engine.doc, &[1]).expect("back");
        assert!(
            (back.x0 - start.x0).abs() < 1e-9,
            "one undo returns it: {back:?} vs {start:?}"
        );
        assert_eq!(
            app.engine.doc.shape_count(),
            1,
            "and the shape itself is still there"
        );
    }

    #[test]
    fn clipboard_nudge_arrange_and_center_follow_silhouette() {
        use stencilcraft_engine::document::{ArrangeOrder, PasteMode};
        let mut app = StencilApp::new(None);
        for x in [10.0, 40.0] {
            app.run_command(Command::ShapeAddRect {
                x,
                y: 10.0,
                width: 20.0,
                height: 10.0,
            });
        }
        let px = 1.0 / f64::from(app.view.zoom);
        let bounds = |app: &StencilApp, id| app.engine.doc.get_shape(id).unwrap().bounds().unwrap();
        app.set_selection([1]);
        press_key(&mut app, egui::Key::C, egui::Modifiers::COMMAND);
        press_key(&mut app, egui::Key::V, egui::Modifiers::COMMAND);
        assert_eq!(app.engine.doc.shape_count(), 3);
        let pasted = app.selected_shapes.clone();
        assert_eq!(pasted.len(), 1, "the paste is selected");
        let (a, b) = (bounds(&app, 1), bounds(&app, pasted[0]));
        assert!((b.x0 - a.x0 - 80.0 * px).abs() < 1e-6 && (b.y0 - a.y0 - 80.0 * px).abs() < 1e-6);
        // Paste in Front: exactly on top of the copied shape.
        press_key(&mut app, egui::Key::F, egui::Modifiers::COMMAND);
        let front = app.selected_shapes.clone();
        assert_eq!(bounds(&app, front[0]), a);

        // Arrow keys nudge by 2 px, Shift by 16 px.
        app.set_selection([1]);
        press_key(&mut app, egui::Key::ArrowRight, egui::Modifiers::NONE);
        assert!((bounds(&app, 1).x0 - (a.x0 + 2.0 * px)).abs() < 1e-6);
        press_key(&mut app, egui::Key::ArrowDown, egui::Modifiers::SHIFT);
        assert!((bounds(&app, 1).y0 - (a.y0 + 16.0 * px)).abs() < 1e-6);

        // Arrange: Bring to Front puts shape 1 last in drawing order.
        app.handle_menu_action(TopMenuAction::Arrange(ArrangeOrder::Front));
        let order: Vec<u64> = app.engine.doc.shapes().map(|(_, s)| s.id).collect();
        assert_eq!(order.last(), Some(&1), "{order:?}");

        // Center to Page works for one shape (it used to need two).
        app.handle_menu_action(TopMenuAction::CenterToPage);
        let board = &app.engine.doc.board;
        let centre = bounds(&app, 1).center();
        assert!((centre.x - board.width_mm / 2.0).abs() < 1e-6);
        assert!((centre.y - board.height_mm / 2.0).abs() < 1e-6);

        // Cut removes and keeps it for pasting.
        press_key(&mut app, egui::Key::X, egui::Modifiers::COMMAND);
        assert!(app.engine.doc.get_shape(1).is_none());
        app.clipboard_paste(PasteMode::InPlace);
        assert_eq!(app.selected_shapes.len(), 1);
    }

    #[test]
    fn clipboard_events_from_the_desktop_backend_copy_and_paste() {
        // Regression: the desktop backend sends Ctrl+C/V as Copy/Paste events, not keys, so
        // the clipboard did nothing in the real app.
        let ctx = egui::Context::default();
        crate::framework::install_headless(&ctx).unwrap();
        let mut app = StencilApp::new(None);
        app.run_command(Command::ShapeAddRect {
            x: 10.0,
            y: 10.0,
            width: 20.0,
            height: 10.0,
        });
        app.set_selection([1]);
        // The runtime leaves egui's clipboard events to the app while its canvas is the
        // window's document canvas (`framework::host_canvas`, which the canvas view declares
        // each pass); otherwise they stand for their chord. Each pass declares a canvas the way
        // the app's canvas view does, and a first pass registers it before any event arrives.
        let send = |app: &mut StencilApp, event: Option<egui::Event>| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events: event.into_iter().collect(),
                    ..Default::default()
                },
                |ui| {
                    let response = ui.allocate_rect(
                        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(200.0, 200.0)),
                        egui::Sense::click_and_drag(),
                    );
                    crate::framework::host_canvas(ui.ctx(), response.id);
                    app.handle_keyboard_shortcuts(ui.ctx());
                },
            );
            output.textures_delta.clear();
            output
        };
        send(&mut app, None);
        let output = send(&mut app, Some(egui::Event::Copy));
        let copied = output
            .platform_output
            .commands
            .iter()
            .any(|c| matches!(c, egui::OutputCommand::CopyText(text) if text.contains("<svg")));
        assert!(
            copied,
            "Copy puts the selection on the system clipboard as SVG"
        );
        send(&mut app, Some(egui::Event::Paste(String::new())));
        assert_eq!(app.engine.doc.shape_count(), 2, "Paste pastes the shapes");
        send(&mut app, Some(egui::Event::Cut));
        assert_eq!(
            app.engine.doc.shape_count(),
            1,
            "Cut removes the pasted copy"
        );
    }

    #[test]
    fn box_handles_rotate_band_and_guides_are_hit() {
        let view = stencilcraft_ui_egui::View::default();
        let bounds = stencilcraft_geom::Rect::new(10.0, 10.0, 50.0, 40.0);
        let at = |x: f64, y: f64| view.world_to_screen(stencilcraft_geom::Point::new(x, y));
        assert!(
            over_selection_box_part(&view, bounds, at(50.0, 25.0)),
            "edge handle"
        );
        assert!(
            over_selection_box_part(&view, bounds, at(10.0, 10.0)),
            "corner"
        );
        let corner = at(50.0, 40.0);
        assert!(
            over_selection_box_part(&view, bounds, corner + egui::vec2(15.0, 12.0)),
            "rotate band"
        );
        assert!(
            !over_selection_box_part(&view, bounds, at(30.0, 25.0)),
            "inside"
        );
        let guides = [stencilcraft_doc::Guide {
            axis: stencilcraft_doc::GuideAxis::Vertical,
            position_mm: 30.0,
            locked: false,
        }];
        assert!(over_guide_line(&view, &guides, at(30.0, 80.0)));
        assert!(!over_guide_line(&view, &guides, at(60.0, 80.0)));
    }

    #[test]
    fn double_clicking_a_selected_shape_by_a_box_handle_edits_its_points() {
        // Regression: next to a box handle the canvas widget only hovers, and the double click
        // that enters point editing was lost.
        let ctx = egui::Context::default();
        crate::framework::install_headless(&ctx).unwrap();
        let mut app = StencilApp::new(None);
        app.run_command(Command::ShapeAddRect {
            x: 20.0,
            y: 20.0,
            width: 40.0,
            height: 30.0,
        });
        app.set_selection([1]);
        app.active_tool = ActiveTool::Select;
        let canvas: fn(&mut StencilApp, &mut egui::Ui) = |app, ui| app.canvas_body(ui);
        frame(&ctx, &mut app, vec![], canvas);
        frame(&ctx, &mut app, vec![], canvas);
        // On the left edge, at its handle.
        let at = app
            .view
            .world_to_screen(stencilcraft_geom::Point::new(20.0, 35.0));
        let button = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(&ctx, &mut app, vec![egui::Event::PointerMoved(at)], canvas);
        // A double click: the runtime hands one press a pass, so the passes are 50 ms apart
        // (the harness's usual half second is longer than a double click).
        for pressed in [true, false, true, false] {
            frame_after(&ctx, &mut app, vec![button(pressed)], canvas, 0.05);
        }
        frame(&ctx, &mut app, vec![], canvas);
        assert_eq!(app.active_tool, ActiveTool::DirectSelect);
    }

    #[test]
    fn dragging_a_selection_box_handle_resizes_in_one_step() {
        // Regression: a press on the box's handle went to the app's canvas widget, which
        // cleared the selection instead of resizing.
        let ctx = egui::Context::default();
        crate::framework::install_headless(&ctx).unwrap();
        let mut app = StencilApp::new(None);
        app.run_command(Command::ShapeAddRect {
            x: 20.0,
            y: 20.0,
            width: 40.0,
            height: 30.0,
        });
        app.set_selection([1]);
        app.active_tool = ActiveTool::Select;
        app.engine.doc.snap = Default::default();
        let canvas: fn(&mut StencilApp, &mut egui::Ui) = |app, ui| app.canvas_body(ui);
        frame(&ctx, &mut app, vec![], canvas);
        frame(&ctx, &mut app, vec![], canvas);
        let edge = app
            .view
            .world_to_screen(stencilcraft_geom::Point::new(60.0, 35.0));
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(edge)],
            canvas,
        );
        frame(&ctx, &mut app, vec![button(edge, true)], canvas);
        let mut at = edge;
        for _ in 0..4 {
            at += egui::vec2(15.0, 0.0);
            frame(&ctx, &mut app, vec![egui::Event::PointerMoved(at)], canvas);
        }
        frame(&ctx, &mut app, vec![button(at, false)], canvas);
        frame(&ctx, &mut app, vec![], canvas);
        assert_eq!(app.selected_shapes, vec![1], "the shape stays selected");
        let b = stencilcraft_ui_egui::selected_world_bounds(&app.engine.doc, &[1]).unwrap();
        assert!(b.width() > 41.0, "the box stretched the shape: {b:?}");
        assert!(app.engine.undo());
        let b = stencilcraft_ui_egui::selected_world_bounds(&app.engine.doc, &[1]).unwrap();
        assert!((b.width() - 40.0).abs() < 1e-6, "one undo step: {b:?}");
    }

    #[test]
    fn dragging_a_point_of_a_selected_path_reshapes_it_in_one_step() {
        // Regression: in Direct Select a press on a node started the canvas's marquee, which
        // cleared the selection ("No selection") and left the rectangle as it was.
        let ctx = egui::Context::default();
        crate::framework::install_headless(&ctx).unwrap();
        let mut app = StencilApp::new(None);
        app.run_command(Command::ShapeAddRect {
            x: 20.0,
            y: 20.0,
            width: 40.0,
            height: 30.0,
        });
        app.set_selection([1]);
        app.active_tool = ActiveTool::DirectSelect;
        let canvas: fn(&mut StencilApp, &mut egui::Ui) = |app, ui| app.canvas_body(ui);
        frame(&ctx, &mut app, vec![], canvas);
        frame(&ctx, &mut app, vec![], canvas);
        let before = app.engine.doc.get_shape(1).unwrap().path.clone();
        let corner = app
            .view
            .world_to_screen(stencilcraft_geom::Point::new(20.0, 20.0));
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(corner)],
            canvas,
        );
        frame(&ctx, &mut app, vec![button(corner, true)], canvas);
        let mut at = corner;
        for _ in 0..4 {
            at += egui::vec2(10.0, 8.0);
            frame(&ctx, &mut app, vec![egui::Event::PointerMoved(at)], canvas);
            assert_eq!(
                app.selected_shapes,
                vec![1],
                "the dragged path stays selected"
            );
        }
        assert_eq!(
            app.engine.doc.get_shape(1).unwrap().path,
            before,
            "the drag previews; the document changes on release"
        );
        frame(&ctx, &mut app, vec![button(at, false)], canvas);
        frame(&ctx, &mut app, vec![], canvas);
        assert_eq!(app.selected_shapes, vec![1], "{}", app.status);
        assert_eq!(app.active_tool, ActiveTool::DirectSelect);
        let after = app.engine.doc.get_shape(1).unwrap().path.clone();
        assert_ne!(after, before, "the point moved and the path reshaped");
        assert!(app.engine.undo());
        assert_eq!(
            app.engine.doc.get_shape(1).unwrap().path,
            before,
            "one undo step"
        );
    }

    #[test]
    fn text_style_justification_and_direction_pickers_fit_a_narrow_panel() {
        // Regression: in the default panel width the pin elided the Justification and
        // Direction segmented strips ("Ce…", "R…", "Horizo…"); they are pickers now.
        use egui::accesskit::Role;
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        crate::framework::install_headless(&ctx).unwrap();
        let mut app = StencilApp::new(None);
        // The dock's default panel width.
        let width = 300.0;
        let mut out = None;
        for pass in 0..3 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1000.0, 800.0),
                    )),
                    time: Some(f64::from(pass) * 0.5),
                    ..Default::default()
                },
                |ui| {
                    ui.allocate_ui(egui::vec2(width, 800.0), |ui| {
                        ui.set_max_width(width);
                        app.text_style_panel(ui);
                    });
                },
            );
            output.textures_delta.clear();
            out = Some(output);
        }
        let out = out.unwrap();
        let tree = out.platform_output.accesskit_update.as_ref().unwrap();
        for (name, value) in [("Justification", "Left"), ("Direction", "Horizontal")] {
            let node = &tree
                .nodes
                .iter()
                .find(|(_, node)| node.role() == Role::ComboBox && node.label() == Some(name))
                .unwrap_or_else(|| panic!("missing picker {name}"))
                .1;
            assert_eq!(node.value(), Some(value), "{name} shows its whole value");
            let b = node.bounds().unwrap();
            assert!(
                b.x1 <= f64::from(width) + 1.0,
                "{name} fits the panel: {b:?}"
            );
        }
    }

    #[test]
    fn a_guide_drag_previews_its_updates_and_changes_the_document_once() {
        // Regression: one drag out of the left ruler added a guide per drag update (four).
        use craft_behavior::{Axis, EditEvent, EditEvents, GestureId, Mixed};
        use craft_ui::canvas::{Guide, GuideKey};
        let mut app = StencilApp::new(None);
        let initial = app.engine.doc.guides.len();
        let shown = |app: &StencilApp| {
            stencilcraft_ui_egui::canvas_guides(&app.engine.doc.guides, app.guide_preview)
        };
        let before =
            |app: &StencilApp| stencilcraft_ui_egui::canvas_guides(&app.engine.doc.guides, None);
        let with = |list: &[Guide], guide: Guide| {
            let mut list = list.to_vec();
            list.push(guide);
            list
        };
        let pass = |app: &mut StencilApp, event: EditEvent<Vec<Guide>>| {
            let mut events = EditEvents::new();
            events.push(event);
            let guides = before(app);
            app.apply_guide_edit(&guides, &events);
        };
        let id = GestureId::new(1);
        let start = before(&app);

        // Out of the left ruler: a vertical guide, under a key the control gives it.
        let new_key = GuideKey::new(1000);
        pass(
            &mut app,
            EditEvent::begin(id, Mixed::Same(start.clone()), "Guides".into(), None),
        );
        for x in [10.0, 20.0, 30.0, 40.0] {
            pass(
                &mut app,
                EditEvent::update(id, with(&start, Guide::new(new_key, Axis::Y, x))),
            );
            assert_eq!(
                app.engine.doc.guides.len(),
                initial,
                "an update only previews"
            );
            let previewed = shown(&app);
            assert!(
                previewed
                    .iter()
                    .any(|g| g.key == new_key && g.position == x),
                "the dragged guide follows the pointer: {previewed:?}"
            );
        }
        pass(
            &mut app,
            EditEvent::commit(
                id,
                Mixed::Same(start.clone()),
                with(&start, Guide::new(new_key, Axis::Y, 40.0)),
            ),
        );
        assert_eq!(
            app.engine.doc.guides.len(),
            initial + 1,
            "added once, on release"
        );
        assert_eq!(app.guide_preview, None);
        let added = app.engine.doc.guides.last().unwrap().clone();
        assert_eq!(added.axis, stencilcraft_doc::GuideAxis::Vertical);
        assert_eq!(added.position_mm, 40.0);
        assert!(app.engine.undo());
        assert_eq!(app.engine.doc.guides.len(), initial, "one undo step");
        assert!(app.engine.redo());

        // Dragging the guide moves it, once.
        let placed = before(&app);
        let key = placed.last().unwrap().key;
        let moved = |at: f64| {
            let mut list = placed.clone();
            if let Some(guide) = list.last_mut() {
                guide.position = at;
            }
            list
        };
        let id = id.next();
        pass(&mut app, EditEvent::update(id, moved(50.0)));
        pass(&mut app, EditEvent::update(id, moved(60.0)));
        assert_eq!(app.engine.doc.guides.last().unwrap().position_mm, 40.0);
        assert!(
            shown(&app)
                .iter()
                .any(|g| g.key == key && g.position == 60.0)
        );
        pass(
            &mut app,
            EditEvent::commit(id, Mixed::Same(placed.clone()), moved(60.0)),
        );
        assert_eq!(app.engine.doc.guides.len(), initial + 1);
        assert_eq!(app.engine.doc.guides.last().unwrap().position_mm, 60.0);
        assert!(app.engine.undo());
        assert_eq!(app.engine.doc.guides.last().unwrap().position_mm, 40.0);
        assert!(app.engine.redo());

        // Dropping it on a ruler removes it.
        let placed = before(&app);
        let gone: Vec<Guide> = placed.iter().filter(|g| g.key != key).cloned().collect();
        let id = id.next();
        pass(
            &mut app,
            EditEvent::commit(id, Mixed::Same(placed.clone()), gone),
        );
        assert_eq!(app.engine.doc.guides.len(), initial);
    }

    #[test]
    fn shortcuts_work_while_a_canvas_control_has_keyboard_focus() {
        // Regression: after dragging a selection handle the box kept keyboard focus and
        // Cmd+Z (every shortcut but Delete) did nothing.
        let ctx = egui::Context::default();
        crate::framework::install_headless(&ctx).unwrap();
        let mut app = StencilApp::new(None);
        app.run_command(Command::ShapeAddRect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        });
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::Z,
                    physical_key: Some(egui::Key::Z),
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                }],
                ..Default::default()
            },
            |ui| {
                ui.memory_mut(|m| m.request_focus(egui::Id::unique("selection-box")));
                assert!(ui.ctx().egui_wants_keyboard_input());
                app.handle_keyboard_shortcuts(ui.ctx());
            },
        );
        output.textures_delta.clear();
        assert_eq!(app.engine.doc.shape_count(), 0, "Cmd+Z undid the rectangle");
    }

    #[test]
    fn groups_select_as_one_object_and_ungroup_releases_compounds() {
        let mut app = StencilApp::new(None);
        for x in [0.0, 30.0, 60.0] {
            app.run_command(Command::ShapeAddRect {
                x,
                y: 0.0,
                width: 20.0,
                height: 10.0,
            });
        }
        app.set_selection([1, 2]);
        press_key(&mut app, egui::Key::G, egui::Modifiers::COMMAND);
        // Selecting either member selects the group.
        app.set_selection([2]);
        assert_eq!(app.selected_shapes, vec![1, 2]);
        app.set_selection([3]);
        assert_eq!(app.selected_shapes, vec![3], "loose shapes stay single");
        // A lone loose shape cannot be grouped.
        app.group_selection();
        assert_eq!(app.engine.doc.groups.len(), 1);

        app.set_selection([1]);
        press_key(
            &mut app,
            egui::Key::G,
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        );
        assert!(app.engine.doc.groups.is_empty());
        assert_eq!(app.selected_shapes, vec![1, 2], "the parts stay selected");
        app.set_selection([1]);
        assert_eq!(app.selected_shapes, vec![1], "now separate objects");

        // Ungroup on a compound path releases it, as in Silhouette.
        app.set_selection([1, 3]);
        app.handle_edit_action(panels::EditAction::MakeCompound);
        let compound = app.selected_shapes.clone();
        assert_eq!(compound.len(), 1);
        app.ungroup_selection();
        assert_eq!(
            app.selected_shapes.len(),
            2,
            "released into its two contours"
        );
    }

    #[test]
    fn placed_images_select_move_and_delete_like_objects() {
        use craft_raster_codecs::common::{Pixel, Pixels};
        let mut app = StencilApp::new(None);
        // 600 × 400 px at Silhouette's 300 dpi: 2 × 1.333 in at the page's top left.
        let pixels = Pixels::from_pixel(600, 400, Pixel([200, 20, 20, 255]));
        let image = stencilcraft_raster::place(&pixels, "red", 300.0).expect("placed");
        app.place_image(image);
        let id = app.selected_image.expect("the placed image is picked");
        let placed = app.engine.doc.get_image(id).expect("image");
        assert!((placed.width_mm - 50.8).abs() < 1e-9 && placed.x_mm == 0.0 && placed.y_mm == 0.0);
        app.move_image(id, stencilcraft_geom::Vec2::new(10.0, 5.0));
        let moved = app.engine.doc.get_image(id).expect("image");
        assert_eq!((moved.x_mm, moved.y_mm), (10.0, 5.0));
        assert!(app.engine.undo(), "a move is one undo step");
        assert_eq!(app.engine.doc.get_image(id).map(|i| i.x_mm), Some(0.0));
        press_key(&mut app, egui::Key::Delete, egui::Modifiers::NONE);
        assert!(
            app.engine.doc.images.is_empty(),
            "Delete removes the picked image"
        );
        assert!(app.engine.undo());
        app.selected_image = Some(id);
        app.run_command(Command::ShapeAddRect {
            x: 0.0,
            y: 0.0,
            width: 5.0,
            height: 5.0,
        });
        let shape = app.engine.doc.layers[0].shapes[0].id;
        app.set_selection([shape]);
        assert_eq!(app.selected_image, None, "picking shapes drops the image");
    }

    #[test]
    fn pen_keys_belong_to_the_draft_while_drawing() {
        // Regression: Esc discarded an unfinished pen path, Delete removed selected shapes
        // and Cmd+Z undid the previous document command instead of the last anchor.
        let mut app = StencilApp::new(None);
        app.run_command(Command::ShapeAddRect {
            x: 3.0,
            y: 4.0,
            width: 12.0,
            height: 8.0,
        });
        app.set_selection([1]);
        app.active_tool = ActiveTool::Pen;
        app.pen_draft = Some(EditSubpath::new(
            vec![
                craft_draw::ui::pen::Knot::corner(stencilcraft_geom::Point::new(0.0, 0.0)),
                craft_draw::ui::pen::Knot::corner(stencilcraft_geom::Point::new(10.0, 0.0)),
            ],
            false,
        ));
        press_key(&mut app, egui::Key::Delete, egui::Modifiers::NONE);
        press_key(&mut app, egui::Key::Escape, egui::Modifiers::NONE);
        assert_eq!(app.engine.doc.shape_count(), 1, "the rectangle survived");
        assert!(
            app.pen_draft.is_some(),
            "Esc is left for the pen to end the path"
        );
        press_key(&mut app, egui::Key::Z, egui::Modifiers::COMMAND);
        assert!(app.pen_undo_requested, "Cmd+Z goes to the draft");
        assert_eq!(
            app.engine.doc.shape_count(),
            1,
            "the document was not undone"
        );
    }

    #[test]
    fn delete_preserves_text_input_but_works_with_button_focus() {
        for text_focus in [true, false] {
            let ctx = egui::Context::default();
            crate::framework::install_headless(&ctx).unwrap();
            let mut app = StencilApp::new(None);
            app.run_command(Command::ShapeAddRect {
                x: 3.0,
                y: 4.0,
                width: 12.0,
                height: 8.0,
            });
            app.set_selection([1]);
            let mut text = String::from("abc");
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                if text_focus {
                    ui.text_edit_singleline(&mut text).request_focus();
                } else {
                    ui.button("Control").request_focus();
                }
            });
            output.textures_delta.clear();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events: vec![egui::Event::Key {
                        key: egui::Key::Delete,
                        physical_key: Some(egui::Key::Delete),
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                    ..Default::default()
                },
                |ui| {
                    app.handle_keyboard_shortcuts(ui.ctx());
                    if text_focus {
                        ui.text_edit_singleline(&mut text);
                    } else {
                        let _ = ui.button("Control");
                    }
                },
            );
            output.textures_delta.clear();
            assert_eq!(app.engine.doc.shape_count(), usize::from(text_focus));
        }
    }

    /// One pass `step` seconds after the last (`frame` steps half a second).
    fn frame_after(
        ctx: &egui::Context,
        app: &mut StencilApp,
        events: Vec<egui::Event>,
        panel: fn(&mut StencilApp, &mut egui::Ui),
        step: f64,
    ) -> egui::FullOutput {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 800.0),
                )),
                events,
                time: Some(ctx.input(|input| input.time) + step),
                ..Default::default()
            },
            |ui| panel(app, ui),
        );
        output.textures_delta.clear();
        output
    }

    fn frame(
        ctx: &egui::Context,
        app: &mut StencilApp,
        events: Vec<egui::Event>,
        panel: fn(&mut StencilApp, &mut egui::Ui),
    ) -> egui::FullOutput {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 800.0),
                )),
                events,
                // Half a second a pass: separate clicks at one spot stay separate clicks, not a
                // double click that opens and closes a list again.
                time: Some(ctx.input(|input| input.time) + 0.5),
                ..Default::default()
            },
            |ui| panel(app, ui),
        );
        output.textures_delta.clear();
        output
    }

    /// The bounds of the control named `label`. The pin's `Text` is an AccessKit `Label` node,
    /// so a heading drawn above a control with the same name ("Material" over the Material
    /// select) shares its name: the control is the named node that is not a plain label.
    fn node_rect(output: &egui::FullOutput, label: &str) -> egui::Rect {
        let tree = output.platform_output.accesskit_update.as_ref().unwrap();
        // A control named by a visible word label (`PropertyRow::word`) has no label of its own:
        // it is `labelled_by` the label's node.
        let named = |node: &egui::accesskit::Node| {
            node.label() == Some(label) || node.value() == Some(label)
        };
        let label_ids: Vec<_> = tree
            .nodes
            .iter()
            .filter(|(_, node)| named(node))
            .map(|(id, _)| *id)
            .collect();
        let named_control = |node: &egui::accesskit::Node| {
            node.label() == Some(label) && node.role() != egui::accesskit::Role::Label
        };
        let node = tree
            .nodes
            .iter()
            .find(|(_, node)| named_control(node))
            .or_else(|| {
                tree.nodes
                    .iter()
                    .find(|(_, node)| node.labelled_by().iter().any(|id| label_ids.contains(id)))
            })
            .or_else(|| {
                tree.nodes
                    .iter()
                    .find(|(_, node)| node.label() == Some(label))
            })
            .unwrap_or_else(|| panic!("missing node {label}"));
        let bounds = node.1.bounds().unwrap();
        egui::Rect::from_min_max(
            egui::pos2(bounds.x0 as f32, bounds.y0 as f32),
            egui::pos2(bounds.x1 as f32, bounds.y1 as f32),
        )
    }

    fn node_center(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
        node_rect(output, label).center()
    }

    fn click(
        ctx: &egui::Context,
        app: &mut StencilApp,
        panel: fn(&mut StencilApp, &mut egui::Ui),
        pos: egui::Pos2,
    ) {
        frame(ctx, app, vec![egui::Event::PointerMoved(pos)], panel);
        for pressed in [true, false] {
            frame(
                ctx,
                app,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }],
                panel,
            );
        }
    }

    fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    #[test]
    fn shared_kerf_slider_and_inspector_field_edit_the_document() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        crate::framework::install_headless(&ctx).unwrap();
        let mut app = StencilApp::new(None);
        app.engine
            .run(Command::ShapeAddRect {
                x: 0.0,
                y: 0.0,
                width: 5.0,
                height: 5.0,
            })
            .unwrap();
        app.set_selection([1]);
        let out = frame(&ctx, &mut app, vec![], StencilApp::inspector);
        let pos = node_center(&out, "Kerf");
        click(&ctx, &mut app, StencilApp::inspector, pos);
        assert!(app.engine.doc.kerf.width_mm > 0.0);
        assert!(app.engine.undo());
        assert_eq!(app.engine.doc.kerf.width_mm, 0.0);
        let out = frame(&ctx, &mut app, vec![], StencilApp::inspector);
        let pos = node_center(&out, "X");
        click(&ctx, &mut app, StencilApp::inspector, pos);
        frame(
            &ctx,
            &mut app,
            vec![
                key(egui::Key::A, egui::Modifiers::COMMAND),
                egui::Event::Text("12".into()),
            ],
            StencilApp::inspector,
        );
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
            StencilApp::inspector,
        );
        assert_eq!(
            app.engine.doc.get_shape(1).unwrap().bounds().unwrap().x0,
            12.0
        );
    }

    #[test]
    fn kerf_drag_cancel_preserves_history_and_commit_is_one_undo_step() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        crate::framework::install_headless(&ctx).unwrap();
        let mut app = StencilApp::new(None);
        app.engine.run(Command::KerfSet { width_mm: 0.2 }).unwrap();
        let out = frame(&ctx, &mut app, vec![], StencilApp::inspector);
        let start = node_center(&out, "Kerf");
        let end = start + egui::vec2(35.0, 0.0);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(start), button(start, true)],
            StencilApp::inspector,
        );
        // The pin's slider begins the edit on the pass the press passes the slop and proposes
        // the value from the next pass: the pointer held at `end` for one more pass.
        for _ in 0..2 {
            frame(
                &ctx,
                &mut app,
                vec![egui::Event::PointerMoved(end)],
                StencilApp::inspector,
            );
        }
        assert!(app.kerf_draft.is_some_and(|value| value != 0.2));
        assert_eq!(app.engine.doc.kerf.width_mm, 0.2);
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
            StencilApp::inspector,
        );
        assert_eq!(app.kerf_draft, None);
        frame(
            &ctx,
            &mut app,
            vec![button(end, false)],
            StencilApp::inspector,
        );
        assert_eq!(app.engine.doc.kerf.width_mm, 0.2);
        assert!(app.engine.undo());
        assert_eq!(app.engine.doc.kerf.width_mm, 0.0);
        assert!(app.engine.redo());
        assert_eq!(app.engine.doc.kerf.width_mm, 0.2);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(start), button(start, true)],
            StencilApp::inspector,
        );
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(end)],
            StencilApp::inspector,
        );
        let final_pos = end + egui::vec2(15.0, 0.0);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(final_pos)],
            StencilApp::inspector,
        );
        frame(
            &ctx,
            &mut app,
            vec![button(final_pos, false)],
            StencilApp::inspector,
        );
        let committed = app.engine.doc.kerf.width_mm;
        assert_ne!(committed, 0.2);
        assert!(app.engine.undo());
        assert_eq!(app.engine.doc.kerf.width_mm, 0.2);
        assert!(app.engine.redo());
        assert_eq!(app.engine.doc.kerf.width_mm, committed);
    }

    #[test]
    fn pressure_slider_drag_cancel_restores_value() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        crate::framework::install_headless(&ctx).unwrap();
        let mut app = StencilApp::new(None);
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        let start = node_center(&out, "Pressure");
        let end = start + egui::vec2(35.0, 0.0);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(start), button(start, true)],
            StencilApp::send_tab,
        );
        // The pin's slider begins the edit on the pass the press passes the slop and proposes
        // the value from the next pass: the pointer held at `end` for one more pass.
        for _ in 0..2 {
            frame(
                &ctx,
                &mut app,
                vec![egui::Event::PointerMoved(end)],
                StencilApp::send_tab,
            );
        }
        assert_ne!(app.send.pressure_override, 10);
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
            StencilApp::send_tab,
        );
        assert_eq!(app.send.pressure_override, 10);
        frame(
            &ctx,
            &mut app,
            vec![button(end, false)],
            StencilApp::send_tab,
        );
        assert_eq!(app.send.pressure_override, 10);
    }

    #[test]
    fn the_cutter_and_material_are_remembered() {
        let mut app = StencilApp::new(None);
        app.send.model = stencilcraft_machine::Model::Cameo5;
        app.send.media_index = 3;
        app.mat = stencilcraft_machine::Mat::Cameo12x24;
        app.remember_machine_prefs();
        let saved = app.prefs.clone();
        let mut next = StencilApp::new(None);
        next.prefs = saved;
        next.restore_machine_prefs();
        assert_eq!(next.send.model, stencilcraft_machine::Model::Cameo5);
        assert_eq!(next.send.media_index, 3);
        assert_eq!(next.mat, stencilcraft_machine::Mat::Cameo12x24);
        next.prefs.machine_model = Some("no-such-cutter".into());
        next.restore_machine_prefs();
        assert_eq!(next.send.model, stencilcraft_machine::Model::Cameo5);
    }

    #[test]
    fn drawing_snaps_to_the_grid_when_snap_to_grid_is_on() {
        let mut app = StencilApp::new(None);
        let off = stencilcraft_geom::Point::new(10.4, 19.7);
        assert_eq!(
            app.snap_world_point(off),
            off,
            "no snapping by default for the grid"
        );
        app.handle_menu_action(TopMenuAction::ToggleSnapGrid);
        let snapped = app.snap_world_point(off);
        assert!(
            (snapped.x - 10.0).abs() < 1e-9 && (snapped.y - 20.0).abs() < 1e-9,
            "{snapped:?}"
        );
    }

    #[test]
    fn resizing_snaps_edges_and_ctrl_suspends_snapping() {
        use craft_behavior::kurbo::Affine;
        let mut app = StencilApp::new(None);
        app.handle_menu_action(TopMenuAction::ToggleSnapGrid);
        let bounds = stencilcraft_geom::Rect::new(0.0, 0.0, 20.0, 10.0);
        // Stretch the right edge to 29.7: it snaps to the 30 mm grid line.
        let stretch = Affine::new([29.7 / 20.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
        let snapped = app
            .snap_box_scale(bounds, stretch)
            .transform_rect_bbox(bounds);
        assert!((snapped.x1 - 30.0).abs() < 1e-9, "{snapped:?}");
        assert!(
            (snapped.y1 - 10.0).abs() < 1e-9,
            "height unchanged: {snapped:?}"
        );
        // A proportional corner resize keeps its proportions.
        let corner = Affine::scale(1.49);
        let snapped = app
            .snap_box_scale(bounds, corner)
            .transform_rect_bbox(bounds);
        assert!(
            (snapped.width() / snapped.height() - 2.0).abs() < 1e-9,
            "{snapped:?}"
        );
        assert!((snapped.x1 - 30.0).abs() < 1e-9, "{snapped:?}");
        // Rotations pass through.
        let turn = Affine::rotate(0.3);
        assert_eq!(app.snap_box_scale(bounds, turn), turn);
        // Holding Ctrl suspends snapping.
        app.snap_suspended = true;
        let free = app
            .snap_box_scale(bounds, stretch)
            .transform_rect_bbox(bounds);
        assert!((free.x1 - 29.7).abs() < 1e-9, "{free:?}");
        let off = stencilcraft_geom::Point::new(10.4, 19.7);
        assert_eq!(app.snap_world_point(off), off);
    }

    #[test]
    fn smart_snap_only_within_media_keeps_off_page_points_free() {
        let mut app = StencilApp::new(None);
        app.engine.doc.snap.grid = false;
        app.engine.doc.snap.objects = true;
        // Just right of the 100 mm page edge: the page edge is a smart-snap target.
        let off = stencilcraft_geom::Point::new(101.0, 75.0);
        let snapped = app.snap_world_point(off);
        assert!(
            (snapped.x - 100.0).abs() < 1e-9,
            "snaps to the page edge: {snapped:?}"
        );
        app.prefs.snap_within_media = true;
        assert_eq!(
            app.snap_world_point(off),
            off,
            "off the media nothing smart snaps"
        );
        let on = stencilcraft_geom::Point::new(99.0, 75.0);
        assert!(
            (app.snap_world_point(on).x - 100.0).abs() < 1e-9,
            "on the media it does"
        );
    }

    #[test]
    fn dragged_nodes_snap_but_not_to_their_own_shape() {
        let mut app = StencilApp::new(None);
        app.run_command(Command::ShapeAddRect {
            x: 0.0,
            y: 0.0,
            width: 20.0,
            height: 20.0,
        });
        let off = stencilcraft_geom::Point::new(40.4, 39.6);
        assert_eq!(
            snap_point_in(&app.engine.doc, off, &[], false),
            off,
            "snapping off"
        );
        app.handle_menu_action(TopMenuAction::ToggleSnapGrid);
        let snapped = snap_point_in(&app.engine.doc, off, &[1], false);
        assert!(
            (snapped.x - 40.0).abs() < 1e-9 && (snapped.y - 40.0).abs() < 1e-9,
            "{snapped:?}"
        );
    }

    #[test]
    fn type_menu_outlines_and_letters_and_next_object_steps_the_stack() {
        let mut app = StencilApp::new(None);
        app.text_family = stencilcraft_engine::text::SINGLE_LINE_FAMILY.to_string();
        app.begin_text_at(stencilcraft_geom::Point::new(20.0, 40.0));
        app.text_insert("Hi");
        let text = app.text_session.as_ref().and_then(|s| s.id).expect("text");
        app.end_text_session();
        app.set_selection([text]);
        app.handle_menu_action(TopMenuAction::TextUngroupLetters);
        assert_eq!(app.selected_shapes.len(), 2, "one text per letter");
        assert!(app.selected_text_ids().count() == 2);
        app.handle_menu_action(TopMenuAction::TextToOutlines);
        assert_eq!(app.selected_shapes.len(), 2);
        assert_eq!(
            app.selected_text_ids().count(),
            0,
            "outlines are plain paths"
        );
        assert!(
            app.engine
                .doc
                .get_shape(text)
                .is_none_or(|s| s.text.is_none())
        );

        // Three stacked rectangles: Next Object Above/Below walk the stack and stop at its ends.
        let mut app = StencilApp::new(None);
        for x in [0.0, 5.0, 10.0] {
            app.run_command(Command::ShapeAddRect {
                x,
                y: 0.0,
                width: 20.0,
                height: 20.0,
            });
        }
        let ids: Vec<u64> = app.engine.doc.layers[0]
            .shapes
            .iter()
            .map(|s| s.id)
            .collect();
        app.set_selection([ids[0]]);
        app.handle_menu_action(TopMenuAction::SelectNextObject { above: true });
        assert_eq!(app.selected_shapes, vec![ids[1]]);
        app.handle_menu_action(TopMenuAction::SelectNextObject { above: true });
        app.handle_menu_action(TopMenuAction::SelectNextObject { above: true });
        assert_eq!(app.selected_shapes, vec![ids[2]], "stays at the top");
        app.handle_menu_action(TopMenuAction::SelectNextObject { above: false });
        assert_eq!(app.selected_shapes, vec![ids[1]]);
    }

    /// Feed raw events through the keyboard handler, as one frame.
    fn type_events(app: &mut StencilApp, events: Vec<egui::Event>) {
        let ctx = egui::Context::default();
        crate::framework::install_headless(&ctx).unwrap();
        let mut output = ctx.run_ui(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ui| app.handle_keyboard_shortcuts(ui.ctx()),
        );
        output.textures_delta.clear();
    }

    #[test]
    fn text_is_typed_on_the_canvas_one_undo_step_per_key() {
        let mut app = StencilApp::new(None);
        assert_eq!(app.text_size, 72.0, "Silhouette's default size");
        app.text_family = stencilcraft_engine::text::SINGLE_LINE_FAMILY.to_string();
        app.text_size = 24.0;
        app.begin_text_at(stencilcraft_geom::Point::new(30.0, 50.0));
        assert!(
            app.engine.doc.shapes().next().is_none(),
            "nothing until a letter"
        );
        // Letters type; "t" and "r" must not switch tools while typing.
        type_events(&mut app, vec![egui::Event::Text("Hext".into())]);
        type_events(
            &mut app,
            vec![key(egui::Key::Backspace, egui::Modifiers::NONE)],
        );
        type_events(
            &mut app,
            vec![key(egui::Key::ArrowLeft, egui::Modifiers::NONE)],
        );
        type_events(
            &mut app,
            vec![key(egui::Key::Backspace, egui::Modifiers::NONE)],
        );
        type_events(&mut app, vec![egui::Event::Text("llo r".into())]);
        type_events(&mut app, vec![key(egui::Key::End, egui::Modifiers::NONE)]);
        type_events(&mut app, vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
        type_events(&mut app, vec![egui::Event::Text("t".into())]);
        assert_eq!(app.active_tool, ActiveTool::Text);
        assert_eq!(app.session_text(), "Hllo rx\nt");
        let id = app
            .text_session
            .as_ref()
            .and_then(|s| s.id)
            .expect("text shape");
        let shape = app.engine.doc.get_shape(id).expect("shape");
        let spec = shape.text.as_deref().expect("live text");
        assert_eq!((spec.size, spec.x, spec.y), (24.0, 30.0, 50.0));
        assert_eq!(shape.style, CutStyle::Engrave, "single-line text is drawn");
        // Cmd+Z takes back the last key and keeps typing.
        type_events(&mut app, vec![key(egui::Key::Z, egui::Modifiers::COMMAND)]);
        assert_eq!(app.session_text(), "Hllo rx\n");
        assert!(app.text_session.is_some());
        // Esc ends typing and returns to the Select tool with nothing selected.
        type_events(
            &mut app,
            vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
        );
        assert!(app.text_session.is_none());
        assert_eq!(app.active_tool, ActiveTool::Select);
        assert!(app.selected_shapes.is_empty());
        // Edit Selected Text picks it up again; deleting every letter removes the shape.
        app.set_selection([id]);
        app.handle_menu_action(TopMenuAction::EditText);
        assert_eq!(app.text_session.as_ref().and_then(|s| s.id), Some(id));
        for _ in 0..12 {
            app.text_erase(false);
        }
        assert!(app.engine.doc.get_shape(id).is_none());
        let saved = app.engine.doc.save_json().expect("saves");
        assert!(stencilcraft_doc::Document::load_json(&saved).is_ok());
    }

    #[test]
    fn text_style_changes_the_text_being_typed() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        crate::framework::install_headless(&ctx).unwrap();
        let mut app = StencilApp::new(None);
        app.text_family = stencilcraft_engine::text::SINGLE_LINE_FAMILY.to_string();
        app.begin_text_at(stencilcraft_geom::Point::new(10.0, 40.0));
        app.text_insert("AV");
        let id = app.text_session.as_ref().and_then(|s| s.id).expect("text");
        frame(&ctx, &mut app, vec![], StencilApp::text_style_panel);
        assert_eq!(app.text_style.char_pct, 100.0);
        // The panel shows the text's own style: Character Spacing on Silhouette's scale.
        let edit = stencilcraft_engine::text::TextEdit {
            id,
            size: Some(36.0),
            letter_spacing: Some(text_tool::letter_spacing_for(200.0)),
            ..Default::default()
        };
        app.engine.run(Command::TextEdit(edit)).expect("edit");
        frame(&ctx, &mut app, vec![], StencilApp::text_style_panel);
        assert!((app.text_style.char_pct - 200.0).abs() < 1e-9);
        assert_eq!(app.text_size, 36.0);
    }

    #[test]
    fn edit_text_requires_live_text_and_invalid_font_is_reported() {
        let mut app = StencilApp::new(None);
        app.handle_menu_action(TopMenuAction::EditText);
        assert!(app.text_session.is_none());
        assert!(app.status.contains("select a live text"));
        app.text_family = "StencilCraft font that does not exist".to_string();
        app.begin_text_at(stencilcraft_geom::Point::new(10.0, 10.0));
        app.text_insert("Hello");
        assert!(app.engine.doc.shapes().next().is_none());
        assert!(app.status.contains("not installed"), "{}", app.status);
    }

    #[test]
    fn shared_send_sliders_accept_input() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        crate::framework::install_headless(&ctx).unwrap();
        let mut app = StencilApp::new(None);
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Pressure"),
        );
        assert_ne!(app.send.pressure_override, 10);
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Speed"),
        );
        assert_ne!(app.send.speed_override, 5);
    }

    #[test]
    fn inspector_and_layers_panels_draw_with_lists_in_a_scrolling_panel() {
        // Regression: at startup the Inspector's Objects list sat in the scrolling panel without
        // natural height or capped rows, and the framework's collection check panicked.
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        crate::framework::install_headless(&ctx).unwrap();
        let mut app = StencilApp::new(None);
        for x in [0.0, 30.0] {
            app.run_command(Command::ShapeAddRect {
                x,
                y: 0.0,
                width: 20.0,
                height: 20.0,
            });
        }
        app.set_selection([1]);
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![], StencilApp::inspector_panel);
            frame(&ctx, &mut app, vec![], StencilApp::inspector);
            frame(&ctx, &mut app, vec![], StencilApp::layers_panel);
        }
    }

    #[test]
    fn shared_zoom_slider_changes_the_canvas_view() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        crate::framework::install_headless(&ctx).unwrap();
        let mut app = StencilApp::new(None);
        frame(&ctx, &mut app, vec![], StencilApp::canvas_body);
        let old_zoom = app.view.zoom;
        let out = frame(&ctx, &mut app, vec![], StencilApp::status_bar);
        // The slider takes the value a screen reader sets (the pin's `Slider` reads AccessKit's
        // SetValue as its set-value intent): 400 % on the slider's node.
        use egui::accesskit::{Action, ActionData, ActionRequest, Role, TreeId};
        let tree = out.platform_output.accesskit_update.as_ref().unwrap();
        let (slider, _) = tree
            .nodes
            .iter()
            .find(|(_, node)| node.label() == Some("Zoom") && node.role() == Role::Slider)
            .unwrap();
        let set = egui::Event::AccessKitActionRequest(ActionRequest {
            action: Action::SetValue,
            target_tree: TreeId::ROOT,
            target_node: *slider,
            data: Some(ActionData::NumericValue(400.0)),
        });
        frame(&ctx, &mut app, vec![set], StencilApp::status_bar);
        assert!(app.pending_zoom.is_some());
        assert_eq!(app.pending_zoom, Some(4.0), "400 % is a zoom of 4");
        frame(&ctx, &mut app, vec![], StencilApp::canvas_body);
        assert_ne!(app.view.zoom, old_zoom);
        assert!(app.pending_zoom.is_none());
    }

    #[test]
    fn material_selection_updates_overrides_and_cancel_restores_selection() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        crate::framework::install_headless(&ctx).unwrap();
        let mut app = StencilApp::new(None);
        let initial = app.send.media_index;
        assert!(initial + 1 < stencilcraft_machine::MEDIA_PRESETS.len());
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Material"),
        );
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::ArrowDown, egui::Modifiers::NONE)],
            StencilApp::send_tab,
        );
        frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        assert_eq!(app.send.media_index, initial + 1);
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
            StencilApp::send_tab,
        );
        assert_eq!(app.send.media_index, initial);
        frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Material"),
        );
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::ArrowDown, egui::Modifiers::NONE)],
            StencilApp::send_tab,
        );
        frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
            StencilApp::send_tab,
        );
        frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        let media = &stencilcraft_machine::MEDIA_PRESETS[initial + 1];
        assert_eq!(app.send.media_index, initial + 1);
        assert_eq!(app.send.pressure_override, media.pressure.unwrap_or(10));
        assert_eq!(app.send.speed_override, media.speed.unwrap_or(5));
    }

    #[test]
    fn material_selection_changes_encoded_dry_run_commands() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        crate::framework::install_headless(&ctx).unwrap();
        let mut app = StencilApp::new(None);
        app.engine
            .run(Command::ShapeAddRect {
                x: 2.0,
                y: 3.0,
                width: 5.0,
                height: 4.0,
            })
            .unwrap();
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Encode dry run"),
        );
        let initial = app.send.summary.clone().unwrap();
        assert!(initial.iter().any(|line| line == "FX10,1"));
        assert!(initial.iter().any(|line| line == "!5,1"));
        assert!(initial.iter().any(|line| line.starts_with('D')));

        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Material"),
        );
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::ArrowDown, egui::Modifiers::NONE)],
            StencilApp::send_tab,
        );
        frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
            StencilApp::send_tab,
        );
        frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        assert_eq!(
            stencilcraft_machine::MEDIA_PRESETS[app.send.media_index].id,
            106
        );
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Encode dry run"),
        );
        let updated = app.send.summary.as_ref().unwrap();
        assert!(updated.iter().any(|line| line == "FX14,1"));
        assert!(updated.iter().any(|line| line == "!10,1"));
        assert!(!updated.iter().any(|line| line == "FX10,1"));
        assert!(!updated.iter().any(|line| line == "!5,1"));
        assert!(updated.iter().any(|line| line.starts_with('D')));
    }

    #[test]
    fn selected_model_and_mat_change_dry_run_setup() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        crate::framework::install_headless(&ctx).unwrap();
        let mut app = StencilApp::new(None);
        app.engine
            .run(Command::ShapeAddRect {
                x: 2.0,
                y: 3.0,
                width: 5.0,
                height: 4.0,
            })
            .unwrap();
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Encode dry run"),
        );
        let baseline = app.send.summary.clone().unwrap();
        assert!(baseline.iter().any(|line| line == "TG9"));
        assert!(baseline.iter().any(|line| line == "Z60000,12000"));
        assert!(baseline[0].contains("Cameo 4 Pro"));

        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Model"),
        );
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::End, egui::Modifiers::NONE)],
            StencilApp::send_tab,
        );
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
            StencilApp::send_tab,
        );
        frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        assert_eq!(app.send.model, stencilcraft_machine::Model::Portrait3);
        let out = frame(&ctx, &mut app, vec![], StencilApp::inspector);
        click(
            &ctx,
            &mut app,
            StencilApp::inspector,
            node_center(&out, "Cutting mat"),
        );
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::Home, egui::Modifiers::NONE)],
            StencilApp::inspector,
        );
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
            StencilApp::inspector,
        );
        frame(&ctx, &mut app, vec![], StencilApp::inspector);
        assert_eq!(app.mat, stencilcraft_machine::Mat::None);
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Encode dry run"),
        );
        let updated = app.send.summary.as_ref().unwrap();
        assert!(updated.iter().any(|line| line == "TG0"));
        assert!(updated.iter().any(|line| line == "Z365800,4060"));
        assert!(
            !updated
                .iter()
                .any(|line| line == "TG9" || line == "Z60000,12000")
        );
        assert!(updated[0].contains("Portrait 3"));
        assert!(updated.iter().any(|line| line.contains("no mat")));
        assert!(updated.iter().any(|line| line.starts_with('D')));
    }

    #[test]
    fn screen_reader_actions_select_material_and_update_dry_run() {
        use egui::accesskit::{Action, ActionRequest, Role, TreeId};
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        crate::framework::install_headless(&ctx).unwrap();
        let mut app = StencilApp::new(None);
        app.engine
            .run(Command::ShapeAddRect {
                x: 2.0,
                y: 3.0,
                width: 5.0,
                height: 4.0,
            })
            .unwrap();
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        let tree = out.platform_output.accesskit_update.as_ref().unwrap();
        // The "Material" heading above the select is a `Label` node of the same name.
        let (selector_id, selector) = tree
            .nodes
            .iter()
            .find(|(_, node)| node.label() == Some("Material") && node.role() != Role::Label)
            .unwrap();
        assert_eq!(selector.role(), Role::ComboBox);
        assert!(selector.supports_action(Action::Click));
        let request = |action, target_node| {
            egui::Event::AccessKitActionRequest(ActionRequest {
                action,
                target_tree: TreeId::ROOT,
                target_node,
                data: None,
            })
        };
        frame(
            &ctx,
            &mut app,
            vec![request(Action::Click, *selector_id)],
            StencilApp::send_tab,
        );
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        let tree = out.platform_output.accesskit_update.as_ref().unwrap();
        let (option_id, option) = tree
            .nodes
            .iter()
            .find(|(_, node)| {
                node.role() == Role::ListBoxOption && node.label() == Some("Film Labels (ID 106)")
            })
            .unwrap();
        assert!(option.supports_action(Action::Click));
        frame(
            &ctx,
            &mut app,
            vec![request(Action::Click, *option_id)],
            StencilApp::send_tab,
        );
        frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        assert_eq!(
            stencilcraft_machine::MEDIA_PRESETS[app.send.media_index].id,
            106
        );
        assert_eq!(
            (app.send.pressure_override, app.send.speed_override),
            (14, 10)
        );
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Encode dry run"),
        );
        let summary = app.send.summary.as_ref().unwrap();
        assert!(summary.iter().any(|line| line == "FX14,1"));
        assert!(summary.iter().any(|line| line == "!10,1"));
    }

    #[test]
    fn material_selector_supports_accessible_focus_and_keyboard_selection() {
        use egui::accesskit::{Action, ActionRequest, HasPopup, Role, TreeId};
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        crate::framework::install_headless(&ctx).unwrap();
        let mut app = StencilApp::new(None);
        let initial = app.send.media_index;
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        let tree = out.platform_output.accesskit_update.as_ref().unwrap();
        // The "Material" heading above the select is a `Label` node of the same name.
        let (id, node) = tree
            .nodes
            .iter()
            .find(|(_, node)| node.label() == Some("Material") && node.role() != Role::Label)
            .unwrap();
        assert_eq!(node.role(), Role::ComboBox);
        assert_eq!(node.value(), Some(media_label(initial).as_str()));
        assert_eq!(node.is_expanded(), Some(false));
        assert_eq!(node.has_popup(), Some(HasPopup::Listbox));
        let material_id = *id;
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::AccessKitActionRequest(ActionRequest {
                action: Action::Focus,
                target_tree: TreeId::ROOT,
                target_node: material_id,
                data: None,
            })],
            StencilApp::send_tab,
        );
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::ArrowDown, egui::Modifiers::ALT)],
            StencilApp::send_tab,
        );
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        let tree = out.platform_output.accesskit_update.as_ref().unwrap();
        let combo = tree
            .nodes
            .iter()
            .find(|(id, _)| *id == material_id)
            .unwrap();
        assert_eq!(combo.1.is_expanded(), Some(true));
        let initial_label = media_label(initial);
        let option = tree
            .nodes
            .iter()
            .find(|(_, node)| {
                node.label() == Some(initial_label.as_str()) && node.role() == Role::ListBoxOption
            })
            .unwrap();
        assert_eq!(option.1.is_selected(), Some(true));
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::ArrowDown, egui::Modifiers::NONE)],
            StencilApp::send_tab,
        );
        frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        assert_eq!(app.send.media_index, initial + 1);
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
            StencilApp::send_tab,
        );
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        let tree = out.platform_output.accesskit_update.as_ref().unwrap();
        let combo = tree
            .nodes
            .iter()
            .find(|(id, _)| *id == material_id)
            .unwrap();
        assert_eq!(combo.1.is_expanded(), Some(false));
        assert_eq!(combo.1.value(), Some(media_label(initial + 1).as_str()));
        let media = &stencilcraft_machine::MEDIA_PRESETS[initial + 1];
        assert_eq!(
            (app.send.pressure_override, app.send.speed_override),
            (media.pressure.unwrap_or(10), media.speed.unwrap_or(5))
        );
        let settings = (app.send.pressure_override, app.send.speed_override);
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::Space, egui::Modifiers::NONE)],
            StencilApp::send_tab,
        );
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::ArrowDown, egui::Modifiers::NONE)],
            StencilApp::send_tab,
        );
        frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        assert_eq!(app.send.media_index, initial + 2);
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
            StencilApp::send_tab,
        );
        frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        assert_eq!(app.send.media_index, initial + 1);
        assert_eq!(
            (app.send.pressure_override, app.send.speed_override),
            settings
        );
    }

    #[test]
    fn custom_overrides_and_bounding_box_preview_reach_dry_run_program() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        crate::framework::install_headless(&ctx).unwrap();
        let mut app = StencilApp::new(None);
        let mut path = stencilcraft_geom::BezPath::new();
        path.move_to(stencilcraft_geom::Point::new(2.0, 3.0));
        path.line_to(stencilcraft_geom::Point::new(7.0, 3.0));
        path.line_to(stencilcraft_geom::Point::new(4.0, 7.0));
        path.close_path();
        app.engine
            .run(Command::ShapeAddPath {
                path,
                style: stencilcraft_doc::CutStyle::Cut,
            })
            .unwrap();

        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Material"),
        );
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::Home, egui::Modifiers::NONE)],
            StencilApp::send_tab,
        );
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
            StencilApp::send_tab,
        );
        frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        assert_eq!(
            stencilcraft_machine::MEDIA_PRESETS[app.send.media_index].id,
            300
        );
        assert_eq!(
            (app.send.pressure_override, app.send.speed_override),
            (10, 5)
        );

        for label in ["Pressure", "Speed"] {
            let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
            click(
                &ctx,
                &mut app,
                StencilApp::send_tab,
                node_center(&out, label),
            );
            frame(
                &ctx,
                &mut app,
                vec![key(egui::Key::End, egui::Modifiers::NONE)],
                StencilApp::send_tab,
            );
        }
        assert_eq!(
            (app.send.pressure_override, app.send.speed_override),
            (33, 30)
        );
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Encode dry run"),
        );
        let normal = app.send.summary.clone().unwrap();
        assert!(normal.iter().any(|line| line == "FX33,1"));
        assert!(normal.iter().any(|line| line == "!30,1"));
        assert!(normal.iter().any(|line| line == "D140,80"));
        assert!(!normal.iter().any(|line| line == "D140,140"));

        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Bounding box preview"),
        );
        assert!(app.send.bbox_only);
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Encode dry run"),
        );
        let preview = app.send.summary.as_ref().unwrap();
        assert!(preview.iter().any(|line| line == "FX33,1"));
        assert!(preview.iter().any(|line| line == "!30,1"));
        let plot: Vec<&str> = preview
            .iter()
            .filter(|line| line.starts_with('M') || line.starts_with('D'))
            .map(String::as_str)
            .collect();
        assert_eq!(
            plot,
            [
                "M60,40", "D60,140", "D140,140", "D140,40", "D60,40", "M140,0"
            ]
        );
        assert!(!preview.iter().any(|line| line == "D140,80"));
    }

    #[test]
    fn checkbox_interactions_change_preview_and_layer_visibility() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        crate::framework::install_headless(&ctx).unwrap();
        let mut app = StencilApp::new(None);
        let layer_label = format!("{}  ·  0", app.engine.doc.layers[0].name);
        let out = frame(&ctx, &mut app, vec![], StencilApp::inspector);
        click(
            &ctx,
            &mut app,
            StencilApp::inspector,
            node_center(&out, &layer_label),
        );
        assert!(!app.engine.doc.layers[0].visible);
        let out = frame(&ctx, &mut app, vec![], StencilApp::inspector);
        click(
            &ctx,
            &mut app,
            StencilApp::inspector,
            node_center(&out, "Preview kerf compensation"),
        );
        assert!(app.kerf_preview);
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Bounding box preview"),
        );
        assert!(app.send.bbox_only);
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Bounding box preview"),
        );
        assert!(!app.send.bbox_only);
    }

    #[test]
    fn design_system_controls_dispatch_app_actions() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        crate::framework::install_headless(&ctx).unwrap();
        let mut app = StencilApp::new(None);
        {
            let mut frame = |events: Vec<egui::Event>| {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(800.0, 600.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| app.tools_panel(ui),
                );
                output.textures_delta.clear();
                output
            };
            let output = frame(vec![]);
            let tree = output.platform_output.accesskit_update.unwrap();
            let node = tree
                .nodes
                .iter()
                .find(|(_, node)| node.label() == Some("Rectangle"))
                .unwrap();
            assert_eq!(node.1.role(), egui::accesskit::Role::Button);
            let bounds = node.1.bounds().unwrap();
            let pos = egui::pos2(
                ((bounds.x0 + bounds.x1) / 2.0) as f32,
                ((bounds.y0 + bounds.y1) / 2.0) as f32,
            );
            let pointer = |pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            let _ = frame(vec![egui::Event::PointerMoved(pos)]);
            let _ = frame(vec![pointer(true)]);
            let _ = frame(vec![pointer(false)]);
            let tree = frame(vec![]).platform_output.accesskit_update.unwrap();
            let node = tree
                .nodes
                .iter()
                .find(|(_, node)| node.label() == Some("Duplicate"))
                .unwrap();
            assert_eq!(node.1.role(), egui::accesskit::Role::Button);
            let bounds = node.1.bounds().unwrap();
            let pos = egui::pos2(
                ((bounds.x0 + bounds.x1) / 2.0) as f32,
                ((bounds.y0 + bounds.y1) / 2.0) as f32,
            );
            let pointer = |pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            let _ = frame(vec![egui::Event::PointerMoved(pos)]);
            let _ = frame(vec![pointer(true)]);
            let _ = frame(vec![pointer(false)]);
        }
        assert_eq!(app.engine.doc.shape_count(), 2);
        assert_eq!(app.selected_shape, Some(2));
        assert_eq!(app.selected_shapes, vec![2]);
        // Silhouette places a duplicate 40 screen pixels to the right.
        let step = 40.0 / f64::from(app.view.zoom);
        let original = app.engine.doc.get_shape(1).unwrap().bounds().unwrap();
        let copy = app.engine.doc.get_shape(2).unwrap().bounds().unwrap();
        assert!(
            (copy.x0 - original.x0 - step).abs() < 1e-6,
            "{original:?} {copy:?}"
        );
        assert!((copy.y0 - original.y0).abs() < 1e-6);
        assert_eq!(
            app.engine.doc.get_shape(1).unwrap().style,
            app.engine.doc.get_shape(2).unwrap().style
        );
    }

    #[test]
    fn rulers_measure_canvas_and_reset_origin() {
        let mut app = StencilApp::new(None);
        assert_eq!(app.ruler_origin, stencilcraft_geom::Point::ZERO);
        app.ruler_origin = stencilcraft_geom::Point::new(50.0, 25.0);
        assert_eq!(app.ruler_origin, stencilcraft_geom::Point::new(50.0, 25.0));
        app.ruler_origin = stencilcraft_geom::Point::ZERO;
        assert_eq!(app.ruler_origin, stencilcraft_geom::Point::ZERO);
    }

    #[test]
    fn tool_rail_switches_active_tools_and_shortcuts() {
        let mut app = StencilApp::new(None);
        assert_eq!(app.active_tool, ActiveTool::Select);
        assert!(!app.active_tool.is_shape_creator());

        app.active_tool = ActiveTool::Pen;
        assert_eq!(app.active_tool.label(), "Pen");

        app.active_tool = ActiveTool::Rect;
        assert!(app.active_tool.is_shape_creator());
        assert_eq!(app.active_tool.label(), "Rectangle");

        app.active_tool = ActiveTool::Ellipse;
        assert!(app.active_tool.is_shape_creator());
        assert_eq!(app.active_tool.label(), "Ellipse");
    }

    #[test]
    fn pen_draft_commit_creates_cut_path() {
        let mut app = StencilApp::new(None);
        let initial_count = app.engine.doc.shape_count();
        let k1 = craft_draw::ui::pen::Knot::corner(stencilcraft_geom::Point::new(10.0, 10.0));
        let k2 = craft_draw::ui::pen::Knot::corner(stencilcraft_geom::Point::new(50.0, 10.0));
        let k3 = craft_draw::ui::pen::Knot::corner(stencilcraft_geom::Point::new(50.0, 40.0));
        let sub = craft_draw::ui::pen::EditSubpath::new(vec![k1, k2, k3], true);
        app.pen_draft = Some(sub);
        app.commit_pen_draft();
        assert_eq!(app.engine.doc.shape_count(), initial_count + 1);
        assert!(app.engine.can_undo());
        let _ = app.engine.undo();
        assert_eq!(app.engine.doc.shape_count(), initial_count);
    }
}
