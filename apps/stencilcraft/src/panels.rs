//! Silhouette Studio's right-hand panels where the engine has the commands: Transform, Replicate,
//! Offset, Modify and Point Editing (`docs/reference-apps.md`), plus the menu actions behind them.
//! Every button runs one engine command, so each is one undo step.

use craft_draw::ui::pen::PathSelection;
use craft_theme::TypeRole;
use craft_ui::canvas::quad::Anchor9;
use craft_ui::composites::SectionHeader;
use craft_ui::controls::{Button, ButtonVariant, ControlSize, NumberField, Segmented};
use craft_ui::edit::EditEvent;
use craft_ui::icons::Icon;
use craft_ui::localized::{LocaleNumbers, Unit};
use craft_ui::patterns::{AlignBar, Panel};
use craft_ui::primitives::{Content, Text};
use eframe::egui;
use stencilcraft_geom::BezPath;
use stencilcraft_engine::{
    BooleanOperation, Command, DistributionAxis, Effect, NodeAxis, NodeRef, NodeType,
    SegmentType, Side,
};
use stencilcraft_kerf::JoinStyle;

use crate::{AlignCommand, StencilApp, align_command};

/// A side as the user sees it on the canvas. The canvas draws document y downwards while the
/// engine's sides are y-up (`above` = larger y), so screen Above is the engine's `below`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScreenSide {
    Left,
    Right,
    Above,
    Below,
}

impl ScreenSide {
    pub const ALL: [ScreenSide; 4] = [Self::Left, Self::Right, Self::Above, Self::Below];

    pub fn engine(self) -> Side {
        match self {
            Self::Left => Side::Left,
            Self::Right => Side::Right,
            Self::Above => Side::Below,
            Self::Below => Side::Above,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Left => "Left",
            Self::Right => "Right",
            Self::Above => "Above",
            Self::Below => "Below",
        }
    }

    fn icon(self) -> Icon {
        match self {
            Self::Left => Icon::ChevronLeft,
            Self::Right => Icon::ChevronRight,
            Self::Above => Icon::ChevronUp,
            Self::Below => Icon::ChevronDown,
        }
    }
}

/// An editing action the menus, shortcuts and panels share.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditAction {
    Duplicate(ScreenSide),
    Mirror(ScreenSide),
    /// A row (horizontal) or column (vertical) of the Replicate panel's copy count.
    Array(DistributionAxis),
    FillPage,
    /// A radial array of the Replicate panel's count.
    RotateCopies,
    Lock,
    UnlockAll,
    Hide,
    ShowAll,
    ToPath,
    MakeCompound,
    ReleaseCompound,
    /// With the Point Editing panel's tolerance.
    Simplify,
    /// Join the first two selected shapes' open ends.
    Join,
    /// Average the selected nodes.
    Average(NodeAxis),
    MergeNodes,
    DeleteNodes,
    /// Break the path at the first selected node.
    Break,
    Close,
    NodeType(NodeType),
    Segment(SegmentType),
    /// With the Modify panel's stroke width.
    OutlineStroke,
    SelectSameFill,
    SelectSameStroke,
    SelectInverse,
    /// Open the Offset panel, set to an external or internal offset.
    Offset {
        internal: bool,
    },
    TogglePointEdit,
}

/// The Offset panel's settings, kept while the panel is closed.
#[derive(Clone, Debug, PartialEq)]
pub struct OffsetDraft {
    pub distance_mm: f64,
    pub internal: bool,
    pub join: JoinStyle,
    pub limit: f64,
    /// The preview shows (a control was touched and neither Apply nor Cancel came since).
    pub armed: bool,
}

impl Default for OffsetDraft {
    fn default() -> Self {
        Self {
            distance_mm: 2.0,
            internal: false,
            join: JoinStyle::Round,
            limit: stencilcraft_kerf::DEFAULT_MITER_LIMIT,
            armed: false,
        }
    }
}

impl OffsetDraft {
    /// The `shapes.offset` command for `ids`.
    pub fn command(&self, ids: Vec<u64>) -> Command {
        let distance = self.distance_mm.abs();
        Command::ShapesOffset {
            ids,
            distance_mm: if self.internal { -distance } else { distance },
            join: self.join,
            miter_limit: self.limit,
        }
    }
}

/// Preview outlines and what they were made from, so they are not rebuilt every frame.
#[derive(Clone, Debug, PartialEq)]
pub struct OffsetPreview {
    pub source: Vec<BezPath>,
    pub draft: OffsetDraft,
    pub result: Result<Vec<BezPath>, String>,
}

/// State of the Silhouette-style panels.
#[derive(Clone, Debug, PartialEq)]
pub struct PanelState {
    pub gap_mm: f64,
    pub copies: f64,
    pub rotate_count: f64,
    pub offset: OffsetDraft,
    pub offset_preview: Option<OffsetPreview>,
    /// The canvas paints the preview only in a frame the Offset panel was drawn in.
    pub offset_preview_shown: bool,
    pub simplify_mm: f64,
    pub stroke_mm: f64,
    pub move_mm: [f64; 2],
    pub rotate_deg: f64,
    pub scale_pct: [f64; 2],
    pub scale_linked: bool,
    /// The options bar's reference point for X/Y/W/H.
    pub pivot: Anchor9,
}

impl Default for PanelState {
    fn default() -> Self {
        Self {
            gap_mm: 1.0,
            copies: 3.0,
            rotate_count: 4.0,
            offset: OffsetDraft::default(),
            offset_preview: None,
            offset_preview_shown: false,
            simplify_mm: 0.1,
            stroke_mm: 1.0,
            move_mm: [10.0, 10.0],
            rotate_deg: 45.0,
            scale_pct: [100.0, 100.0],
            scale_linked: true,
            pivot: Anchor9::TopLeft,
        }
    }
}

/// The selection bounds `[x, y, w, h]` after `value` is typed into options-bar field `field`
/// (0 X, 1 Y, 2 W, 3 H). X and Y place the reference point `pivot`; W and H resize about it,
/// the other side following in proportion when `linked`.
pub fn edit_bounds(
    bounds: [f64; 4],
    pivot: Anchor9,
    field: usize,
    value: f64,
    linked: bool,
) -> [f64; 4] {
    let (u, v) = pivot.uv();
    let [x, y, w, h] = bounds;
    let (px, py) = (x + u * w, y + v * h);
    let (nw, nh) = match field {
        0 => return [value - u * w, y, w, h],
        1 => return [x, value - v * h, w, h],
        2 if linked && w > 1e-9 => (value, h * value / w),
        2 => (value, h),
        3 if linked && h > 1e-9 => (w * value / h, value),
        3 => (w, value),
        _ => return bounds,
    };
    [px - u * nw, py - v * nh, nw, nh]
}

/// The `shapes.transform` that takes `ids` from bounds `from` to `to` (`[x, y, w, h]`): it scales
/// about the bounds centre, then moves the centre. A zero-width or -height side keeps its scale.
pub fn bounds_command(ids: Vec<u64>, from: [f64; 4], to: [f64; 4]) -> Option<Command> {
    let scale = |new: f64, old: f64| if old > 1e-9 { new / old } else { 1.0 };
    let (scale_x, scale_y) = (scale(to[2], from[2]), scale(to[3], from[3]));
    let dx_mm = (to[0] + to[2] / 2.0) - (from[0] + from[2] / 2.0);
    let dy_mm = (to[1] + to[3] / 2.0) - (from[1] + from[3] / 2.0);
    let finite = [scale_x, scale_y, dx_mm, dy_mm].iter().all(|v| v.is_finite());
    (finite && scale_x > 0.0 && scale_y > 0.0 && !ids.is_empty()).then_some(Command::ShapesTransform {
        ids,
        dx_mm,
        dy_mm,
        scale_x,
        scale_y,
        degrees: 0.0,
    })
}

/// The engine's node addresses for the Direct Select anchors (contours and nodes are counted
/// the same way: a closed contour does not repeat its first node).
pub fn node_refs(selection: &PathSelection) -> Vec<NodeRef> {
    selection
        .anchors
        .iter()
        .map(|k| NodeRef {
            contour: k.subpath,
            node: k.knot,
        })
        .collect()
}

/// A number field that reports a committed value.
fn number(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    unit: &str,
    numbers: &LocaleNumbers,
) -> bool {
    let edit = NumberField::new(id, label, *value, numbers)
        .range(range)
        .decimals(0..=2)
        .unit(Unit::new(unit))
        .show(ui);
    match edit.event {
        Some(EditEvent::Commit { value: v, .. } | EditEvent::Set { value: v, .. }) => {
            *value = v;
            true
        }
        _ => false,
    }
}

fn labelled(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.add(Text::new(label).role(TypeRole::Label));
        add(ui);
    });
}

fn button(ui: &mut egui::Ui, label: &str, enabled: bool) -> bool {
    ui.add(Button::new(label).size(ControlSize::Sm).enabled(enabled))
        .clicked()
}

fn hint(ui: &mut egui::Ui, text: &str) {
    ui.add(
        Text::new(text)
            .role(TypeRole::Caption)
            .content(Content::Muted),
    );
}

impl StencilApp {
    /// Run `command`; when it adds shapes, select them (with the old selection when `keep`).
    fn run_adding(&mut self, command: Command, keep: bool, done: &str) {
        match self.engine.run(command) {
            Ok(Effect::ShapesAdded { ids } | Effect::Selection { ids }) => {
                let mut selection = if keep {
                    self.selected_shapes.clone()
                } else {
                    Vec::new()
                };
                selection.extend(ids);
                self.set_selection(selection);
                self.status = done.to_string();
            }
            Ok(_) => {
                self.retain_valid_selection();
                self.status = done.to_string();
            }
            Err(error) => self.status = error.to_string(),
        }
    }

    fn node_shape(&self) -> Option<u64> {
        self.path_selection
            .path
            .filter(|id| self.selected_shapes.contains(id))
            .or(self.selected_shape)
    }

    pub fn handle_edit_action(&mut self, action: EditAction) {
        let ids = self.selected_shapes.clone();
        let gap_mm = self.panels.gap_mm;
        match action {
            EditAction::Duplicate(side) => self.run_adding(
                Command::ShapesReplicateSide {
                    ids,
                    side: side.engine(),
                    gap_mm,
                },
                false,
                "duplicated",
            ),
            EditAction::Mirror(side) => self.run_adding(
                Command::ShapesMirrorCopy {
                    ids,
                    side: side.engine(),
                    gap_mm,
                },
                false,
                "mirrored copy added",
            ),
            EditAction::Array(axis) => self.run_adding(
                Command::ShapesReplicateArray {
                    ids,
                    axis,
                    count: self.panels.copies.clamp(2.0, 1000.0) as u32,
                    gap_mm,
                },
                true,
                "copies added",
            ),
            EditAction::FillPage => self.run_adding(
                Command::ShapesReplicateFill { ids, gap_mm },
                true,
                "page filled",
            ),
            EditAction::RotateCopies => self.run_adding(
                Command::ShapesRotateCopies {
                    ids,
                    count: self.panels.rotate_count.clamp(2.0, 1000.0) as u32,
                },
                true,
                "rotated copies added",
            ),
            EditAction::Lock => {
                self.run_command(Command::ShapesSetLocked { ids, locked: true });
                self.retain_valid_selection();
            }
            EditAction::UnlockAll | EditAction::ShowAll => {
                let lock = action == EditAction::UnlockAll;
                let flagged: Vec<u64> = self
                    .engine
                    .doc
                    .shapes()
                    .filter(|(_, s)| if lock { s.locked } else { s.hidden })
                    .map(|(_, s)| s.id)
                    .collect();
                if flagged.is_empty() {
                    self.status = "nothing to change".to_string();
                    return;
                }
                let command = if lock {
                    Command::ShapesSetLocked {
                        ids: flagged.clone(),
                        locked: false,
                    }
                } else {
                    Command::ShapesSetHidden {
                        ids: flagged.clone(),
                        hidden: false,
                    }
                };
                self.run_command(command);
                self.set_selection(flagged);
            }
            EditAction::Hide => {
                self.run_command(Command::ShapesSetHidden { ids, hidden: true });
                self.retain_valid_selection();
            }
            EditAction::ToPath => self.run_adding(Command::ShapesToPath { ids }, true, "converted"),
            EditAction::MakeCompound => self.run_adding(
                Command::ShapesMakeCompound { ids },
                false,
                "compound path made",
            ),
            EditAction::ReleaseCompound => self.run_adding(
                Command::ShapesReleaseCompound { ids },
                false,
                "compound path released",
            ),
            EditAction::Simplify => self.run_adding(
                Command::PathSimplify {
                    ids,
                    tolerance_mm: self.panels.simplify_mm,
                },
                true,
                "simplified",
            ),
            EditAction::OutlineStroke => self.run_adding(
                Command::PathOutlineStroke {
                    ids,
                    width_mm: self.panels.stroke_mm,
                    join: stencilcraft_engine::StrokeJoin::Round,
                    cap: stencilcraft_engine::StrokeCap::Round,
                    miter_limit: None,
                },
                true,
                "stroke outlined",
            ),
            EditAction::Join => {
                let (Some(&a), Some(&b)) = (ids.first(), ids.get(1).or(ids.first())) else {
                    self.status = "select two open paths to join".to_string();
                    return;
                };
                // One shape: join its first two contours.
                let b_contour = usize::from(a == b);
                self.run_adding(
                    Command::PathJoin {
                        a,
                        a_contour: 0,
                        b,
                        b_contour,
                        a_end: None,
                        b_end: None,
                    },
                    true,
                    "joined",
                );
            }
            EditAction::Close => {
                let Some(id) = self.node_shape() else { return };
                let contour = self.path_selection.anchors.first().map_or(0, |k| k.subpath);
                self.run_command(Command::PathClose { id, contour });
            }
            EditAction::Average(axis) => self.node_command(|id, nodes| Command::PathAverageNodes {
                id,
                nodes,
                axis,
            }),
            EditAction::MergeNodes => {
                self.node_command(|id, nodes| Command::PathMergeNodes { id, nodes });
                self.path_selection.anchors.clear();
            }
            EditAction::DeleteNodes => {
                self.node_command(|id, nodes| Command::PathDeleteNodes { id, nodes });
                self.path_selection.anchors.clear();
            }
            EditAction::NodeType(kind) => {
                self.node_command(|id, nodes| Command::PathSetNodeType { id, nodes, kind });
            }
            EditAction::Break => {
                let Some(first) = self.path_selection.anchors.first().copied() else {
                    self.status = "select a node with Direct Select (A) first".to_string();
                    return;
                };
                self.node_command(|id, _| Command::PathBreak {
                    id,
                    contour: first.subpath,
                    node: first.knot,
                });
                self.path_selection.anchors.clear();
            }
            EditAction::Segment(kind) => {
                let Some(id) = self.node_shape() else { return };
                let Some(at) = self
                    .path_selection
                    .segment
                    .or_else(|| self.path_selection.anchors.first().copied())
                else {
                    self.status = "select a segment or node first".to_string();
                    return;
                };
                self.run_command(Command::PathSetSegment {
                    id,
                    contour: at.subpath,
                    segment: at.knot,
                    kind,
                });
            }
            EditAction::SelectSameFill | EditAction::SelectSameStroke => {
                let Some(id) = self.selected_shape else { return };
                let command = if action == EditAction::SelectSameFill {
                    Command::SelectSameFill { id }
                } else {
                    Command::SelectSameStroke { id }
                };
                self.run_adding(command, false, "selected");
            }
            EditAction::SelectInverse => {
                self.run_adding(Command::SelectInverse { ids }, false, "selection inverted");
            }
            EditAction::Offset { internal } => {
                self.panels.offset.internal = internal;
                self.panels.offset.armed = true;
                self.handle_menu_action(crate::TopMenuAction::ShowPanel(
                    crate::WorkspacePanel::Offset,
                ));
            }
            EditAction::TogglePointEdit => {
                self.active_tool = if self.active_tool == crate::ActiveTool::DirectSelect {
                    crate::ActiveTool::Select
                } else {
                    crate::ActiveTool::DirectSelect
                };
            }
        }
    }

    /// Run a command on the Direct Select anchors of the edited shape.
    fn node_command(&mut self, make: impl FnOnce(u64, Vec<NodeRef>) -> Command) {
        let nodes = node_refs(&self.path_selection);
        let Some(id) = self.node_shape() else { return };
        if nodes.is_empty() {
            self.status = "select nodes with Direct Select (A) first".to_string();
            return;
        }
        self.run_command(make(id, nodes));
    }

    pub fn replicate_panel(&mut self, ui: &mut egui::Ui) {
        let any = !self.selected_shapes.is_empty();
        let numbers = self.locale_numbers.clone();
        let mut action = None;
        let state = &mut self.panels;
        let _ = Panel::new("stencilcraft-replicate-panel").show(ui, |ui| {
            labelled(ui, "Spacing", |ui| {
                number(ui, "rep-gap", "Spacing", &mut state.gap_mm, 0.0..=1000.0, "{n} mm", &numbers);
            });
            for (id, title, mirror) in [("rep-dup", "Duplicate", false), ("rep-mirror", "Mirror", true)] {
                let _ = SectionHeader::new(id, title).show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for side in ScreenSide::ALL {
                            let label = format!("{title} {}", side.label());
                            let b = Button::new(side.label())
                                .icon(if mirror { Icon::SymmetryVertical } else { side.icon() })
                                .size(ControlSize::Sm)
                                .enabled(any);
                            if ui.add(b).on_hover_text(label).clicked() {
                                action = Some(if mirror {
                                    EditAction::Mirror(side)
                                } else {
                                    EditAction::Duplicate(side)
                                });
                            }
                        }
                    });
                });
            }
            let _ = SectionHeader::new("rep-array", "Rows and columns").show(ui, |ui| {
                labelled(ui, "Copies", |ui| {
                    number(ui, "rep-copies", "Copies", &mut state.copies, 2.0..=1000.0, "{n}", &numbers);
                });
                ui.horizontal_wrapped(|ui| {
                    if button(ui, "Row", any) {
                        action = Some(EditAction::Array(DistributionAxis::Horizontal));
                    }
                    if button(ui, "Column", any) {
                        action = Some(EditAction::Array(DistributionAxis::Vertical));
                    }
                    if ui
                        .add(Button::new("Fill Page").icon(Icon::Grid).size(ControlSize::Sm).enabled(any))
                        .clicked()
                    {
                        action = Some(EditAction::FillPage);
                    }
                });
            });
            let _ = SectionHeader::new("rep-rotate", "Rotate copies").show(ui, |ui| {
                labelled(ui, "Count", |ui| {
                    number(ui, "rep-rot", "Rotate count", &mut state.rotate_count, 2.0..=1000.0, "{n}", &numbers);
                });
                ui.horizontal_wrapped(|ui| {
                    for n in [2.0, 4.0, 6.0, 8.0] {
                        if button(ui, &format!("{n:.0}"), any) {
                            state.rotate_count = n;
                            action = Some(EditAction::RotateCopies);
                        }
                    }
                    if ui
                        .add(Button::new("Rotate").icon(Icon::RotateCw).size(ControlSize::Sm).enabled(any))
                        .clicked()
                    {
                        action = Some(EditAction::RotateCopies);
                    }
                });
            });
            if !any {
                hint(ui, "Select shapes to replicate.");
            }
        });
        if let Some(action) = action {
            self.handle_edit_action(action);
        }
    }

    /// Rebuild the offset preview when the selection, its paths or the draft changed.
    pub fn refresh_offset_preview(&mut self) {
        let source: Vec<BezPath> = self
            .selected_shapes
            .iter()
            .filter_map(|id| self.engine.doc.get_shape(*id).map(|s| s.path.clone()))
            .collect();
        let draft = self.panels.offset.clone();
        if self
            .panels
            .offset_preview
            .as_ref()
            .is_some_and(|p| p.source == source && p.draft == draft)
        {
            return;
        }
        let distance = if draft.internal {
            -draft.distance_mm.abs()
        } else {
            draft.distance_mm.abs()
        };
        let options = stencilcraft_kerf::OffsetOptions {
            join: draft.join,
            miter_limit: draft.limit,
        };
        let result = source
            .iter()
            .map(|path| {
                stencilcraft_kerf::offset_path(path, distance, &options).map_err(|e| e.to_string())
            })
            .collect();
        self.panels.offset_preview = Some(OffsetPreview {
            source,
            draft,
            result,
        });
    }

    pub fn offset_panel(&mut self, ui: &mut egui::Ui) {
        let any = !self.selected_shapes.is_empty();
        let numbers = self.locale_numbers.clone();
        let before = self.panels.offset.clone();
        let mut apply = false;
        let mut cancel = false;
        let error = self
            .panels
            .offset_preview
            .as_ref()
            .and_then(|p| p.result.as_ref().err().cloned());
        let draft = &mut self.panels.offset;
        let _ = Panel::new("stencilcraft-offset-panel").show(ui, |ui| {
            let _ = Segmented::new("offset-kind", "Offset direction", &mut draft.internal)
                .item(false, "Offset")
                .item(true, "Internal Offset")
                .size(ControlSize::Sm)
                .show(ui);
            labelled(ui, "Distance", |ui| {
                number(ui, "offset-distance", "Offset distance", &mut draft.distance_mm, 0.01..=1000.0, "{n} mm", &numbers);
            });
            labelled(ui, "Corner", |ui| {
                let _ = Segmented::new("offset-join", "Corner", &mut draft.join)
                    .icon_item(JoinStyle::Miter, Icon::StrokeJoinMiter, "Sharp")
                    .icon_item(JoinStyle::Round, Icon::StrokeJoinRound, "Round")
                    .icon_item(JoinStyle::Bevel, Icon::StrokeJoinBevel, "Bevel")
                    .size(ControlSize::Sm)
                    .show(ui);
            });
            if draft.join == JoinStyle::Miter {
                labelled(ui, "Corner limit", |ui| {
                    number(ui, "offset-limit", "Corner limit", &mut draft.limit, 1.0..=100.0, "{n} ×", &numbers);
                });
            }
            if let Some(error) = &error {
                ui.add(Text::new(error).content(Content::Muted));
            }
            ui.horizontal(|ui| {
                cancel = ui
                    .add(Button::new("Cancel").size(ControlSize::Sm).enabled(draft.armed))
                    .clicked();
                apply = ui
                    .add(
                        Button::new("Apply")
                            .variant(ButtonVariant::Primary)
                            .size(ControlSize::Sm)
                            .enabled(any && error.is_none()),
                    )
                    .clicked();
            });
            hint(
                ui,
                if any {
                    "The blue outline previews the offset; Apply adds it."
                } else {
                    "Select shapes to offset."
                },
            );
        });
        if self.panels.offset != before {
            self.panels.offset.armed = true;
        }
        if cancel {
            self.panels.offset.armed = false;
        }
        if apply {
            let command = self.panels.offset.command(self.selected_shapes.clone());
            self.run_adding(command, false, "offset added");
            self.panels.offset.armed = false;
        }
        self.panels.offset_preview_shown = self.panels.offset.armed && any;
        if self.panels.offset_preview_shown {
            self.refresh_offset_preview();
        }
    }

    pub fn modify_panel(&mut self, ui: &mut egui::Ui) {
        let count = self.selected_shapes.len();
        let numbers = self.locale_numbers.clone();
        let mut op = None;
        let mut action = None;
        let stroke = &mut self.panels.stroke_mm;
        let _ = Panel::new("stencilcraft-modify-panel").show(ui, |ui| {
            let _ = SectionHeader::new("modify-weld", "Combine").show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (label, o) in [
                        ("Weld", BooleanOperation::Union),
                        ("Subtract", BooleanOperation::Difference),
                        ("Subtract All", BooleanOperation::SubtractAll),
                        ("Intersect", BooleanOperation::Intersect),
                        ("Crop", BooleanOperation::Crop),
                        ("Divide", BooleanOperation::Divide),
                        ("Exclude", BooleanOperation::Xor),
                    ] {
                        if button(ui, label, count >= 2) {
                            op = Some(o);
                        }
                    }
                });
            });
            let _ = SectionHeader::new("modify-pathfinder", "Pathfinder").show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (label, o) in [
                        ("Trim", BooleanOperation::Trim),
                        ("Merge", BooleanOperation::Merge),
                        ("Outline", BooleanOperation::Outline),
                        ("Minus Back", BooleanOperation::MinusBack),
                    ] {
                        if button(ui, label, count >= 2) {
                            op = Some(o);
                        }
                    }
                });
            });
            let _ = SectionHeader::new("modify-compound", "Compound paths").show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (label, a, enabled) in [
                        ("Make Compound", EditAction::MakeCompound, count >= 2),
                        ("Release Compound", EditAction::ReleaseCompound, count >= 1),
                        ("Convert to Path", EditAction::ToPath, count >= 1),
                    ] {
                        if button(ui, label, enabled) {
                            action = Some(a);
                        }
                    }
                });
            });
            let _ = SectionHeader::new("modify-stroke", "Outline stroke").show(ui, |ui| {
                labelled(ui, "Width", |ui| {
                    number(ui, "modify-stroke-w", "Stroke width", stroke, 0.01..=1000.0, "{n} mm", &numbers);
                });
                if button(ui, "Outline Stroke", count >= 1) {
                    action = Some(EditAction::OutlineStroke);
                }
            });
            if count < 2 {
                hint(ui, "Select two or more shapes to weld, subtract or intersect.");
            }
        });
        if let Some(op) = op {
            self.run_pathfinder(op);
        }
        if let Some(a) = action {
            self.handle_edit_action(a);
        }
    }

    pub fn points_panel(&mut self, ui: &mut egui::Ui) {
        let editing = self.active_tool == crate::ActiveTool::DirectSelect;
        let ids = self.selected_shapes.clone();
        let nodes = !self.path_selection.anchors.is_empty();
        let segment = self.path_selection.segment.is_some() || nodes;
        let counts = (!ids.is_empty())
            .then(|| self.engine.run(Command::QueryNodeCount { ids: ids.clone() }).ok())
            .flatten();
        let numbers = self.locale_numbers.clone();
        let mut action = None;
        let tolerance = &mut self.panels.simplify_mm;
        let _ = Panel::new("stencilcraft-points-panel").show(ui, |ui| {
            let label = if editing { "Stop Editing Points" } else { "Edit Points" };
            if ui
                .add(Button::new(label).icon(Icon::DirectSelect).size(ControlSize::Sm).enabled(!ids.is_empty() || editing))
                .clicked()
            {
                action = Some(EditAction::TogglePointEdit);
            }
            match counts {
                Some(Effect::NodeCount(c)) => {
                    ui.add(Text::new(format!(
                        "{} nodes · {} contours ({} corner, {} curve)",
                        c.nodes, c.contours, c.linear, c.bezier
                    )));
                }
                _ => hint(ui, "Double-click a shape to edit its points."),
            }
            let _ = SectionHeader::new("points-nodes", "Nodes").show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (label, a, enabled) in [
                        ("Delete", EditAction::DeleteNodes, nodes),
                        ("Break", EditAction::Break, nodes),
                        ("Corner", EditAction::NodeType(NodeType::Corner), nodes),
                        ("Smooth", EditAction::NodeType(NodeType::Smooth), nodes),
                        ("Merge", EditAction::MergeNodes, nodes),
                        ("Average", EditAction::Average(NodeAxis::Both), nodes),
                    ] {
                        if button(ui, label, enabled) {
                            action = Some(a);
                        }
                    }
                });
            });
            let _ = SectionHeader::new("points-segments", "Segments").show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (label, a, enabled) in [
                        ("Make Flat", EditAction::Segment(SegmentType::Line), segment),
                        ("Make Curve", EditAction::Segment(SegmentType::Curve), segment),
                        ("Close Path", EditAction::Close, !ids.is_empty()),
                        ("Join", EditAction::Join, !ids.is_empty()),
                    ] {
                        if button(ui, label, enabled) {
                            action = Some(a);
                        }
                    }
                });
            });
            let _ = SectionHeader::new("points-path", "Path").show(ui, |ui| {
                labelled(ui, "Tolerance", |ui| {
                    number(ui, "points-tol", "Simplify tolerance", tolerance, 0.001..=1000.0, "{n} mm", &numbers);
                });
                ui.horizontal_wrapped(|ui| {
                    if button(ui, "Simplify", !ids.is_empty()) {
                        action = Some(EditAction::Simplify);
                    }
                    if button(ui, "Convert to Path", !ids.is_empty()) {
                        action = Some(EditAction::ToPath);
                    }
                });
            });
        });
        if let Some(a) = action {
            self.handle_edit_action(a);
        }
    }

    pub fn transform_panel(&mut self, ui: &mut egui::Ui) {
        let ids = self.selected_shapes.clone();
        let any = !ids.is_empty();
        let many = ids.len() >= 2;
        let numbers = self.locale_numbers.clone();
        let mut command = None;
        let mut center = false;
        let state = &mut self.panels;
        let _ = Panel::new("stencilcraft-transform-panel").show(ui, |ui| {
            let _ = SectionHeader::new("transform-align", "Align").show(ui, |ui| {
                let picked = AlignBar::new("transform-align-bar")
                    .enabled(|a| many && align_command(a).is_some())
                    .show(ui)
                    .picked;
                command = match picked.and_then(align_command) {
                    Some(AlignCommand::Align(alignment)) => Some(Command::ShapesAlign {
                        ids: ids.clone(),
                        alignment,
                        to_board: false,
                    }),
                    Some(AlignCommand::Distribute(axis)) => Some(Command::ShapesDistribute {
                        ids: ids.clone(),
                        axis,
                    }),
                    None => None,
                };
                center = button(ui, "Center to Page", any);
            });
            let transform = |dx: f64, dy: f64, sx: f64, sy: f64, deg: f64| Command::ShapesTransform {
                ids: ids.clone(),
                dx_mm: dx,
                dy_mm: dy,
                scale_x: sx,
                scale_y: sy,
                degrees: deg,
            };
            let _ = SectionHeader::new("transform-rotate", "Rotate").show(ui, |ui| {
                labelled(ui, "Angle", |ui| {
                    number(ui, "transform-deg", "Rotation angle", &mut state.rotate_deg, -360.0..=360.0, "{n}°", &numbers);
                });
                ui.horizontal_wrapped(|ui| {
                    for deg in [-90.0, -45.0, 45.0, 90.0, 180.0] {
                        if button(ui, &format!("{deg:+.0}°"), any) {
                            command = Some(transform(0.0, 0.0, 1.0, 1.0, deg));
                        }
                    }
                    if button(ui, "Rotate", any) {
                        command = Some(transform(0.0, 0.0, 1.0, 1.0, state.rotate_deg));
                    }
                });
            });
            let _ = SectionHeader::new("transform-scale", "Scale").show(ui, |ui| {
                ui.horizontal(|ui| {
                    let [mut w, mut h] = state.scale_pct;
                    let wc = number(ui, "transform-sw", "Scale width", &mut w, 1.0..=10_000.0, "{n} %", &numbers);
                    let _ = craft_ui::controls::LinkButton::new(
                        "transform-scale-link",
                        &mut state.scale_linked,
                        craft_ui::controls::LinkMode::Ratio,
                    )
                    .size(ControlSize::Sm)
                    .show(ui);
                    let hc = number(ui, "transform-sh", "Scale height", &mut h, 1.0..=10_000.0, "{n} %", &numbers);
                    if state.scale_linked && wc {
                        h = w;
                    } else if state.scale_linked && hc {
                        w = h;
                    }
                    state.scale_pct = [w, h];
                });
                ui.horizontal_wrapped(|ui| {
                    for pct in [50.0, 200.0] {
                        if button(ui, &format!("{pct:.0}%"), any) {
                            command = Some(transform(0.0, 0.0, pct / 100.0, pct / 100.0, 0.0));
                        }
                    }
                    if button(ui, "Scale", any) {
                        let [w, h] = state.scale_pct;
                        command = Some(transform(0.0, 0.0, w / 100.0, h / 100.0, 0.0));
                    }
                });
            });
            let _ = SectionHeader::new("transform-move", "Move by").show(ui, |ui| {
                ui.horizontal(|ui| {
                    let [mut dx, mut dy] = state.move_mm;
                    ui.add(Text::new("X").role(TypeRole::Label));
                    number(ui, "transform-dx", "Move horizontally", &mut dx, -10_000.0..=10_000.0, "{n} mm", &numbers);
                    ui.add(Text::new("Y").role(TypeRole::Label));
                    number(ui, "transform-dy", "Move vertically", &mut dy, -10_000.0..=10_000.0, "{n} mm", &numbers);
                    state.move_mm = [dx, dy];
                });
                if button(ui, "Move", any) {
                    let [dx, dy] = state.move_mm;
                    command = Some(transform(dx, dy, 1.0, 1.0, 0.0));
                }
            });
            if !any {
                hint(ui, "Select shapes to transform.");
            }
        });
        if let Some(command) = command {
            self.run_command(command);
        }
        if center {
            self.handle_menu_action(crate::TopMenuAction::CenterToPage);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_edits_resize_about_the_reference_point() {
        let b = [10.0, 20.0, 40.0, 20.0];
        // Top left stays put; linked height follows.
        assert_eq!(edit_bounds(b, Anchor9::TopLeft, 2, 80.0, true), [10.0, 20.0, 80.0, 40.0]);
        // Centre stays put; unlinked height stays.
        assert_eq!(edit_bounds(b, Anchor9::Center, 2, 80.0, false), [-10.0, 20.0, 80.0, 20.0]);
        // Bottom right fixed, height edit.
        assert_eq!(edit_bounds(b, Anchor9::BottomRight, 3, 10.0, false), [10.0, 30.0, 40.0, 10.0]);
        // X and Y place the reference point.
        assert_eq!(edit_bounds(b, Anchor9::Center, 0, 0.0, true), [-20.0, 20.0, 40.0, 20.0]);
        assert_eq!(edit_bounds(b, Anchor9::Bottom, 1, 100.0, true), [10.0, 80.0, 40.0, 20.0]);
        // A line has no height to keep in proportion.
        let line = [0.0, 0.0, 10.0, 0.0];
        assert_eq!(edit_bounds(line, Anchor9::TopLeft, 3, 5.0, true), [0.0, 0.0, 10.0, 5.0]);
    }

    #[test]
    fn bounds_command_scales_then_moves_the_centre() {
        let from = [0.0, 0.0, 10.0, 10.0];
        let Some(Command::ShapesTransform {
            dx_mm,
            dy_mm,
            scale_x,
            scale_y,
            ..
        }) = bounds_command(vec![1], from, [0.0, 0.0, 20.0, 10.0])
        else {
            panic!("expected a transform");
        };
        assert_eq!((dx_mm, dy_mm, scale_x, scale_y), (5.0, 0.0, 2.0, 1.0));
        assert!(bounds_command(vec![1], from, [0.0, 0.0, 0.0, 10.0]).is_none());
        assert!(bounds_command(vec![], from, from).is_none());
    }

    #[test]
    fn screen_sides_flip_vertically_for_the_y_up_engine() {
        assert_eq!(ScreenSide::Above.engine(), Side::Below);
        assert_eq!(ScreenSide::Below.engine(), Side::Above);
        assert_eq!(ScreenSide::Left.engine(), Side::Left);
    }

    #[test]
    fn offset_draft_signs_internal_offsets() {
        let mut draft = OffsetDraft {
            distance_mm: 2.0,
            internal: true,
            ..OffsetDraft::default()
        };
        assert!(matches!(
            draft.command(vec![1]),
            Command::ShapesOffset { distance_mm, .. } if distance_mm == -2.0
        ));
        draft.internal = false;
        assert!(matches!(
            draft.command(vec![1]),
            Command::ShapesOffset { distance_mm, .. } if distance_mm == 2.0
        ));
    }
}
