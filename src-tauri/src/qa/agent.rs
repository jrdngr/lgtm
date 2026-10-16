//! The agent steps of a Q&A session: building the questions from the prompt, and applying
//! each answer to the questions that depend on it. Discussing a question is in `discuss`.
//! Every step continues one conversation per session.

use anyhow::Context;
use serde::Deserialize;

use super::queue::{self, NewQuestion, QState, Update};
use super::text;
use super::Session;
use crate::agent::{self, Progress, Runs};
use crate::prompts::{self, Prompt};
use crate::review::RepoCtx;
use crate::workspace::Workspace;

const CHOICE: &str = r#"{
  "type": "object",
  "properties": {
    "label": { "type": "string" },
    "means": { "type": "string" },
    "pros": { "type": "array", "items": { "type": "string" } },
    "cons": { "type": "array", "items": { "type": "string" } },
    "risks": { "type": "array", "items": { "type": "string" } }
  },
  "required": ["label", "means", "pros", "cons", "risks"],
  "additionalProperties": false
}"#;

fn question_props() -> String {
    format!(
        r#""subject": {{ "type": "string" }},
        "context": {{ "type": "array", "items": {{ "type": "string" }} }},
        "options": {{ "type": "array", "items": {CHOICE}, "minItems": 1 }},
        "why": {{ "type": "string" }}"#
    )
}

/// A new question, with `auto` for the agent's own answer when it may give one.
fn new_question(auto: bool) -> String {
    let (prop, req) = if auto {
        (r#", "auto": { "type": "string" }"#, r#", "auto""#)
    } else {
        ("", "")
    };
    format!(
        r#"{{
      "type": "object",
      "properties": {{
        "sub": {{ "type": "boolean" }},
        {},
        "sources": {{ "type": "array", "items": {{ "type": "string" }} }},
        "depends": {{ "type": "array", "items": {{ "type": "string" }} }}{prop}
      }},
      "required": ["sub", "subject", "context", "options", "why", "sources", "depends"{req}],
      "additionalProperties": false
    }}"#,
        question_props()
    )
}

/// A rewritten question, with `auto` for keeping an auto answer when it may have one.
pub(super) fn rewrite(auto: bool) -> String {
    let (prop, req) = if auto {
        (r#", "auto": { "type": "string" }"#, r#", "auto""#)
    } else {
        ("", "")
    };
    format!(
        r#"{{
      "type": "object",
      "properties": {{
        "id": {{ "type": "string" }},
        "what": {{ "type": "string" }},
        {}{prop}
      }},
      "required": ["id", "what", "subject", "context", "options", "why"{req}],
      "additionalProperties": false
    }}"#,
        question_props()
    )
}

fn update_schema() -> String {
    format!(
        r#"{{
  "type": "object",
  "properties": {{
    "drop": {{
      "type": "array",
      "items": {{
        "type": "object",
        "properties": {{ "id": {{ "type": "string" }}, "why": {{ "type": "string" }} }},
        "required": ["id", "why"],
        "additionalProperties": false
      }}
    }},
    "rewrite": {{ "type": "array", "items": {} }},
    "add": {{ "type": "array", "items": {} }}
  }},
  "required": ["drop", "rewrite", "add"],
  "additionalProperties": false
}}"#,
        rewrite(true),
        new_question(false)
    )
}

fn ask_schema(auto: bool) -> String {
    format!(
        r#"{{
  "type": "object",
  "properties": {{
    "title": {{ "type": "string" }},
    "questions": {{ "type": "array", "items": {} }}
  }},
  "required": ["title", "questions"],
  "additionalProperties": false
}}"#,
        new_question(auto)
    )
}

const AUTO_RULE: &str = "Answer the obvious questions yourself: when option 1 is clearly right \
and the other options are non-starters, still write the question in full, and set `auto` to \
why, in a short line. Leave `auto` empty for every question I should decide.";

/// How a rewrite keeps or gives up an auto answer.
pub(super) const REAUTO_RULE: &str = "When you rewrite an auto-answered question, set `auto` to \
why option 1 is still obvious if it is, in a short line, or leave it empty so I decide it. \
Leave `auto` empty for every other question.";

#[derive(Deserialize)]
struct Asked {
    title: String,
    questions: Vec<NewQuestion>,
}

pub(super) fn parse<T: serde::de::DeserializeOwned>(
    reply: &agent::Reply,
    what: &str,
) -> anyhow::Result<T> {
    let value = reply
        .structured
        .clone()
        .with_context(|| format!("the {what} returned no result"))?;
    serde_json::from_value(value)
        .with_context(|| format!("the {what} returned an unexpected result"))
}

/// One step of the session's conversation.
pub(super) struct Step {
    pub turn: String,
    pub schema: Option<String>,
    /// Starts a new conversation instead of continuing the session's.
    pub fresh: bool,
    /// The turn to send instead when a new conversation starts, for a `turn` that leans on
    /// earlier ones.
    pub restart: Option<String>,
}

/// Every decision, a line each.
fn decisions(s: &Session) -> String {
    s.queue
        .questions
        .iter()
        .filter(|q| q.state == QState::Decided)
        .map(|q| format!("- {} ({}): {}", q.id, q.text.subject, text::answer_text(q)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The files the user asked the agent to read first, as a line after the prompt.
fn read_first(s: &Session) -> String {
    if s.attach.is_empty() {
        String::new()
    } else {
        format!("\n\nRead these first: {}", s.attach.join(", "))
    }
}

/// What a conversation that starts partway through the session needs to know.
fn context(s: &Session) -> String {
    let decided = decisions(s);
    let decided = if decided.is_empty() {
        String::new()
    } else {
        format!("\n\nDecided so far:\n\n{decided}")
    };
    format!(
        "Here's what I want:\n\n{}{}{decided}",
        s.prompt.trim(),
        read_first(s)
    )
}

/// The opening of the session's conversation: where things are and the rules.
fn qa_intro() -> anyhow::Result<String> {
    let qq = prompts::load(Prompt::Qq)?;
    Ok(format!(
        "We're working through the decisions for a change to this repository, one question at \
         a time, before it's written. You're in the repository and can read any file. Don't \
         edit files. You can't run commands.\n\n{qq}"
    ))
}

impl RepoCtx {
    fn qa_key(&self, ws: &Workspace, id: &str) -> String {
        self.agent_key(&format!("qa-{}-{id}", ws.name.as_deref().unwrap_or("")))
    }

    pub(super) async fn qa_step(
        &self,
        ws: &Workspace,
        id: &str,
        runs: &Runs,
        step: Step,
        on: impl Fn(Progress),
    ) -> anyhow::Result<agent::Reply> {
        let s = self.qa_session(ws, id)?;
        let session = if step.fresh { None } else { s.agent.clone() };
        let first = || -> anyhow::Result<String> {
            let intro = qa_intro()?;
            let turn = step.restart.as_ref().unwrap_or(&step.turn);
            Ok(if step.fresh {
                format!("{intro}\n\n{turn}")
            } else {
                format!("{intro}\n\n{}\n\n{turn}", context(&s))
            })
        };
        let prompt = match session {
            Some(_) => step.turn.clone(),
            None => first()?,
        };
        let key = self.qa_key(ws, id);
        let task = |prompt| agent::Task {
            cwd: &ws.root,
            prompt,
            resume: None,
            fork: false,
            edit: false,
            schema: step.schema.as_deref(),
            purpose: agent::Purpose::Qa,
        };
        // Until it has a conversation of its own, the agent on a terminal agent's questions
        // starts from a copy of that agent's conversation, so it knows what was said there.
        let fork = session
            .is_none()
            .then_some(s.fork_from.as_deref())
            .flatten();
        let resume = session.as_deref().or(fork);
        let mut reply = agent::run(
            runs,
            &key,
            agent::Task {
                resume,
                fork: fork.is_some(),
                ..task(&prompt)
            },
            &on,
        )
        .await;
        // A session Claude Code no longer has starts over with the intro.
        if resume.is_some() && reply.as_ref().err().is_some_and(agent::session_missing) {
            let prompt = first()?;
            reply = agent::run(runs, &key, task(&prompt), &on).await;
        }
        let reply = reply?;
        if !reply.stopped && reply.session.is_some() {
            self.edit_qa(ws, id, |s| {
                s.agent = reply.session.clone();
                Ok(())
            })?;
        }
        Ok(reply)
    }

    /// Has the agent turn the prompt into questions, answering the obvious ones itself when
    /// `auto` is on. Replaces the queue.
    pub async fn ask_qa(
        &self,
        ws: &Workspace,
        id: &str,
        auto: bool,
        runs: &Runs,
        on_tool: impl Fn(String),
    ) -> anyhow::Result<Session> {
        let s = self.qa_session(ws, id)?;
        let mut turn = format!(
            "Here's what I want:\n\n{}{}\n\nRead whatever in the repo it touches. \
             Then return a short title for it and every question I need to answer before you \
             write it, in the order to ask them.",
            s.prompt.trim(),
            read_first(&s)
        );
        if auto {
            turn.push_str(&format!("\n\n{AUTO_RULE}"));
        }
        let reply = self
            .qa_step(
                ws,
                id,
                runs,
                Step {
                    turn,
                    schema: Some(ask_schema(auto)),
                    fresh: true,
                    restart: None,
                },
                tool_only(on_tool),
            )
            .await?;
        if !reply.stopped {
            let Asked { title, questions } = parse(&reply, "questions")?;
            self.edit_qa(ws, id, |s| {
                if !title.trim().is_empty() {
                    s.title = title.trim().to_string();
                }
                s.queue = queue::Queue::default();
                s.queue.add_round(questions);
                s.asked = true;
                s.sent = None;
                Ok(())
            })?;
        }
        self.qa_session(ws, id)
    }

    /// Applies the answer to `qid` to the questions that depend on it: drops, rewrites and
    /// new ones. Skips the agent when nothing depends on it.
    pub async fn apply_answer(
        &self,
        ws: &Workspace,
        id: &str,
        qid: &str,
        runs: &Runs,
    ) -> anyhow::Result<Session> {
        let q = self.qa_session(ws, id)?.queue;
        let answered = q.get(qid).with_context(|| format!("no question {qid}"))?;
        let dependents = q.dependents(qid);
        if dependents.is_empty() {
            return self.qa_session(ws, id);
        }
        let listed: Vec<String> = dependents
            .iter()
            .map(|d| match d.state {
                QState::Decided if d.answer.as_ref().is_some_and(queue::Answer::is_auto) => {
                    format!(
                        "Auto-answered ({}):\n\n{}",
                        d.answer
                            .as_ref()
                            .and_then(|a| a.auto.as_deref())
                            .unwrap_or(""),
                        text::question_text(d)
                    )
                }
                QState::Decided => format!(
                    "Decided ({}):\n\n{}",
                    text::answer_text(d),
                    text::question_text(d)
                ),
                _ => format!("Open:\n\n{}", text::question_text(d)),
            })
            .collect();
        let turn = format!(
            "I answered {qid} ({}): {}.\n\nApply that to the questions below, which depend on \
             it. Drop the open ones it settles, with why in a few words (\"settled by {qid}\"). \
             Rewrite the ones it reshapes, saying in a few words what changed. Rewrite a decided \
             one only if this answer means I have to answer it again; rewrite an auto-answered \
             one if this answer reshapes it. {REAUTO_RULE} Add any new decision it raises, with \
             its depends; new questions number on from Q{}. Leave out questions it doesn't \
             change.\n\n{}",
            answered.text.subject,
            text::answer_text(answered),
            q.last_number(),
            listed.join("\n\n---\n\n")
        );
        let reply = self
            .qa_step(
                ws,
                id,
                runs,
                Step {
                    turn,
                    schema: Some(update_schema()),
                    fresh: false,
                    restart: None,
                },
                |_| {},
            )
            .await?;
        if !reply.stopped {
            let update: Update = parse(&reply, "update")?;
            self.edit_qa(ws, id, |s| {
                s.queue.apply(qid, update);
                Ok(())
            })?;
        }
        self.qa_session(ws, id)
    }

    /// Stops whatever the agent is doing on the session.
    pub fn stop_qa(&self, ws: &Workspace, id: &str, runs: &Runs) {
        runs.stop_all(&self.qa_key(ws, id));
    }
}

fn tool_only(on_tool: impl Fn(String)) -> impl Fn(Progress) {
    move |p| {
        if let Progress::Tool(t) = p {
            on_tool(t)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_schemas_are_json() {
        for s in [
            ask_schema(true),
            ask_schema(false),
            update_schema(),
            rewrite(true),
            rewrite(false),
        ] {
            serde_json::from_str::<serde_json::Value>(&s).unwrap();
        }
    }
}
