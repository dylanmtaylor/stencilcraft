//! Machine profiles: the mats each cutter takes, its roll (mat-free) limits and its Print Then
//! Cut limits. Figures come from the vendors' public documentation summarised in
//! `docs/cricut-design-space/materials-and-tools.md`, `print-then-cut.md` and
//! `docs/reference-apps.md` (Cutters); values marked "to verify" there are our best reading.

use serde::Serialize;

const IN: f64 = 25.4;

/// One mat size: its name and its largest cut area (not the mat's outer size).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MatSize {
    pub name: &'static str,
    pub width_mm: f64,
    pub length_mm: f64,
}

/// Mat-free (roll / Smart Materials) feeding limits.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Roll {
    pub width_mm: f64,
    /// Longest single cut.
    pub max_length_mm: f64,
    /// Shortest piece of material the rollers can hold.
    pub min_length_mm: f64,
    /// Space the rollers need before the design (leading edge) and after it.
    pub lead_mm: f64,
    pub trail_mm: f64,
}

/// Registration mark layouts for Print Then Cut. The geometry is our own design.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MarkStyle {
    /// A solid square outside the top-left corner and L brackets at top-right and bottom-left.
    #[default]
    Silhouette,
    /// A solid square outside the top-left corner and L brackets at the other three corners.
    Cricut,
    /// L brackets at all four corners (four-mark registration).
    FourCorner,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MachineProfile {
    pub id: &'static str,
    pub name: &'static str,
    pub mats: &'static [MatSize],
    pub roll: Option<Roll>,
    /// Largest Print Then Cut image (width × height), where documented.
    pub print_max_mm: Option<(f64, f64)>,
    pub marks: MarkStyle,
}

const CRICUT_MATS: &[MatSize] = &[
    MatSize {
        name: "12x12",
        width_mm: 11.5 * IN,
        length_mm: 11.5 * IN,
    },
    MatSize {
        name: "12x24",
        width_mm: 11.5 * IN,
        length_mm: 23.5 * IN,
    },
];
const JOY_MATS: &[MatSize] = &[
    MatSize {
        name: "4.5x6.5",
        width_mm: 4.25 * IN,
        length_mm: 6.25 * IN,
    },
    MatSize {
        name: "4.5x12",
        width_mm: 4.25 * IN,
        length_mm: 11.75 * IN,
    },
];
const CAMEO_MATS: &[MatSize] = &[
    MatSize {
        name: "12x12",
        width_mm: 12.0 * IN,
        length_mm: 12.0 * IN,
    },
    MatSize {
        name: "12x24",
        width_mm: 12.0 * IN,
        length_mm: 24.0 * IN,
    },
];

const SILHOUETTE_ROLL_MM: f64 = 3000.0;

/// Every machine profile, by id.
pub const MACHINES: &[MachineProfile] = &[
    MachineProfile {
        id: "cricut-maker",
        name: "Cricut Maker series",
        mats: CRICUT_MATS,
        roll: Some(Roll {
            width_mm: 11.7 * IN,
            max_length_mm: 144.0 * IN,
            min_length_mm: 6.0 * IN,
            lead_mm: 0.0,
            trail_mm: 0.0,
        }),
        print_max_mm: Some((10.62 * IN, 7.2 * IN)),
        marks: MarkStyle::Cricut,
    },
    MachineProfile {
        id: "cricut-explore",
        name: "Cricut Explore series",
        mats: CRICUT_MATS,
        roll: Some(Roll {
            width_mm: 11.7 * IN,
            max_length_mm: 144.0 * IN,
            min_length_mm: 6.0 * IN,
            lead_mm: 0.0,
            trail_mm: 0.0,
        }),
        // Not documented separately; the Maker figure is the nearest public one.
        print_max_mm: Some((10.62 * IN, 7.2 * IN)),
        marks: MarkStyle::Cricut,
    },
    MachineProfile {
        id: "cricut-joy",
        name: "Cricut Joy / Joy 2",
        mats: JOY_MATS,
        roll: Some(Roll {
            width_mm: 4.5 * IN,
            max_length_mm: 48.0 * IN,
            min_length_mm: 4.0 * IN,
            lead_mm: 1.0 * IN,
            trail_mm: 0.5 * IN,
        }),
        // Joy 2 (the first Joy has no Print Then Cut).
        print_max_mm: Some((10.62 * IN, 3.07 * IN)),
        marks: MarkStyle::Cricut,
    },
    MachineProfile {
        id: "cricut-joy-xtra",
        name: "Cricut Joy Xtra",
        mats: &[],
        roll: Some(Roll {
            width_mm: 8.5 * IN,
            max_length_mm: 48.0 * IN,
            min_length_mm: 6.0 * IN,
            lead_mm: 0.0,
            trail_mm: 0.0,
        }),
        print_max_mm: None,
        marks: MarkStyle::Cricut,
    },
    MachineProfile {
        id: "cricut-venture",
        name: "Cricut Venture",
        mats: &[],
        roll: Some(Roll {
            width_mm: 24.0 * IN,
            max_length_mm: 144.0 * IN,
            min_length_mm: 6.0 * IN,
            lead_mm: 0.0,
            trail_mm: 0.0,
        }),
        print_max_mm: Some((24.0 * IN, 28.0 * IN)),
        marks: MarkStyle::Cricut,
    },
    MachineProfile {
        id: "silhouette-cameo",
        name: "Silhouette CAMEO 3/4/5",
        mats: CAMEO_MATS,
        roll: Some(Roll {
            width_mm: 304.8,
            max_length_mm: SILHOUETTE_ROLL_MM,
            min_length_mm: 0.0,
            lead_mm: 0.0,
            trail_mm: 0.0,
        }),
        print_max_mm: None,
        marks: MarkStyle::Silhouette,
    },
    MachineProfile {
        id: "silhouette-cameo-plus",
        name: "Silhouette CAMEO 4 Plus / 5 Plus",
        mats: &[MatSize {
            name: "15x15",
            width_mm: 15.0 * IN,
            length_mm: 15.0 * IN,
        }],
        roll: Some(Roll {
            width_mm: 372.0,
            max_length_mm: SILHOUETTE_ROLL_MM,
            min_length_mm: 0.0,
            lead_mm: 0.0,
            trail_mm: 0.0,
        }),
        print_max_mm: None,
        marks: MarkStyle::Silhouette,
    },
    MachineProfile {
        id: "silhouette-cameo-pro",
        name: "Silhouette CAMEO 4 Pro",
        mats: &[MatSize {
            name: "24x24",
            width_mm: 24.0 * IN,
            length_mm: 24.0 * IN,
        }],
        roll: Some(Roll {
            width_mm: 600.0,
            max_length_mm: SILHOUETTE_ROLL_MM,
            min_length_mm: 0.0,
            lead_mm: 0.0,
            trail_mm: 0.0,
        }),
        print_max_mm: None,
        marks: MarkStyle::FourCorner,
    },
    MachineProfile {
        id: "silhouette-portrait",
        name: "Silhouette Portrait 3/4",
        mats: &[MatSize {
            name: "8x12",
            width_mm: 8.0 * IN,
            length_mm: 12.0 * IN,
        }],
        roll: Some(Roll {
            width_mm: 203.0,
            max_length_mm: SILHOUETTE_ROLL_MM,
            min_length_mm: 0.0,
            lead_mm: 0.0,
            trail_mm: 0.0,
        }),
        print_max_mm: None,
        marks: MarkStyle::Silhouette,
    },
];

pub fn machine(id: &str) -> Option<&'static MachineProfile> {
    MACHINES.iter().find(|m| m.id == id)
}
