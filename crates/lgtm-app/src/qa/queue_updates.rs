//! How the agent's updates and rewrites reshape the queue after an answer.

use super::*;

impl Queue {
    /// Rewrites a question. An auto answer stays when the rewrite keeps `auto`, and otherwise
    /// opens as a plain question. Any other decided one opens again, keeping what it said
    /// before as a change; an open one changes quietly.
    pub(super) fn rewrite(&mut self, r: Rewrite, after: &str) -> anyhow::Result<()> {
        let q = self.get_mut(&r.id)?;
        if q.state == QState::Dropped {
            return Ok(());
        }
        let text = QText {
            subject: r.subject,
            context: r.context,
            options: r.options,
            why: r.why,
        };
        if text == q.text {
            return Ok(());
        }
        if let Some(a) = q.answer.as_mut().filter(|a| a.is_auto())
            && q.state == QState::Decided
        {
            match r.auto.as_deref().map(str::trim).filter(|w| !w.is_empty()) {
                Some(why) => a.auto = Some(why.to_string()),
                None => {
                    q.state = QState::Open;
                    q.answer = None;
                }
            }
        } else if q.state == QState::Decided {
            // A second rewrite before it's seen still compares with what the user last saw.
            let (previous, answer) = match q.change.take() {
                Some(c) => (c.previous, c.answer),
                None => (q.text.clone(), q.answer.take()),
            };
            q.state = QState::Open;
            q.answer = None;
            q.change = Some(Change {
                after: after.to_string(),
                what: r.what,
                previous,
                answer,
            });
        }
        q.text = text;
        Ok(())
    }

    /// Applies what the answer to `after` does to the rest of the queue. Decided questions
    /// are never dropped, and rewritten only when they depend on `after`; ids the queue
    /// doesn't have are skipped.
    pub fn apply(&mut self, after: &str, update: Update) {
        let mut before = vec![];
        for d in update.drop {
            if d.id == after {
                continue;
            }
            if let Some(q) = self.questions.iter_mut().find(|q| q.id == d.id)
                && q.state == QState::Open
            {
                before.push(q.clone());
                q.state = QState::Dropped;
                q.dropped = Some(d.why);
            }
        }
        for r in update.rewrite {
            if r.id == after {
                continue;
            }
            let reopens = self.dependents(after).iter().any(|q| q.id == r.id);
            let Some(was) = self
                .get(&r.id)
                .filter(|q| q.state == QState::Open || reopens)
                .cloned()
            else {
                continue;
            };
            self.rewrite(r, after).ok();
            if self.get(&was.id) != Some(&was) && !before.iter().any(|b| b.id == was.id) {
                before.push(was);
            }
        }
        let round = self.round().max(1);
        let added = self.push(update.add, round);
        // A second update after the same answer adds to the first; undo goes back past both.
        match self.effects.iter_mut().find(|e| e.after == after) {
            Some(e) => {
                for b in before {
                    if !e.before.iter().any(|x| x.id == b.id) {
                        e.before.push(b);
                    }
                }
                e.added.extend(added);
            }
            None => self.effects.push(Effect {
                after: after.to_string(),
                before,
                added,
            }),
        }
    }

    /// Puts back what the update after `id` did: dropped questions return, rewritten ones
    /// still open or auto-answered get their old text and any answer they had, and added
    /// ones nobody has decided or discussed go.
    pub(super) fn revert(&mut self, id: &str) {
        let Some(i) = self.effects.iter().position(|e| e.after == id) else {
            return;
        };
        let effect = self.effects.remove(i);
        for was in effect.before {
            if let Some(q) = self.questions.iter_mut().find(|q| q.id == was.id)
                && (q.state != QState::Decided || q.answer.as_ref().is_some_and(Answer::is_auto))
            {
                let discussion = std::mem::take(&mut q.discussion);
                *q = QaQuestion { discussion, ..was };
            }
        }
        self.questions.retain(|q| {
            !effect.added.contains(&q.id) || q.state != QState::Open || !q.discussion.is_empty()
        });
    }

    /// Rewrites one question after a discussion.
    pub fn reask(&mut self, r: Rewrite) -> anyhow::Result<()> {
        self.rewrite(r, "discussion")
    }
}
