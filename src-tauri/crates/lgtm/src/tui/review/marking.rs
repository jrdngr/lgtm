//! Marking from the keyboard: r and f on the cursor's hunk, R and F on its file, and moving on
//! to the next hunk that isn't reviewed.

use std::collections::HashSet;

use lgtm_core::anchor::MatchConfig;
use lgtm_core::review::file::ReviewFile;
use lgtm_core::store::{MarkColor, Store};

use super::Review;
use super::marks::{Marks, State};
use super::undo::Step;

impl Review {
    /// Takes every file's review state and matches marks onto it. The first time it folds
    /// reviewed files; after that it opens files that stopped being reviewed.
    pub fn built(&mut self, files: Vec<ReviewFile>, cfg: MatchConfig, store: Option<&Store>) {
        let before = self.reviewed_paths();
        let hunk_rows = self
            .files
            .iter()
            .map(|f| f.hunks.iter().map(|h| h.rows.len()).collect())
            .collect();
        self.marks = Some(Marks::new(files, cfg, self.revset.clone(), hunk_rows));
        if let Some(store) = store {
            self.rematch(store);
        }
        let reviewed = self.reviewed_paths();
        if !std::mem::replace(&mut self.built_once, true) {
            self.folded.extend(reviewed);
        } else {
            // A reviewed file that changed since opens again.
            for path in before.difference(&reviewed) {
                self.folded.remove(path);
            }
        }
        self.relayout();
    }

    /// The reviewed files' paths, by the marks as last matched.
    pub(super) fn reviewed_paths(&self) -> HashSet<String> {
        let Some(marks) = &self.marks else {
            return HashSet::new();
        };
        (0..marks.file_count())
            .filter(|&f| marks.state(f, None) == State::Reviewed)
            .map(|f| marks.path(f).to_string())
            .collect()
    }

    /// Matches marks and comments again, after the store changed.
    pub fn rematch(&mut self, store: &Store) {
        if let Some(m) = self.marks.as_mut()
            && let Err(e) = m.rematch(store)
        {
            self.notice = Some(format!("{e:#}"));
        }
        self.load_comments(store);
        self.load_threads(store);
    }

    pub fn hunk_state(&self, file: usize, hunk: usize) -> State {
        self.marks
            .as_ref()
            .map_or(State::None, |m| m.state(file, Some(hunk)))
    }

    pub fn file_state(&self, file: usize) -> State {
        self.marks
            .as_ref()
            .map_or(State::None, |m| m.state(file, None))
    }

    pub fn percent(&self) -> Option<u32> {
        self.marks.as_ref()?.percent()
    }

    /// Moves to the start of the next hunk after the cursor that isn't reviewed, wrapping
    /// around.
    fn next_unreviewed(&mut self) {
        let wanted: Vec<usize> = self
            .hunk_starts()
            .into_iter()
            .filter(|&i| {
                let line = self.lines[i];
                let hunk = self.hunk_at(line).unwrap_or_default();
                self.hunk_state(line.file(), hunk) != State::Reviewed
            })
            .collect();
        let found = wanted
            .iter()
            .find(|&&i| i > self.cursor)
            .or(wanted.first())
            .copied();
        if let Some(i) = found {
            self.cursor = i;
        }
    }

    /// Handles a marking key. Returns whether `key` was one.
    pub(super) fn mark_key(&mut self, key: char, store: Option<&mut Store>) -> bool {
        if !"rfRF".contains(key) {
            return false;
        }
        let hunk = self.current().and_then(|l| self.hunk_at(l));
        let (Some(line), Some(marks), Some(store)) = (self.current(), self.marks.as_mut(), store)
        else {
            return true;
        };
        let file = line.file();
        let result = match key {
            'r' | 'f' if hunk.is_none() => return true,
            'r' => marks.toggle(store, file, hunk, MarkColor::Green),
            'f' => marks.toggle(store, file, hunk, MarkColor::Red),
            'R' => marks.toggle_checks(store, &[file]),
            _ => marks.toggle(store, file, None, MarkColor::Red),
        };
        match result {
            Ok(edit) => self.history.did(Step::Marks(edit)),
            Err(e) => {
                self.notice = Some(format!("{e:#}"));
                return true;
            }
        }
        // A file folds once it's all reviewed, and the cursor moves on.
        if matches!(key, 'r' | 'R') {
            let reviewed = self.file_state(file) == State::Reviewed;
            if reviewed || key == 'R' {
                self.set_folded(file, reviewed);
            }
            if reviewed {
                self.next_unreviewed();
            }
        }
        true
    }
}
