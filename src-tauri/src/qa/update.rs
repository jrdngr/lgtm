//! Applying an answer to the questions that depend on it. The terminal agent reads what
//! depends on an answer and sends back drops, rewrites and new questions.

use anyhow::Context;

use super::Session;
use super::queue::{self, QState};
use super::text;

/// How a rewrite keeps or gives up an auto answer.
pub const REAUTO_RULE: &str = "When you rewrite an auto-answered question, set `auto` to why \
option 1 is still obvious if it is, in a short line, or leave it out so I decide it. Leave \
`auto` out for every other question.";

/// The answer to `qid`, the questions that depend on it, and how to update them.
pub fn dependents_text(s: &Session, qid: &str) -> anyhow::Result<String> {
    let q = &s.queue;
    let answered = q.get(qid).with_context(|| format!("no question {qid}"))?;
    let listed: Vec<String> = q
        .dependents(qid)
        .iter()
        .map(|d| match d.state {
            QState::Decided if d.answer.as_ref().is_some_and(queue::Answer::is_auto) => format!(
                "Auto-answered ({}):\n\n{}",
                d.answer
                    .as_ref()
                    .and_then(|a| a.auto.as_deref())
                    .unwrap_or(""),
                text::question_text(d)
            ),
            QState::Decided => format!(
                "Decided ({}):\n\n{}",
                text::answer_text(d),
                text::question_text(d)
            ),
            _ => format!("Open:\n\n{}", text::question_text(d)),
        })
        .collect();
    if listed.is_empty() {
        return Ok(format!("Nothing depends on {qid}. Don't call qa_update."));
    }
    Ok(format!(
        "I answered {qid} ({}): {}.\n\nApply that to the questions below, which depend on it, \
         with one qa_update call (after: {qid}). Drop the open ones it settles, with why in a \
         few words (\"settled by {qid}\"). Rewrite the ones it reshapes, saying in a few words \
         what changed. Rewrite a decided one only if this answer means I have to answer it \
         again; rewrite an auto-answered one if this answer reshapes it. {REAUTO_RULE} Add any \
         new decision it raises, with its depends; new questions number on from Q{}. Leave out \
         questions it doesn't change, and call qa_update with empty lists when it changes none. \
         Then say in a line what changed, and stop.\n\n{}",
        answered.text.subject,
        text::answer_text(answered),
        q.last_number(),
        listed.join("\n\n---\n\n")
    ))
}
