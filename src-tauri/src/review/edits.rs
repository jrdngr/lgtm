//! Marking rows and checking files, as undoable edits to the store.

use std::collections::HashSet;

use super::{RepoCtx, Review, RowRef, StateDto, now_ms};
use crate::anchor::Side;
use crate::store::{Edit, FileCheck, Mark, MarkColor, Store};

impl RepoCtx {
    /// Marks changed rows, or clears their marks when `color` is `None`. `check` lists files
    /// to check in the same step, for a whole-file selection marked with the first color.
    pub fn mark(
        &self,
        id: u32,
        rows: &[RowRef],
        color: Option<MarkColor>,
        check: &[u32],
    ) -> anyhow::Result<StateDto> {
        self.edit(id, |review, store| {
            let mut edit = review.mark_edit(store, rows, color)?;
            let checks = review.check_edit(check, true);
            edit.checks_removed = checks.checks_removed;
            edit.checks_added = checks.checks_added;
            Ok(edit)
        })
    }

    /// Clears every mark in a file and unchecks it, in one undo step.
    pub fn clear_file(&self, id: u32, file: u32) -> anyhow::Result<StateDto> {
        self.edit(id, |review, store| {
            let rows: Vec<RowRef> = review
                .files
                .get(file as usize)
                .map(|f| {
                    f.keyed
                        .iter()
                        .map(|k| RowRef::new(file as usize, k.row))
                        .collect()
                })
                .unwrap_or_default();
            let mut edit = review.mark_edit(store, &rows, None)?;
            edit.checks_removed = review.check_edit(&[file], false).checks_removed;
            Ok(edit)
        })
    }

    pub fn set_checked(&self, id: u32, files: &[u32], checked: bool) -> anyhow::Result<StateDto> {
        self.edit(id, |review, _| Ok(review.check_edit(files, checked)))
    }

    /// Removes the marks and checks this review's revset created.
    pub fn reset(&self, id: u32) -> anyhow::Result<StateDto> {
        self.edit(id, |review, store| {
            Ok(Edit {
                marks_removed: store.marks_for_review(&review.label)?,
                checks_removed: store.checks_for_review(&review.label)?,
                ..Edit::default()
            })
        })
    }

    fn edit(
        &self,
        id: u32,
        make: impl FnOnce(&Review, &Store) -> anyhow::Result<Edit>,
    ) -> anyhow::Result<StateDto> {
        let review = self.review(id)?;
        let mut review = review.lock().unwrap();
        let mut store = self.store.lock().unwrap();
        let edit = make(&review, &store)?;
        if !edit.is_empty() {
            let applied = store.apply(&edit)?;
            review.undo.push(applied);
            review.redo.clear();
            review.rematch(&store)?;
            review.save_session(&store)?;
        }
        Ok(review.state())
    }

    pub fn undo(&self, id: u32) -> anyhow::Result<StateDto> {
        self.step(id, true)
    }

    pub fn redo(&self, id: u32) -> anyhow::Result<StateDto> {
        self.step(id, false)
    }

    fn step(&self, id: u32, undo: bool) -> anyhow::Result<StateDto> {
        let review = self.review(id)?;
        let mut review = review.lock().unwrap();
        let mut store = self.store.lock().unwrap();
        let last = if undo {
            review.undo.last()
        } else {
            review.redo.last()
        };
        if let Some(edit) = last.cloned() {
            // Popped only once applied, so a failed step stays on its stack.
            if undo {
                store.apply(&edit.inverse())?;
                review.undo.pop();
                review.redo.push(edit);
            } else {
                store.apply(&edit)?;
                review.redo.pop();
                review.undo.push(edit);
            }
            review.rematch(&store)?;
            review.save_session(&store)?;
        }
        Ok(review.state())
    }
}

impl Review {
    fn mark_edit(
        &self,
        store: &Store,
        rows: &[RowRef],
        color: Option<MarkColor>,
    ) -> anyhow::Result<Edit> {
        let created_at = now_ms();
        let mut edit = Edit::default();
        let wanted: HashSet<RowRef> = rows.iter().copied().collect();
        for (file_idx, file) in self.files.iter().enumerate() {
            for k in &file.keyed {
                let rref = RowRef::new(file_idx, k.row);
                if !wanted.contains(&rref) {
                    continue;
                }
                let existing = self.matched.get(&rref);
                if existing.map(|m| m.color) == color {
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
                    review: self.label.clone(),
                    created_at,
                });
            }
        }
        Ok(edit)
    }

    /// Checking replaces any older check with one covering the file's changed rows now.
    fn check_edit(&self, files: &[u32], checked: bool) -> Edit {
        let mut edit = Edit::default();
        for &f in files {
            let (Some(file), Some(existing)) =
                (self.files.get(f as usize), self.checks.get(f as usize))
            else {
                continue;
            };
            if checked && existing.as_ref().is_some_and(|c| c.current) {
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
                    review: self.label.clone(),
                    created_at: now_ms(),
                });
            }
        }
        edit
    }
}
