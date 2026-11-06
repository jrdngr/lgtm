//! A session as the app and the terminal draw it.

use serde::{Deserialize, Serialize};
use specta::Type;

use super::parse::{Label, Part};
use super::session::{Fate, FileChoice, Session};
use super::tint::{Tint, tints};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SideView {
    /// 0 for the base, else the side's number.
    pub key: u8,
    pub label: Label,
    pub text: String,
    /// The words it changes from the base.
    pub tints: Vec<Tint>,
    /// Whether it has an edit, and whether it shows the text from before it.
    pub edited: bool,
    pub original: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConflictView {
    pub index: u32,
    pub resolved: bool,
    /// The file line, from 1, its text starts on once saved.
    pub line: u32,
    /// The sides it keeps, in order.
    pub picks: Vec<SideView>,
    /// The sides it doesn't, numbered sides first and the base last.
    pub rest: Vec<SideView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PartView {
    Text { line: u32, text: String },
    Conflict(ConflictView),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ResolveView {
    pub path: String,
    pub fate: Fate,
    pub choice: Option<FileChoice>,
    /// Whether saving removes the file.
    pub removing: bool,
    pub parts: Vec<PartView>,
    pub resolved: u32,
    pub total: u32,
    /// Whether nothing is left to resolve.
    pub done: bool,
    pub dirty: bool,
    pub can_undo: bool,
    pub can_redo: bool,
}

fn lines(text: &str) -> u32 {
    text.lines().count() as u32
}

impl Session {
    fn side_view(&self, c: usize, key: u8, text: &str, edited: bool, original: bool) -> SideView {
        let base = self.side_text(c, 0);
        SideView {
            key,
            label: self.side_label(c, key).clone(),
            text: text.to_string(),
            tints: if key == 0 && !edited {
                vec![]
            } else {
                tints(base, text)
            },
            edited,
            original,
        }
    }

    pub fn view(&self) -> ResolveView {
        let mut line = 1;
        let mut parts = vec![];
        for part in &self.parsed.parts {
            match part {
                Part::Text(text) => {
                    parts.push(PartView::Text {
                        line,
                        text: text.clone(),
                    });
                    line += lines(text);
                }
                Part::Conflict(c) => {
                    let c = *c;
                    let picks: Vec<SideView> = self
                        .picks(c)
                        .iter()
                        .map(|p| {
                            let text = self.pick_text(c, p);
                            self.side_view(c, p.key, text, p.edited.is_some(), p.original)
                        })
                        .collect();
                    let sides = self.parsed.conflicts[c].sides.len() as u8;
                    let rest = (1..=sides)
                        .chain([0])
                        .filter(|&k| self.pickable(c, k))
                        .filter(|&k| !self.picks(c).iter().any(|p| p.key == k))
                        .map(|k| self.side_view(c, k, self.side_text(c, k), false, false))
                        .collect();
                    parts.push(PartView::Conflict(ConflictView {
                        index: c as u32,
                        resolved: self.resolved(c),
                        line,
                        picks,
                        rest,
                    }));
                    line += lines(&self.conflict_text(c));
                }
            }
        }
        let (resolved, total) = self.count();
        ResolveView {
            path: self.path.clone(),
            fate: self.fate.clone(),
            choice: self.choice(),
            removing: self.removing(),
            parts,
            resolved: resolved as u32,
            total: total as u32,
            done: self.done(),
            dirty: self.dirty(),
            can_undo: self.can_undo(),
            can_redo: self.can_redo(),
        }
    }
}
