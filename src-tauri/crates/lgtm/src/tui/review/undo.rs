//! `u` undoes the review's last mark, check or comment change, and `U` redoes it.

use lgtm_core::store::{AgentComment, Edit, Store};

use super::Review;

/// A change `u` can undo.
pub enum Step {
    /// Marks and checks, as the store applied them.
    Marks(Edit),
    Added(AgentComment),
    Deleted(AgentComment),
    Edited {
        number: i64,
        before: String,
        after: String,
    },
    /// A comment resolved, or opened again.
    Resolved {
        number: i64,
        done: bool,
    },
}

#[derive(Default)]
pub struct History {
    undo: Vec<Step>,
    redo: Vec<Step>,
}

impl History {
    pub fn did(&mut self, step: Step) {
        if matches!(&step, Step::Marks(e) if e.is_empty()) {
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

/// Puts comment `c` back, or deletes it. Returns it as stored.
fn set_comment(store: &Store, c: AgentComment, there: bool) -> anyhow::Result<AgentComment> {
    if there {
        return Ok(store.restore_agent_comment(&c)?);
    }
    store.delete_agent_comment(c.number)?;
    Ok(c)
}

impl Step {
    /// Undoes the step, or redoes it. Returns it as it now stands, to go the other way.
    pub(super) fn run(self, store: &mut Store, back: bool) -> anyhow::Result<Step> {
        Ok(match self {
            Step::Marks(edit) if back => Step::Marks(store.apply(&edit.inverse())?.inverse()),
            Step::Marks(edit) => Step::Marks(store.apply(&edit)?),
            Step::Added(c) => Step::Added(set_comment(store, c, !back)?),
            Step::Deleted(c) => Step::Deleted(set_comment(store, c, back)?),
            Step::Resolved { number, done } => {
                store.set_agent_comment_done(number, done != back)?;
                Step::Resolved { number, done }
            }
            Step::Edited {
                number,
                before,
                after,
            } => {
                store.set_agent_comment_body(number, if back { &before } else { &after })?;
                Step::Edited {
                    number,
                    before,
                    after,
                }
            }
        })
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
