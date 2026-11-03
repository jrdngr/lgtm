//! Marking from the keyboard: r and f on the cursor's hunk, R and F on its file, n to the next
//! hunk that isn't reviewed.

use lgtm_core::anchor::MatchConfig;
use lgtm_core::review::file::ReviewFile;
use lgtm_core::store::{MarkColor, Store};

use super::Review;
use super::marks::{Marks, State};
use super::rows::{self, Line};

impl Review {
    /// Takes every file's review state, matches marks onto it, and folds reviewed hunks.
    pub fn built(&mut self, files: Vec<ReviewFile>, cfg: MatchConfig, store: Option<&Store>) {
        let hunk_rows = self
            .files
            .iter()
            .map(|f| f.hunks.iter().map(|h| h.rows.len()).collect())
            .collect();
        self.marks = Some(Marks::new(files, cfg, self.revset.clone(), hunk_rows));
        if let Some(store) = store {
            self.rematch(store);
        }
        if !std::mem::replace(&mut self.built_once, true) {
            let reviewed: Vec<(usize, usize)> = self
                .hunks()
                .filter(|&(f, h)| self.hunk_state(f, h) == State::Reviewed)
                .collect();
            self.folded.extend(reviewed);
            self.relayout();
        }
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

    fn hunks(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.files
            .iter()
            .enumerate()
            .flat_map(|(f, d)| (0..d.hunks.len()).map(move |h| (f, h)))
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

    /// Moves to the next hunk header after the cursor that isn't reviewed, wrapping around.
    fn next_unreviewed(&mut self) {
        let wanted = |l: &Line| match *l {
            Line::Hunk(f, h) => self.hunk_state(f, h) != State::Reviewed,
            _ => false,
        };
        let after = self.lines.iter().skip(self.cursor + 1).position(wanted);
        let found = after
            .map(|i| self.cursor + 1 + i)
            .or_else(|| self.lines.iter().position(wanted));
        if let Some(i) = found {
            self.cursor = i;
        }
    }

    fn set_folds(&mut self, keys: impl IntoIterator<Item = (usize, usize)>, folded: bool) {
        for key in keys {
            if folded {
                self.folded.insert(key);
            } else {
                self.folded.remove(&key);
            }
        }
        let spot = self.current().map(|l| l.spot());
        self.lines = self.layout();
        self.cursor = spot.map_or(0, |s| rows::find(&self.lines, s));
    }

    /// Handles a marking key. Returns whether `key` was one.
    pub(super) fn mark_key(&mut self, key: char, store: Option<&mut Store>) -> bool {
        if !"rfRFn".contains(key) {
            return false;
        }
        if key == 'n' {
            self.next_unreviewed();
            return true;
        }
        let (Some(line), Some(marks), Some(store)) = (self.current(), self.marks.as_mut(), store)
        else {
            return true;
        };
        let file = line.file();
        let hunk = line.hunk();
        let result = match key {
            'r' | 'f' if hunk.is_none() => return true,
            'r' => marks.toggle(store, file, hunk, MarkColor::Green),
            'f' => marks.toggle(store, file, hunk, MarkColor::Red),
            'R' => marks.toggle_check(store, file),
            _ => marks.toggle(store, file, None, MarkColor::Red),
        };
        if let Err(e) = result {
            self.notice = Some(format!("{e:#}"));
            return true;
        }
        match key {
            'r' => {
                let hunk = hunk.unwrap_or_default();
                let reviewed = self.hunk_state(file, hunk) == State::Reviewed;
                self.set_folds([(file, hunk)], reviewed);
                if reviewed {
                    self.next_unreviewed();
                }
            }
            'R' => {
                let reviewed = self.file_state(file) == State::Reviewed;
                let hunks: Vec<_> = (0..self.files[file].hunks.len())
                    .map(|h| (file, h))
                    .collect();
                self.set_folds(hunks, reviewed);
                if reviewed {
                    self.next_unreviewed();
                }
            }
            _ => {}
        }
        true
    }
}
