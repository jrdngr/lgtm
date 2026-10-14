//! A Q&A session's question queue: questions in ask order, each built the qqueue way (context,
//! options with the recommendation first, the reason for it), and what was decided. Answers
//! reshape the open questions through updates from the agent.

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Choice {
    pub label: String,
    /// What it means and what it costs.
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
}

/// A rewrite the user hasn't looked at yet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    /// What it was rewritten after: a question id, or "discussion".
    pub after: String,
    /// What changed, in a few words.
    pub what: String,
    pub previous: QText,
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

fn number(id: &str) -> u32 {
    id.trim_start_matches('Q')
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .unwrap_or(0)
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

    /// Appends questions in `round`, numbering on from the last one.
    fn push(&mut self, new: Vec<NewQuestion>, round: u32) -> Vec<String> {
        let mut top = self
            .questions
            .iter()
            .map(|q| number(&q.id))
            .max()
            .unwrap_or(0);
        let mut parent: Option<(u32, u8)> = None;
        let mut ids = vec![];
        for n in new {
            let id = match (n.sub, parent) {
                (true, Some((p, letter))) => {
                    parent = Some((p, letter + 1));
                    format!("Q{p}{}", (b'a' + letter) as char)
                }
                _ => {
                    top += 1;
                    parent = Some((top, 0));
                    format!("Q{top}")
                }
            };
            ids.push(id.clone());
            self.questions.push(QaQuestion {
                id,
                round,
                text: n.text(),
                sources: n.sources,
                state: QState::Open,
                answer: None,
                dropped: None,
                change: None,
                discussion: vec![],
            });
        }
        ids
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

    /// Reopens the last decision and undoes what it did to the other questions. Returns its
    /// id.
    pub fn undo(&mut self) -> Option<String> {
        let last = self
            .questions
            .iter_mut()
            .filter(|q| q.state == QState::Decided)
            .max_by(|a, b| {
                let at = |q: &QaQuestion| q.answer.as_ref().map_or(0.0, |a| a.at);
                at(a).total_cmp(&at(b))
            })?;
        last.state = QState::Open;
        last.answer = None;
        let id = last.id.clone();
        self.revert(&id);
        Some(id)
    }

    /// Reopens a decided question so it can be answered again.
    pub fn reopen(&mut self, id: &str) -> anyhow::Result<()> {
        let q = self.get_mut(id)?;
        q.state = QState::Open;
        q.answer = None;
        Ok(())
    }

    /// Moves a question to the end of the queue.
    pub fn later(&mut self, id: &str) -> anyhow::Result<()> {
        let i = self
            .questions
            .iter()
            .position(|q| q.id == id)
            .ok_or_else(|| anyhow::anyhow!("no question {id}"))?;
        let q = self.questions.remove(i);
        self.questions.push(q);
        Ok(())
    }

    /// Rewrites a question, keeping what it said before. A decided one opens again.
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
        // A second rewrite before it's seen still compares with what the user last saw.
        let previous = q.change.take().map_or(q.text.clone(), |c| c.previous);
        q.text = text;
        q.state = QState::Open;
        q.answer = None;
        q.change = Some(Change {
            after: after.to_string(),
            what: r.what,
            previous,
        });
        Ok(())
    }

    /// Applies what the answer to `after` does to the rest of the queue. Decided questions
    /// are never dropped or rewritten; ids the queue doesn't have are skipped.
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
            let Some(was) = self.get(&r.id).filter(|q| q.state == QState::Open).cloned() else {
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
    /// still open get their old text, and added ones nobody has decided or discussed go.
    fn revert(&mut self, id: &str) {
        let Some(i) = self.effects.iter().position(|e| e.after == id) else {
            return;
        };
        let effect = self.effects.remove(i);
        for was in effect.before {
            if let Some(q) = self.questions.iter_mut().find(|q| q.id == was.id)
                && q.state != QState::Decided
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

/// An answer in words, for the agent and for copying.
pub fn answer_text(q: &QaQuestion) -> String {
    let Some(a) = &q.answer else {
        return "not decided".into();
    };
    let mut out = match (a.option, &a.other) {
        (Some(o), _) => {
            let label = q
                .text
                .options
                .get(o as usize)
                .map_or("", |c| c.label.as_str());
            format!("option {}: {label}", o + 1)
        }
        (None, Some(other)) => format!("none of the options: {other}"),
        (None, None) => "none of the options".into(),
    };
    if let Some(note) = a.note.as_deref().filter(|n| !n.trim().is_empty()) {
        out.push_str(&format!(" (note: {})", note.trim()));
    }
    out
}

/// A question in the qqueue template, as plain text.
pub fn question_text(q: &QaQuestion) -> String {
    let t = &q.text;
    let mut out = format!("{} — {}\n\nContext:\n\n", q.id, t.subject);
    for c in &t.context {
        out.push_str(&format!("- {c}\n"));
    }
    out.push_str("\nOptions:\n\n");
    for (i, o) in t.options.iter().enumerate() {
        let rec = if i == 0 { " (recommended)" } else { "" };
        out.push_str(&format!("{}. {}{rec}\n", i + 1, o.label));
        for b in &o.bullets {
            out.push_str(&format!("   - {b}\n"));
        }
    }
    out.push_str(&format!("\nRecommendation: option 1. {}", t.why));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new(subject: &str, sub: bool) -> NewQuestion {
        NewQuestion {
            sub,
            subject: subject.into(),
            context: vec!["c".into()],
            options: vec![
                Choice {
                    label: "yes".into(),
                    bullets: vec![],
                },
                Choice {
                    label: "no".into(),
                    bullets: vec![],
                },
            ],
            why: "because".into(),
            sources: vec![],
        }
    }

    fn pick(option: u32, at: f64) -> Answer {
        Answer {
            option: Some(option),
            note: None,
            other: None,
            at,
        }
    }

    fn ids(q: &Queue) -> Vec<&str> {
        q.questions.iter().map(|q| q.id.as_str()).collect()
    }

    #[test]
    fn subquestions_number_under_their_parent_and_rounds_continue() {
        let mut q = Queue::default();
        q.add_round(vec![
            new("a", false),
            new("b", false),
            new("b1", true),
            new("b2", true),
        ]);
        assert_eq!(ids(&q), ["Q1", "Q2", "Q2a", "Q2b"]);
        q.add_round(vec![new("c", true), new("d", false)]);
        // A subquestion with no parent in its batch starts a new number.
        assert_eq!(ids(&q)[4..], ["Q3", "Q4"]);
        assert_eq!(q.get("Q4").unwrap().round, 2);
    }

    #[test]
    fn undo_reopens_the_latest_decision() {
        let mut q = Queue::default();
        q.add_round(vec![new("a", false), new("b", false)]);
        q.decide("Q2", pick(1, 5.0)).unwrap();
        q.decide("Q1", pick(0, 9.0)).unwrap();
        assert!(q.decide("Q1", pick(2, 10.0)).is_err());
        assert_eq!(q.undo().as_deref(), Some("Q1"));
        assert_eq!(q.get("Q1").unwrap().state, QState::Open);
        assert_eq!(q.undo().as_deref(), Some("Q2"));
        assert_eq!(q.undo(), None);
    }

    #[test]
    fn an_update_never_rewrites_a_decided_question() {
        let mut q = Queue::default();
        q.add_round(vec![new("a", false), new("b", false)]);
        q.decide("Q1", pick(0, 1.0)).unwrap();
        q.decide("Q2", pick(1, 2.0)).unwrap();
        q.apply(
            "Q2",
            Update {
                rewrite: vec![Rewrite {
                    id: "Q1".into(),
                    what: "new option".into(),
                    subject: "a2".into(),
                    context: vec![],
                    options: vec![],
                    why: String::new(),
                }],
                ..Update::default()
            },
        );
        let a = q.get("Q1").unwrap();
        assert_eq!(a.state, QState::Decided);
        assert_eq!(a.answer, Some(pick(0, 1.0)));
        assert_eq!(a.text.subject, "a");
    }

    #[test]
    fn an_update_drops_rewrites_and_adds_without_touching_decisions() {
        let mut q = Queue::default();
        q.add_round(vec![new("a", false), new("b", false), new("c", false)]);
        q.decide("Q1", pick(0, 1.0)).unwrap();
        let rw = |id: &str, subject: &str| Rewrite {
            id: id.into(),
            what: "new option".into(),
            subject: subject.into(),
            context: vec![],
            options: vec![],
            why: String::new(),
        };
        q.apply(
            "Q1",
            Update {
                drop: vec![
                    Drop {
                        id: "Q2".into(),
                        why: "settled by Q1".into(),
                    },
                    Drop {
                        id: "Q1".into(),
                        why: "self".into(),
                    },
                ],
                rewrite: vec![rw("Q3", "c2"), rw("Q9", "nope")],
                add: vec![new("d", false)],
            },
        );
        assert_eq!(q.get("Q1").unwrap().state, QState::Decided);
        assert_eq!(
            q.get("Q2").unwrap().dropped.as_deref(),
            Some("settled by Q1")
        );
        let c = q.get("Q3").unwrap();
        assert_eq!(c.text.subject, "c2");
        assert_eq!(c.change.as_ref().unwrap().previous.subject, "c");
        assert_eq!(ids(&q), ["Q1", "Q2", "Q3", "Q4"]);
        assert_eq!(q.open_count(), 2);

        // A second rewrite keeps the text the user last saw as the previous one.
        q.apply(
            "Q1",
            Update {
                rewrite: vec![rw("Q3", "c3")],
                ..Update::default()
            },
        );
        assert_eq!(
            q.get("Q3")
                .unwrap()
                .change
                .as_ref()
                .unwrap()
                .previous
                .subject,
            "c"
        );

        // Undoing Q1 puts back everything its updates did, and drops the question they added.
        assert_eq!(q.undo().as_deref(), Some("Q1"));
        assert_eq!(ids(&q), ["Q1", "Q2", "Q3"]);
        assert_eq!(q.get("Q2").unwrap().state, QState::Open);
        assert_eq!(q.get("Q3").unwrap().text.subject, "c");
        assert!(q.get("Q3").unwrap().change.is_none());
        assert!(q.effects.is_empty());
    }

    #[test]
    fn rewriting_a_decided_question_asks_it_again() {
        let mut q = Queue::default();
        q.add_round(vec![new("a", false), new("b", false)]);
        q.decide("Q2", pick(1, 1.0)).unwrap();
        q.reask(Rewrite {
            id: "Q2".into(),
            what: "narrowed".into(),
            subject: "b?".into(),
            context: vec![],
            options: vec![],
            why: String::new(),
        })
        .unwrap();
        let b = q.get("Q2").unwrap();
        assert_eq!((b.state, b.answer.is_none()), (QState::Open, true));
        assert_eq!(b.change.as_ref().unwrap().after, "discussion");
    }

    #[test]
    fn later_moves_a_question_to_the_end() {
        let mut q = Queue::default();
        q.add_round(vec![new("a", false), new("b", false), new("c", false)]);
        q.later("Q1").unwrap();
        assert_eq!(ids(&q), ["Q2", "Q3", "Q1"]);
    }

    #[test]
    fn the_text_follows_the_qqueue_template() {
        let mut q = Queue::default();
        q.add_round(vec![new("Where files live", false)]);
        q.decide(
            "Q1",
            Answer {
                note: Some("keep it simple".into()),
                ..pick(1, 1.0)
            },
        )
        .unwrap();
        let first = q.get("Q1").unwrap();
        assert_eq!(answer_text(first), "option 2: no (note: keep it simple)");
        assert_eq!(
            question_text(first),
            "Q1 — Where files live\n\nContext:\n\n- c\n\nOptions:\n\n1. yes (recommended)\n\
             2. no\n\nRecommendation: option 1. because"
        );
    }
}
