//! What a machine can do: the vendor-neutral `Capabilities` model, loaded
//! from `machines.toml`. A `None`/absent value means *not publicly
//! documented*; validation treats it as "refuse", never as "guess".

use serde::{Deserialize, Serialize};

/// Which wire protocol (and, for Silhouette, which generation of it) a
/// machine speaks. Selects the `Protocol` implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProtocolId {
    /// Pre-Cameo-3 Silhouette/Craft Robo GP-GL: `FW` media, holder-less `FX`/`!`.
    SilhouetteLegacy,
    /// Cameo 3: tool-holder-addressed commands, mat select, AutoBlade depth.
    SilhouetteCameo3,
    /// Cameo 4 line (incl. Cameo 5, Portrait 3/4): doubled force/offset preamble.
    SilhouetteCameo4,
    /// Plain Graphtec GP-GL (`M`/`D` at 0.1 mm steps, ETX-terminated).
    Gpgl,
    /// HP-GL / HP-GL/2 subset (`IN;SP1;PU…;PD…;`, 40 units per mm).
    Hpgl,
    /// Houston Instrument DMPL (`;:` … `@`, 0.001 in units). Unverified.
    Dmpl,
    /// Grbl-style G-code (lasers, knife/pen CNCs) over USB-CDC serial.
    Gcode,
    /// No documented protocol: export SVG/DXF and use the vendor's software.
    ExportOnly,
    /// A driver slot whose protocol is deliberately not implemented (Cricut).
    Unsupported,
}

/// Older name kept for callers written against the first catalog.
pub type Dialect = ProtocolId;

impl ProtocolId {
    pub fn id(self) -> &'static str {
        match self {
            ProtocolId::SilhouetteLegacy => "silhouette-legacy",
            ProtocolId::SilhouetteCameo3 => "silhouette-cameo3",
            ProtocolId::SilhouetteCameo4 => "silhouette-cameo4",
            ProtocolId::Gpgl => "gpgl",
            ProtocolId::Hpgl => "hpgl",
            ProtocolId::Dmpl => "dmpl",
            ProtocolId::Gcode => "gcode",
            ProtocolId::ExportOnly => "export-only",
            ProtocolId::Unsupported => "unsupported",
        }
    }

    pub fn is_silhouette(self) -> bool {
        matches!(
            self,
            ProtocolId::SilhouetteLegacy
                | ProtocolId::SilhouetteCameo3
                | ProtocolId::SilhouetteCameo4
        )
    }
}

/// How far StencilCraft supports a machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Support {
    #[default]
    Supported,
    Experimental,
    ExportOnly,
    Unsupported,
}

/// Inclusive range the encoder may send, with its unit (`steps`, `mm/min`, …).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Range {
    pub min: u32,
    pub max: u32,
    #[serde(default)]
    pub unit: String,
}

impl Range {
    pub fn contains(&self, v: u32) -> bool {
        (self.min..=self.max).contains(&v)
    }
}

/// Everything known about one machine (one `[[machine]]` in `machines.toml`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    /// Stable CLI/MCP id, e.g. `cameo-4-pro`.
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub vendor: String,
    pub protocol: ProtocolId,
    #[serde(default)]
    pub support: Support,
    /// USB `vvvv:pppp` (hex); see `usb_ids`.
    pub usb: Option<String>,
    /// Substring of the firmware-version reply that identifies the model.
    pub firmware: Option<String>,
    /// Cuttable width (cross axis) and length (feed axis), mm.
    pub cut_width_mm: Option<f64>,
    pub cut_length_mm: Option<f64>,
    /// Widest media the machine loads, mm (when published separately).
    pub media_width_mm: Option<f64>,
    /// Cuts from a roll / without a carrier mat.
    #[serde(default)]
    pub roll_feed: bool,
    /// Mats the machine accepts besides "no mat".
    #[serde(default)]
    pub mats: Vec<crate::Mat>,
    pub tool_holders: Option<u8>,
    #[serde(default)]
    pub tools: Vec<crate::ToolKind>,
    /// Published maximum blade force, grams-force.
    pub max_force_gf: Option<u32>,
    /// Force (or laser power) the encoder may send; absent = panel only.
    pub force: Option<Range>,
    /// Speed the encoder may send; absent = panel only.
    pub speed: Option<Range>,
    pub bluetooth: Option<bool>,
    /// Optical registration-mark reading, and the four-mark variant.
    #[serde(default)]
    pub regmarks: bool,
    #[serde(default)]
    pub quad_regmarks: bool,
    /// Encoder-side features; on unless a row says otherwise.
    #[serde(default = "yes")]
    pub overcut: bool,
    #[serde(default = "yes")]
    pub multi_pass: bool,
    #[serde(default)]
    pub notes: String,
}

fn yes() -> bool {
    true
}

impl Capabilities {
    /// Parsed USB vendor/product id.
    pub fn usb_ids(&self) -> Option<(u16, u16)> {
        let (v, p) = self.usb.as_deref()?.split_once(':')?;
        Some((
            u16::from_str_radix(v.trim(), 16).ok()?,
            u16::from_str_radix(p.trim(), 16).ok()?,
        ))
    }

    /// Whether a job may be encoded for this machine at all.
    pub fn encodable(&self) -> bool {
        !matches!(
            self.protocol,
            ProtocolId::ExportOnly | ProtocolId::Unsupported
        ) && !matches!(self.support, Support::ExportOnly | Support::Unsupported)
    }
}
