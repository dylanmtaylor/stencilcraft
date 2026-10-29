//! Graphtec/Silhouette device control: hardware tables, the ASCII command
//! encoder, and the transport layer.
//!
//! The protocol facts here (framing, command vocabulary per model family,
//! media presets, mat codes, hardware geometry) come from community reverse
//! engineering of these cutting plotters — chiefly `inkscape-silhouette`
//! (GPL-2.0, fablabnbg), `inkscape-cutcutgo` (GPL-3.0, virtualabs) and
//! `pklaus/Silhouette-Cutting-Plotters`, which credit the robocut lineage.
//! This crate is an independent, panic-free
//! Rust implementation of those *facts*; no GPL code was ported. See
//! `docs/machine.md` for the protocol reference and licensing note.
//!
//! Layering: depends only on `stencilcraft-geom` (points).

use stencilcraft_geom::Point;

/// Device units: the cutter's native resolution is 0.05 mm, so all movements
/// are sent as integer "Silhouette Units" (20 per mm, 508 per inch).
pub const SU_PER_MM: f64 = 20.0;
pub const SU_PER_INCH: f64 = 508.0;

/// Command terminator: every ASCII command is followed by ETX (0x03).
pub const ETX: u8 = 0x03;
/// Escape `\x1b\x04` (EOT): initializes the device.
pub const ESC_INIT: &str = "\u{1b}\u{4}";
/// Escape `\x1b\x05` (ENQ): device status query, replies one of `0`,`1`,`2`.
pub const ESC_STATUS: &str = "\u{1b}\u{5}";
/// Escape `\x1b\x15` (NAK): tool-setup query ( Cameo 4 family).
pub const ESC_TOOL_SETUP: &str = "\u{1b}\u{15}";
/// Query `FG` — firmware version string, e.g. `CAMEO 4 PRO V1.02`.
pub const QUERY_VERSION: &str = "FG";

/// Convert millimetres to device units by rounding.
fn su(mm: f64) -> i64 {
    (mm * SU_PER_MM).round() as i64
}

/// A known cutter model and its hardware geometry, learned from the
/// community device tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    Cameo3,
    Cameo4,
    Cameo4Plus,
    /// The 24" model: 600 mm cuttable width (not the full 609.6 mm roll —
    /// StencilCraft trims a thin margin), 3000 mm roll length.
    Cameo4Pro,
    Cameo5,
    Cameo5Plus,
    CameoProMk2,
    Portrait3,
    Portrait4,
}

impl Model {
    pub fn width_mm(self) -> f64 {
        match self {
            Model::Cameo3 | Model::Cameo4 => 304.8,
            Model::Cameo4Plus | Model::Cameo5Plus => 372.0,
            Model::Cameo4Pro => 600.0,
            Model::Cameo5 => 330.2,
            Model::CameoProMk2 => 609.0,
            Model::Portrait3 => 203.0,
            Model::Portrait4 => 216.0,
        }
    }

    pub fn length_mm(self) -> f64 {
        match self {
            // Roll-fed machines run metres of material off a roll.
            Model::Cameo4
            | Model::Cameo4Plus
            | Model::Cameo4Pro
            | Model::Cameo5
            | Model::Cameo5Plus
            | Model::CameoProMk2
            | Model::Portrait4 => 3000.0,
            Model::Portrait3 => 18290.0,
            Model::Cameo3 => 3000.0,
        }
    }

    /// Maximum pressure for the model (Cameo 4 family: 33).
    pub fn max_pressure(self) -> u8 {
        33
    }

    /// Maximum speed for the model (Cameo 4 family: 30).
    pub fn max_speed(self) -> u8 {
        30
    }

    /// Identify a model from a firmware-version response (the `FG` query),
    /// matching `CAMEO 4 PRO V1.02`, `CAMEO 5 ALPHA V1.01`, etc. Longer,
    /// more specific names are matched first so `CAMEO 4 PLUS` is never
    /// answered by `CAMEO 4`.
    pub fn from_version(version: &str) -> Option<Model> {
        // (needle, model) pairs, most specific first.
        const KNOWN: &[(&str, Model)] = &[
            ("CAMEO PRO MARK2", Model::CameoProMk2),
            ("CAMEO 5 PLUS", Model::Cameo5Plus),
            ("CAMEO 5", Model::Cameo5),
            ("CAMEO 4 PRO", Model::Cameo4Pro),
            ("CAMEO 4 PLUS", Model::Cameo4Plus),
            ("CAMEO 4", Model::Cameo4),
            ("CAMEO3", Model::Cameo3),
            ("PORTRAIT4", Model::Portrait4),
            ("PORTRAIT3", Model::Portrait3),
        ];
        let upper = version
            .chars()
            .map(|c| c.to_ascii_uppercase())
            .collect::<String>();
        for (needle, model) in KNOWN {
            if upper.contains(needle) {
                return Some(*model);
            }
        }
        None
    }
}

/// A cutting-mat definition: device code plus usable extent in inches
/// (the extents below come from the community mat tables; the Pro mat is
/// 24 in × 24 in even though the tray takes 609.6 mm-wide material).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mat {
    None,
    Cameo12x12,
    Cameo12x24,
    Portrait8x12,
    CameoPlus15x15,
    CameoPro24x24,
    Portrait8_5x12,
}

impl Mat {
    /// The device's `TG` mat code.
    pub fn code(self) -> u8 {
        match self {
            Mat::None => 0,
            Mat::Cameo12x12 => 1,
            Mat::Cameo12x24 => 2,
            Mat::Portrait8x12 => 3,
            Mat::CameoPlus15x15 => 8,
            Mat::CameoPro24x24 => 9,
            Mat::Portrait8_5x12 => 11,
        }
    }

    /// Usable mat extent (down, right) in inches.
    pub fn extent_inch(self) -> (f64, f64) {
        match self {
            Mat::None => (0.0, 0.0),
            Mat::Cameo12x12 => (12.0, 12.0),
            Mat::Cameo12x24 => (24.0, 12.0),
            Mat::Portrait8x12 => (12.0, 8.0),
            Mat::CameoPlus15x15 => (15.0, 15.0),
            Mat::CameoPro24x24 => (24.0, 24.0),
            Mat::Portrait8_5x12 => (12.0, 8.5),
        }
    }
}

/// A media preset: device id plus default pressure/speed/blade-depth. These
/// mirror the community media table (see `docs/machine.md` for provenance);
/// cap colour is design intent, not protocol, and is omitted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Media {
    pub id: u16,
    pub name: &'static str,
    pub pressure: Option<u8>,
    pub speed: Option<u8>,
    pub depth: Option<u8>,
}

impl Media {
    pub fn by_id(id: u16) -> Option<Media> {
        MEDIA_PRESETS.iter().copied().find(|m| m.id == id)
    }

    pub fn by_name(name: &str) -> Option<Media> {
        MEDIA_PRESETS
            .iter()
            .copied()
            .find(|m| m.name.eq_ignore_ascii_case(name))
    }
}

/// The device media table: `(id, name, pressure, speed, depth)`.
/// `Custom` (300) carries no defaults and requires explicit pressure/speed.
pub const MEDIA_PRESETS: &[Media] = &[
    Media {
        id: 300,
        name: "Custom",
        pressure: None,
        speed: None,
        depth: None,
    },
    Media {
        id: 100,
        name: "Card without Craft Paper Backing",
        pressure: Some(27),
        speed: Some(10),
        depth: Some(1),
    },
    Media {
        id: 101,
        name: "Card with Craft Paper Backing",
        pressure: Some(27),
        speed: Some(10),
        depth: Some(1),
    },
    Media {
        id: 102,
        name: "Vinyl Sticker",
        pressure: Some(10),
        speed: Some(5),
        depth: Some(1),
    },
    Media {
        id: 106,
        name: "Film Labels",
        pressure: Some(14),
        speed: Some(10),
        depth: Some(1),
    },
    Media {
        id: 111,
        name: "Thick Media",
        pressure: Some(27),
        speed: Some(10),
        depth: Some(1),
    },
    Media {
        id: 112,
        name: "Thin Media",
        pressure: Some(2),
        speed: Some(10),
        depth: Some(1),
    },
    Media {
        id: 113,
        name: "Pen",
        pressure: Some(18),
        speed: Some(10),
        depth: None,
    },
    Media {
        id: 120,
        name: "Bond Paper 13-28 lbs (105g)",
        pressure: Some(30),
        speed: Some(10),
        depth: Some(1),
    },
    Media {
        id: 121,
        name: "Bristol Paper 57-67 lbs (145g)",
        pressure: Some(30),
        speed: Some(10),
        depth: Some(1),
    },
    Media {
        id: 122,
        name: "Cardstock 40-60 lbs (90g)",
        pressure: Some(30),
        speed: Some(10),
        depth: Some(1),
    },
    Media {
        id: 123,
        name: "Cover 40-60 lbs (170g)",
        pressure: Some(30),
        speed: Some(10),
        depth: Some(1),
    },
    Media {
        id: 124,
        name: "Film, Double Matte Translucent",
        pressure: Some(1),
        speed: Some(10),
        depth: Some(1),
    },
    Media {
        id: 125,
        name: "Film, Vinyl With Adhesive Back",
        pressure: Some(1),
        speed: Some(10),
        depth: Some(1),
    },
    Media {
        id: 126,
        name: "Film, Window With Kling Adhesive",
        pressure: Some(1),
        speed: Some(10),
        depth: Some(1),
    },
    Media {
        id: 127,
        name: "Index 90 lbs (165g)",
        pressure: Some(30),
        speed: Some(10),
        depth: Some(1),
    },
    Media {
        id: 128,
        name: "Inkjet Photo Paper 28-44 lbs (70g)",
        pressure: Some(20),
        speed: Some(10),
        depth: Some(1),
    },
    Media {
        id: 129,
        name: "Inkjet Photo Paper 45-75 lbs (110g)",
        pressure: Some(27),
        speed: Some(10),
        depth: Some(1),
    },
    Media {
        id: 130,
        name: "Magnetic Sheet",
        pressure: Some(30),
        speed: Some(3),
        depth: Some(1),
    },
    Media {
        id: 131,
        name: "Offset 24-60 lbs (90g)",
        pressure: Some(30),
        speed: Some(10),
        depth: Some(1),
    },
    Media {
        id: 132,
        name: "Print Paper Light Weight",
        pressure: Some(5),
        speed: Some(10),
        depth: Some(1),
    },
    Media {
        id: 133,
        name: "Print Paper Medium Weight",
        pressure: Some(25),
        speed: Some(10),
        depth: Some(1),
    },
    Media {
        id: 134,
        name: "Sticker Sheet",
        pressure: Some(20),
        speed: Some(10),
        depth: Some(1),
    },
    Media {
        id: 135,
        name: "Tag 100 lbs (275g)",
        pressure: Some(20),
        speed: Some(10),
        depth: Some(1),
    },
    Media {
        id: 136,
        name: "Text Paper 24-70 lbs (105g)",
        pressure: Some(30),
        speed: Some(10),
        depth: Some(1),
    },
    Media {
        id: 137,
        name: "Vellum Bristol 57-67 lbs (145g)",
        pressure: Some(30),
        speed: Some(10),
        depth: Some(1),
    },
    Media {
        id: 138,
        name: "Writing Paper 24-70 lbs (105g)",
        pressure: Some(30),
        speed: Some(10),
        depth: Some(1),
    },
];

/// Tool types reported by the tool-setup query (Cameo 4 family carriage 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    Empty,
    RatchetBlade,
    AutoBlade,
    DeepCutBlade,
    KraftBlade,
    RotaryBlade,
    Pen,
    Error,
}

impl ToolKind {
    pub fn from_code(code: u8) -> Option<ToolKind> {
        Some(match code {
            0 => ToolKind::Empty,
            1 => ToolKind::RatchetBlade,
            2 => ToolKind::AutoBlade,
            3 => ToolKind::DeepCutBlade,
            4 => ToolKind::KraftBlade,
            5 => ToolKind::RotaryBlade,
            7 => ToolKind::Pen,
            255 => ToolKind::Error,
            _ => return None,
        })
    }
}

/// Device status from the status escape query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceStatus {
    Ready,
    Moving,
    Unloaded,
    Unknown(u8),
}

/// Parse a status query response (e.g. `b"    0\x03"` — an ASCII digit
/// before the ETX terminator, possibly zero-padded).
pub fn parse_status(response: &[u8]) -> DeviceStatus {
    // The device zero-pads replies ("    0\x03") — skip spaces and ETX,
    // then interpret the digit byte.
    let code = response
        .iter()
        .find(|b| **b != b' ' && **b != 0x03)
        .copied();
    match code {
        Some(b'0') => DeviceStatus::Ready,
        Some(b'1') => DeviceStatus::Moving,
        Some(b'2') => DeviceStatus::Unloaded,
        Some(other) => DeviceStatus::Unknown(other),
        None => DeviceStatus::Unknown(255),
    }
}

/// Per-job cutting configuration. Every field here ends up as part of the
/// setup preamble exactly the way Silhouette Studio sends it to Cameo 4
/// family machines (see `encode_setup`).
#[derive(Debug, Clone, PartialEq)]
pub struct CutSettings {
    /// Tool holder 1 or 2.
    pub toolholder: u8,
    /// Pen mode: no blade kerf, fork-corners disabled, raise head between paths.
    pub pen: bool,
    pub pressure: Option<u8>,
    pub speed: Option<u8>,
    /// Blade depth 0..10 (only applies to the AutoBlade).
    pub depth: Option<u8>,
    /// Blade swivel-circle diameter in mm (0.9 for the stock blade).
    pub blade_diameter_mm: f64,
    /// Lift the head at sharp corners ("sharpen corners") between paths.
    pub sharpen_corners: bool,
    pub sharpen_start_mm: f64,
    pub sharpen_end_mm: f64,
    /// Rollers forward-and-back (needs ≥19 pressure); off by default.
    pub track_enhancing: bool,
    pub mat: Mat,
    /// Landscape media feed (default: portrait, which matches our y-down
    /// coordinates and avoids an alignment "tick" artefact).
    pub landscape: bool,
}

impl CutSettings {
    pub fn from_media(media: &Media) -> CutSettings {
        CutSettings {
            toolholder: 1,
            pen: media.id == 113,
            pressure: media.pressure,
            speed: media.speed,
            depth: media.depth,
            blade_diameter_mm: 0.9,
            sharpen_corners: false,
            sharpen_start_mm: 0.1,
            sharpen_end_mm: 0.1,
            track_enhancing: false,
            mat: Mat::CameoPro24x24,
            landscape: false,
        }
    }
}

/// One device command. `to_ascii` produces the exact text (without the ETX
/// terminator).
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// `J<toolholder>` — select the active tool holder.
    SelectTool(u8),
    /// `FX<p>,<toolholder>` — cutting force.
    Pressure(u8, u8),
    /// `!<speed>,<toolholder>` — cut speed.
    Speed(u8, u8),
    /// `TF<depth>,<toolholder>` — blade depth (AutoBlade).
    Depth(u8, u8),
    /// `FC<x>,<y>,<toolholder>` — blade swivel circle (in device units).
    CutterOffset {
        x_mm: f64,
        y_mm: f64,
        toolholder: u8,
    },
    /// `FE<n>,<toolholder>` — lift head between paths when `n == 1`.
    Lift { lift: bool, toolholder: u8 },
    /// `FF<start>,<end>,<toolholder>` — corner sharpening range, tenths of mm.
    SharpenCorners {
        start_tenths: i64,
        end_tenths: i64,
        toolholder: u8,
    },
    /// `TJ<acceleration>` — acceleration ramp.
    Acceleration(i64),
    /// `M<down>,<right>` — head-up move, mm coordinates.
    Move { down_mm: f64, right_mm: f64 },
    /// `D<down>,<right>` — head-down draw/cut to point, mm coordinates.
    Draw { down_mm: f64, right_mm: f64 },
    /// `\<down>,<right>` — cutting-region upper-left bound.
    BoundaryTopLeft { down_mm: f64, right_mm: f64 },
    /// `Z<down>,<right>` — cutting-region lower-right bound.
    BoundaryBottomRight { down_mm: f64, right_mm: f64 },
    /// `FY<0|1>` — roller track enhancing (`FY0` = on).
    TrackEnhancing(bool),
    /// `TG<code>` — select cutting mat.
    SelectMat(Mat),
    /// `FN0` — device soft reset.
    Reset,
    /// `TB50,0` / `TB50,1` — portrait/landscape feed direction.
    PortraitMode(bool),
    /// `L0` — pen-up marker between plot sections.
    PenUp,
    /// `SO0` — mark origin/service position.
    Origin,
    /// `H` — full home ( Cameo 3 and newer).
    Home,
    /// A raw command we do not (yet) model. Prefer the typed variants above.
    Raw(String),
}

impl Command {
    pub fn to_ascii(&self) -> String {
        match self {
            Command::SelectTool(t) => format!("J{t}"),
            Command::Pressure(p, t) => format!("FX{p},{t}"),
            Command::Speed(s, t) => format!("!{s},{t}"),
            Command::Depth(d, t) => format!("TF{d},{t}"),
            Command::CutterOffset {
                x_mm,
                y_mm,
                toolholder,
            } => {
                format!("FC{},{},{}", su(*x_mm), su(*y_mm), toolholder)
            }
            Command::Lift { lift, toolholder } => {
                format!("FE{},{}", u8::from(*lift), toolholder)
            }
            Command::SharpenCorners {
                start_tenths,
                end_tenths,
                toolholder,
            } => {
                format!("FF{start_tenths},{end_tenths},{toolholder}")
            }
            Command::Acceleration(a) => format!("TJ{a}"),
            Command::Move { down_mm, right_mm } => {
                format!("M{},{}", su(*down_mm), su(*right_mm))
            }
            Command::Draw { down_mm, right_mm } => {
                format!("D{},{}", su(*down_mm), su(*right_mm))
            }
            Command::BoundaryTopLeft { down_mm, right_mm } => {
                format!("\\{},{}", su(*down_mm), su(*right_mm))
            }
            Command::BoundaryBottomRight { down_mm, right_mm } => {
                format!("Z{},{}", su(*down_mm), su(*right_mm))
            }
            Command::TrackEnhancing(on) => format!("FY{}", if *on { 0 } else { 1 }),
            Command::SelectMat(mat) => format!("TG{}", mat.code()),
            Command::Reset => "FN0".to_string(),
            Command::PortraitMode(landscape) => {
                format!("TB50,{}", if *landscape { 1 } else { 0 })
            }
            Command::PenUp => "L0".to_string(),
            Command::Origin => "SO0".to_string(),
            Command::Home => "H".to_string(),
            Command::Raw(s) => s.clone(),
        }
    }
}

/// Delimit commands into a byte stream the device accepts: each command's
/// ASCII followed by an ETX byte.
pub fn delimit(commands: &[Command]) -> Vec<u8> {
    let mut out = Vec::with_capacity(commands.len() * 8);
    for cmd in commands {
        out.extend_from_slice(cmd.to_ascii().as_bytes());
        out.push(ETX);
    }
    out
}

/// Clamp helpers shared by the encoder.
fn clamp_u8(value: u8, min: u8, max: u8) -> u8 {
    value.clamp(min, max)
}

/// The setup preamble for Cameo 4-family machines: tool selection, pressure,
/// speed, blade offset, corner sharpening, mat select, portrait feed, and the
/// full-media cutting boundary. This mirrors the observed Silhouette Studio
/// command order for these models.
pub fn encode_setup(model: Model, settings: &CutSettings) -> Vec<Command> {
    let t = settings.toolholder;
    let mut commands = Vec::new();
    commands.push(Command::SelectTool(t));

    if let Some(pressure) = settings.pressure {
        let capped = pressure.min(model.max_pressure());
        commands.push(Command::Pressure(capped, t));
        // On first connect acceleration is set to 0, then bumped later.
        commands.push(Command::Acceleration(0));
    }
    if let Some(speed) = settings.speed {
        let capped = speed.clamp(1, model.max_speed());
        commands.push(Command::Speed(capped, t));
    }

    // The first cutter-offset command always reads 0 mm × 0.05 mm.
    commands.push(Command::CutterOffset {
        x_mm: 0.0,
        y_mm: 0.05,
        toolholder: t,
    });
    commands.push(Command::Lift {
        lift: settings.sharpen_corners,
        toolholder: t,
    });

    // Sharpen-corner ranges are transmitted in tenths of a millimetre
    // (not device units); pens always send 0,0.
    if settings.pen {
        commands.push(Command::SharpenCorners {
            start_tenths: 0,
            end_tenths: 0,
            toolholder: t,
        });
    } else {
        // Truncated, not rounded — matching the observed device stream.
        let start = ((settings.sharpen_start_mm + 0.05) * 10.0) as i64;
        let end = ((settings.sharpen_end_mm + 0.05) * 10.0) as i64;
        commands.push(Command::SharpenCorners {
            start_tenths: start,
            end_tenths: end,
            toolholder: t,
        });
    }

    // Pressure and acceleration go out a second time (observed behaviour).
    if let Some(pressure) = settings.pressure {
        let clamped = clamp_u8(pressure, 1, model.max_pressure());
        commands.push(Command::Pressure(clamped, t));
        commands.push(Command::Acceleration(3));
    }

    // Second cutter-offset with the blade's actual swivel diameter.
    let swivel = if settings.pen {
        0.0
    } else {
        settings.blade_diameter_mm
    };
    commands.push(Command::CutterOffset {
        x_mm: swivel,
        y_mm: 0.05,
        toolholder: t,
    });

    if settings.track_enhancing {
        commands.push(Command::TrackEnhancing(true));
    }

    commands.push(Command::SelectMat(settings.mat));
    commands.push(Command::Reset);
    commands.push(Command::PortraitMode(settings.landscape));
    commands.push(Command::BoundaryTopLeft {
        down_mm: 0.0,
        right_mm: 0.0,
    });
    commands.push(Command::BoundaryBottomRight {
        down_mm: model.length_mm(),
        right_mm: model.width_mm(),
    });
    commands
}

/// A parsed plot region and per-job options for `encode_plot`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlotOptions {
    /// Top margin (mm along the feed axis).
    pub margin_down_mm: f64,
    /// Left margin (mm along the cross axis).
    pub margin_right_mm: f64,
    /// Extra media feed after the last cut (negative backs into the design).
    pub end_paper_offset_mm: f64,
}

impl Default for PlotOptions {
    fn default() -> Self {
        PlotOptions {
            margin_down_mm: 0.0,
            margin_right_mm: 0.0,
            end_paper_offset_mm: 0.0,
        }
    }
}

/// Bounding box in mm `(down-min, right-min, down-max, right-max)` — i.e.
/// `(top-left down, top-left right, bottom-right down, bottom-right right)`.
pub type BBoxMm = (f64, f64, f64, f64);

/// Compute the mm bounding box of a path list, or `None` when empty.
pub fn find_bbox(paths: &[Vec<(f64, f64)>]) -> Option<BBoxMm> {
    let mut bbox: Option<BBoxMm> = None;
    for path in paths {
        for &(x, y) in path {
            // x = right axis, y = down axis.
            bbox = Some(match bbox {
                None => (y, x, y, x),
                Some((d0, r0, d1, r1)) => (d0.min(y), r0.min(x), d1.max(y), r1.max(x)),
            });
        }
    }
    bbox
}

/// Encode the plot section: move/draw commands with offset and margins, or
/// (when `bbox_only`) a four-line preview rectangle instead of the design.
pub fn encode_plot(
    paths: &[Vec<(f64, f64)>],
    opts: &PlotOptions,
    bbox_only: bool,
) -> (Vec<Command>, Option<BBoxMm>) {
    let Some(bb) = find_bbox(paths) else {
        return (Vec::new(), None);
    };
    let mut commands = Vec::new();
    if bbox_only {
        commands.push(Command::Move {
            down_mm: bb.0 + opts.margin_down_mm,
            right_mm: bb.1 + opts.margin_right_mm,
        });
        commands.push(Command::Draw {
            down_mm: bb.0 + opts.margin_down_mm,
            right_mm: bb.3 + opts.margin_right_mm,
        });
        commands.push(Command::Draw {
            down_mm: bb.2 + opts.margin_down_mm,
            right_mm: bb.3 + opts.margin_right_mm,
        });
        commands.push(Command::Draw {
            down_mm: bb.2 + opts.margin_down_mm,
            right_mm: bb.1 + opts.margin_right_mm,
        });
        commands.push(Command::Draw {
            down_mm: bb.0 + opts.margin_down_mm,
            right_mm: bb.1 + opts.margin_right_mm,
        });
    } else {
        let (dx, rx) = (opts.margin_down_mm, opts.margin_right_mm);
        for path in paths {
            if path.len() < 2 {
                continue;
            }
            commands.push(Command::Move {
                down_mm: path[0].1 + dx,
                right_mm: path[0].0 + rx,
            });
            for &(x, y) in &path[1..] {
                commands.push(Command::Draw {
                    down_mm: y + dx,
                    right_mm: x + rx,
                });
            }
        }
    }
    // Park below the cut so the next job (or the cross-cutter) starts there.
    if let Some(bb) = find_bbox(paths) {
        commands.push(Command::Move {
            down_mm: bb.2 + opts.end_paper_offset_mm,
            right_mm: 0.0,
        });
        commands.push(Command::Origin);
    }
    (commands, Some(bb))
}

/// A full cut program: setup preamble, plot body, trailer. `to_stream`
/// serialises to the exact bytes a transport writes.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CutProgram {
    pub commands: Vec<Command>,
    pub bbox_mm: Option<BBoxMm>,
}

/// Encode a complete cut: setup for the model + plot body + park trailer.
/// Transport-level palaver (the init escape `ESC_INIT`, `FG` version query,
/// status polling) lives with the session, not in the program stream.
pub fn encode_cut(
    model: Model,
    settings: &CutSettings,
    paths: &[Vec<(f64, f64)>],
    opts: &PlotOptions,
    bbox_only: bool,
) -> CutProgram {
    let mut commands = encode_setup(model, settings);
    let (plot, bbox) = encode_plot(paths, opts, bbox_only);
    commands.extend(plot);
    CutProgram {
        commands,
        bbox_mm: bbox,
    }
}

impl CutProgram {
    /// The exact byte stream for a transport to write: ASCII per command
    /// each terminated with ETX.
    pub fn to_stream(&self) -> Vec<u8> {
        delimit(&self.commands)
    }
}

/// Errors surfaced by transports and sessions.
#[derive(Debug, Clone, PartialEq)]
pub enum MachineError {
    /// Transport I/O problem (string description; no panic ever escapes).
    Transport(String),
    /// The device answered something we could not decode.
    Protocol(String),
}

impl core::fmt::Display for MachineError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MachineError::Transport(e) => write!(f, "transport: {e}"),
            MachineError::Protocol(e) => write!(f, "protocol: {e}"),
        }
    }
}

impl std::error::Error for MachineError {}

/// A byte-pipe to a cutter. USB bulk transports, serial-over-BLE, BLE GATT,
/// or recordings all implement this. Never panics; everything returns.
pub trait Transport {
    fn write(&mut self, data: &[u8]) -> Result<usize, MachineError>;
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, MachineError>;
}

/// Dry-run transport: records every write exactly as it would be sent.
/// (Queries answer nothing; callers handle empty reads.)
#[derive(Debug, Clone, Default)]
pub struct RecordingTransport {
    pub bytes: Vec<u8>,
}

impl RecordingTransport {
    pub fn new() -> Self {
        RecordingTransport { bytes: Vec::new() }
    }

    /// Human-readable transcript: one command per line, ETX becomes newline.
    pub fn transcript(&self) -> String {
        String::from_utf8_lossy(&self.bytes).replace('\u{3}', "\n")
    }
}

impl Transport for RecordingTransport {
    fn write(&mut self, data: &[u8]) -> Result<usize, MachineError> {
        self.bytes.extend_from_slice(data);
        Ok(data.len())
    }

    fn read(&mut self, _buf: &mut [u8]) -> Result<usize, MachineError> {
        Ok(0)
    }
}

/// Serial-port transport (behind the `serial` feature): for machines exposed
/// as a tty/COM device, or BLE cutters after OS pairing exposes an rfcomm
/// port. USB bulk devices are reached through a future `nusb` transport —
/// the same `Transport` interface, no encoder changes.
#[cfg(feature = "serial")]
pub struct SerialTransport {
    port: serialport::SerialPort,
}

#[cfg(feature = "serial")]
impl SerialTransport {
    pub fn open(path: &str, baud: u32) -> Result<Self, MachineError> {
        serialport::new(path, baud)
            .timeout(core::time::Duration::from_millis(5000))
            .open()
            .map(|port| SerialTransport { port })
            .map_err(|e| MachineError::Transport(format!("open {path}: {e}")))
    }
}

#[cfg(feature = "serial")]
impl Transport for SerialTransport {
    fn write(&mut self, data: &[u8]) -> Result<usize, MachineError> {
        self.port
            .write(data)
            .map_err(|e| MachineError::Transport(e.to_string()))
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, MachineError> {
        self.port
            .read(buf)
            .map_err(|e| MachineError::Transport(e.to_string()))
    }
}

/// Convert geometric paths (doc coordinates: x right, y down, millimetres)
/// into the machine's move/draw list, closing closed polylines.
pub fn paths_from_polylines(polylines: &[stencilcraft_geom::Polyline]) -> Vec<Vec<(f64, f64)>> {
    let mut out = Vec::with_capacity(polylines.len());
    for pl in polylines {
        if pl.points.len() < 2 {
            continue;
        }
        let mut path: Vec<(f64, f64)> = pl.points.iter().map(|p: &Point| (p.x, p.y)).collect();
        if pl.closed
            && let Some(first) = pl.points.first()
        {
            path.push((first.x, first.y));
        }
        out.push(path);
    }
    out
}

/// One-shot session helper: send a program through any transport. Reads and
/// status polling are the caller's concern here; this is the minimal write
/// path that both dry-run and real transports share.
pub fn send_program(
    transport: &mut dyn Transport,
    program: &CutProgram,
) -> Result<usize, MachineError> {
    let stream = program.to_stream();
    transport.write(&stream)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square_paths() -> Vec<Vec<(f64, f64)>> {
        // A 10 mm square at the origin (x right, y down).
        vec![vec![
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 10.0),
            (0.0, 10.0),
            (0.0, 0.0),
        ]]
    }

    fn vinyl_pro() -> (Model, CutSettings) {
        let media = Media::by_id(102).expect("vinyl preset");
        (Model::Cameo4Pro, CutSettings::from_media(&media))
    }

    #[test]
    fn unit_conversions() {
        assert_eq!(su(0.05), 1);
        assert_eq!(su(0.9), 18); // stock blade diameter
        assert_eq!(su(600.0), 12_000);
        assert_eq!(su(3000.0), 60_000);
        // Round-trip through the constant ratio.
        assert_eq!(18.0 / SU_PER_MM, 0.9);
        // Inch conversion for mat extents.
        assert_eq!((24.0 * SU_PER_INCH) as i64, 12_192);
    }

    #[test]
    fn model_from_version_matches_specific_names_first() {
        assert_eq!(
            Model::from_version("CAMEO 4 PRO V1.02"),
            Some(Model::Cameo4Pro)
        );
        assert_eq!(Model::from_version("CAMEO 4 PLUS"), Some(Model::Cameo4Plus));
        assert_eq!(Model::from_version("CAMEO 4 V0.99"), Some(Model::Cameo4));
        assert_eq!(
            Model::from_version("PORTRAIT4 V1.1"),
            Some(Model::Portrait4)
        );
        assert_eq!(Model::from_version("UNKNOWN PLOTTER"), None);
    }

    #[test]
    fn media_table_lookups() {
        assert_eq!(Media::by_id(102).expect("vinyl").name, "Vinyl Sticker");
        assert_eq!(Media::by_id(102).expect("vinyl").pressure, Some(10));
        assert_eq!(Media::by_id(102).expect("vinyl").speed, Some(5));
        assert!(Media::by_id(300).expect("custom").pressure.is_none());
        assert_eq!(Media::by_name("vinyl sticker").expect("vinyl").id, 102);
        assert!(Media::by_name("unobtanium").is_none());
    }

    #[test]
    fn setup_matches_cameo4_sequence() {
        let (model, settings) = vinyl_pro();
        let cmds = encode_setup(model, &settings);
        let expected = vec![
            "J1",
            "FX10,1",
            "TJ0",
            "!5,1",
            "FC0,1,1",
            "FE0,1",
            "FF1,1,1",
            "FX10,1",
            "TJ3",
            "FC18,1,1",
            "TG9",
            "FN0",
            "TB50,0",
            "\\0,0",
            "Z60000,12000",
        ];
        let actual: Vec<String> = cmds.iter().map(|c| c.to_ascii()).collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn plot_moves_and_draws_swapped_axes() {
        let paths = square_paths();
        let (cmds, bb) = encode_plot(&paths, &PlotOptions::default(), false);
        // First: move to (down=0, right=0), then draws with y in first field.
        assert_eq!(cmds[0].to_ascii(), "M0,0");
        assert_eq!(cmds[1].to_ascii(), "D0,200"); // x=10 → right=200, y=0 → down=0
        assert_eq!(cmds[2].to_ascii(), "D200,200");
        assert_eq!(cmds[3].to_ascii(), "D200,0");
        assert_eq!(cmds[4].to_ascii(), "D0,0");
        // Trailer: park below the cut (down=10mm), then origin marker.
        assert_eq!(cmds[5].to_ascii(), "M200,0");
        assert_eq!(cmds[6].to_ascii(), "SO0");
        assert_eq!(bb, Some((0.0, 0.0, 10.0, 10.0)));
    }

    #[test]
    fn bbox_only_draws_a_rectangle() {
        let paths = square_paths();
        let (cmds, _) = encode_plot(&paths, &PlotOptions::default(), true);
        let ascii: Vec<String> = cmds.iter().map(|c| c.to_ascii()).collect();
        assert_eq!(
            ascii,
            vec![
                "M0,0", "D0,200", "D200,200", "D200,0", "D0,0", "M200,0", "SO0"
            ]
        );
    }

    #[test]
    fn delimit_joins_with_etx() {
        let stream = delimit(&[Command::Home, Command::Origin]);
        assert_eq!(stream, b"H\x03SO0\x03".to_vec());
    }

    #[test]
    fn full_program_starts_and_ends_correctly() {
        let (model, settings) = vinyl_pro();
        let program = encode_cut(
            model,
            &settings,
            &square_paths(),
            &PlotOptions::default(),
            false,
        );
        let commands = &program.commands;
        assert_eq!(commands.first().expect("first"), &Command::SelectTool(1));
        assert_eq!(commands.last().expect("last"), &Command::Origin);
        // Setup preamble ends at the boundary, plot follows, trailer parks.
        assert_eq!(commands[1].to_ascii(), "FX10,1");
        assert_eq!(program.bbox_mm, Some((0.0, 0.0, 10.0, 10.0)));
    }

    #[test]
    fn recording_transport_captures_writes() {
        let mut transport = RecordingTransport::new();
        let program = CutProgram {
            commands: vec![Command::Home],
            bbox_mm: None,
        };
        send_program(&mut transport, &program).expect("write");
        assert_eq!(transport.bytes, b"H\x03".to_vec());
        assert_eq!(transport.transcript(), "H\n");
        // Empty reads are calm, not errors: dry runs answer nothing.
        let mut buf = [0u8; 16];
        assert_eq!(transport.read(&mut buf).expect("read"), 0);
    }

    #[test]
    fn parse_status_decodes_known_replies() {
        assert_eq!(parse_status(b"0"), DeviceStatus::Ready);
        assert_eq!(parse_status(b"1"), DeviceStatus::Moving);
        assert_eq!(parse_status(b"2"), DeviceStatus::Unloaded);
        // Zero-padded replies still parse (the digit is the first byte).
        assert_eq!(parse_status(b"    0\x03"), DeviceStatus::Ready);
        // Unknown replies keep their raw byte for diagnostics.
        assert_eq!(parse_status(b"9"), DeviceStatus::Unknown(b'9'));
        assert_eq!(parse_status(b""), DeviceStatus::Unknown(255));
    }

    #[test]
    fn paths_close_closed_polylines() {
        let pl = Vec::from([stencilcraft_geom::Polyline {
            points: Vec::from([Point::new(0.0, 0.0), Point::new(5.0, 0.0)]),
            closed: true,
        }]);
        let paths = paths_from_polylines(&pl);
        assert_eq!(paths, vec![vec![(0.0, 0.0), (5.0, 0.0), (0.0, 0.0)]]);
    }

    #[test]
    fn pressure_and_speed_clamp_to_model_limits() {
        let media = Media::by_id(102).expect("vinyl");
        let mut settings = CutSettings::from_media(&media);
        settings.pressure = Some(99);
        settings.speed = Some(199);
        let cmds = encode_setup(Model::Cameo4Pro, &settings);
        let pressures: Vec<String> = cmds
            .iter()
            .map(|c| c.to_ascii())
            .filter(|s| s.starts_with("FX"))
            .collect::<Vec<String>>();
        assert_eq!(pressures, vec!["FX33,1".to_string(), "FX33,1".to_string()]);
        assert!(cmds.iter().any(|c| c.to_ascii() == "!30,1"));
    }
}
