//! Talking a question over with the terminal agent. The user writes in the question's card,
//! the agent answers in its terminal, and both land in the question's discussion. The agent's
//! tools rewrite or decide the question.

use anyhow::Context;

use super::Session;
use super::queue::{Answer, QState, QaQuestion, Turn};
use super::text;
use super::update::REAUTO_RULE;
use crate::ctx::{RepoCtx, now_ms};
use crate::workspace::Workspace;

fn is_auto(q: &QaQuestion) -> bool {
    q.state == QState::Decided && q.answer.as_ref().is_some_and(Answer::is_auto)
}

/// How to discuss question `q`.
fn rules(q: &QaQuestion) -> String {
    let qid = &q.id;
    let auto = if is_auto(q) {
        format!(" {qid} is auto-answered. {REAUTO_RULE}")
    } else {
        String::new()
    };
    format!(
        "We're discussing {qid}: answer briefly. If the discussion changes its context, options \
         or recommendation, rewrite it with qa_rewrite.{auto} If I pick an option or otherwise \
         settle it, record that with qa_decide. Don't ask me new questions here: after I decide, \
         you update the open questions."
    )
}

/// Question `qid`, its discussion so far and how to discuss it, for the agent.
pub fn question_text(s: &Session, qid: &str) -> anyhow::Result<String> {
    let q = s
        .queue
        .get(qid)
        .with_context(|| format!("no question {qid}"))?;
    let talk: Vec<String> = q
        .discussion
        .iter()
        .map(|t| format!("{}: {}", if t.you { "Me" } else { "You" }, t.text))
        .collect();
    let talk = if talk.is_empty() {
        String::new()
    } else {
        format!("\n\nOur discussion so far:\n\n{}", talk.join("\n\n"))
    };
    let state = match q.state {
        QState::Decided => format!("Decided: {}", text::answer_text(q)),
        QState::Open => "Open".into(),
        QState::Deferred => "Deferred".into(),
        QState::Dropped => "Dropped".into(),
    };
    Ok(format!(
        "{}\n\n{state}{talk}\n\n{}",
        text::question_text(q),
        rules(q)
    ))
}

/// A decision made in a discussion. `option` counts from 1.
pub struct Decision {
    pub option: Option<u32>,
    pub note: Option<String>,
    pub other: Option<String>,
}

impl RepoCtx {
    /// Adds a turn to question `qid`'s discussion.
    pub fn add_qa_turn(
        &self,
        ws: &Workspace,
        id: &str,
        qid: &str,
        turn: Turn,
    ) -> anyhow::Result<Session> {
        self.edit_qa(ws, id, |s| s.queue.discuss(qid, turn))?;
        self.qa_session(ws, id)
    }

    /// Decides open question `qid` with what the user settled in its discussion.
    pub fn decide_in_discussion(
        &self,
        ws: &Workspace,
        id: &str,
        qid: &str,
        d: Decision,
    ) -> anyhow::Result<()> {
        let other = d.other.filter(|o| !o.trim().is_empty());
        anyhow::ensure!(
            d.option.is_some_and(|o| o >= 1) || other.is_some(),
            "give an option from 1, or `other` with the answer in the user's words"
        );
        self.edit_qa(ws, id, |s| {
            let q = s
                .queue
                .get(qid)
                .with_context(|| format!("no question {qid}"))?;
            anyhow::ensure!(q.state == QState::Open, "{qid} isn't open");
            s.queue.decide(
                qid,
                Answer {
                    option: d.option.and_then(|o| o.checked_sub(1)),
                    note: d.note.filter(|n| !n.trim().is_empty()),
                    other,
                    at: now_ms() as f64,
                    auto: None,
                },
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qa::queue::{Choice, NewQuestion};

    fn question(subject: &str) -> NewQuestion {
        NewQuestion {
            sub: false,
            subject: subject.into(),
            context: vec![],
            options: ["a", "b"]
                .map(|l| Choice {
                    label: l.into(),
                    ..Choice::default()
                })
                .to_vec(),
            why: "w".into(),
            sources: vec![],
            depends: vec![],
            auto: None,
        }
    }

    #[tokio::test]
    async fn a_discussion_decides_only_an_open_question_with_an_answer() {
        let (_dir, ctx) = crate::review::tests::repo();
        let ws = ctx.workspace(None).await.unwrap();
        let id = ctx.create_qa(&ws, "p", vec![]).unwrap().id;
        ctx.edit_qa(&ws, &id, |s| Ok(s.queue.add_round(vec![question("x")])))
            .unwrap();
        let pick = |option, other: Option<&str>| Decision {
            option,
            note: Some(" ".into()),
            other: other.map(Into::into),
        };
        assert!(
            ctx.decide_in_discussion(&ws, &id, "Q1", pick(None, Some(" ")))
                .is_err()
        );
        ctx.decide_in_discussion(&ws, &id, "Q1", pick(Some(2), None))
            .unwrap();
        let q = ctx.qa_session(&ws, &id).unwrap().queue;
        let answer = q.get("Q1").unwrap().answer.clone().unwrap();
        assert_eq!((answer.option, answer.note), (Some(1), None));
        assert!(
            ctx.decide_in_discussion(&ws, &id, "Q1", pick(Some(1), None))
                .is_err(),
            "already decided"
        );
    }
}
