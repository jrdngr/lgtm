//! How statuses and field values look: theme colors by name, and each issue's mark.

use lgtm_tui_kit::theme::{ACCENT, AMBER, CYAN, FAINT, FG, FUCHSIA, GREEN, MUTED, RED};
use ratatui::style::Color;

use crate::issue::Issue;
use crate::workflow::Workflow;

/// A theme color by the name `STATUS.md` and `workflow.md` use, muted when unknown.
pub fn color(name: &str) -> Color {
    match name {
        "fg" => FG,
        "faint" => FAINT,
        "accent" => ACCENT,
        "cyan" => CYAN,
        "green" => GREEN,
        "amber" => AMBER,
        "red" => RED,
        "fuchsia" => FUCHSIA,
        _ => MUTED,
    }
}

/// The mark before an issue: amber when a question waits, `⊘` while blocked, else its
/// status's glyph and color.
pub fn mark(issue: &Issue, wf: &Workflow, all: &[Issue]) -> (String, Color) {
    if !issue.waiting().is_empty() {
        return ("●".into(), AMBER);
    }
    if !issue.open_blockers(all, |s| wf.is_done(s)).is_empty() {
        return ("⊘".into(), RED);
    }
    match wf.status(&issue.status) {
        Some(s) => (s.glyph.clone(), color(&s.color)),
        None => ("○".into(), MUTED),
    }
}
