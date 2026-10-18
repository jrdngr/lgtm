//! Q&A tool parameters, with the JSON schemas agents see.

use crate::qa::queue::{Choice, NewQuestion, Rewrite};
use rmcp::schemars::{self, JsonSchema};
use serde::Deserialize;
#[derive(Deserialize, JsonSchema)]
pub struct QaSession {
    /// The session's id, from list_qa or qa_ask.
    pub session: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct QaChoice {
    pub label: String,
    /// What it means, in a line.
    pub means: String,
    /// What it gets you, a short line each.
    #[serde(default)]
    pub pros: Vec<String>,
    /// What it costs, a short line each.
    #[serde(default)]
    pub cons: Vec<String>,
    /// Downsides worse than a con: what could go badly wrong.
    #[serde(default)]
    pub risks: Vec<String>,
}

impl From<QaChoice> for Choice {
    fn from(o: QaChoice) -> Self {
        Choice {
            label: o.label,
            means: o.means,
            pros: o.pros,
            cons: o.cons,
            risks: o.risks,
            bullets: vec![],
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct QaQuestion {
    /// A subquestion of the top-level question before it.
    #[serde(default)]
    pub sub: bool,
    /// A few words naming the decision.
    pub subject: String,
    /// Only the facts that change the answer, one per bullet.
    pub context: Vec<String>,
    /// The recommended option first, always.
    pub options: Vec<QaChoice>,
    /// Why option 1 is recommended, in a line or two.
    pub why: String,
    /// Files you read to build the question.
    #[serde(default)]
    pub sources: Vec<String>,
    /// Ids of questions whose answers could change this one; leave it empty when none could.
    /// Questions in this call are numbered in the order given, on from the session's last
    /// number (Q1, Q2, ... in a new session), with subquestions lettered under the question
    /// before them (Q2a, Q2b). lgtm moves each question after the ones it depends on, so
    /// the returned ids can differ.
    #[serde(default)]
    pub depends: Vec<String>,
    /// Only when option 1 is obvious and the other options are non-starters: why, in a short
    /// line. lgtm records option 1 as the answer, and the human can reopen it.
    pub auto: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct QaAsk {
    /// The session to add to, from list_qa. Leave it out to start one.
    pub session: Option<String>,
    /// For a new session: what it's deciding, in a line.
    pub title: Option<String>,
    /// For a new session: everything from this conversation that an agent in another
    /// terminal would need to take the session over: the goal, what's settled and why, what
    /// was ruled out, and the human's constraints. Leave out what the repo says.
    pub brief: Option<String>,
    /// For a new session: workspace paths to read before working on its questions.
    #[serde(default)]
    pub read_first: Vec<String>,
    /// In the order to ask them.
    pub questions: Vec<QaQuestion>,
}

#[derive(Deserialize, JsonSchema)]
pub struct QaRewritten {
    /// The question's id, like Q3.
    pub id: String,
    /// What changed, in a few words.
    pub what: String,
    pub subject: String,
    /// Only the facts that change the answer, one per bullet.
    pub context: Vec<String>,
    /// The recommended option first, always.
    pub options: Vec<QaChoice>,
    /// Why option 1 is recommended, in a line or two.
    pub why: String,
    /// Only for an auto-answered question: why option 1 is still obvious, which keeps the auto
    /// answer. Leave it out to have the human decide it.
    pub auto: Option<String>,
}

impl From<QaRewritten> for Rewrite {
    fn from(r: QaRewritten) -> Self {
        Rewrite {
            id: r.id,
            what: r.what,
            subject: r.subject,
            context: r.context,
            options: r.options.into_iter().map(Choice::from).collect(),
            why: r.why,
            auto: r.auto,
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct QaRewrite {
    /// The session, from list_qa or qa_ask.
    pub session: String,
    #[serde(flatten)]
    pub question: QaRewritten,
}

#[derive(Deserialize, JsonSchema)]
pub struct QaQuestionRef {
    /// The session, from list_qa or qa_ask.
    pub session: String,
    /// The question's id, like Q3.
    pub id: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct QaDrop {
    pub id: String,
    /// Why, in a few words, like "settled by Q3".
    pub why: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct QaUpdate {
    /// The session, from list_qa or qa_ask.
    pub session: String,
    /// The question whose answer this applies, like Q3.
    pub after: String,
    /// Open questions the answer settles.
    #[serde(default)]
    pub drop: Vec<QaDrop>,
    /// Questions the answer reshapes, each whole.
    #[serde(default)]
    pub rewrite: Vec<QaRewritten>,
    /// New decisions the answer raises.
    #[serde(default)]
    pub add: Vec<QaQuestion>,
}

#[derive(Deserialize, JsonSchema)]
pub struct QaDecide {
    /// The session, from list_qa or qa_ask.
    pub session: String,
    /// The question's id, like Q3.
    pub id: String,
    /// The option the human picked, counting from 1. Leave it out for an answer outside the
    /// options.
    pub option: Option<u32>,
    /// What the human added to the option.
    pub note: Option<String>,
    /// The human's answer in their words, when it's none of the options.
    pub other: Option<String>,
}

impl From<QaQuestion> for NewQuestion {
    fn from(q: QaQuestion) -> Self {
        NewQuestion {
            sub: q.sub,
            subject: q.subject,
            context: q.context,
            options: q.options.into_iter().map(Choice::from).collect(),
            why: q.why,
            sources: q.sources,
            depends: q.depends,
            auto: q.auto,
        }
    }
}
