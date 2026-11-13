//! The jj menu on the revisions side: Space, then a motion's keys. Some ask
//! for a name or a description first, Set bookmark picks one of the nearest bookmarks, and a
//! push that would create bookmarks on the remote asks first.

use lgtm_core::jj::{JjAction, PushPlan};
use ratatui::crossterm::event::{KeyCode, KeyEvent};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    NewAfter,
    NewBefore,
    Edit,
    Describe,
    Squash,
    BookmarkAdvance,
    BookmarkCreate,
    BookmarkSet,
    Undo,
    Redo,
    Fetch,
    Push,
    Rebase,
}

/// The motions, by their keys, in groups.
const MOTIONS: &[&[(&str, &str, Kind)]] = &[
    &[
        ("na", "New after", Kind::NewAfter),
        ("nb", "New before", Kind::NewBefore),
        ("e", "Edit", Kind::Edit),
    ],
    &[("d", "Describe", Kind::Describe)],
    &[("s", "Squash into parent", Kind::Squash)],
    &[
        ("ba", "Advance bookmark", Kind::BookmarkAdvance),
        ("bc", "Create bookmark", Kind::BookmarkCreate),
        ("bs", "Set bookmark", Kind::BookmarkSet),
    ],
    &[("u", "Undo", Kind::Undo), ("U", "Redo", Kind::Redo)],
    &[
        ("gf", "Fetch", Kind::Fetch),
        ("gp", "Push", Kind::Push),
        ("gr", "Rebase onto trunk", Kind::Rebase),
    ],
];

/// What the motions act on: the cursor's commit and the picked range, as revsets, and the
/// cursor commit's description.
#[derive(Clone, Debug, Default)]
pub struct Target {
    pub cursor: String,
    pub picked: String,
    pub description: String,
}

enum State {
    /// The keys typed so far.
    Keys(String),
    /// Typing a new bookmark's name, or the cursor commit's description.
    Ask(Kind, String),
    /// Waiting for the nearest bookmarks.
    Loading,
    /// Picking one of them by its number.
    Pick(Vec<String>),
    /// Waiting for what a push would do.
    Planning,
    /// Asking whether to push new bookmarks.
    Confirm(PushPlan),
}

pub struct Menu {
    target: Target,
    state: State,
}

/// What a key in the menu asks for.
#[derive(Debug, PartialEq)]
pub enum Step {
    Stay,
    Close,
    Run(JjAction),
    /// Load the bookmarks nearest the cursor's commit.
    Bookmarks(String),
    /// Find what a push of these commits' bookmarks would do, and which it would create on
    /// the remote.
    PushPlan(String),
}

/// A description's first line, which Describe edits.
fn title(description: &str) -> &str {
    description.lines().next().unwrap_or("")
}

impl Menu {
    pub fn new(target: Target) -> Self {
        Self {
            target,
            state: State::Keys(String::new()),
        }
    }

    /// Whether no motion is under way: no keys typed, nothing asked or loading.
    pub fn idle(&self) -> bool {
        matches!(&self.state, State::Keys(typed) if typed.is_empty())
    }

    fn action(&self, kind: Kind, text: &str) -> JjAction {
        let t = &self.target;
        let rev = t.cursor.clone();
        match kind {
            Kind::NewAfter => JjAction::NewAfter { rev },
            Kind::NewBefore => JjAction::NewBefore { rev },
            Kind::Edit => JjAction::Edit { rev },
            Kind::Describe => {
                // The typed line replaces the first; the body stays.
                let body = t.description.find('\n').map_or("", |i| &t.description[i..]);
                JjAction::Describe {
                    rev,
                    message: format!("{text}{body}"),
                }
            }
            Kind::Squash => JjAction::Squash {
                revs: t.picked.clone(),
            },
            Kind::BookmarkAdvance => JjAction::BookmarkAdvance,
            Kind::BookmarkCreate | Kind::BookmarkSet => {
                let name = text.to_string();
                if kind == Kind::BookmarkCreate {
                    JjAction::BookmarkCreate { name, rev }
                } else {
                    JjAction::BookmarkSet { name, rev }
                }
            }
            Kind::Undo => JjAction::Undo { steps: 1 },
            Kind::Redo => JjAction::Redo { steps: 1 },
            Kind::Fetch => JjAction::Fetch,
            Kind::Push => JjAction::Push {
                remote: None,
                range: None,
                new_bookmarks: vec![],
            },
            Kind::Rebase => JjAction::RebaseOnTrunk { rev },
        }
    }

    /// Starts the motion: runs it, or first asks for what it needs.
    fn start(&mut self, kind: Kind) -> Step {
        match kind {
            Kind::Describe => {
                let text = title(&self.target.description).to_string();
                self.state = State::Ask(kind, text);
                Step::Stay
            }
            Kind::BookmarkCreate => {
                self.state = State::Ask(kind, String::new());
                Step::Stay
            }
            Kind::BookmarkSet => {
                self.state = State::Loading;
                Step::Bookmarks(self.target.cursor.clone())
            }
            Kind::Push => {
                self.state = State::Planning;
                Step::PushPlan(self.target.picked.clone())
            }
            _ => Step::Run(self.action(kind, "")),
        }
    }

    pub fn key(&mut self, key: KeyEvent) -> Step {
        if key.code == KeyCode::Esc {
            return Step::Close;
        }
        match &mut self.state {
            State::Keys(typed) => {
                let KeyCode::Char(c) = key.code else {
                    return Step::Close;
                };
                typed.push(c);
                let all = MOTIONS.iter().flat_map(|g| g.iter());
                if let Some(&(_, _, kind)) = all.clone().find(|(k, ..)| k == typed) {
                    return self.start(kind);
                }
                if all.clone().any(|(k, ..)| k.starts_with(typed.as_str())) {
                    Step::Stay
                } else {
                    Step::Close
                }
            }
            State::Ask(kind, text) => match key.code {
                KeyCode::Enter if text.trim().is_empty() && *kind != Kind::Describe => Step::Stay,
                KeyCode::Enter => {
                    let (kind, text) = (*kind, text.trim().to_string());
                    Step::Run(self.action(kind, &text))
                }
                KeyCode::Backspace => {
                    text.pop();
                    Step::Stay
                }
                KeyCode::Char(c) => {
                    text.push(c);
                    Step::Stay
                }
                _ => Step::Stay,
            },
            State::Loading | State::Planning => Step::Stay,
            State::Confirm(plan) => match key.code {
                KeyCode::Char('y') => Step::Run(push(plan)),
                _ => Step::Close,
            },
            State::Pick(names) => {
                let picked = match key.code {
                    KeyCode::Char(c) => c
                        .to_digit(10)
                        .and_then(|n| names.get((n as usize).checked_sub(1)?)),
                    _ => None,
                };
                match picked {
                    Some(name) => {
                        let name = name.clone();
                        Step::Run(self.action(Kind::BookmarkSet, &name))
                    }
                    None => Step::Stay,
                }
            }
        }
    }

    /// Takes the bookmarks nearest the cursor: sets the only one, or asks which.
    pub fn bookmarks(&mut self, names: Vec<String>) -> Step {
        match names.as_slice() {
            [] => Step::Close,
            [one] => Step::Run(self.action(Kind::BookmarkSet, one)),
            _ => {
                self.state = State::Pick(names.into_iter().take(9).collect());
                Step::Stay
            }
        }
    }

    /// Takes what a push would do: pushes when it makes no new bookmarks, else asks.
    pub fn push_plan(&mut self, plan: PushPlan) -> Step {
        if plan.new_bookmarks.is_empty() {
            return Step::Run(push(&plan));
        }
        self.state = State::Confirm(plan);
        Step::Stay
    }

    /// The bottom line while typing or asking.
    pub fn prompt(&self) -> Option<String> {
        match &self.state {
            State::Confirm(plan) => {
                let noun = if plan.new_bookmarks.len() == 1 {
                    "bookmark"
                } else {
                    "bookmarks"
                };
                let names = plan.new_bookmarks.join(", ");
                Some(format!("Push new {noun} {names} to {}? y/n", plan.remote))
            }
            State::Ask(Kind::Describe, text) => Some(format!("Describe: {text}")),
            State::Ask(_, text) => Some(format!("Bookmark name: {text}")),
            _ => None,
        }
    }

    /// The menu's lines: the motions the typed keys could still finish, a blank line between
    /// groups, or the bookmarks to pick from.
    pub fn items(&self) -> Vec<(String, &'static str)> {
        match &self.state {
            State::Keys(typed) => {
                let mut out = Vec::new();
                for group in MOTIONS {
                    let shown: Vec<_> = group
                        .iter()
                        .filter(|(k, ..)| k.starts_with(typed.as_str()))
                        .map(|&(k, label, _)| (k.to_string(), label))
                        .collect();
                    if shown.is_empty() {
                        continue;
                    }
                    if !out.is_empty() {
                        out.push((String::new(), ""));
                    }
                    out.extend(shown);
                }
                out
            }
            State::Loading => vec![(String::new(), "Loading bookmarks")],
            State::Planning => vec![(String::new(), "Checking the push")],
            State::Pick(names) => names
                .iter()
                .enumerate()
                .map(|(i, n)| (format!("{} {n}", i + 1), "Set bookmark"))
                .collect(),
            State::Ask(..) | State::Confirm(_) => vec![],
        }
    }
}

/// The push `plan` describes.
fn push(plan: &PushPlan) -> JjAction {
    JjAction::Push {
        remote: Some(plan.remote.clone()),
        range: Some(plan.range.clone()),
        new_bookmarks: plan.new_bookmarks.clone(),
    }
}

/// What lgtm shows while `action` runs.
pub fn doing(action: &JjAction) -> &'static str {
    match action {
        JjAction::Edit { .. } => "editing",
        JjAction::Undo { .. } => "undoing",
        JjAction::Redo { .. } => "redoing",
        JjAction::NewAfter { .. } | JjAction::NewBefore { .. } => "making a change",
        JjAction::Discard { .. } => "discarding",
        JjAction::Squash { .. } => "squashing",
        JjAction::RebaseOnTrunk { .. } => "rebasing",
        JjAction::BookmarkAdvance
        | JjAction::BookmarkCreate { .. }
        | JjAction::BookmarkSet { .. } => "moving bookmarks",
        JjAction::Describe { .. } => "describing",
        JjAction::Fetch => "fetching",
        JjAction::Push { .. } => "pushing",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn menu() -> Menu {
        Menu::new(Target {
            cursor: "abc".into(),
            picked: "abc::def".into(),
            description: "old title\n\nbody".into(),
        })
    }

    fn keys(m: &mut Menu, keys: &str) -> Step {
        let mut last = Step::Stay;
        for k in crate::edit::spec::keys(keys) {
            last = m.key(k);
        }
        last
    }

    #[test]
    fn keys_run_motions_on_the_cursor_or_the_picked_range() {
        assert_eq!(
            keys(&mut menu(), "na"),
            Step::Run(JjAction::NewAfter { rev: "abc".into() })
        );
        assert_eq!(
            keys(&mut menu(), "s"),
            Step::Run(JjAction::Squash {
                revs: "abc::def".into()
            })
        );
        assert_eq!(
            keys(&mut menu(), "e"),
            Step::Run(JjAction::Edit { rev: "abc".into() })
        );
        assert_eq!(keys(&mut menu(), "n"), Step::Stay);
        assert_eq!(keys(&mut menu(), "x"), Step::Close);
    }

    #[test]
    fn describe_edits_the_first_line_and_keeps_the_body() {
        let mut m = menu();
        keys(&mut m, "d");
        assert_eq!(m.prompt().as_deref(), Some("Describe: old title"));
        let step = keys(&mut m, "<bs><bs><bs><bs><bs>name<ret>");
        assert_eq!(
            step,
            Step::Run(JjAction::Describe {
                rev: "abc".into(),
                message: "old name\n\nbody".into()
            })
        );
    }

    #[test]
    fn a_push_making_new_bookmarks_names_them_once_confirmed() {
        let plan = |names: &[&str]| PushPlan {
            remote: "origin".into(),
            range: "r".into(),
            new_bookmarks: names.iter().map(|n| n.to_string()).collect(),
        };
        let mut m = menu();
        assert_eq!(keys(&mut m, "gp"), Step::PushPlan("abc::def".into()));
        assert_eq!(
            m.push_plan(plan(&[])),
            Step::Run(JjAction::Push {
                remote: Some("origin".into()),
                range: Some("r".into()),
                new_bookmarks: vec![]
            })
        );
        let mut m = menu();
        keys(&mut m, "gp");
        assert_eq!(m.push_plan(plan(&["feat"])), Step::Stay);
        assert_eq!(
            m.prompt().as_deref(),
            Some("Push new bookmark feat to origin? y/n")
        );
        assert_eq!(
            keys(&mut m, "y"),
            Step::Run(JjAction::Push {
                remote: Some("origin".into()),
                range: Some("r".into()),
                new_bookmarks: vec!["feat".into()]
            })
        );
        let mut m = menu();
        keys(&mut m, "gp");
        m.push_plan(plan(&["feat"]));
        assert_eq!(keys(&mut m, "n"), Step::Close);
    }

    #[test]
    fn set_bookmark_sets_the_only_nearest_one_or_picks_by_number() {
        let mut m = menu();
        assert_eq!(keys(&mut m, "bs"), Step::Bookmarks("abc".into()));
        let set = |name: &str| {
            Step::Run(JjAction::BookmarkSet {
                name: name.into(),
                rev: "abc".into(),
            })
        };
        assert_eq!(m.bookmarks(vec!["main".into()]), set("main"));
        let mut m = menu();
        keys(&mut m, "bs");
        assert_eq!(m.bookmarks(vec!["a".into(), "b".into()]), Step::Stay);
        assert_eq!(keys(&mut m, "3"), Step::Stay);
        assert_eq!(keys(&mut m, "2"), set("b"));
    }
}
