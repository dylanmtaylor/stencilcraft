//! Vendor-neutral cutting-machine drivers.
//!
//! A job is described once as a [`CutJob`] (ordered operations: geometry +
//! operation kind + tool + settings). A [`MachineDriver`] binds three parts:
//!
//! - [`Capabilities`] — what the machine can do, loaded from `machines.toml`
//!   through the [`DeviceRegistry`] (adding a model is data, not code);
//! - a [`Protocol`] — Silhouette/Graphtec GP-GL, plain GP-GL, HP-GL, DMPL,
//!   Grbl G-code — that turns the job into bytes and runs the send session;
//! - a [`Transport`] — USB bulk (`usb` feature), serial/USB-CDC (`serial`),
//!   TCP, file/stdout spool, mock.
//!
//! Safety contract: [`validate`] checks every job against the capabilities
//! and returns an error instead of encoding anything out of range or
//! undocumented; encoding never touches hardware; a real send is a separate,
//! explicit [`MachineDriver::send`]. See `docs/machine-architecture.md`.
//!
//! Protocol facts come from public documentation and community specification
//! (`inkscape-silhouette` GPL-2.0, read for facts only; Graphtec GP-GL list;
//! Grbl wiki). No GPL code was ported. See `docs/machine.md`.
//!
//! Layering: depends on `stencilcraft-geom` (points), `serde` + `toml` (the
//! machine table), and optionally `serialport` / `nusb`.

mod capabilities;
mod driver;
mod job;
pub mod protocol;
mod registry;
pub mod transport;

pub use capabilities::{Capabilities, Dialect, ProtocolId, Range, Support};
pub use driver::{MachineDriver, dry_run};
pub use job::{
    BBoxMm, CutJob, CutOrder, LayerPaths, LayerSettings, MAX_BLADE_DIAMETER_MM,
    MAX_CLOSED_OVERCUT_MM, MAX_OPERATIONS, MAX_OVERCUT_MM, MAX_PASSES, MAX_POINTS, MAX_SORT_PATHS,
    OpKind, OpSettings, Operation, Registration, apply_job_settings, extend_closed, find_bbox,
    order_paths, validate, weld_paths,
};
pub use protocol::{
    DriverEvent, MachineStatus, NoHooks, Protocol, SendReport, SessionHooks, WaitFn,
    encode_registration, protocol_for,
};
pub use registry::{BUILTIN_MACHINES_TOML, DeviceRegistry, USB_VENDORS, UsbIds};
#[cfg(feature = "serial")]
pub use transport::SerialTransport;
pub use transport::{
    FileTransport, MockTransport, RecordingTransport, TcpTransport, Transport, TransportSpec,
};
#[cfg(feature = "usb")]
pub use transport::{UsbTransport, list_usb_devices};

use stencilcraft_geom::Point;

/// Device units: the Silhouette native resolution is 0.05 mm, so all
/// movements are sent as integer "Silhouette Units" (20 per mm, 508 per inch).
pub const SU_PER_MM: f64 = 20.0;
pub const SU_PER_INCH: f64 = 508.0;

/// Command terminator: every GP-GL command is followed by ETX (0x03).
pub const ETX: u8 = 0x03;
/// Escape `\x1b\x04` (EOT): initializes the device.
pub const ESC_INIT: &str = "\u{1b}\u{4}";
/// Escape `\x1b\x05` (ENQ): device status query, replies one of `0`,`1`,`2`.
pub const ESC_STATUS: &str = "\u{1b}\u{5}";
/// Escape `\x1b\x15` (NAK): tool-setup query (Cameo 4 family).
pub const ESC_TOOL_SETUP: &str = "\u{1b}\u{15}";
/// Query `FG` — firmware version string, e.g. `CAMEO 4 PRO V1.02`.
pub const QUERY_VERSION: &str = "FG";

/// Convert millimetres to device units by rounding.
pub(crate) fn su(mm: f64) -> i64 {
    (mm * SU_PER_MM).round() as i64
}

/// A typed handle for the built-in machines, kept for the desktop app and
/// older callers. New code addresses machines by registry id
/// (`DeviceRegistry::builtin().by_id("cameo-4-pro")`), which also covers
/// user-defined `machines.toml` entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    Portrait,
    Portrait2,
    Portrait3,
    Portrait4,
    Cameo,
    Cameo2,
    Cameo3,
    Cameo4,
    Cameo4Plus,
    /// The 24" model: 600 mm cuttable width (not the full 609.6 mm roll).
    Cameo4Pro,
    CameoProMk2,
    Cameo5,
    Cameo5Plus,
    Cameo5Alpha,
    Cameo5AlphaPlus,
    Curio,
    Curio2,
    CraftRoboCc200,
    CraftRoboCc300,
    SilhouetteSd1,
    SilhouetteSd2,
    GraphtecCe6000_40,
    GraphtecCe6000_60,
    GraphtecCe6000_120,
    GraphtecCe7000_40,
    GraphtecCe7000_60,
    GraphtecCe7000_130,
    GraphtecCe7000_160,
    GraphtecCeLite50,
    GenericGpgl,
    GenericHpgl,
    GenericDmpl,
}

impl Model {
    pub const ALL: &'static [Model] = &[
        Model::Portrait,
        Model::Portrait2,
        Model::Portrait3,
        Model::Portrait4,
        Model::Cameo,
        Model::Cameo2,
        Model::Cameo3,
        Model::Cameo4,
        Model::Cameo4Plus,
        Model::Cameo4Pro,
        Model::CameoProMk2,
        Model::Cameo5,
        Model::Cameo5Plus,
        Model::Cameo5Alpha,
        Model::Cameo5AlphaPlus,
        Model::Curio,
        Model::Curio2,
        Model::CraftRoboCc200,
        Model::CraftRoboCc300,
        Model::SilhouetteSd1,
        Model::SilhouetteSd2,
        Model::GraphtecCe6000_40,
        Model::GraphtecCe6000_60,
        Model::GraphtecCe6000_120,
        Model::GraphtecCe7000_40,
        Model::GraphtecCe7000_60,
        Model::GraphtecCe7000_130,
        Model::GraphtecCe7000_160,
        Model::GraphtecCeLite50,
        Model::GenericGpgl,
        Model::GenericHpgl,
        Model::GenericDmpl,
    ];

    /// The registry id.
    pub fn id(self) -> &'static str {
        match self {
            Model::Portrait => "portrait",
            Model::Portrait2 => "portrait-2",
            Model::Portrait3 => "portrait-3",
            Model::Portrait4 => "portrait-4",
            Model::Cameo => "cameo",
            Model::Cameo2 => "cameo-2",
            Model::Cameo3 => "cameo-3",
            Model::Cameo4 => "cameo-4",
            Model::Cameo4Plus => "cameo-4-plus",
            Model::Cameo4Pro => "cameo-4-pro",
            Model::CameoProMk2 => "cameo-pro-mk2",
            Model::Cameo5 => "cameo-5",
            Model::Cameo5Plus => "cameo-5-plus",
            Model::Cameo5Alpha => "cameo-5-alpha",
            Model::Cameo5AlphaPlus => "cameo-5-alpha-plus",
            Model::Curio => "curio",
            Model::Curio2 => "curio-2",
            Model::CraftRoboCc200 => "craft-robo-cc200-20",
            Model::CraftRoboCc300 => "craft-robo-cc300-20",
            Model::SilhouetteSd1 => "silhouette-sd-1",
            Model::SilhouetteSd2 => "silhouette-sd-2",
            Model::GraphtecCe6000_40 => "graphtec-ce6000-40",
            Model::GraphtecCe6000_60 => "graphtec-ce6000-60",
            Model::GraphtecCe6000_120 => "graphtec-ce6000-120",
            Model::GraphtecCe7000_40 => "graphtec-ce7000-40",
            Model::GraphtecCe7000_60 => "graphtec-ce7000-60",
            Model::GraphtecCe7000_130 => "graphtec-ce7000-130",
            Model::GraphtecCe7000_160 => "graphtec-ce7000-160",
            Model::GraphtecCeLite50 => "graphtec-ce-lite-50",
            Model::GenericGpgl => "generic-gpgl",
            Model::GenericHpgl => "generic-hpgl",
            Model::GenericDmpl => "generic-dmpl",
        }
    }

    /// The model's capabilities from the built-in registry.
    pub fn caps(self) -> Option<&'static Capabilities> {
        DeviceRegistry::builtin().by_id(self.id())
    }

    /// Look a model up by its registry id, case-insensitively.
    pub fn by_id(id: &str) -> Option<Model> {
        Model::ALL
            .iter()
            .copied()
            .find(|m| m.id().eq_ignore_ascii_case(id.trim()))
    }

    /// The model for a USB vendor/product id pair.
    pub fn by_usb(vendor_id: u16, product_id: u16) -> Option<Model> {
        DeviceRegistry::builtin()
            .by_usb(vendor_id, product_id)
            .and_then(|c| Model::by_id(&c.id))
    }

    /// Identify a model from a firmware-version response (`FG`), matching
    /// `CAMEO 4 PRO V1.02`, `CAMEO 5 ALPHA V1.01`, etc. The longest
    /// registry `firmware` needle wins.
    pub fn from_version(version: &str) -> Option<Model> {
        DeviceRegistry::builtin()
            .by_firmware(version)
            .and_then(|c| Model::by_id(&c.id))
    }

    /// Cuttable width in mm (0 when undocumented).
    pub fn width_mm(self) -> f64 {
        self.caps().and_then(|c| c.cut_width_mm).unwrap_or(0.0)
    }

    /// Cuttable length in mm (0 when undocumented).
    pub fn length_mm(self) -> f64 {
        self.caps().and_then(|c| c.cut_length_mm).unwrap_or(0.0)
    }

    /// Maximum force step (0 when force is panel-only).
    pub fn max_pressure(self) -> u8 {
        self.caps()
            .and_then(|c| c.force.as_ref())
            .map_or(0, |r| u8::try_from(r.max).unwrap_or(u8::MAX))
    }

    /// Maximum speed step (0 when speed is panel-only).
    pub fn max_speed(self) -> u8 {
        self.caps()
            .and_then(|c| c.speed.as_ref())
            .map_or(0, |r| u8::try_from(r.max).unwrap_or(u8::MAX))
    }
}

/// A cutting-mat definition: device code plus usable extent in inches
/// (the extents below come from the community mat tables; the Pro mat is
/// 24 in × 24 in even though the tray takes 609.6 mm-wide material).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Mat {
    None,
    Cameo12x12,
    Cameo12x24,
    Portrait8x12,
    CameoPlus15x15,
    CameoPro24x24,
    Portrait8_5x12,
}

impl TryFrom<String> for Mat {
    type Error = String;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Mat::parse(&s).ok_or_else(|| format!("unknown mat `{s}`"))
    }
}

impl From<Mat> for String {
    fn from(m: Mat) -> String {
        m.id().to_string()
    }
}

impl Mat {
    pub const ALL: &'static [Mat] = &[
        Mat::None,
        Mat::Cameo12x12,
        Mat::Cameo12x24,
        Mat::Portrait8x12,
        Mat::CameoPlus15x15,
        Mat::CameoPro24x24,
        Mat::Portrait8_5x12,
    ];

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

    /// CLI/MCP id (`12x24`, `none`, …).
    pub fn id(self) -> &'static str {
        match self {
            Mat::None => "none",
            Mat::Cameo12x12 => "12x12",
            Mat::Cameo12x24 => "12x24",
            Mat::Portrait8x12 => "8x12",
            Mat::CameoPlus15x15 => "15x15",
            Mat::CameoPro24x24 => "24x24",
            Mat::Portrait8_5x12 => "8.5x12",
        }
    }

    pub fn parse(s: &str) -> Option<Mat> {
        let s = s.trim().to_ascii_lowercase();
        if s == "portrait8x12" {
            return Some(Mat::Portrait8x12);
        }
        Mat::ALL.iter().copied().find(|m| m.id() == s)
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

    /// Usable mat extent (down, right) in mm.
    pub fn extent_mm(self) -> (f64, f64) {
        let (d, r) = self.extent_inch();
        (d * 25.4, r * 25.4)
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

/// Tools a machine can hold. Codes 0–7 and 255 are what the Cameo 4 family's
/// tool-setup query reports; the rest are registry-only (never reported).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum ToolKind {
    Empty,
    RatchetBlade,
    AutoBlade,
    DeepCutBlade,
    KraftBlade,
    RotaryBlade,
    Pen,
    Punch,
    Emboss,
    Deboss,
    Etch,
    /// A generic swivel drag knife (sign cutters, Graphtec CE, fine-point blades).
    DragBlade,
    /// A heavy fixed/driven knife blade (balsa, chipboard).
    KnifeBlade,
    Foil,
    Score,
    Perforate,
    Engrave,
    Laser,
    Error,
}

const SELECTABLE_TOOLS: &[ToolKind] = &[
    ToolKind::RatchetBlade,
    ToolKind::AutoBlade,
    ToolKind::DeepCutBlade,
    ToolKind::KraftBlade,
    ToolKind::RotaryBlade,
    ToolKind::Pen,
    ToolKind::Punch,
    ToolKind::Emboss,
    ToolKind::Deboss,
    ToolKind::Etch,
    ToolKind::DragBlade,
    ToolKind::KnifeBlade,
    ToolKind::Foil,
    ToolKind::Score,
    ToolKind::Perforate,
    ToolKind::Engrave,
    ToolKind::Laser,
];

impl TryFrom<String> for ToolKind {
    type Error = String;
    fn try_from(s: String) -> Result<Self, String> {
        ToolKind::parse(&s).ok_or_else(|| format!("unknown tool `{s}`"))
    }
}

impl From<ToolKind> for String {
    fn from(t: ToolKind) -> String {
        t.id().to_string()
    }
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

    /// CLI/MCP/registry id.
    pub fn id(self) -> &'static str {
        match self {
            ToolKind::Empty => "empty",
            ToolKind::RatchetBlade => "ratchet",
            ToolKind::AutoBlade => "autoblade",
            ToolKind::DeepCutBlade => "deep-cut",
            ToolKind::KraftBlade => "kraft",
            ToolKind::RotaryBlade => "rotary",
            ToolKind::Pen => "pen",
            ToolKind::Punch => "punch",
            ToolKind::Emboss => "emboss",
            ToolKind::Deboss => "deboss",
            ToolKind::Etch => "etch",
            ToolKind::DragBlade => "drag-blade",
            ToolKind::KnifeBlade => "knife-blade",
            ToolKind::Foil => "foil",
            ToolKind::Score => "score",
            ToolKind::Perforate => "perforate",
            ToolKind::Engrave => "engrave",
            ToolKind::Laser => "laser",
            ToolKind::Error => "error",
        }
    }

    /// Parse a selectable tool id (not `empty`/`error`).
    pub fn parse(s: &str) -> Option<ToolKind> {
        let s = s.trim();
        SELECTABLE_TOOLS
            .iter()
            .copied()
            .find(|t| t.id().eq_ignore_ascii_case(s))
    }
}

/// Parse a tool-setup reply (`"2,7\x03"`: holder 1, holder 2 codes).
pub fn parse_tool_setup(response: &[u8]) -> Vec<Option<ToolKind>> {
    String::from_utf8_lossy(response)
        .trim_matches(|c: char| c == '\u{3}' || c.is_whitespace())
        .split(',')
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().parse::<u8>().ok().and_then(ToolKind::from_code))
        .collect()
}

/// Device status from the Silhouette status escape query.
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

/// Single-operation job settings, the shape the desktop app and the first
/// CLI use. `to_job` turns it into a vendor-neutral `CutJob`.
#[derive(Debug, Clone, PartialEq)]
pub struct CutSettings {
    /// Tool holder 1 or 2.
    pub toolholder: u8,
    /// Pen mode: no blade kerf, fork-corners disabled, raise head between paths.
    pub pen: bool,
    pub pressure: Option<u8>,
    pub speed: Option<u8>,
    /// Blade depth 0..10 (only sent for the AutoBlade, holder 1).
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
    /// Landscape media feed (default: portrait).
    pub landscape: bool,
    /// Media preset id for the legacy `FW` command.
    pub media_id: Option<u16>,
    /// The loaded tool; `None` = whatever is in the holder.
    pub tool: Option<ToolKind>,
    /// Cut each path this many times (1..=10).
    pub passes: u8,
    /// Line-segment overcut at sharp corners, mm (0 = off).
    pub overcut_mm: f64,
    pub registration: Option<Registration>,
    /// Loaded media (width across, length along the feed), mm.
    pub media_size_mm: Option<(f64, f64)>,
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
            media_id: Some(media.id),
            tool: None,
            passes: 1,
            overcut_mm: 0.0,
            registration: None,
            media_size_mm: None,
        }
    }

    /// Media defaults suited to a machine: Silhouette media presets only
    /// carry over to Silhouette protocols, a mat only to machines that take
    /// one, and panel-only machines get no force/speed at all.
    pub fn for_caps(caps: &Capabilities, media: &Media) -> CutSettings {
        let mut s = CutSettings::from_media(media);
        if !caps.protocol.is_silhouette() {
            s.media_id = None;
            s.depth = None;
            s.speed = None;
            s.pressure = None;
        }
        if caps.force.is_none() {
            s.pressure = None;
        }
        if caps.speed.is_none() {
            s.speed = None;
        }
        s.mat = caps.mats.last().copied().unwrap_or(Mat::None);
        s
    }

    pub fn for_model(model: Model, media: &Media) -> CutSettings {
        match model.caps() {
            Some(caps) => CutSettings::for_caps(caps, media),
            None => CutSettings::from_media(media),
        }
    }

    /// The equivalent one-operation `CutJob`.
    pub fn to_job(&self, paths: &[Vec<(f64, f64)>], opts: &PlotOptions, bbox_only: bool) -> CutJob {
        CutJob {
            operations: vec![Operation {
                kind: if self.pen { OpKind::Draw } else { OpKind::Cut },
                tool: self.tool,
                holder: self.toolholder,
                paths: paths.to_vec(),
                settings: OpSettings {
                    force: self.pressure.map(u32::from),
                    speed: self.speed.map(u32::from),
                    passes: self.passes,
                    depth: self.depth,
                    overcut_mm: self.overcut_mm,
                    blade_offset_mm: self.blade_diameter_mm,
                    sharpen_corners: self.sharpen_corners,
                    sharpen_start_mm: self.sharpen_start_mm,
                    sharpen_end_mm: self.sharpen_end_mm,
                },
                ..Operation::default()
            }],
            origin_down_mm: opts.margin_down_mm,
            origin_right_mm: opts.margin_right_mm,
            end_feed_mm: opts.end_paper_offset_mm,
            landscape: self.landscape,
            media_size_mm: self.media_size_mm,
            mat: self.mat,
            registration: self.registration,
            track_enhancing: self.track_enhancing,
            media_id: self.media_id,
            bbox_only,
            ..CutJob::default()
        }
    }
}

/// One device command. `to_ascii` produces the exact text (without the
/// terminator the program's `Framing` adds).
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
    /// `H` — full home (Cameo 3 and newer).
    Home,
    /// `FW<id>` — media select (legacy dialect only).
    MediaSelect(u16),
    /// `FX<p>` — force without a holder (legacy dialect).
    PressureLegacy(u8),
    /// `!<s>` — speed without a holder (legacy dialect).
    SpeedLegacy(u8),
    /// `FC<d>` — blade offset without a holder (legacy dialect).
    CutterOffsetLegacy(f64),
    /// A command built by another protocol (HP-GL, DMPL, G-code, plain
    /// GP-GL) or a fixed-text Silhouette command (`TB99`, …).
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
            } => format!("FC{},{},{}", su(*x_mm), su(*y_mm), toolholder),
            Command::Lift { lift, toolholder } => format!("FE{},{}", u8::from(*lift), toolholder),
            Command::SharpenCorners {
                start_tenths,
                end_tenths,
                toolholder,
            } => format!("FF{start_tenths},{end_tenths},{toolholder}"),
            Command::Acceleration(a) => format!("TJ{a}"),
            Command::Move { down_mm, right_mm } => format!("M{},{}", su(*down_mm), su(*right_mm)),
            Command::Draw { down_mm, right_mm } => format!("D{},{}", su(*down_mm), su(*right_mm)),
            Command::BoundaryTopLeft { down_mm, right_mm } => {
                format!("\\{},{}", su(*down_mm), su(*right_mm))
            }
            Command::BoundaryBottomRight { down_mm, right_mm } => {
                format!("Z{},{}", su(*down_mm), su(*right_mm))
            }
            Command::TrackEnhancing(on) => format!("FY{}", if *on { 0 } else { 1 }),
            Command::SelectMat(mat) => format!("TG{}", mat.code()),
            Command::Reset => "FN0".to_string(),
            Command::PortraitMode(landscape) => format!("TB50,{}", u8::from(*landscape)),
            Command::PenUp => "L0".to_string(),
            Command::Origin => "SO0".to_string(),
            Command::Home => "H".to_string(),
            Command::MediaSelect(id) => format!("FW{id}"),
            Command::PressureLegacy(p) => format!("FX{p}"),
            Command::SpeedLegacy(s) => format!("!{s}"),
            Command::CutterOffsetLegacy(d) => format!("FC{}", su(*d)),
            Command::Raw(s) => s.clone(),
        }
    }
}

/// Delimit commands into a GP-GL byte stream: each command's ASCII followed
/// by an ETX byte.
pub fn delimit(commands: &[Command]) -> Vec<u8> {
    let mut out = Vec::with_capacity(commands.len() * 8);
    for cmd in commands {
        out.extend_from_slice(cmd.to_ascii().as_bytes());
        out.push(ETX);
    }
    out
}

/// Margins and end feed for the single-operation API.
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

/// Silhouette setup preamble for a model and settings (no validation).
/// Other protocols have no separate preamble and return nothing.
pub fn encode_setup(model: Model, settings: &CutSettings) -> Vec<Command> {
    let Some(caps) = model.caps() else {
        return Vec::new();
    };
    let job = settings.to_job(&[], &PlotOptions::default(), false);
    match (caps.protocol.is_silhouette(), job.operations.first()) {
        (true, Some(op)) => protocol::gpgl_setup(caps.protocol, caps, &job, op, true),
        _ => Vec::new(),
    }
}

/// Silhouette plot section: move/draw commands with margins (or a preview
/// rectangle), then the park trailer.
pub fn encode_plot(
    paths: &[Vec<(f64, f64)>],
    opts: &PlotOptions,
    bbox_only: bool,
) -> (Vec<Command>, Option<BBoxMm>) {
    let Some(bb) = find_bbox(paths) else {
        return (Vec::new(), None);
    };
    let body = if bbox_only {
        vec![job::bbox_rect(bb)]
    } else {
        paths.to_vec()
    };
    let mut commands = protocol::gpgl_plot_body(&body, opts.margin_down_mm, opts.margin_right_mm);
    commands.extend(protocol::gpgl_park(bb, opts.end_paper_offset_mm));
    (commands, Some(bb))
}

/// Validate and encode a single-operation job for a built-in model.
pub fn encode_job(
    model: Model,
    settings: &CutSettings,
    paths: &[Vec<(f64, f64)>],
    opts: &PlotOptions,
    bbox_only: bool,
) -> Result<CutProgram, MachineError> {
    let caps = model.caps().ok_or_else(|| {
        MachineError::Config(format!("{} is missing from the registry", model.id()))
    })?;
    dry_run(caps, &settings.to_job(paths, opts, bbox_only))
}

/// Preview encoding for the desktop transcript: force, speed and passes are
/// clamped into the model's range and area is not checked. It never sends;
/// real sends go through `MachineDriver::send`, which rejects instead.
pub fn encode_cut(
    model: Model,
    settings: &CutSettings,
    paths: &[Vec<(f64, f64)>],
    opts: &PlotOptions,
    bbox_only: bool,
) -> CutProgram {
    let Some(caps) = model.caps() else {
        return CutProgram::default();
    };
    let clamp = |v: Option<u8>, r: &Option<Range>| -> Option<u8> {
        let r = r.as_ref()?;
        let (lo, hi) = (
            u8::try_from(r.min).unwrap_or(u8::MAX),
            u8::try_from(r.max).unwrap_or(u8::MAX),
        );
        v.map(|v| v.clamp(lo, hi.max(lo)))
    };
    let mut s = settings.clone();
    s.pressure = clamp(s.pressure, &caps.force);
    s.speed = clamp(s.speed, &caps.speed);
    s.passes = s.passes.clamp(1, MAX_PASSES);
    let job = s.to_job(paths, opts, bbox_only);
    let job = apply_job_settings(caps, &job).unwrap_or(job);
    protocol_for(caps.protocol).encode(caps, &job)
}

/// How a program's commands are framed on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Framing {
    /// GP-GL: each command followed by ETX.
    #[default]
    Etx,
    /// HP-GL / DMPL / G-code: commands carry their own terminators; newline between.
    Newline,
}

/// An encoded program: the commands, their framing, and where a session must
/// pause for the registration-mark search.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CutProgram {
    pub commands: Vec<Command>,
    pub bbox_mm: Option<BBoxMm>,
    pub framing: Framing,
    /// Registration: after this many commands the sender must wait for the
    /// device to report the marks found before streaming the rest.
    pub registration_wait: Option<usize>,
}

impl CutProgram {
    /// The exact byte stream for a transport to write.
    pub fn to_stream(&self) -> Vec<u8> {
        stream_of(&self.commands, self.framing)
    }

    /// One command per line, for dry-run transcripts.
    pub fn transcript(&self) -> Vec<String> {
        self.commands.iter().map(Command::to_ascii).collect()
    }
}

pub(crate) fn stream_of(commands: &[Command], framing: Framing) -> Vec<u8> {
    match framing {
        Framing::Etx => delimit(commands),
        Framing::Newline => {
            let mut out = Vec::new();
            for c in commands {
                out.extend_from_slice(c.to_ascii().as_bytes());
                out.push(b'\n');
            }
            out
        }
    }
}

/// Errors surfaced by encoders, transports and sessions.
#[derive(Debug, Clone, PartialEq)]
pub enum MachineError {
    /// Transport I/O problem (string description; no panic ever escapes).
    Transport(String),
    /// The device answered something we could not decode, or refused.
    Protocol(String),
    /// A job setting outside the machine's documented limits; nothing was sent.
    Limit(String),
    /// A bad machine table, device string, or missing build feature.
    Config(String),
    /// The protocol has no such operation (pause on HP-GL, …).
    Unsupported(String),
}

impl core::fmt::Display for MachineError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MachineError::Transport(e) => write!(f, "transport: {e}"),
            MachineError::Protocol(e) => write!(f, "protocol: {e}"),
            MachineError::Limit(e) => write!(f, "limit: {e}"),
            MachineError::Config(e) => write!(f, "config: {e}"),
            MachineError::Unsupported(e) => write!(f, "unsupported: {e}"),
        }
    }
}

impl std::error::Error for MachineError {}

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

/// The whole program stream in one write, no session.
pub fn send_program(
    transport: &mut dyn Transport,
    program: &CutProgram,
) -> Result<usize, MachineError> {
    transport.write(&program.to_stream())
}

fn read_reply(transport: &mut dyn Transport) -> Result<Vec<u8>, MachineError> {
    let mut buf = [0u8; 64];
    let n = transport.read(&mut buf)?;
    Ok(buf.get(..n).map(<[u8]>::to_vec).unwrap_or_default())
}

/// Ask a Silhouette-family device for its status (`ESC ENQ`).
pub fn query_status(transport: &mut dyn Transport) -> Result<DeviceStatus, MachineError> {
    transport.write(ESC_STATUS.as_bytes())?;
    Ok(parse_status(&read_reply(transport)?))
}

/// Ask for the firmware version string (`FG`), trimmed of ETX/padding.
pub fn query_version(transport: &mut dyn Transport) -> Result<String, MachineError> {
    transport.write(b"FG\x03")?;
    let reply = read_reply(transport)?;
    Ok(String::from_utf8_lossy(&reply)
        .trim_matches(|c: char| c == '\u{3}' || c.is_whitespace())
        .to_string())
}

/// Ask a Cameo-4-family device which tools sit in its holders (`ESC NAK`).
pub fn query_tools(transport: &mut dyn Transport) -> Result<Vec<Option<ToolKind>>, MachineError> {
    transport.write(ESC_TOOL_SETUP.as_bytes())?;
    Ok(parse_tool_setup(&read_reply(transport)?))
}

/// Run a built-in model's send session for an already-encoded program
/// (`pause` is called between status polls).
pub fn send_job(
    transport: &mut dyn Transport,
    model: Model,
    program: &CutProgram,
    pause: &mut dyn FnMut(),
) -> Result<SendReport, MachineError> {
    let caps = model.caps().ok_or_else(|| {
        MachineError::Config(format!("{} is missing from the registry", model.id()))
    })?;
    protocol_for(caps.protocol).send(transport, caps, program, &mut WaitFn(pause))
}

/// Keep the enumerated devices that are built-in models.
pub fn match_cutters(devices: impl IntoIterator<Item = UsbIds>) -> Vec<(UsbIds, Model)> {
    devices
        .into_iter()
        .filter_map(|d| Model::by_usb(d.vendor_id, d.product_id).map(|m| (d, m)))
        .collect()
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_arch;
