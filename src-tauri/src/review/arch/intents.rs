//! Checking intents against the diff, and marking what they say on components and edges.

use super::group::Grouping;
use super::model::{Diff, Model};
use super::{EdgeStatus, IntentDto, IntentKind, IntentState};

/// Each intent's state, with its ids mapped through the lens's merges.
pub fn check(intents: &[(u32, IntentKind)], d: &Diff, m: &Model, g: &Grouping) -> Vec<IntentDto> {
    intents
        .iter()
        .map(|(id, kind)| IntentDto {
            id: *id,
            state: state(kind, d, m, g),
            kind: kind.clone(),
        })
        .collect()
}

fn state(kind: &IntentKind, d: &Diff, m: &Model, g: &Grouping) -> IntentState {
    match kind {
        IntentKind::Wrong { from, to, .. } => {
            match d.edge(&g.resolve(from), &g.resolve(to)).map(|e| e.status) {
                Some(EdgeStatus::Added) => IntentState::Broken,
                Some(EdgeStatus::Same) => IntentState::Open,
                Some(EdgeStatus::Removed) | None => IntentState::Met,
            }
        }
        IntentKind::Move { path, symbol, to } => {
            let to = g.resolve(to);
            if move_met(path, symbol.as_deref(), &to, m, g) {
                return IntentState::Met;
            }
            let touched = match symbol {
                Some(s) => m
                    .changed_symbols
                    .iter()
                    .any(|c| c.path == *path && c.name == *s),
                None => m
                    .changed
                    .iter()
                    .any(|c| c.path == *path || c.old_path.as_ref() == Some(path)),
            };
            if touched {
                IntentState::Broken
            } else {
                IntentState::Open
            }
        }
        IntentKind::Risk { .. } | IntentKind::Trust { .. } | IntentKind::Note { .. } => {
            IntentState::Tag
        }
    }
}

/// Whether the file, or the symbol, now sits in component `to`. A symbol that left its file
/// counts as moved when a changed file in `to` has it.
fn move_met(path: &str, symbol: Option<&str>, to: &str, m: &Model, g: &Grouping) -> bool {
    let has = |p: &str, s: &str| {
        m.symbols
            .get(p)
            .is_some_and(|names| names.iter().any(|n| n == s))
    };
    match symbol {
        Some(s) if m.head.contains_key(path) && has(path, s) => g.comp(path) == to,
        Some(s) => m
            .changed
            .iter()
            .any(|c| m.head.contains_key(&c.path) && g.comp(&c.path) == to && has(&c.path, s)),
        None => {
            // A moved file shows up as a rename away from `path`.
            let now = m
                .changed
                .iter()
                .find(|c| c.old_path.as_deref() == Some(path))
                .map_or(path, |c| c.path.as_str());
            m.head.contains_key(now) && g.comp(now) == to
        }
    }
}

/// Marks tags, notes, misplaced items and wrong edges on the diff.
pub fn apply(intents: &[IntentDto], d: &mut Diff, m: &Model, g: &Grouping) {
    for i in intents {
        match &i.kind {
            IntentKind::Wrong { from, to, .. } => {
                let (from, to) = (g.resolve(from), g.resolve(to));
                for e in d.edges.iter_mut().filter(|e| e.from == from && e.to == to) {
                    e.wrong = Some(i.id);
                }
            }
            IntentKind::Move { path, symbol, .. } if i.state != IntentState::Met => {
                if !m.head.contains_key(path) {
                    continue;
                }
                let item = match symbol {
                    Some(s) => format!("{path}#{s}"),
                    None => path.clone(),
                };
                let comp = g.comp(path);
                if let Some(c) = d.components.iter_mut().find(|c| c.id == comp) {
                    c.misplaced.push(item);
                }
            }
            IntentKind::Move { .. } => {}
            IntentKind::Risk { component }
            | IntentKind::Trust { component }
            | IntentKind::Note { component, .. } => {
                let id = g.resolve(component);
                let Some(c) = d.components.iter_mut().find(|c| c.id == id) else {
                    continue;
                };
                match &i.kind {
                    IntentKind::Risk { .. } => c.risk = true,
                    IntentKind::Trust { .. } => c.trust = true,
                    IntentKind::Note { text, .. } => {
                        c.note = Some(match c.note.take() {
                            Some(prev) => format!("{prev}\n\n{text}"),
                            None => text.clone(),
                        })
                    }
                    _ => {}
                }
            }
        }
    }
}
