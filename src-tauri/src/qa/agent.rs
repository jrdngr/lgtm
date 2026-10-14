//! The agent steps of a Q&A session: building the questions from the prompt, applying each
//! answer to the questions that depend on it, writing a plan, and applying the decisions to
//! the repo. Discussing a question is in `discuss`. Every step continues one conversation per
//! session.

use anyhow::Context;
use serde::Deserialize;

use super::queue::{self, NewQuestion, QState, Update};
use super::{Applied, Plan, PlanFile, Session};
use crate::agent::{self, Progress, Runs};
use crate::prompts::{self, Prompt};
use crate::review::{RepoCtx, now_ms};
use crate::workspace::Workspace;

const CHOICE: &str = r#"{
  "type": "object",
  "properties": {
    "label": { "type": "string" },
    "bullets": { "type": "array", "items": { "type": "string" } }
  },
  "required": ["label", "bullets"],
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

fn new_question() -> String {
    format!(
        r#"{{
      "type": "object",
      "properties": {{
        "sub": {{ "type": "boolean" }},
        {},
        "sources": {{ "type": "array", "items": {{ "type": "string" }} }},
        "depends": {{ "type": "array", "items": {{ "type": "string" }} }}
      }},
      "required": ["sub", "subject", "context", "options", "why", "sources", "depends"],
      "additionalProperties": false
    }}"#,
        question_props()
    )
}

pub(super) fn rewrite() -> String {
    format!(
        r#"{{
      "type": "object",
      "properties": {{
        "id": {{ "type": "string" }},
        "what": {{ "type": "string" }},
        {}
      }},
      "required": ["id", "what", "subject", "context", "options", "why"],
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
        rewrite(),
        new_question()
    )
}

fn ask_schema() -> String {
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
        new_question()
    )
}

const PLAN_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "files": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "path": { "type": "string" },
          "change": { "enum": ["new", "edit", "delete"] },
          "edits": { "type": "array", "items": { "type": "string" } },
          "decisions": { "type": "array", "items": { "type": "string" } }
        },
        "required": ["path", "change", "edits", "decisions"],
        "additionalProperties": false
      }
    }
  },
  "required": ["files"],
  "additionalProperties": false
}"#;

#[derive(Deserialize)]
struct Asked {
    title: String,
    questions: Vec<NewQuestion>,
}

#[derive(Deserialize)]
struct Planned {
    files: Vec<PlanFile>,
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
    pub edit: bool,
    pub schema: Option<String>,
    /// Starts a new conversation instead of continuing the session's.
    pub fresh: bool,
    /// The turn to send instead when a new conversation starts, for a `turn` that leans on
    /// earlier ones.
    pub restart: Option<String>,
    /// `turn` lists the decisions, so a new conversation's context leaves them out.
    pub decided: bool,
}

/// Every decision, a line each, for the plan and the write.
fn decisions(s: &Session) -> String {
    s.queue
        .questions
        .iter()
        .filter(|q| q.state == QState::Decided)
        .map(|q| format!("- {} ({}): {}", q.id, q.text.subject, queue::answer_text(q)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The decisions for a turn that lists them, saying so when there are none.
fn decided_list(s: &Session) -> String {
    let d = decisions(s);
    if d.is_empty() {
        "- None: nothing needed deciding.".into()
    } else {
        d
    }
}

/// The questions still open, as a paragraph telling the agent to leave them out of `what`.
fn still_open(s: &Session, what: &str) -> String {
    let open = s
        .queue
        .questions
        .iter()
        .filter(|q| q.state == QState::Open)
        .map(|q| format!("- {} ({})", q.id, q.text.subject))
        .collect::<Vec<_>>();
    if open.is_empty() {
        String::new()
    } else {
        format!(
            "These are still open; leave them out of the {what}:\n\n{}\n\n",
            open.join("\n")
        )
    }
}

/// The deferred questions, as a paragraph telling the agent to leave them unimplemented.
fn deferred(s: &Session) -> String {
    let put_off = s
        .queue
        .questions
        .iter()
        .filter(|q| q.state == QState::Deferred)
        .map(|q| format!("{} ({})", q.id, q.text.subject))
        .collect::<Vec<_>>();
    if put_off.is_empty() {
        String::new()
    } else {
        format!("Deferred, leave unimplemented: {}\n\n", put_off.join(", "))
    }
}

/// The files the user asked the agent to read first, as a line after the prompt.
fn read_first(s: &Session) -> String {
    if s.attach.is_empty() {
        String::new()
    } else {
        format!("\n\nRead these first: {}", s.attach.join(", "))
    }
}

/// What a conversation that starts partway through the session needs to know, with the
/// decisions unless the turn has them.
fn context(s: &Session, with_decisions: bool) -> String {
    let decided = if with_decisions {
        decisions(s)
    } else {
        String::new()
    };
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

/// The plan as text, with the user's comments on it.
fn plan_text(plan: &Plan) -> String {
    let mut out = String::new();
    for f in &plan.files {
        let change = match f.change {
            super::FileChange::New => "new",
            super::FileChange::Edit => "edit",
            super::FileChange::Delete => "delete",
        };
        out.push_str(&format!(
            "- {} ({change}; {})\n",
            f.path,
            f.decisions.join(", ")
        ));
        for e in &f.edits {
            out.push_str(&format!("  - {e}\n"));
        }
    }
    for c in &plan.comments {
        out.push_str(&format!(
            "\nMy comment on \"{}\": {}\n",
            c.quote.trim(),
            c.body.trim()
        ));
    }
    out
}

/// A path from a tool call, relative to the workspace root.
fn written(tool: &str, root: &std::path::Path) -> Option<String> {
    let path = tool
        .strip_prefix("Edit ")
        .or_else(|| tool.strip_prefix("Write "))?;
    let rel = std::path::Path::new(path)
        .strip_prefix(root)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string());
    Some(rel)
}

/// The opening of the session's conversation: where things are and the rules.
fn qa_intro() -> anyhow::Result<String> {
    let qq = prompts::load(Prompt::Qq)?;
    let write = prompts::load(Prompt::QaWrite)?;
    Ok(format!(
        "We're working through the decisions for a change to this repository, one question at \
         a time, before you write it. You're in the repository and can read any file. Don't \
         edit files until I ask you to apply. You can't run commands.\n\n{qq}\n\n{write}"
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
                format!("{intro}\n\n{}\n\n{turn}", context(&s, !step.decided))
            })
        };
        let prompt = match session {
            Some(_) => step.turn.clone(),
            None => first()?,
        };
        let key = self.qa_key(ws, id);
        let _busy = if step.edit { Some(ws.claim()?) } else { None };
        let task = |prompt| agent::Task {
            cwd: &ws.root,
            prompt,
            resume: None,
            edit: step.edit,
            schema: step.schema.as_deref(),
            purpose: agent::Purpose::Qa,
        };
        let mut reply = agent::run(
            runs,
            &key,
            agent::Task {
                resume: session.as_deref(),
                ..task(&prompt)
            },
            &on,
        )
        .await;
        // A session Claude Code no longer has starts over with the intro.
        if session.is_some() && reply.as_ref().err().is_some_and(agent::session_missing) {
            let prompt = first()?;
            reply = agent::run(runs, &key, task(&prompt), &on).await;
        }
        if step.edit {
            ws.snapshot().await.ok();
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

    /// Has the agent turn the prompt into questions. Replaces the queue.
    pub async fn ask_qa(
        &self,
        ws: &Workspace,
        id: &str,
        runs: &Runs,
        on_tool: impl Fn(String),
    ) -> anyhow::Result<Session> {
        let s = self.qa_session(ws, id)?;
        let turn = format!(
            "Here's what I want:\n\n{}{}\n\nRead whatever in the repo it touches. \
             Then return a short title for it and every question I need to answer before you \
             write it, in the order to ask them.",
            s.prompt.trim(),
            read_first(&s)
        );
        let reply = self
            .qa_step(
                ws,
                id,
                runs,
                Step {
                    turn,
                    edit: false,
                    schema: Some(ask_schema()),
                    fresh: true,
                    restart: None,
                    decided: false,
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
                s.plan = None;
                s.sent = None;
                s.applied = None;
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
                QState::Decided => format!(
                    "Decided ({}):\n\n{}",
                    queue::answer_text(d),
                    queue::question_text(d)
                ),
                _ => format!("Open:\n\n{}", queue::question_text(d)),
            })
            .collect();
        let turn = format!(
            "I answered {qid} ({}): {}.\n\nApply that to the questions below, which depend on \
             it. Drop the open ones it settles, with why in a few words (\"settled by {qid}\"). \
             Rewrite the ones it reshapes, saying in a few words what changed. Rewrite a decided \
             one only if this answer means I have to answer it again. Add any new decision it \
             raises, with its depends; new questions number on from Q{}. Leave out questions it \
             doesn't change.\n\n{}",
            answered.text.subject,
            queue::answer_text(answered),
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
                    edit: false,
                    schema: Some(update_schema()),
                    fresh: false,
                    restart: None,
                    decided: false,
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

    /// Has the agent plan the write from the decisions, or revise its plan with the user's
    /// comments on it. Nothing is written.
    pub async fn write_plan(
        &self,
        ws: &Workspace,
        id: &str,
        runs: &Runs,
        on_tool: impl Fn(String),
    ) -> anyhow::Result<Session> {
        let s = self.qa_session(ws, id)?;
        let (turn, decided) = match &s.plan {
            Some(plan) if !plan.comments.is_empty() => (
                format!(
                    "Revise your plan with my comments. Don't edit files yet.\n\n{}",
                    plan_text(plan)
                ),
                false,
            ),
            _ => (
                format!(
                    "These are decided:\n\n{}\n\n{}{}{}Plan the write by the planning \
                     rules. Don't edit files yet.",
                    decided_list(&s),
                    still_open(&s, "plan"),
                    deferred(&s),
                    note_line(&s.note)
                ),
                true,
            ),
        };
        let reply = self
            .qa_step(
                ws,
                id,
                runs,
                Step {
                    turn,
                    edit: false,
                    schema: Some(PLAN_SCHEMA.into()),
                    fresh: false,
                    restart: None,
                    decided,
                },
                tool_only(on_tool),
            )
            .await?;
        if !reply.stopped {
            let Planned { files } = parse(&reply, "plan")?;
            self.edit_qa(ws, id, |s| {
                s.plan = Some(Plan {
                    files,
                    comments: vec![],
                });
                Ok(())
            })?;
        }
        self.qa_session(ws, id)
    }

    /// Has the agent write the decisions into the workspace's @, following the plan when
    /// there is one. Archives the session when it succeeds.
    pub async fn apply_qa(
        &self,
        ws: &Workspace,
        id: &str,
        runs: &Runs,
        on_tool: impl Fn(String),
    ) -> anyhow::Result<Session> {
        let s = self.qa_session(ws, id)?;
        let plan = s
            .plan
            .as_ref()
            .map(|p| format!("Follow this plan:\n\n{}\n\n", plan_text(p)))
            .unwrap_or_default();
        let turn = format!(
            "Apply it now: write the changes the prompt and these decisions call for, by the \
             applying rules.\n\n{}\n\n{}{}{}{plan}",
            decided_list(&s),
            still_open(&s, "write"),
            deferred(&s),
            note_line(&s.note)
        );
        let files = std::sync::Mutex::new(Vec::<String>::new());
        let reply = self
            .qa_step(
                ws,
                id,
                runs,
                Step {
                    turn,
                    edit: true,
                    schema: None,
                    fresh: false,
                    restart: None,
                    decided: true,
                },
                |p| {
                    if let Progress::Tool(t) = p {
                        if let Some(f) = written(&t, &ws.root) {
                            let mut files = files.lock().unwrap();
                            if !files.contains(&f) {
                                files.push(f);
                            }
                        }
                        on_tool(t)
                    }
                },
            )
            .await?;
        if !reply.stopped {
            let files = files.into_inner().unwrap();
            self.edit_qa(ws, id, |s| {
                s.applied = Some(Applied {
                    summary: reply.text.trim().to_string(),
                    files,
                    at: now_ms() as f64,
                });
                s.archived = true;
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

fn note_line(note: &str) -> String {
    if note.trim().is_empty() {
        String::new()
    } else {
        format!("Also: {}\n\n", note.trim())
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
        for s in [ask_schema(), update_schema(), rewrite(), PLAN_SCHEMA.into()] {
            serde_json::from_str::<serde_json::Value>(&s).unwrap();
        }
    }

    #[test]
    fn writes_are_read_from_tool_calls() {
        let root = std::path::Path::new("/r/ws");
        assert_eq!(
            written("Edit /r/ws/src/a.rs", root).as_deref(),
            Some("src/a.rs")
        );
        assert_eq!(written("Write /tmp/x", root).as_deref(), Some("/tmp/x"));
        assert_eq!(written("Read /r/ws/src/a.rs", root), None);
    }
}
