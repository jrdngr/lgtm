//! The file list: moving through its files and folders, a folder's state, and marking what's
//! picked in it.

use lgtm_core::store::{MarkColor, Store};

use super::Review;
use super::marks::State;
use super::rows::{self, Line};
use super::tree::{self, Entry};

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

impl Review {
    pub(super) fn entries(&self) -> Vec<Entry> {
        tree::tree(self.files.iter().map(|f| f.path.as_str()))
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

    /// The list entry that's picked: the folder picked in the list, else the cursor's file.
    pub(super) fn list_at(&self, entries: &[Entry]) -> usize {
        let file = self.current().map_or(0, Line::file);
        entries
            .iter()
            .position(|e| match (e, &self.list_dir) {
                (Entry::Dir { path, .. }, Some(dir)) => path == dir,
                (Entry::File { file: f, .. }, None) => *f == file,
                _ => false,
            })
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
        let (file, dir) = match entry {
            Entry::File { file, .. } => (Some(*file), None),
            Entry::Dir { path, .. } => (self.files_at(path).first().copied(), Some(path.clone())),
        };
        self.list_dir = dir;
        if let Some(f) = file {
            self.cursor = rows::find(&self.lines, (f, None, None));
        }
    }

    /// The files R and F act on from the list: the picked folder's, or the cursor's file.
    pub(super) fn list_files(&self) -> Vec<usize> {
        match &self.list_dir {
            Some(dir) => self.files_at(dir),
            None => self.current().map(Line::file).into_iter().collect(),
        }
    }

    /// R checks the list's pick and F flags it, or undoes that when it's all done already.
    pub(super) fn list_mark(&mut self, key: char, store: Option<&mut Store>) {
        let files = self.list_files();
        let (Some(marks), Some(store)) = (self.marks.as_mut(), store) else {
            return;
        };
        let result = match key {
            'R' => marks.toggle_checks(store, &files),
            _ => marks.toggle_files(store, &files, MarkColor::Red),
        };
        match result {
            Ok(()) => self.refold(&files),
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
