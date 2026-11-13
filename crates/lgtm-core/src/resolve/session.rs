//! Resolving one file: which sides each conflict keeps and in what order, edits to them,
//! what happens to a file a side deleted or moved, and one undo history for all of it.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::parse::{Conflict, Label, Parsed, Part, parse};

/// What to do with a file one side deleted or moved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileChoice {
    /// Deleted: keep the file and resolve its conflicts.
    Keep,
    /// Deleted: remove the file.
    Delete,
    /// Moved: make this file's edits to the file it moved to, and remove this one.
    Carry,
    /// Moved: keep this file as well, resolving its conflicts.
    Both,
    /// Moved: remove this file and its edits.
    Drop,
}

impl FileChoice {
    /// Whether saving removes the file, which leaves nothing to resolve in it.
    pub fn removes(self) -> bool {
        matches!(
            self,
            FileChoice::Delete | FileChoice::Carry | FileChoice::Drop
        )
    }
}

/// What a side did to the file as a whole.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Fate {
    /// No side removed it.
    Here,
    Deleted {
        by: Label,
    },
    Moved {
        to: String,
        by: Label,
    },
}

/// A side a conflict keeps: key 0 is the base, `n` the marker's `n`th side.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pick {
    pub key: u8,
    /// Its text as edited, when it was.
    pub edited: Option<String>,
    /// Whether it shows and saves its text from before the edit.
    pub original: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct State {
    /// Per conflict, the sides it keeps in order.
    picks: Vec<Vec<Pick>>,
    choice: Option<FileChoice>,
}

/// What saving did.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Saved {
    /// Whether the file was removed.
    pub removed: bool,
    /// A file the edits were carried to that came out with conflicts of its own.
    pub clash: Option<String>,
}

pub struct Session {
    /// The file, from the workspace root.
    pub path: String,
    pub(super) parsed: Parsed,
    pub fate: Fate,
    /// The keys of sides that removed the file, which can't be picked.
    pub(super) gone: Vec<u8>,
    state: State,
    /// The state as last saved, or as read.
    saved: State,
    undo: Vec<State>,
    redo: Vec<State>,
}

impl Session {
    pub fn new(path: &str, text: &str) -> Self {
        let parsed = parse(text);
        let state = State {
            picks: vec![vec![]; parsed.conflicts.len()],
            choice: None,
        };
        Self {
            path: path.into(),
            parsed,
            fate: Fate::Here,
            gone: vec![],
            saved: state.clone(),
            state,
            undo: vec![],
            redo: vec![],
        }
    }

    pub fn conflicts(&self) -> &[Conflict] {
        &self.parsed.conflicts
    }

    /// Every label in the markers, to fill in from jj.
    pub fn labels_mut(&mut self) -> impl Iterator<Item = &mut Label> {
        self.parsed
            .conflicts
            .iter_mut()
            .flat_map(|c| c.sides.iter_mut().chain([&mut c.base]))
            .map(|s| &mut s.label)
    }

    /// Takes what a side did to the file; `gone` are the keys of the sides that removed it.
    pub fn set_fate(&mut self, fate: Fate, gone: Vec<u8>) {
        self.fate = fate;
        self.gone = gone;
    }

    pub fn picks(&self, conflict: usize) -> &[Pick] {
        &self.state.picks[conflict]
    }

    pub fn choice(&self) -> Option<FileChoice> {
        self.state.choice
    }

    /// The text of side `key` of `conflict`, 0 for the base.
    pub fn side_text(&self, conflict: usize, key: u8) -> &str {
        let c = &self.parsed.conflicts[conflict];
        match key {
            0 => &c.base.text,
            k => &c.sides[k as usize - 1].text,
        }
    }

    pub fn side_label(&self, conflict: usize, key: u8) -> &Label {
        let c = &self.parsed.conflicts[conflict];
        match key {
            0 => &c.base.label,
            k => &c.sides[k as usize - 1].label,
        }
    }

    /// What `pick` of `conflict` shows and saves.
    pub fn pick_text<'a>(&'a self, conflict: usize, pick: &'a Pick) -> &'a str {
        match (&pick.edited, pick.original) {
            (Some(t), false) => t,
            _ => self.side_text(conflict, pick.key),
        }
    }

    /// Whether `key` names a side of `conflict` that can be picked.
    pub fn pickable(&self, conflict: usize, key: u8) -> bool {
        let sides = self
            .parsed
            .conflicts
            .get(conflict)
            .map_or(0, |c| c.sides.len());
        (key as usize) <= sides && !self.gone.contains(&key)
    }

    /// Makes a change as one undo step, when `change` says it changed something.
    fn change(&mut self, change: impl FnOnce(&mut State) -> bool) -> bool {
        let before = self.state.clone();
        if !change(&mut self.state) || self.state == before {
            self.state = before;
            return false;
        }
        self.undo.push(before);
        self.redo.clear();
        true
    }

    /// Adds side `key` to the end of what `conflict` keeps, or takes it out when it's there.
    pub fn toggle(&mut self, conflict: usize, key: u8) -> bool {
        if !self.pickable(conflict, key) {
            return false;
        }
        self.change(|s| {
            let picks = &mut s.picks[conflict];
            match picks.iter().position(|p| p.key == key) {
                Some(i) => {
                    picks.remove(i);
                }
                None => picks.push(Pick {
                    key,
                    edited: None,
                    original: false,
                }),
            }
            true
        })
    }

    /// Moves pick `from` of `conflict` to place `to`. Nothing happens past either end.
    pub fn place(&mut self, conflict: usize, from: usize, to: usize) -> bool {
        self.change(|s| {
            let picks = &mut s.picks[conflict];
            if from >= picks.len() || to >= picks.len() {
                return false;
            }
            let p = picks.remove(from);
            picks.insert(to, p);
            true
        })
    }

    /// Replaces the text of pick `pick` of `conflict`. Text the same as the side's clears
    /// the edit.
    pub fn edit(&mut self, conflict: usize, pick: usize, text: &str) -> bool {
        let Some(key) = self.state.picks[conflict].get(pick).map(|p| p.key) else {
            return false;
        };
        let same = self.side_text(conflict, key) == text;
        self.change(|s| {
            let p = &mut s.picks[conflict][pick];
            p.edited = (!same).then(|| text.to_string());
            p.original = false;
            true
        })
    }

    /// Switches an edited pick between its edit and its text from before.
    pub fn flip_original(&mut self, conflict: usize, pick: usize) -> bool {
        self.change(|s| match s.picks[conflict].get_mut(pick) {
            Some(p) if p.edited.is_some() => {
                p.original = !p.original;
                true
            }
            _ => false,
        })
    }

    /// Chooses what happens to a file a side removed; the same choice again clears it.
    pub fn choose(&mut self, choice: FileChoice) -> bool {
        let fits = match &self.fate {
            Fate::Here => false,
            Fate::Deleted { .. } => matches!(choice, FileChoice::Keep | FileChoice::Delete),
            Fate::Moved { .. } => matches!(
                choice,
                FileChoice::Carry | FileChoice::Both | FileChoice::Drop
            ),
        };
        fits && self.change(|s| {
            s.choice = if s.choice == Some(choice) {
                None
            } else {
                Some(choice)
            };
            true
        })
    }

    pub fn undo(&mut self) -> bool {
        let Some(prev) = self.undo.pop() else {
            return false;
        };
        self.redo.push(std::mem::replace(&mut self.state, prev));
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(std::mem::replace(&mut self.state, next));
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Whether anything changed since the file was read or saved.
    pub fn dirty(&self) -> bool {
        self.state != self.saved
    }

    /// Takes the labels and fate jj filled into `filled`, a session on the same text.
    pub fn adopt(&mut self, filled: &Session) {
        let labels: Vec<Label> = filled
            .parsed
            .conflicts
            .iter()
            .flat_map(|c| c.sides.iter().chain([&c.base]))
            .map(|s| s.label.clone())
            .collect();
        if labels.len() == self.labels_mut().count() {
            for (mine, label) in self.labels_mut().zip(labels) {
                *mine = label;
            }
        }
        self.fate = filled.fate.clone();
        self.gone = filled.gone.clone();
    }

    /// Whether saving removes the file.
    pub fn removing(&self) -> bool {
        self.state.choice.is_some_and(FileChoice::removes)
    }

    pub fn resolved(&self, conflict: usize) -> bool {
        self.removing() || !self.state.picks[conflict].is_empty()
    }

    /// How many conflicts are resolved, of how many.
    pub fn count(&self) -> (usize, usize) {
        let n = self.parsed.conflicts.len();
        ((0..n).filter(|&c| self.resolved(c)).count(), n)
    }

    /// Whether nothing is left to resolve.
    pub fn done(&self) -> bool {
        let (resolved, total) = self.count();
        resolved == total && (self.fate == Fate::Here || self.state.choice.is_some())
    }

    /// What conflict `c` comes out as: its picks, else its markers.
    pub fn conflict_text(&self, c: usize) -> String {
        let picks = &self.state.picks[c];
        if picks.is_empty() {
            return self.parsed.conflicts[c].raw.clone();
        }
        picks.iter().map(|p| self.pick_text(c, p)).collect()
    }

    /// The file as saving writes it: resolved conflicts as their picks, the rest as markers.
    pub fn text(&self) -> String {
        self.with(|c| self.conflict_text(c))
    }

    /// The file with each conflict as `side` says.
    fn with(&self, side: impl Fn(usize) -> String) -> String {
        self.parsed
            .parts
            .iter()
            .map(|p| match p {
                Part::Text(t) => t.clone(),
                Part::Conflict(c) => side(*c),
            })
            .collect()
    }

    /// Writes the file into `root`, or removes it, and carries edits to a moved file. The
    /// state then counts as saved.
    pub fn save(&mut self, root: &Path) -> std::io::Result<Saved> {
        let file = root.join(&self.path);
        let mut saved = Saved::default();
        if let (Some(FileChoice::Carry), Fate::Moved { to, by }) = (self.state.choice, &self.fate) {
            let base = self.with(|c| self.parsed.conflicts[c].base.text.clone());
            let edited = self.with(|c| match self.state.picks[c].is_empty() {
                true => self.kept_side(c).to_string(),
                false => self.conflict_text(c),
            });
            let target = root.join(to);
            let ours = std::fs::read_to_string(&target).unwrap_or_default();
            let theirs = format!("{} {}", self.path, by.description);
            let merged = super::merge::merge(&base, &ours, &edited, [to, "base", &theirs]);
            let text = merged.clone().unwrap_or_else(|t| t);
            if merged.is_err() {
                saved.clash = Some(to.clone());
            }
            std::fs::write(&target, text)?;
        }
        if self.removing() {
            match std::fs::remove_file(&file) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
                _ => saved.removed = true,
            }
        } else {
            std::fs::write(&file, self.text())?;
        }
        self.saved = self.state.clone();
        Ok(saved)
    }

    /// The text of the first side of `c` that kept the file.
    fn kept_side(&self, c: usize) -> &str {
        let conflict = &self.parsed.conflicts[c];
        (1..=conflict.sides.len() as u8)
            .find(|k| !self.gone.contains(k))
            .map_or(&conflict.base.text, |k| {
                &conflict.sides[k as usize - 1].text
            })
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
