//! The Effect menu's generators as one Effects panel: Rhinestone, Sketch Fill, Stipple,
//! Emboss/Deboss patterns, Warp (Arc, Flag, Bulge, Perspective…), Puzzle, Pop-up Card, Weed
//! Lines and Border and Remove Background. Each effect is an engine command; the panel edits
//! its parameters, previews the result live (the command run on a copy of the document) and
//! Apply runs it for real as one undo step. Effect › Apply Last Effect repeats the last one on
//! the current selection.

use craft_theme::{ControlSize, Variant};
use craft_ui::buttons::Button;
use craft_ui::choice::Checkbox;
use craft_ui::contract::{Bind, Entry, Props as _, Show as _, ShowContainer as _};
use craft_ui::pickers::Select;
use craft_ui::sections::Panel;
use eframe::egui;
use serde_json::{Map, Value, json};
use stencilcraft_engine::{Command, Engine};
use stencilcraft_geom::BezPath;

use crate::panels::{hint, labelled, number};
use crate::{StencilApp, WorkspacePanel};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EffectKind {
    Rhinestone,
    SketchFill,
    Stipple,
    EmbossPattern,
    Warp,
    Puzzle,
    PopupCard,
    WeedLines,
    RemoveBackground,
    Bridges,
    ImageEffects,
    Lattice,
}

/// An Effect or Panels menu item that opens the Effects panel on an effect, maybe preset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectMenu {
    Rhinestone,
    SketchFill,
    Stipple,
    EmbossPattern,
    WarpArc,
    WarpFlag,
    WarpBulge,
    WarpPerspective,
    PopupCard,
    Puzzle,
    WeedLines,
    RemoveBackground,
    Bridges,
    ImageEffects,
    Lattice,
}

impl EffectMenu {
    fn open(self) -> (EffectKind, Vec<(&'static str, Value)>) {
        let warp = |style: &str| (EffectKind::Warp, vec![("style", json!(style))]);
        match self {
            Self::Rhinestone => (EffectKind::Rhinestone, Vec::new()),
            Self::SketchFill => (EffectKind::SketchFill, Vec::new()),
            Self::Stipple => (EffectKind::Stipple, Vec::new()),
            Self::EmbossPattern => (EffectKind::EmbossPattern, Vec::new()),
            Self::WarpArc => warp("arc"),
            Self::WarpFlag => warp("flag"),
            Self::WarpBulge => warp("bulge"),
            Self::WarpPerspective => (
                EffectKind::Warp,
                vec![("style", json!("none")), ("vertical", json!(0.3))],
            ),
            Self::PopupCard => (EffectKind::PopupCard, Vec::new()),
            Self::Puzzle => (EffectKind::Puzzle, Vec::new()),
            Self::WeedLines => (EffectKind::WeedLines, Vec::new()),
            Self::RemoveBackground => (EffectKind::RemoveBackground, Vec::new()),
            Self::Bridges => (EffectKind::Bridges, Vec::new()),
            Self::ImageEffects => (EffectKind::ImageEffects, Vec::new()),
            Self::Lattice => (EffectKind::Lattice, Vec::new()),
        }
    }
}

/// What an effect works on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    /// Every selected shape (`ids`).
    Shapes,
    /// The one selected shape (`id`).
    Shape,
    /// The picked placed image (`id`).
    Image,
    /// Nothing: the effect makes new shapes on the page.
    Page,
}

#[derive(Clone, Copy, Debug)]
enum Kind {
    Num {
        min: f64,
        max: f64,
        unit: &'static str,
    },
    Int {
        min: f64,
        max: f64,
    },
    Bool,
    Choice(&'static [(&'static str, &'static str)]),
}

#[derive(Clone, Copy, Debug)]
struct Field {
    /// A parameter key; dots reach into nested objects (`tool.deboss`).
    key: &'static str,
    label: &'static str,
    kind: Kind,
}

const fn num(
    key: &'static str,
    label: &'static str,
    min: f64,
    max: f64,
    unit: &'static str,
) -> Field {
    Field {
        key,
        label,
        kind: Kind::Num { min, max, unit },
    }
}

const fn int(key: &'static str, label: &'static str, min: f64, max: f64) -> Field {
    Field {
        key,
        label,
        kind: Kind::Int { min, max },
    }
}

const fn choice(
    key: &'static str,
    label: &'static str,
    options: &'static [(&'static str, &'static str)],
) -> Field {
    Field {
        key,
        label,
        kind: Kind::Choice(options),
    }
}

const STONE_SIZES: &[(&str, &str)] = &[
    ("ss6", "SS6"),
    ("ss8", "SS8"),
    ("ss10", "SS10"),
    ("ss12", "SS12"),
    ("ss16", "SS16"),
    ("ss20", "SS20"),
    ("ss30", "SS30"),
];
const STONE_LAYOUTS: &[(&str, &str)] = &[
    ("outline", "Outline"),
    ("fill", "Fill"),
    ("outline-fill", "Outline and fill"),
];
const STONE_PATTERNS: &[(&str, &str)] = &[("hex", "Offset rows"), ("grid", "Grid")];
const SKETCH_PATTERNS: &[(&str, &str)] = &[
    ("lines", "Lines"),
    ("cross-hatch", "Cross-hatch"),
    ("grid", "Grid"),
    ("concentric", "Concentric"),
    ("spiral", "Spiral"),
];
const STIPPLE_PATTERNS: &[(&str, &str)] = &[
    ("stipple-random", "Random dots"),
    ("stipple-grid", "Dot grid"),
];
const WARP_STYLES: &[(&str, &str)] = &[
    ("arc", "Arc"),
    ("arc-lower", "Arc lower"),
    ("arc-upper", "Arc upper"),
    ("arch", "Arch"),
    ("bulge", "Bulge"),
    ("shell-lower", "Shell lower"),
    ("shell-upper", "Shell upper"),
    ("flag", "Flag"),
    ("wave", "Wave"),
    ("fish", "Fish"),
    ("rise", "Rise"),
    ("fisheye", "Fisheye"),
    ("inflate", "Inflate"),
    ("squeeze", "Squeeze"),
    ("twist", "Twist"),
    ("none", "Perspective (distort only)"),
];
const LATTICE_HOLES: &[(&str, &str)] = &[
    ("square", "Squares"),
    ("circle", "Circles"),
    ("diamond", "Diamonds"),
    ("hexagon", "Hexagons"),
];
const BRIDGE_DIRECTIONS: &[(&str, &str)] = &[
    ("vertical", "Above and below"),
    ("horizontal", "Left and right"),
    ("both", "All four sides"),
];
const WEED_DIRECTIONS: &[(&str, &str)] = &[
    ("vertical", "Vertical"),
    ("horizontal", "Horizontal"),
    ("both", "Both"),
];

struct EffectDef {
    kind: EffectKind,
    title: &'static str,
    command: &'static str,
    target: Target,
    fields: &'static [Field],
}

const EFFECTS: &[EffectDef] = &[
    EffectDef {
        kind: EffectKind::Lattice,
        title: "Lattice",
        command: "generate.lattice",
        target: Target::Shapes,
        fields: &[
            choice("hole", "Holes", LATTICE_HOLES),
            num("hole_mm", "Hole size", 0.2, 500.0, "mm"),
            num("wall_mm", "Wall", 0.1, 500.0, "mm"),
            num("border_mm", "Border", 0.0, 500.0, "mm"),
        ],
    },
    EffectDef {
        kind: EffectKind::ImageEffects,
        title: "Image Effects",
        command: "image.adjust",
        target: Target::Image,
        fields: &[
            num("brightness", "Brightness", -100.0, 100.0, "%"),
            num("contrast", "Contrast", -100.0, 100.0, "%"),
            num("saturation", "Saturation", -100.0, 100.0, "%"),
            Field {
                key: "grayscale",
                label: "Greyscale",
                kind: Kind::Bool,
            },
            Field {
                key: "invert",
                label: "Invert",
                kind: Kind::Bool,
            },
        ],
    },
    EffectDef {
        kind: EffectKind::Bridges,
        title: "Bridges",
        command: "shapes.bridge",
        target: Target::Shapes,
        fields: &[
            num("width_mm", "Bridge width", 0.1, 100.0, "mm"),
            choice("direction", "Bridges", BRIDGE_DIRECTIONS),
        ],
    },
    EffectDef {
        kind: EffectKind::Rhinestone,
        title: "Rhinestone",
        command: "generate.rhinestone",
        target: Target::Shapes,
        fields: &[
            choice("size", "Stone size", STONE_SIZES),
            num("spacing_mm", "Spacing", 0.0, 50.0, "mm"),
            choice("layout", "Layout", STONE_LAYOUTS),
            int("lines", "Outline lines", 1.0, 2.0),
            choice("pattern", "Fill pattern", STONE_PATTERNS),
        ],
    },
    EffectDef {
        kind: EffectKind::SketchFill,
        title: "Sketch Fill",
        command: "generate.fill",
        target: Target::Shapes,
        fields: &[
            choice("pattern", "Pattern", SKETCH_PATTERNS),
            num("spacing_mm", "Spacing", 0.1, 100.0, "mm"),
            num("angle_deg", "Angle", -180.0, 180.0, "°"),
            num("inset_mm", "Inset", 0.0, 100.0, "mm"),
        ],
    },
    EffectDef {
        kind: EffectKind::Stipple,
        title: "Stipple",
        command: "generate.fill",
        target: Target::Shapes,
        fields: &[
            choice("pattern", "Pattern", STIPPLE_PATTERNS),
            num("spacing_mm", "Spacing", 0.1, 100.0, "mm"),
            num("dot_mm", "Dot size", 0.05, 20.0, "mm"),
            num("angle_deg", "Angle", -180.0, 180.0, "°"),
            int("seed", "Variation", 0.0, 1_000_000.0),
        ],
    },
    EffectDef {
        kind: EffectKind::EmbossPattern,
        title: "Emboss / Deboss Patterns",
        command: "generate.tool-lines",
        target: Target::Shapes,
        fields: &[
            Field {
                key: "tool.deboss",
                label: "Deboss",
                kind: Kind::Bool,
            },
            choice("fill.pattern", "Pattern", SKETCH_PATTERNS),
            num("fill.spacing_mm", "Spacing", 0.1, 100.0, "mm"),
            num("fill.angle_deg", "Angle", -180.0, 180.0, "°"),
        ],
    },
    EffectDef {
        kind: EffectKind::Warp,
        title: "Warp",
        command: "generate.warp",
        target: Target::Shapes,
        fields: &[
            choice("style", "Style", WARP_STYLES),
            num("bend", "Bend", -1.0, 1.0, ""),
            num("horizontal", "Horizontal distortion", -1.0, 1.0, ""),
            num("vertical", "Vertical distortion", -1.0, 1.0, ""),
        ],
    },
    EffectDef {
        kind: EffectKind::Puzzle,
        title: "Puzzle",
        command: "generate.puzzle",
        target: Target::Shape,
        fields: &[
            int("rows", "Rows", 1.0, 50.0),
            int("columns", "Columns", 1.0, 50.0),
            num("tab_size", "Knob size", 0.0, 0.35, ""),
            int("seed", "Variation", 0.0, 1_000_000.0),
        ],
    },
    EffectDef {
        kind: EffectKind::PopupCard,
        title: "Pop-up Card",
        command: "generate.popup",
        target: Target::Page,
        fields: &[
            num("width", "Card width", 20.0, 2000.0, "mm"),
            num("height", "Card height", 20.0, 2000.0, "mm"),
            int("struts", "Struts", 1.0, 50.0),
            num("strut_width_mm", "Strut width", 1.0, 500.0, "mm"),
            num("base_width_pct", "Strut depth", 5.0, 95.0, "%"),
            num("dash_pitch_mm", "Score dashes", 0.0, 50.0, "mm"),
            Field {
                key: "outline",
                label: "Cut the card outline",
                kind: Kind::Bool,
            },
        ],
    },
    EffectDef {
        kind: EffectKind::WeedLines,
        title: "Weed Lines and Border",
        command: "generate.weed",
        target: Target::Shapes,
        fields: &[
            num("margin_mm", "Border margin", 0.0, 1000.0, "mm"),
            num("spacing_mm", "Weed line spacing", 0.0, 1000.0, "mm"),
            choice("direction", "Direction", WEED_DIRECTIONS),
            num("clearance_mm", "Clearance", 0.0, 100.0, "mm"),
        ],
    },
    EffectDef {
        kind: EffectKind::RemoveBackground,
        title: "Remove Background",
        command: "image.remove-background",
        target: Target::Image,
        fields: &[num("tolerance", "Tolerance", 0.0, 100.0, "%")],
    },
];

fn def(kind: EffectKind) -> Option<&'static EffectDef> {
    EFFECTS.iter().find(|d| d.kind == kind)
}

/// Each effect's starting parameters: the generator's own defaults.
fn defaults(kind: EffectKind) -> Map<String, Value> {
    let value = match kind {
        EffectKind::Rhinestone => {
            serde_json::to_value(stencilcraft_generate::rhinestone::RhinestoneParams::default())
        }
        EffectKind::SketchFill => {
            serde_json::to_value(stencilcraft_generate::fill::FillParams::default())
        }
        EffectKind::Stipple => {
            serde_json::to_value(stencilcraft_generate::fill::FillParams::default()).map(|mut v| {
                v["pattern"] = json!("stipple-random");
                v
            })
        }
        EffectKind::EmbossPattern => {
            serde_json::to_value(stencilcraft_generate::fill::FillParams::default())
                .map(|fill| json!({"tool": {"op": "emboss", "deboss": false}, "fill": fill}))
        }
        EffectKind::Warp => {
            serde_json::to_value(stencilcraft_generate::warp::WarpParams::default())
        }
        EffectKind::Puzzle => {
            serde_json::to_value(stencilcraft_generate::puzzle::PuzzleParams::default())
        }
        EffectKind::PopupCard => {
            serde_json::to_value(stencilcraft_generate::popup::PopupParams::default())
        }
        EffectKind::WeedLines => {
            serde_json::to_value(stencilcraft_generate::weed::WeedParams::default())
        }
        EffectKind::RemoveBackground => Ok(json!({"tolerance": 10.0})),
        EffectKind::Bridges => Ok(json!({"width_mm": 1.5, "direction": "vertical"})),
        EffectKind::Lattice => {
            Ok(json!({"hole": "square", "hole_mm": 5.0, "wall_mm": 1.5, "border_mm": 3.0}))
        }
        EffectKind::ImageEffects => serde_json::to_value(stencilcraft_raster::Adjust::default()),
    };
    match value {
        Ok(Value::Object(map)) => map,
        _ => Map::new(),
    }
}

fn get<'a>(params: &'a Map<String, Value>, key: &str) -> Option<&'a Value> {
    let mut parts = key.split('.');
    let first = parts.next()?;
    parts.try_fold(params.get(first)?, |value, part| value.get(part))
}

fn set(params: &mut Map<String, Value>, key: &str, new: Value) {
    let mut parts: Vec<&str> = key.split('.').collect();
    let Some(last) = parts.pop() else {
        return;
    };
    let mut map = params;
    for part in parts {
        let entry = map
            .entry(part.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if !entry.is_object() {
            *entry = Value::Object(Map::new());
        }
        let Some(next) = entry.as_object_mut() else {
            return;
        };
        map = next;
    }
    map.insert(last.to_string(), new);
}

/// The Effects panel's state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EffectsState {
    pub kind: Option<EffectKind>,
    params: std::collections::HashMap<EffectKind, Map<String, Value>>,
    /// The last effect applied, for Apply Last Effect.
    pub last: Option<(EffectKind, Map<String, Value>)>,
    /// The canvas paints the preview only in a frame the panel was drawn in.
    pub shown: bool,
}

impl EffectsState {
    fn params(&mut self, kind: EffectKind) -> &mut Map<String, Value> {
        self.params.entry(kind).or_insert_with(|| defaults(kind))
    }

    /// Choose `kind`, optionally setting some of its parameters (a menu preset).
    pub fn choose(&mut self, kind: EffectKind, preset: &[(&str, Value)]) {
        self.kind = Some(kind);
        let params = self.params(kind);
        for (key, value) in preset {
            set(params, key, value.clone());
        }
    }
}

/// The live preview and what it was made from.
pub struct EffectPreview {
    command: Value,
    doc: stencilcraft_doc::Document,
    pub result: Result<Vec<BezPath>, String>,
}

impl StencilApp {
    /// The command for `kind` with `params` on the current selection, as JSON.
    fn effect_command(
        &self,
        kind: EffectKind,
        params: &Map<String, Value>,
    ) -> Result<Value, String> {
        let d = def(kind).ok_or("unknown effect")?;
        let mut params = params.clone();
        match d.target {
            Target::Shapes => {
                if self.selected_shapes.is_empty() {
                    return Err("Select shapes first.".into());
                }
                params.insert("ids".into(), json!(self.selected_shapes));
            }
            Target::Shape => {
                let [id] = self.selected_shapes.as_slice() else {
                    return Err("Select one shape.".into());
                };
                params.insert("id".into(), json!(id));
            }
            Target::Image => {
                let id = self.selected_image.ok_or("Select a placed image.")?;
                params.insert("id".into(), json!(id));
            }
            Target::Page => {
                if d.kind == EffectKind::PopupCard {
                    // Centre the card on the page.
                    let (w, h) = (
                        params.get("width").and_then(Value::as_f64).unwrap_or(200.0),
                        params
                            .get("height")
                            .and_then(Value::as_f64)
                            .unwrap_or(140.0),
                    );
                    let board = &self.engine.doc.board;
                    params.insert("x".into(), json!((board.width_mm - w) / 2.0));
                    params.insert("y".into(), json!((board.height_mm - h) / 2.0));
                }
            }
        }
        Ok(json!({"command": d.command, "params": params}))
    }

    fn effect_engine_command(command: &Value) -> Result<Command, String> {
        serde_json::from_value(command.clone()).map_err(|e| format!("bad settings: {e}"))
    }

    /// Rebuild the preview when the command or the document changed.
    fn refresh_effect_preview(&mut self, kind: EffectKind) {
        let params = self.effects.params(kind).clone();
        let command = match self.effect_command(kind, &params) {
            Ok(command) => command,
            Err(e) => {
                self.effect_preview = Some(EffectPreview {
                    command: Value::Null,
                    doc: self.engine.doc.clone(),
                    result: Err(e),
                });
                return;
            }
        };
        if self
            .effect_preview
            .as_ref()
            .is_some_and(|p| p.command == command && p.doc == self.engine.doc)
        {
            return;
        }
        let doc = self.engine.doc.clone();
        let result = Self::effect_engine_command(&command).and_then(|cmd| {
            let mut scratch = Engine::new(doc.clone());
            scratch.run(cmd).map_err(|e| e.to_string())?;
            // Shapes that are new or changed are what the effect makes.
            Ok(scratch
                .doc
                .shapes()
                .filter(|(_, s)| doc.get_shape(s.id).is_none_or(|old| old.path != s.path))
                .map(|(_, s)| s.path.clone())
                .collect())
        });
        self.effect_preview = Some(EffectPreview {
            command,
            doc,
            result,
        });
    }

    /// Run `kind` with its panel parameters on the selection (one undo step).
    pub fn apply_effect(&mut self, kind: EffectKind) {
        let params = self.effects.params(kind).clone();
        let command = self
            .effect_command(kind, &params)
            .and_then(|c| Self::effect_engine_command(&c));
        match command {
            Ok(command) => {
                let title = def(kind).map_or("effect", |d| d.title);
                match self.engine.run(command) {
                    Ok(stencilcraft_engine::Effect::ShapesAdded { ids }) => {
                        self.set_selection(ids);
                        self.status = format!("{title} applied");
                    }
                    Ok(_) => self.status = format!("{title} applied"),
                    Err(e) => {
                        self.status = e.to_string();
                        return;
                    }
                }
                self.effects.last = Some((kind, params));
                self.effect_preview = None;
            }
            Err(e) => self.status = e,
        }
    }

    /// Effect › Apply Last Effect.
    pub fn apply_last_effect(&mut self) {
        match self.effects.last.clone() {
            Some((kind, params)) => {
                *self.effects.params(kind) = params;
                self.apply_effect(kind);
            }
            None => self.status = "no effect applied yet".to_string(),
        }
    }

    /// A menu item: show the Effects panel on its effect and preset.
    pub fn open_effect(&mut self, menu: EffectMenu) {
        let (kind, preset) = menu.open();
        self.effects.choose(kind, &preset);
        self.pending_panel = Some(WorkspacePanel::Effects);
    }

    /// Paint the effect preview, only while the Effects panel is on screen.
    pub fn paint_effect_preview(&mut self, painter: &egui::Painter) {
        if !std::mem::take(&mut self.effects.shown) {
            return;
        }
        if let Some(Ok(paths)) = self.effect_preview.as_ref().map(|p| &p.result) {
            let colour = stencilcraft_ui_egui::Palette::for_theme(self.theme).kerf_line;
            stencilcraft_ui_egui::paint_preview_paths(painter, &self.view, paths, colour);
        }
    }

    pub fn effects_panel(&mut self, ui: &mut egui::Ui) {
        self.effects.shown = true;
        let numbers = self.locale_numbers.clone();
        let mut kind = self.effects.kind.unwrap_or(EffectKind::Rhinestone);
        let before = kind;
        let mut apply = false;
        let error = self
            .effect_preview
            .as_ref()
            .and_then(|p| p.result.as_ref().err().cloned());
        let params = self.effects.params(kind);
        let mut changed: Vec<(&'static str, Value)> = Vec::new();
        let kind_entries: Vec<Entry<EffectKind>> =
            EFFECTS.iter().map(|d| Entry::new(d.kind, d.title)).collect();
        let _ = Panel::new("Effects")
            .id_salt("stencilcraft-effects-panel")
            .show(ui, |ui, _| {
            let _ = Select::new("Effect", Bind::bound(&mut kind), &kind_entries)
                .id_salt("effect-kind")
                .show(ui);
            if let Some(d) = def(before) {
                for field in d.fields {
                    let current = get(params, field.key).cloned().unwrap_or(Value::Null);
                    match field.kind {
                        Kind::Num { min, max, .. } | Kind::Int { min, max } => {
                            let mut value = current.as_f64().unwrap_or(min);
                            let format = match field.kind {
                                Kind::Num { unit, .. } if !unit.is_empty() => {
                                    format!("{{n}} {unit}")
                                }
                                _ => "{n}".to_string(),
                            };
                            labelled(ui, field.label, |ui| {
                                number(
                                    ui,
                                    &format!("effect-{}", field.key),
                                    field.label,
                                    &mut value,
                                    min..=max,
                                    &format,
                                    &numbers,
                                );
                            });
                            let value = value.clamp(min, max);
                            let new = if matches!(field.kind, Kind::Int { .. }) {
                                // Clamped to the field's range, so the cast is exact.
                                json!(value.round() as i64)
                            } else {
                                json!(value)
                            };
                            if current.as_f64() != new.as_f64() && value.is_finite() {
                                changed.push((field.key, new));
                            }
                        }
                        Kind::Bool => {
                            let mut on = current.as_bool().unwrap_or(false);
                            let _ = Checkbox::new(Bind::bound(&mut on))
                                .label(field.label)
                                .id_salt(("effect", field.key))
                                .show(ui);
                            if Some(on) != current.as_bool() {
                                changed.push((field.key, json!(on)));
                            }
                        }
                        Kind::Choice(options) => {
                            let mut value = current.as_str().unwrap_or("").to_string();
                            labelled(ui, field.label, |ui| {
                                let entries: Vec<Entry<String>> = options
                                    .iter()
                                    .map(|(v, label)| Entry::new((*v).to_string(), *label))
                                    .collect();
                                let _ = Select::new(field.label, Bind::bound(&mut value), &entries)
                                    .id_salt(format!("effect-{}", field.key))
                                    .show(ui);
                            });
                            if Some(value.as_str()) != current.as_str() && !value.is_empty() {
                                changed.push((field.key, json!(value)));
                            }
                        }
                    }
                }
            }
            if let Some(error) = &error {
                hint(ui, error);
            } else {
                hint(ui, "The blue lines preview the effect; Apply adds it.");
            }
            apply = Button::new("Apply")
                .variant(Variant::PRIMARY)
                .size(ControlSize::Sm)
                .enabled(error.is_none())
                .show(ui)
                .clicked();
        });
        for (key, value) in changed {
            set(params, key, value);
        }
        self.effects.kind = Some(kind);
        if apply {
            self.apply_effect(kind);
        }
        self.refresh_effect_preview(kind);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_effect_default_builds_a_valid_command() {
        for d in EFFECTS {
            let mut params = defaults(d.kind);
            assert!(!params.is_empty(), "{}", d.title);
            for field in d.fields {
                assert!(
                    get(&params, field.key).is_some(),
                    "{} {}",
                    d.title,
                    field.key
                );
            }
            match d.target {
                Target::Shapes => {
                    params.insert("ids".into(), json!([1]));
                }
                Target::Shape | Target::Image => {
                    params.insert("id".into(), json!(1));
                }
                Target::Page => {}
            }
            let command = json!({"command": d.command, "params": params});
            assert!(
                StencilApp::effect_engine_command(&command).is_ok(),
                "{}: {command}",
                d.title
            );
        }
    }

    #[test]
    fn dotted_keys_reach_nested_params() {
        let mut params = defaults(EffectKind::EmbossPattern);
        set(&mut params, "tool.deboss", json!(true));
        assert_eq!(get(&params, "tool.deboss"), Some(&json!(true)));
        set(&mut params, "new.deep.key", json!(1));
        assert_eq!(get(&params, "new.deep.key"), Some(&json!(1)));
    }
}

#[cfg(test)]
mod app_tests {
    use super::*;

    #[test]
    fn effects_apply_to_the_selection_and_repeat() {
        let mut app = StencilApp::new(None);
        app.run_command(Command::ShapeAddRect {
            x: 10.0,
            y: 10.0,
            width: 60.0,
            height: 40.0,
        });
        let rect = app
            .engine
            .doc
            .shapes()
            .map(|(_, s)| s.id)
            .next()
            .expect("rect");
        app.set_selection([rect]);
        app.open_effect(EffectMenu::WarpArc);
        assert_eq!(app.effects.kind, Some(EffectKind::Warp));
        app.refresh_effect_preview(EffectKind::Warp);
        assert!(
            app.effect_preview
                .as_ref()
                .is_some_and(|p| p.result.as_ref().is_ok_and(|v| !v.is_empty())),
            "the warp previews"
        );
        let before = app.engine.doc.get_shape(rect).map(|s| s.path.clone());
        app.apply_effect(EffectKind::Warp);
        assert_ne!(
            app.engine.doc.get_shape(rect).map(|s| s.path.clone()),
            before
        );
        assert!(app.engine.undo(), "one undo step");
        assert_eq!(
            app.engine.doc.get_shape(rect).map(|s| s.path.clone()),
            before
        );
        app.set_selection([rect]);
        app.apply_last_effect();
        assert_ne!(
            app.engine.doc.get_shape(rect).map(|s| s.path.clone()),
            before
        );
        // Rhinestones add stones; Puzzle needs exactly one shape.
        app.set_selection([rect]);
        app.open_effect(EffectMenu::Rhinestone);
        let count = app.engine.doc.shapes().count();
        app.apply_effect(EffectKind::Rhinestone);
        assert!(app.engine.doc.shapes().count() > count, "{}", app.status);
        app.set_selection([]);
        app.apply_effect(EffectKind::Puzzle);
        assert!(app.status.contains("Select one shape"));
    }
}
