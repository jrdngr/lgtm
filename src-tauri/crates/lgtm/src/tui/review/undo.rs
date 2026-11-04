//! `u` undoes the review's last mark, check or comment change, and `U` redoes it.

use lgtm_core::store::Store;

use super::Review;

pub use lgtm_core::review::comments::Step;

#[derive(Default)]
pub struct History {
    undo: Vec<Step>,
    redo: Vec<Step>,
}

impl History {
    pub fn did(&mut self, step: Step) {
        if step.is_empty() {
            return;
        }
        self.undo.push(step);
        self.redo.clear();
    }

    pub fn extend(&mut self, steps: impl IntoIterator<Item = Step>) {
        for step in steps {
            self.did(step);
        }
    }
}

impl Review {
    pub(super) fn undo(&mut self, store: Option<&mut Store>) {
        self.travel(store, true);
    }

    pub(super) fn redo(&mut self, store: Option<&mut Store>) {
        self.travel(store, false);
    }

    /// Undoes or redoes a step. Files it makes all reviewed fold, and files it makes not
    /// reviewed open.
    fn travel(&mut self, store: Option<&mut Store>, back: bool) {
        let Some(store) = store else {
            return;
        };
        let stack = if back {
            &mut self.history.undo
        } else {
            &mut self.history.redo
        };
        let Some(step) = stack.pop() else {
            self.notice = Some(format!("nothing to {}", if back { "undo" } else { "redo" }));
            return;
        };
        let before = self.reviewed_paths();
        match step.run(store, back) {
            Ok(step) if back => self.history.redo.push(step),
            Ok(step) => self.history.undo.push(step),
            Err(e) => self.notice = Some(format!("{e:#}")),
        }
        self.rematch(store);
        let after = self.reviewed_paths();
        for path in before.difference(&after) {
            self.folded.remove(path);
        }
        self.folded.extend(after.difference(&before).cloned());
        self.relayout();
    }
}

#[cfg(test)]
mod tests {
    use lgtm_core::anchor::Side;
    use lgtm_core::store::AgentComment;

    use super::*;

    fn comment(body: &str) -> AgentComment {
        AgentComment {
            id: 0,
            number: 0,
            path: "a.rs".into(),
            side: Side::New,
            line: 1,
            line_text: String::new(),
            start: None,
            body: body.into(),
            review: "@".into(),
            created_at: 0,
            done: false,
        }
    }

    fn bodies(store: &Store) -> Vec<(i64, String)> {
        store
            .agent_comments_for_paths(&["a.rs"])
            .unwrap()
            .into_iter()
            .map(|c| (c.number, c.body))
            .collect()
    }

    #[test]
    fn comment_steps_undo_and_redo_in_order() {
        let mut store = Store::in_memory();
        let mut r = super::super::tests::review();
        let added = store.add_agent_comment(&comment("one")).unwrap();
        r.history.did(Step::Added(added.clone()));
        store.set_agent_comment_body(1, "two").unwrap();
        r.history.did(Step::Edited {
            number: 1,
            before: "one".into(),
            after: "two".into(),
        });
        store.delete_agent_comment(1).unwrap();
        r.history.did(Step::Deleted(AgentComment {
            body: "two".into(),
            ..added
        }));
        assert_eq!(bodies(&store), []);
        r.undo(Some(&mut store));
        assert_eq!(bodies(&store), [(1, "two".into())]);
        r.undo(Some(&mut store));
        assert_eq!(bodies(&store), [(1, "one".into())]);
        r.undo(Some(&mut store));
        assert_eq!(bodies(&store), []);
        r.undo(Some(&mut store));
        assert_eq!(r.notice.as_deref(), Some("nothing to undo"));
        r.redo(Some(&mut store));
        r.redo(Some(&mut store));
        assert_eq!(bodies(&store), [(1, "two".into())]);
        r.redo(Some(&mut store));
        assert_eq!(bodies(&store), []);
    }
}
