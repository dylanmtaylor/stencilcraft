//! The built-in operation-preset catalogue (D-17): Cut, Engrave, Score, Guide.
//! Presets carry `OperationKind`s and default settings; the hammer turns one
//! into a first-class `Document.operations` entry through `OperationCreate`.

use craft_ui::icons::Icon;
use stencilcraft_doc::OperationKind;

/// One built-in operation preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationPreset {
    Cut,
    Engrave,
    Score,
    Guide,
}

impl OperationPreset {
    /// Every built-in preset, in catalogue order.
    pub const ALL: [Self; 4] = [Self::Cut, Self::Engrave, Self::Score, Self::Guide];

    pub fn label(self) -> &'static str {
        match self {
            Self::Cut => "Cut",
            Self::Engrave => "Engrave",
            Self::Score => "Score",
            Self::Guide => "Guide",
        }
    }

    pub fn icon(self) -> Icon {
        match self {
            Self::Cut => Icon::Scissors,
            Self::Engrave => Icon::Flame,
            Self::Score => Icon::Spline,
            Self::Guide => Icon::Ruler,
        }
    }

    /// The first-class operation kind a new operation gets from this preset.
    pub fn kind(self) -> OperationKind {
        match self {
            Self::Cut => OperationKind::Cut,
            Self::Engrave => OperationKind::Engrave,
            Self::Score => OperationKind::Score,
            Self::Guide => OperationKind::Guide,
        }
    }

    /// The icon name stored on the created operation (`Operation.icon`).
    pub fn icon_name(self) -> &'static str {
        match self {
            Self::Cut => "scissors",
            Self::Engrave => "flame",
            Self::Score => "spline",
            Self::Guide => "ruler",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_is_cut_engrave_score_guide_with_matching_kinds() {
        assert_eq!(
            OperationPreset::ALL,
            [
                OperationPreset::Cut,
                OperationPreset::Engrave,
                OperationPreset::Score,
                OperationPreset::Guide
            ]
        );
        assert_eq!(OperationPreset::Cut.kind(), OperationKind::Cut);
        assert_eq!(OperationPreset::Engrave.kind(), OperationKind::Engrave);
        assert_eq!(OperationPreset::Score.kind(), OperationKind::Score);
        assert_eq!(OperationPreset::Guide.kind(), OperationKind::Guide);
        for preset in OperationPreset::ALL {
            assert_eq!(
                preset.kind().is_guide(),
                preset == OperationPreset::Guide,
                "Guide is canvas-only"
            );
            assert!(!preset.label().is_empty());
            assert!(!preset.icon_name().is_empty());
        }
    }
}
