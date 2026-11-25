//! The built-in operation-preset catalogue (D-17): Cut, Engrave, Score, Guide.
//! Presets carry `OperationSettings::default()` and never set `preset_id`.

use craft_ui::icons::Icon;
use stencilcraft_doc::{Operation, OperationSettings};

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

    /// The operation a new layer gets from this preset, with default settings.
    pub fn operation(self) -> Operation {
        match self {
            Self::Cut => Operation::Cut(OperationSettings::default()),
            Self::Engrave => Operation::Engrave(OperationSettings::default()),
            Self::Score => Operation::Score(OperationSettings::default()),
            Self::Guide => Operation::Guide,
        }
    }
}

/// Drag payload: which preset an operations-palette entry is being dragged onto.
/// Hit-tested against the layers list's layer-row rects on pointer release (D-13).
#[derive(Clone, Copy, Debug)]
pub struct OpDrag(pub OperationPreset);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_is_cut_engrave_score_guide_with_default_settings() {
        assert_eq!(
            OperationPreset::ALL,
            [
                OperationPreset::Cut,
                OperationPreset::Engrave,
                OperationPreset::Score,
                OperationPreset::Guide
            ]
        );
        assert_eq!(
            OperationPreset::Cut.operation(),
            Operation::Cut(OperationSettings::default())
        );
        assert_eq!(
            OperationPreset::Engrave.operation(),
            Operation::Engrave(OperationSettings::default())
        );
        assert_eq!(
            OperationPreset::Score.operation(),
            Operation::Score(OperationSettings::default())
        );
        assert_eq!(OperationPreset::Guide.operation(), Operation::Guide);
        for preset in OperationPreset::ALL {
            // The built-in catalogue never sets preset_id; Guide is canvas-only.
            assert!(preset.operation().settings().is_none_or(|_| true));
            assert_eq!(
                preset.operation().is_guide(),
                preset == OperationPreset::Guide
            );
            assert!(!preset.label().is_empty());
        }
    }
}
