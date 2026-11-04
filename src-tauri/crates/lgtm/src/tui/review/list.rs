//! The file list: moving through its files and folders, a folder's state, and marking what's
//! picked in it.

use lgtm_core::store::{MarkColor, Store};

use super::Review;
use super::marks::State;
use super::rows::{self, Line};
use super::tree::{self, Entry};
use super::undo::Step;

/// A folder's state from its files': flagged when any is, reviewed when all are, else partly
/// reviewed when any of them is started.
pub fn combined(states: impl IntoIterator<Item = State>) -> State {
    let states: Vec<State> = states.into_iter().collect();
    let any = |s: State| states.contains(&s);
    if any(State::Flagged) {
        State::Flagged
    } else if any(State::AgentFlagged) {
        State::AgentFlagged
    } else if states.iter().all(|&s| s == State::Reviewed) {
        State::Reviewed
    } else if states
        .iter()
        .all(|&s| matches!(s, State::Reviewed | State::Agent))
    {
        State::Agent
    } else if states.iter().any(|&s| s != State::None) {
        State::Partial
    } else {
        State::None
    }
}

/// What's picked in the file list when it isn't the cursor's file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pick {
    Dir(String),
    Reviewed,
}

impl Review {
    /// Whether the file is reviewed, by the user or an agent.
    pub(super) fn reviewed(&self, file: usize) -> bool {
        matches!(self.file_state(file), State::Reviewed | State::Agent)
    }

    /// The list's lines: every file, or with reviewed files set apart, the files left and
    /// then the reviewed ones under a heading.
    pub(super) fn entries(&self) -> Vec<Entry> {
        let path = |e: &Entry| match e {
            Entry::Dir { path, .. } => Some(path.clone()),
            Entry::File { file, .. } => Some(self.files[*file].path.clone()),
            Entry::Reviewed { .. } => None,
        };
        let mut out = self.all_entries();
        out.retain(|e| {
            path(e).is_none_or(|p| !self.closed_dirs.iter().any(|d| tree::inside(&p, d)))
        });
        out
    }

    /// The files in the order the list shows them, closed folders' too, leaving out
    /// reviewed files set apart under a closed heading.
    pub(super) fn list_order(&self) -> Vec<usize> {
        self.all_entries()
            .into_iter()
            .filter_map(|e| match e {
                Entry::File { file, .. } => Some(file),
                _ => None,
            })
            .collect()
    }

    /// Every entry, closed folders' too.
    fn all_entries(&self) -> Vec<Entry> {
        let files = self
            .files
            .iter()
            .enumerate()
            .map(|(i, f)| (i, f.path.as_str()));
        if !self.group_reviewed {
            return tree::tree(files, 0);
        }
        let (done, left): (Vec<_>, Vec<_>) = files.partition(|&(i, _)| self.reviewed(i));
        let mut out = tree::tree(left, 0);
        if !done.is_empty() {
            out.push(Entry::Reviewed {
                count: done.len(),
                open: self.reviewed_open,
            });
            if self.reviewed_open {
                out.extend(tree::tree(done, 1));
            }
        }
        out
    }

    /// The review files inside the folder `dir`, or the file at `path`.
    pub(super) fn files_at(&self, path: &str) -> Vec<usize> {
        let path = path.trim_end_matches('/');
        (0..self.files.len())
            .filter(|&f| {
                let p = &self.files[f].path;
                p == path || path.is_empty() || tree::inside(p, path)
            })
            .collect()
    }

    pub(super) fn dir_state(&self, dir: &str) -> State {
        combined(self.files_at(dir).into_iter().map(|f| self.file_state(f)))
    }

    /// The list entry that's picked: the folder or heading picked in the list, else the
    /// cursor's file, or the reviewed heading when that file is under it, closed.
    pub(super) fn list_at(&self, entries: &[Entry]) -> usize {
        let file = self.current().map_or(0, Line::file);
        let at = |pick: Option<&Pick>| {
            entries.iter().position(|e| match (e, pick) {
                (Entry::Dir { path, .. }, Some(Pick::Dir(dir))) => path == dir,
                (Entry::Reviewed { .. }, Some(Pick::Reviewed)) => true,
                (Entry::File { file: f, .. }, None) => *f == file,
                _ => false,
            })
        };
        // A file in a closed folder shows as the folder.
        let path = self.files.get(file).map_or("", |f| f.path.as_str());
        let folder = || {
            entries
                .iter()
                .rposition(|e| matches!(e, Entry::Dir { path: d, .. } if tree::inside(path, d)))
        };
        at(self.list_pick.as_ref())
            .or_else(|| at(Some(&Pick::Reviewed)))
            .or_else(folder)
            .unwrap_or(0)
    }

    /// Picks the list entry `n` entries on: a file moves the cursor to it, and a folder to
    /// its first file.
    pub(super) fn list_step(&mut self, n: isize) {
        let entries = self.entries();
        if entries.is_empty() {
            return;
        }
        let to = (self.list_at(&entries) as isize + n).clamp(0, entries.len() as isize - 1);
        self.pick_entry(&entries[to as usize]);
    }

    pub(super) fn pick_entry(&mut self, entry: &Entry) {
        let (file, pick) = match entry {
            Entry::File { file, .. } => (Some(*file), None),
            Entry::Dir { path, .. } => (
                self.files_at(path).first().copied(),
                Some(Pick::Dir(path.clone())),
            ),
            Entry::Reviewed { .. } => (None, Some(Pick::Reviewed)),
        };
        self.list_pick = pick;
        if let Some(f) = file {
            self.cursor = rows::find(&self.lines, (f, None, None));
            self.snap = Some(self.cursor);
        }
    }

    /// The files R and F act on from the list: the picked folder's, the reviewed ones under
    /// their heading, or the cursor's file.
    pub(super) fn list_files(&self) -> Vec<usize> {
        match &self.list_pick {
            Some(Pick::Dir(dir)) => self.files_at(dir),
            Some(Pick::Reviewed) => (0..self.files.len())
                .filter(|&f| self.reviewed(f))
                .collect(),
            None => self.current().map(Line::file).into_iter().collect(),
        }
    }

    /// Opens or closes a folder in the list.
    pub(super) fn toggle_dir(&mut self, dir: &str) {
        if !self.closed_dirs.remove(dir) {
            self.closed_dirs.insert(dir.to_string());
        }
        self.list_pick = Some(Pick::Dir(dir.to_string()));
    }

    /// Opens or closes the reviewed files' heading.
    pub(super) fn toggle_reviewed_open(&mut self) {
        self.reviewed_open = !self.reviewed_open;
        self.list_pick = Some(Pick::Reviewed);
        self.relayout();
    }

    /// Sets reviewed files apart in the list, or puts them back.
    pub(super) fn toggle_group(&mut self) {
        self.group_reviewed = !self.group_reviewed;
        self.list_pick = None;
        self.relayout();
    }

    /// r checks the list's pick and F flags it, or undoes that when it's all done already.
    pub(super) fn list_mark(&mut self, key: char, store: Option<&mut Store>) {
        let files = self.list_files();
        let (Some(marks), Some(store)) = (self.marks.as_mut(), store) else {
            return;
        };
        let result = match key {
            'r' => marks.toggle_checks(store, &files),
            _ => marks.toggle_files(store, &files, MarkColor::Red),
        };
        match result {
            Ok(edit) => {
                self.history.did(Step::Marks(edit));
                self.refold(&files);
            }
            Err(e) => self.notice = Some(format!("{e:#}")),
        }
    }

    /// Folds the files that are now all reviewed and unfolds the rest.
    pub(super) fn refold(&mut self, files: &[usize]) {
        for &f in files {
            let path = self.files[f].path.clone();
            if self.file_state(f) == State::Reviewed {
                self.folded.insert(path);
            } else {
                self.folded.remove(&path);
            }
        }
        self.relayout();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use State::*;

    #[test]
    fn a_folder_is_flagged_if_any_file_is_and_reviewed_when_all_are() {
        assert_eq!(combined([Reviewed, Flagged, None]), Flagged);
        assert_eq!(combined([Reviewed, Reviewed]), Reviewed);
        assert_eq!(combined([Reviewed, Agent]), Agent);
        assert_eq!(combined([Reviewed, None]), Partial);
        assert_eq!(combined([None, None]), None);
    }
}
