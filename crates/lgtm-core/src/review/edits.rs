//! The store edits marking rows and checking files make.

use std::collections::{HashMap, HashSet};

use super::RowRef;
use super::file::ReviewFile;
use super::matching::MatchedCheck;
use crate::anchor::Side;
use crate::store::{Edit, FileCheck, Mark, MarkColor, Store};

/// What marking and checking need of a review: its files, what's matched onto them, and the
/// label its marks and checks record.
pub struct Marking<'a> {
    pub files: &'a [ReviewFile],
    pub matched: &'a HashMap<RowRef, Mark>,
    pub checks: &'a [Option<MatchedCheck>],
    pub label: &'a str,
    /// Unix milliseconds.
    pub now: i64,
}

impl Marking<'_> {
    /// Marks `rows` `color`, or clears them. An `agent` leaves the user's marks alone.
    pub fn mark_edit(
        &self,
        store: &Store,
        rows: &[RowRef],
        color: Option<MarkColor>,
        agent: bool,
    ) -> anyhow::Result<Edit> {
        let mut edit = Edit::default();
        let wanted: HashSet<RowRef> = rows.iter().copied().collect();
        for (file_idx, file) in self.files.iter().enumerate() {
            for k in &file.keyed {
                let rref = RowRef::new(file_idx, k.row);
                if !wanted.contains(&rref) {
                    continue;
                }
                let existing = self.matched.get(&rref);
                if existing.map(|m| m.color) == color
                    || (agent && existing.is_some_and(|m| !m.color.is_agent()))
                {
                    continue;
                }
                let Some(color) = color else {
                    edit.marks_removed.extend(existing.cloned());
                    continue;
                };
                let Some(commit) = file.side(k.side).anchor_commit(store)? else {
                    continue;
                };
                edit.marks_removed.extend(existing.cloned());
                edit.marks_added.push(Mark {
                    id: 0,
                    anchor: k.anchor.clone(),
                    commit,
                    line: k.line,
                    color,
                    review: self.label.to_string(),
                    created_at: self.now,
                });
            }
        }
        Ok(edit)
    }

    /// Checking replaces any older check with one covering the file's changed rows now. The
    /// user's check replaces an agent's. An `agent` never removes the user's check, and
    /// replaces it only once it's out of date.
    pub fn check_edit(&self, files: &[u32], checked: bool, agent: bool) -> Edit {
        let mut edit = Edit::default();
        for &f in files {
            let (Some(file), Some(existing)) =
                (self.files.get(f as usize), self.checks.get(f as usize))
            else {
                continue;
            };
            let current = existing.as_ref().filter(|c| c.current);
            if checked && current.is_some_and(|c| agent || !c.check.agent) {
                continue;
            }
            if !checked && agent && existing.as_ref().is_some_and(|c| !c.check.agent) {
                continue;
            }
            edit.checks_removed
                .extend(existing.as_ref().map(|c| c.check.clone()));
            if checked {
                let hashes = |side| {
                    file.keyed
                        .iter()
                        .filter(|k| k.side == side)
                        .map(|k| k.anchor.line_hash)
                        .collect()
                };
                edit.checks_added.push(FileCheck {
                    id: 0,
                    path: file.path.clone(),
                    old_hashes: hashes(Side::Old),
                    new_hashes: hashes(Side::New),
                    fingerprint: Some(file.fingerprint),
                    review: self.label.to_string(),
                    created_at: self.now,
                    agent,
                });
            }
        }
        edit
    }
}
