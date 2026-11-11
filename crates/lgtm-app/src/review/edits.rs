//! Marking rows and checking files, as undoable edits to the store.

use lgtm_core::review::edits::Marking;

use super::{RepoCtx, Review, RowRef, StateDto, now_ms};
use lgtm_core::review::comments::Step;

use crate::store::{Edit, MarkColor, Store};

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
            let mut edit = review.mark_edit(store, rows, color, false)?;
            let checks = review.check_edit(check, true, false);
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
            let mut edit = review.mark_edit(store, &rows, None, false)?;
            edit.checks_removed = review.check_edit(&[file], false, false).checks_removed;
            Ok(edit)
        })
    }

    pub fn set_checked(&self, id: u32, files: &[u32], checked: bool) -> anyhow::Result<StateDto> {
        self.edit(id, |review, _| Ok(review.check_edit(files, checked, false)))
    }

    /// Checks or unchecks files as an agent. The user's checks stay as they are.
    pub fn agent_set_checked(
        &self,
        id: u32,
        files: &[u32],
        checked: bool,
    ) -> anyhow::Result<StateDto> {
        self.edit(id, |review, _| Ok(review.check_edit(files, checked, true)))
    }

    /// Marks rows with an agent's `color`, or clears the agent's marks on them when it's
    /// `None`. Rows the user marked keep their marks.
    pub fn agent_mark(
        &self,
        id: u32,
        rows: &[RowRef],
        color: Option<MarkColor>,
    ) -> anyhow::Result<StateDto> {
        if color.is_some_and(|c| !c.is_agent()) {
            anyhow::bail!("agents mark only with their own colors");
        }
        self.edit(id, |review, store| {
            review.mark_edit(store, rows, color, true)
        })
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
            review.undo.push(Step::Marks(applied));
            review.redo.clear();
            review.rematch_edit(&store, &edit)?;
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
        let stack = if undo {
            &mut review.undo
        } else {
            &mut review.redo
        };
        if let Some(step) = stack.last().cloned() {
            let marks = step.marks().cloned();
            // Popped only once run, so a failed step stays on its stack.
            let done = step.run(&mut store, undo)?;
            stack.pop();
            if undo {
                review.redo.push(done);
            } else {
                review.undo.push(done);
            }
            match marks {
                Some(edit) => review.rematch_edit(&store, &edit)?,
                None => review.rematch_agent_comments(&store)?,
            }
            review.save_session(&store)?;
        }
        Ok(review.state())
    }
}

impl Review {
    fn marking(&self) -> Marking<'_> {
        Marking {
            files: &self.files,
            matched: &self.matched,
            checks: &self.checks,
            label: &self.label,
            now: now_ms(),
        }
    }

    /// Marks `rows` `color`, or clears them. An `agent` leaves the user's marks alone.
    fn mark_edit(
        &self,
        store: &Store,
        rows: &[RowRef],
        color: Option<MarkColor>,
        agent: bool,
    ) -> anyhow::Result<Edit> {
        self.marking().mark_edit(store, rows, color, agent)
    }

    /// See `Marking::check_edit`.
    fn check_edit(&self, files: &[u32], checked: bool, agent: bool) -> Edit {
        self.marking().check_edit(files, checked, agent)
    }
}
