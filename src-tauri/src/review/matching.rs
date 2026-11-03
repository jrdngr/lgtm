//! Matches stored marks and file checks onto a review's rows, and finds amber rows: unreviewed
//! rows that replaced a line whose mark was lost, or that a checked file didn't have.

use std::collections::{HashMap, HashSet};

use lgtm_core::review::matching::{self, check_covers, replaced_lines};
pub use lgtm_core::review::matching::{MatchedCheck, Remaps};

use super::build::Keyed;
use super::{Review, RowRef};
use crate::store::{Edit, Mark, Store};

impl Review {
    /// Rematches marks, checks, and comments, recomputes amber rows, and remembers what was
    /// matched.
    pub(super) fn rematch(&mut self, store: &Store) -> anyhow::Result<()> {
        let marks = store.marks_for_paths(&self.paths())?;
        self.matched = self.match_marks(store, &marks);
        let checks = store.checks_for_paths(&self.paths())?;
        self.checks = matching::match_checks(&self.files, &checks);
        self.baseline.extend(self.matched.values().map(|m| m.id));
        self.amber = self.find_amber(store, &marks);
        self.rematch_comments(store)
    }

    /// Rematches after `edit`, which changes only marks and checks: marks only when it
    /// changes some, then checks and amber rows. Comments stay as they were.
    pub(super) fn rematch_edit(&mut self, store: &Store, edit: &Edit) -> anyhow::Result<()> {
        let marks = store.marks_for_paths(&self.paths())?;
        if !edit.marks_added.is_empty() || !edit.marks_removed.is_empty() {
            self.matched = self.match_marks(store, &marks);
            self.baseline.extend(self.matched.values().map(|m| m.id));
        }
        let checks = store.checks_for_paths(&self.paths())?;
        self.checks = matching::match_checks(&self.files, &checks);
        self.amber = self.find_amber(store, &marks);
        Ok(())
    }

    /// Every path a mark or check in this review could be stored under.
    pub(super) fn paths(&self) -> Vec<&str> {
        matching::paths(&self.files)
    }

    fn match_marks(&self, store: &Store, marks: &[Mark]) -> HashMap<RowRef, Mark> {
        matching::match_marks(store, &self.files, self.cfg, marks)
    }

    fn find_amber(&self, store: &Store, marks: &[Mark]) -> Vec<RowRef> {
        let matched_ids: HashSet<i64> = self.matched.values().map(|m| m.id).collect();
        let mut amber = HashSet::new();
        for (file_idx, file) in self.files.iter().enumerate() {
            let at = |k: &Keyed| RowRef::new(file_idx, k.row);
            if let Some(c) = self.checks[file_idx].as_ref().filter(|c| !c.current) {
                amber.extend(
                    file.keyed
                        .iter()
                        .filter(|k| !check_covers(&c.check, k))
                        .map(at),
                );
            }
            let lost = marks.iter().filter(|m| {
                self.baseline.contains(&m.id)
                    && !matched_ids.contains(&m.id)
                    && file.side(m.anchor.side).path == m.anchor.path
            });
            for m in lost {
                let Some(lines) = replaced_lines(store, file, m) else {
                    continue;
                };
                amber.extend(
                    file.keyed
                        .iter()
                        .filter(|k| k.side == m.anchor.side && lines.contains(&k.line))
                        .map(at),
                );
            }
        }
        let mut amber: Vec<RowRef> = amber
            .into_iter()
            .filter(|r| !self.matched.contains_key(r))
            .collect();
        amber.sort_by_key(|r| (r.file, r.row));
        amber
    }
}
