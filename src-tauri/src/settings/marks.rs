//! What each mark color means: its label, whether it counts as done, and whether it's one of
//! the colors in use. The order of the colors in use sets their number keys.
//!
//! ```toml
//! [[marks]]
//! color = "cyan"
//! label = "reviewed"
//! done = true
//! shown = true
//! ```

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::store::MarkColor;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MarkSlot {
    pub color: MarkColor,
    /// Empty for a color with no label.
    pub label: String,
    /// Rows with this mark count toward progress.
    pub done: bool,
    /// In the review header and on a number key. Hidden colors keep their marks.
    pub shown: bool,
}

fn slot(color: MarkColor, label: &str, done: bool, shown: bool) -> MarkSlot {
    MarkSlot {
        color,
        label: label.into(),
        done,
        shown,
    }
}

/// Reviewed, flagged and skipped in use; the other colors unlabeled and hidden.
pub fn defaults() -> Vec<MarkSlot> {
    let mut slots = vec![
        slot(MarkColor::Cyan, "reviewed", true, true),
        slot(MarkColor::Red, "flagged", false, true),
        slot(MarkColor::Gray, "skipped", true, true),
    ];
    normalize(&mut slots);
    slots
}

/// Keeps one slot per color, adding any missing color hidden at the end.
pub fn normalize(slots: &mut Vec<MarkSlot>) {
    let mut seen = Vec::new();
    slots.retain(|s| {
        let first = !seen.contains(&s.color);
        seen.push(s.color);
        first
    });
    for color in MarkColor::ALL {
        if !seen.contains(&color) {
            slots.push(slot(color, "", true, false));
        }
    }
}

/// Whether rows with `color` count as done.
pub fn is_done(slots: &[MarkSlot], color: MarkColor) -> bool {
    slots
        .iter()
        .find(|s| s.color == color)
        .is_none_or(|s| s.done)
}

/// The label a color goes by, or its name when it has none.
pub fn label(slots: &[MarkSlot], color: MarkColor) -> String {
    slots
        .iter()
        .find(|s| s.color == color && !s.label.trim().is_empty())
        .map_or_else(|| color.name(), |s| s.label.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_drops_repeats_and_appends_missing_colors_hidden() {
        let mut slots = vec![
            slot(MarkColor::Tan, "later", false, true),
            slot(MarkColor::Tan, "again", true, true),
        ];
        normalize(&mut slots);
        assert_eq!(slots.len(), MarkColor::ALL.len());
        assert_eq!(slots[0], slot(MarkColor::Tan, "later", false, true));
        assert_eq!(slots[1], slot(MarkColor::Cyan, "", true, false));
        assert!(!is_done(&slots, MarkColor::Tan));
        assert_eq!(label(&slots, MarkColor::Tan), "later");
        assert_eq!(label(&slots, MarkColor::Cyan), "cyan");
    }
}
