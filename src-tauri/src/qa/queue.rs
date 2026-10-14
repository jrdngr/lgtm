//! A Q&A session's question queue: questions in ask order, each built the qqueue way (context,
//! options with the recommendation first, the reason for it), what it depends on, and what
//! was decided. Answers reshape the questions that depend on them through updates from the
//! agent.

use anyhow::Context;
use serde::{Deserialize, Serialize};
use specta::Type;

use super::order::{self, is_sub};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Choice {
    pub label: String,
    /// What it means, in a line.
    #[serde(default)]
    pub means: String,
    #[serde(default)]
    pub pros: Vec<String>,
    #[serde(default)]
    pub cons: Vec<String>,
    /// Downsides worse than a con.
    #[serde(default)]
    pub risks: Vec<String>,
    /// What it means and what it costs, from sessions before pros and cons.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bullets: Vec<String>,
}

/// What a question says. The first option is always the recommendation.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct QText {
    pub subject: String,
    pub context: Vec<String>,
    pub options: Vec<Choice>,
    /// Why option 1 is recommended.
    pub why: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum QState {
    Open,
    Decided,
    /// Put off: resolved for the queue, and left out of the write.
    Deferred,
    Dropped,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    /// The option picked, from 0. `None` for an answer outside the options.
    pub option: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// The answer in the user's words, when it's none of the options.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub other: Option<String>,
    /// When it was decided, in ms, so the last decision can be undone.
    pub at: f64,
    /// Why the agent answered it itself, with option 1, when it did. Auto answers come with
    /// the questions, so undo leaves them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto: Option<String>,
}

impl Answer {
    pub fn is_auto(&self) -> bool {
        self.auto.is_some()
    }
}

/// A rewrite of a decided question, which opened it again, or an auto answer the user
/// reopened, that the user hasn't looked at yet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    /// What it was rewritten after: a question id, "discussion", or "auto" for a reopened
    /// auto answer.
    pub after: String,
    /// What changed, in a few words.
    pub what: String,
    pub previous: QText,
    /// The answer it had before it reopened.
    #[serde(default)]
    pub answer: Option<Answer>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Turn {
    pub you: bool,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct QaQuestion {
    /// `Q4`, or `Q4a` for a subquestion.
    pub id: String,
    pub round: u32,
    pub text: QText,
    /// Files the agent read to build it.
    #[serde(default)]
    pub sources: Vec<String>,
    /// Ids of questions whose answers could change this one. A subquestion also depends on
    /// its parent without listing it.
    #[serde(default)]
    pub depends: Vec<String>,
    pub state: QState,
    #[serde(default)]
    pub answer: Option<Answer>,
    /// Why it was dropped, like "settled by Q3".
    #[serde(default)]
    pub dropped: Option<String>,
    #[serde(default)]
    pub change: Option<Change>,
    /// A discussion that isn't a decision.
    #[serde(default)]
    pub discussion: Vec<Turn>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Queue {
    pub questions: Vec<QaQuestion>,
    /// What each answer's update did to the other questions, so undo can put them back.
    #[serde(default)]
    pub effects: Vec<Effect>,
    /// Each defer, so undo can take it back in one step.
    #[serde(default)]
    pub deferrals: Vec<Deferral>,
    /// Each auto answer the user reopened, so undo can put it back.
    #[serde(default)]
    pub reopens: Vec<Reopen>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Reopen {
    pub id: String,
    pub answer: Answer,
    pub at: f64,
}

/// The questions one defer put off: `id` and the open questions that depend on it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Deferral {
    pub id: String,
    pub ids: Vec<String>,
    /// When, in ms, so undo takes the latest of decisions and defers.
    pub at: f64,
}

/// The questions an update after `after` dropped or rewrote, as they were, and the ones it
/// added.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Effect {
    pub after: String,
    pub before: Vec<QaQuestion>,
    pub added: Vec<String>,
}

/// A question from the agent. A subquestion belongs to the top-level question before it.
/// `depends` uses the ids the batch would get in the order given.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct NewQuestion {
    #[serde(default)]
    pub sub: bool,
    pub subject: String,
    pub context: Vec<String>,
    pub options: Vec<Choice>,
    pub why: String,
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub depends: Vec<String>,
    /// Why option 1 is obvious, when the agent answers it itself. Empty means ask.
    #[serde(default)]
    pub auto: Option<String>,
}

impl NewQuestion {
    fn text(&self) -> QText {
        QText {
            subject: self.subject.clone(),
            context: self.context.clone(),
            options: self.options.clone(),
            why: self.why.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Drop {
    pub id: String,
    pub why: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Rewrite {
    pub id: String,
    pub what: String,
    pub subject: String,
    pub context: Vec<String>,
    pub options: Vec<Choice>,
    pub why: String,
    /// For a question with an auto answer: why option 1 is still obvious, which keeps it.
    /// Empty opens it.
    #[serde(default)]
    pub auto: Option<String>,
}

/// What an answer does to the rest of the queue.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct Update {
    #[serde(default)]
    pub drop: Vec<Drop>,
    #[serde(default)]
    pub rewrite: Vec<Rewrite>,
    #[serde(default)]
    pub add: Vec<NewQuestion>,
}

impl Queue {
    pub fn get(&self, id: &str) -> Option<&QaQuestion> {
        self.questions.iter().find(|q| q.id == id)
    }

    fn get_mut(&mut self, id: &str) -> anyhow::Result<&mut QaQuestion> {
        self.questions
            .iter_mut()
            .find(|q| q.id == id)
            .ok_or_else(|| anyhow::anyhow!("no question {id}"))
    }

    pub fn round(&self) -> u32 {
        self.questions.iter().map(|q| q.round).max().unwrap_or(0)
    }

    /// The number of the last question, which new ones number on from.
    pub fn last_number(&self) -> u32 {
        self.questions
            .iter()
            .map(|q| order::number(&q.id))
            .max()
            .unwrap_or(0)
    }

    /// Appends questions in `round`, numbering on from the last one and putting each after
    /// the ones it depends on. Returns their ids in the order given.
    fn push(&mut self, new: Vec<NewQuestion>, round: u32) -> Vec<String> {
        let placed = order::place(new, self.last_number(), |id| self.get(id).is_some());
        let mut ids = vec![String::new(); placed.len()];
        for p in placed {
            ids[p.index] = p.id.clone();
            let n = p.question;
            let auto = n.auto.clone().filter(|w| !w.trim().is_empty());
            self.questions.push(QaQuestion {
                id: p.id,
                round,
                text: n.text(),
                sources: n.sources,
                depends: n.depends,
                state: if auto.is_some() {
                    QState::Decided
                } else {
                    QState::Open
                },
                answer: auto.map(|why| Answer {
                    option: Some(0),
                    note: None,
                    other: None,
                    at: 0.0,
                    auto: Some(why.trim().to_string()),
                }),
                dropped: None,
                change: None,
                discussion: vec![],
            });
        }
        ids
    }

    /// The questions an answer to `id` could change: ones that depend on it and its
    /// subquestions, open or decided.
    pub fn dependents(&self, id: &str) -> Vec<&QaQuestion> {
        self.questions
            .iter()
            .filter(|q| q.id != id && matches!(q.state, QState::Open | QState::Decided))
            .filter(|q| q.depends.iter().any(|d| d == id) || is_sub(&q.id, id))
            .collect()
    }

    /// Starts a new round of questions, after a draft, revision or comments.
    pub fn add_round(&mut self, new: Vec<NewQuestion>) -> Vec<String> {
        let round = self.round() + 1;
        self.push(new, round)
    }

    pub fn decide(&mut self, id: &str, answer: Answer) -> anyhow::Result<()> {
        let q = self.get_mut(id)?;
        anyhow::ensure!(q.state != QState::Dropped, "{id} was dropped");
        if let Some(o) = answer.option {
            anyhow::ensure!(
                (o as usize) < q.text.options.len(),
                "{id} has no option {}",
                o + 1
            );
        }
        q.state = QState::Decided;
        q.answer = Some(answer);
        q.change = None;
        Ok(())
    }

    /// The open questions deferring `id` would also defer: those that depend on it, and
    /// on them in turn.
    pub fn defer_preview(&self, id: &str) -> Vec<String> {
        let mut out: Vec<String> = vec![];
        let mut next = vec![id.to_string()];
        while let Some(at) = next.pop() {
            for d in self.dependents(&at) {
                if d.state == QState::Open && d.id != id && !out.contains(&d.id) {
                    out.push(d.id.clone());
                    next.push(d.id.clone());
                }
            }
        }
        out.sort_by_key(|d| self.questions.iter().position(|q| &q.id == d));
        out
    }

    /// Defers an open question and the open questions that depend on it. Returns their ids.
    pub fn defer(&mut self, id: &str, at: f64) -> anyhow::Result<Vec<String>> {
        let q = self.get(id).with_context(|| format!("no question {id}"))?;
        anyhow::ensure!(q.state == QState::Open, "{id} isn't open");
        let mut ids = vec![id.to_string()];
        ids.extend(self.defer_preview(id));
        for q in self.questions.iter_mut().filter(|q| ids.contains(&q.id)) {
            q.state = QState::Deferred;
        }
        self.deferrals.push(Deferral {
            id: id.to_string(),
            ids: ids.clone(),
            at,
        });
        Ok(ids)
    }

    /// Takes back the last decision, defer or reopened auto answer, and what it did to the
    /// other questions. Returns the id it was on.
    pub fn undo(&mut self) -> Option<String> {
        let decided = self
            .questions
            .iter()
            .filter(|q| q.state == QState::Decided)
            .filter_map(|q| Some((q.answer.as_ref().filter(|a| !a.is_auto())?.at, q)))
            .max_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(at, q)| (at, q.id.clone()));
        let deferred = self.deferrals.last().map(|d| d.at);
        let reopened = self.reopens.last().map(|r| r.at);
        let latest = [decided.as_ref().map(|d| d.0), deferred, reopened]
            .into_iter()
            .flatten()
            .max_by(f64::total_cmp)?;
        if deferred == Some(latest) {
            let d = self.deferrals.pop()?;
            for q in self.questions.iter_mut().filter(|q| d.ids.contains(&q.id)) {
                if q.state == QState::Deferred {
                    q.state = QState::Open;
                }
            }
            return Some(d.id);
        }
        if reopened == Some(latest) {
            let r = self.reopens.pop()?;
            if let Ok(q) = self.get_mut(&r.id)
                && q.state == QState::Open
            {
                q.state = QState::Decided;
                q.answer = Some(r.answer);
                q.change = None;
            }
            return Some(r.id);
        }
        let (_, id) = decided?;
        self.reopen(&id).ok()?;
        self.revert(&id);
        Some(id)
    }

    fn reopen(&mut self, id: &str) -> anyhow::Result<()> {
        let q = self.get_mut(id)?;
        q.state = QState::Open;
        q.answer = None;
        Ok(())
    }

    /// Opens a decided question so the user can answer it again. A reopened auto answer
    /// shows as a change, and undo can put it back.
    pub fn change_answer(&mut self, id: &str, at: f64) -> anyhow::Result<()> {
        let q = self.get_mut(id)?;
        anyhow::ensure!(q.state == QState::Decided, "{id} isn't decided");
        let auto = q.answer.clone().filter(Answer::is_auto);
        if let Some(a) = &auto {
            q.change = Some(Change {
                after: "auto".into(),
                what: a.auto.clone().unwrap_or_default(),
                previous: q.text.clone(),
                answer: auto.clone(),
            });
        }
        self.reopen(id)?;
        if let Some(answer) = auto {
            self.reopens.push(Reopen {
                id: id.to_string(),
                answer,
                at,
            });
        }
        Ok(())
    }

    /// Rewrites a question. An auto answer stays when the rewrite keeps `auto`, and otherwise
    /// opens as a plain question. Any other decided one opens again, keeping what it said
    /// before as a change; an open one changes quietly.
    fn rewrite(&mut self, r: Rewrite, after: &str) -> anyhow::Result<()> {
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
    fn revert(&mut self, id: &str) {
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

    /// The user looked at a rewritten question.
    pub fn seen(&mut self, id: &str) -> anyhow::Result<()> {
        self.get_mut(id)?.change = None;
        Ok(())
    }

    pub fn discuss(&mut self, id: &str, turn: Turn) -> anyhow::Result<()> {
        self.get_mut(id)?.discussion.push(turn);
        Ok(())
    }

    pub fn open_count(&self) -> usize {
        self.questions
            .iter()
            .filter(|q| q.state == QState::Open)
            .count()
    }
}

#[cfg(test)]
#[path = "queue_tests.rs"]
mod tests;
