//! Agent steps on a PR's threads: triage gives each new thread a take, a summary and a draft
//! reply; a fix run changes the code for the threads to fix. The rules for takes and replies
//! are the user's `reply.md` prompt. Agents never post anything.

use std::sync::Arc;

use anyhow::Context;
use serde::Deserialize;

use super::workshop::Ask;
use super::{RepoCtx, StateDto, ThreadDto};
use crate::agent::{self, Progress, Runs};
use crate::prompts::{self, Prompt};
use crate::store::{Author, Comment, Message, Stage, Take, TakeVerdict};
use crate::workspace::Workspace;

const VERDICTS: &str = r#"{ "enum": ["agree", "agreeWithChanges", "pushBack", "needsYou"] }"#;

fn triage_schema() -> String {
    format!(
        r#"{{
  "type": "object",
  "properties": {{
    "threads": {{
      "type": "array",
      "items": {{
        "type": "object",
        "properties": {{
          "id": {{ "type": "integer" }},
          "skip": {{ "type": "boolean" }},
          "summary": {{ "type": "string" }},
          "verdict": {VERDICTS},
          "why": {{ "type": "string" }},
          "reply": {{ "type": "string" }},
          "question": {{ "type": "string" }}
        }},
        "required": ["id", "skip", "summary", "verdict", "why"],
        "additionalProperties": false
      }}
    }}
  }},
  "required": ["threads"],
  "additionalProperties": false
}}"#
    )
}

const FIX_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "threads": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "id": { "type": "integer" },
          "fixed": { "type": "boolean" },
          "differs": { "type": "boolean" },
          "note": { "type": "string" },
          "reply": { "type": "string" }
        },
        "required": ["id", "fixed", "differs", "note"],
        "additionalProperties": false
      }
    }
  },
  "required": ["threads"],
  "additionalProperties": false
}"#;

/// The schema a chat turn on a thread answers with.
pub(super) fn chat_schema() -> String {
    format!(
        r#"{{
  "type": "object",
  "properties": {{
    "message": {{ "type": "string" }},
    "verdict": {VERDICTS},
    "why": {{ "type": "string" }},
    "question": {{ "type": "string" }},
    "reply": {{ "type": "string" }}
  }},
  "required": ["message"],
  "additionalProperties": false
}}"#
    )
}

#[derive(Deserialize)]
struct Triaged {
    id: i64,
    skip: bool,
    summary: String,
    verdict: TakeVerdict,
    why: String,
    reply: Option<String>,
    question: Option<String>,
}

#[derive(Deserialize)]
struct Fixed {
    id: i64,
    fixed: bool,
    differs: bool,
    note: String,
    reply: Option<String>,
}

#[derive(Deserialize)]
struct List<T> {
    threads: Vec<T>,
}

/// One chat turn's answer: what to say, and the take and reply when they change.
#[derive(Deserialize)]
pub(super) struct ChatTurn {
    pub message: String,
    pub verdict: Option<TakeVerdict>,
    pub why: Option<String>,
    pub question: Option<String>,
    pub reply: Option<String>,
}

fn take(verdict: TakeVerdict, why: String, question: Option<String>) -> Take {
    let question = question.filter(|q| !q.trim().is_empty());
    Take {
        verdict,
        why,
        question,
    }
}

/// Where triage puts a thread: a call for the user, code to change (as asked or otherwise),
/// or a reply to work out.
fn triaged_stage(skip: bool, verdict: TakeVerdict, reply: bool) -> Stage {
    match (skip, verdict, reply) {
        (true, _, _) => Stage::Done,
        (_, TakeVerdict::NeedsYou, _) => Stage::Decide,
        (_, TakeVerdict::Agree, false) | (_, TakeVerdict::AgreeWithChanges, _) => Stage::ToFix,
        _ => Stage::Reply,
    }
}

/// Where a fixed thread goes: ready when it was fixed as asked with nothing to say, else to
/// its reply.
fn fixed_stage(fixed: bool, has_reply: bool) -> Stage {
    match (fixed, has_reply) {
        (false, _) => Stage::ToFix,
        (true, false) => Stage::Ready,
        (true, true) => Stage::Reply,
    }
}

/// A thread for a prompt: its id, where it is, the code, what was said, and where it stands.
pub(super) fn thread_text(c: &Comment) -> String {
    let at = if c.path.is_empty() {
        "a comment on the PR itself".to_string()
    } else {
        match c.github.as_ref().and_then(|g| g.line) {
            Some(line) => format!("{}:{line}", c.path),
            None => c.path.clone(),
        }
    };
    let mut out = format!("Thread {} on {at}", c.id);
    if let Some(g) = &c.github {
        if g.outdated {
            out.push_str(" (outdated: the code moved since)");
        }
        for p in &g.posts {
            out.push_str(&format!(
                "\n{} wrote:\n> {}",
                p.author,
                p.body.trim().replace('\n', "\n> ")
            ));
        }
    }
    if !c.snippet.is_empty() {
        out.push_str(&format!("\nThe code:\n```diff\n{}\n```", c.snippet));
    }
    if let Some(t) = c.thread.as_ref().and_then(|t| t.take.as_ref()) {
        out.push_str(&format!("\nThe take so far: {:?}: {}", t.verdict, t.why));
    }
    if !c.body.trim().is_empty() {
        out.push_str(&format!("\nMy reply so far:\n{}", c.body.trim()));
    }
    out
}

impl RepoCtx {
    /// Context for an agent working on review `id`'s PR threads: the change and the prompt.
    pub(super) async fn replies_context(
        &self,
        id: u32,
    ) -> anyhow::Result<(Arc<Workspace>, String, String)> {
        let (ws, revset, from, to) = {
            let review = self.review(id)?;
            let r = review.lock().unwrap();
            (r.ws.clone(), r.revset.clone(), r.from.clone(), r.to.clone())
        };
        let pr = self
            .pr_record(ws.name.as_deref())?
            .context("this workspace isn't a PR's")?;
        let rules = prompts::load(Prompt::Reply)?;
        let change = crate::bots::describe(&ws, &revset, from, to).await?;
        let context = format!(
            "You're working on review comments on PR #{} ({}) by {}.\n\n{change}",
            pr.number, pr.title, pr.author
        );
        Ok((ws, context, rules))
    }

    /// Threads on review `id`'s PR at `stage`.
    fn threads_at(&self, id: u32, stage: Stage) -> anyhow::Result<Vec<Comment>> {
        let ws = self.review_workspace(id)?;
        let pr = self
            .pr_record(ws.name.as_deref())?
            .context("this workspace isn't a PR's")?;
        Ok(self
            .store
            .lock()
            .unwrap()
            .pr_threads(&pr.repo.to_string(), pr.number)?
            .into_iter()
            .filter(|c| c.thread.as_ref().is_some_and(|t| t.stage == stage))
            .collect())
    }

    /// Triages the new threads on review `id`'s PR. `on_tool` hears each tool call.
    pub async fn triage(
        &self,
        id: u32,
        runs: &Runs,
        on_tool: impl Fn(String),
    ) -> anyhow::Result<StateDto> {
        let new = self.threads_at(id, Stage::New)?;
        if new.is_empty() {
            return self.comment_edit(id, |_, _| Ok(()));
        }
        let run = runs.register(&self.agent_key(&format!("triage-{id}")))?;
        let (ws, context, rules) = self.replies_context(id).await?;
        let threads: Vec<String> = new.iter().map(thread_text).collect();
        let prompt = format!(
            "{context}\n\nYou're in the repository and can read any file. Don't edit files.\n\n\
             {rules}\n\nTriage each thread below. Return one result per thread with its id: \
             skip, a three to six word summary, your verdict and why, the reply (left out when \
             I'd fix it exactly as asked), and for needsYou the question I have to answer.\n\n{}",
            threads.join("\n\n---\n\n")
        );
        let schema = triage_schema();
        let reply = agent::run_registered(
            run,
            agent::Task {
                cwd: &ws.root,
                prompt: &prompt,
                resume: None,
                edit: false,
                schema: Some(&schema),
                purpose: agent::Purpose::Replies,
            },
            |p| {
                if let Progress::Tool(t) = p {
                    on_tool(t)
                }
            },
        )
        .await?;
        if reply.stopped {
            return self.comment_edit(id, |_, _| Ok(()));
        }
        let list: List<Triaged> =
            serde_json::from_value(reply.structured.context("triage returned no result")?)
                .context("triage returned an unexpected result")?;
        self.comment_edit(id, |_, store| {
            for r in list.threads {
                let Some(c) = store.comment(r.id)? else {
                    continue;
                };
                // Threads moved by hand while it ran keep where they are.
                let Some(mut t) = c.thread.filter(|t| t.stage == Stage::New) else {
                    continue;
                };
                let reply = r.reply.filter(|s| !s.trim().is_empty());
                t.stage = triaged_stage(r.skip, r.verdict, reply.is_some());
                t.summary = r.summary;
                t.take = Some(take(r.verdict, r.why, r.question));
                store.set_thread(c.id, &Some(t))?;
                if let (Some(reply), true) = (reply, c.body.trim().is_empty()) {
                    store.set_comment_body(c.id, &reply, super::now_ms())?;
                }
            }
            Ok(())
        })
    }

    /// Has an agent change the code in `@` for the threads to fix. `on_tool` hears each tool
    /// call.
    pub async fn fix_threads(
        &self,
        id: u32,
        runs: &Runs,
        on_tool: impl Fn(String),
    ) -> anyhow::Result<StateDto> {
        let to_fix = self.threads_at(id, Stage::ToFix)?;
        anyhow::ensure!(!to_fix.is_empty(), "no threads to fix");
        let run = runs.register(&self.agent_key(&format!("fix-threads-{id}")))?;
        let (ws, context, rules) = self.replies_context(id).await?;
        let revset = self.review(id)?.lock().unwrap().revset.clone();
        anyhow::ensure!(
            ws.jj.includes_working_copy(&revset).await?,
            super::NOT_AT_WORKING_COPY
        );
        let _busy = ws.claim()?;
        let threads: Vec<String> = to_fix.iter().map(thread_text).collect();
        let prompt = format!(
            "{context}\n\nYou're in the repository. Make the changes these review threads ask \
             for by editing the files directly, changing only what's needed. You can't run \
             commands.\n\n{rules}\n\nReturn one result per thread with its id: whether you \
             fixed it, whether your fix differs from what the reviewer asked, a sentence on \
             what you did, and when it differs, a reply saying what changed and why.\n\n{}",
            threads.join("\n\n---\n\n")
        );
        let reply = agent::run_registered(
            run,
            agent::Task {
                cwd: &ws.root,
                prompt: &prompt,
                resume: None,
                edit: true,
                schema: Some(FIX_SCHEMA),
                purpose: agent::Purpose::Replies,
            },
            |p| {
                if let Progress::Tool(t) = p {
                    on_tool(t)
                }
            },
        )
        .await;
        // Pick up the edits now rather than waiting on the watcher.
        ws.snapshot().await.ok();
        let reply = reply?;
        if reply.stopped {
            return self.comment_edit(id, |_, _| Ok(()));
        }
        let list: List<Fixed> =
            serde_json::from_value(reply.structured.context("the fix returned no result")?)
                .context("the fix returned an unexpected result")?;
        self.comment_edit(id, |_, store| {
            for r in list.threads {
                let Some(mut c) = store.comment(r.id)? else {
                    continue;
                };
                let Some(mut t) = c.thread.take().filter(|t| t.stage == Stage::ToFix) else {
                    continue;
                };
                let reply = r.reply.filter(|s| r.differs && !s.trim().is_empty());
                if let (Some(reply), true) = (&reply, c.body.trim().is_empty()) {
                    store.set_comment_body(c.id, reply, super::now_ms())?;
                }
                let has_reply = reply.is_some() || !c.body.trim().is_empty();
                t.stage = fixed_stage(r.fixed, has_reply);
                store.set_thread(c.id, &Some(t))?;
                c.workshop.messages.push(Message {
                    author: Author::Agent,
                    text: format!("Fix: {}", r.note),
                    change: false,
                    updated: false,
                });
                store.set_workshop(c.id, &c.workshop)?;
            }
            Ok(())
        })
    }

    /// The threads of the review open in `workspace`, for a terminal agent.
    pub fn agent_threads(&self, workspace: Option<&str>) -> anyhow::Result<Vec<ThreadDto>> {
        let (_, threads) = self.with_current(workspace, |r| r.threads.clone())?;
        anyhow::ensure!(
            !threads.is_empty(),
            "this workspace's PR has no threads in lgtm"
        );
        Ok(threads)
    }

    /// Sets a thread's take, and its reply when given, for a terminal agent.
    pub fn agent_set_take(
        &self,
        workspace: Option<&str>,
        thread: u32,
        take: Take,
        reply: Option<String>,
    ) -> anyhow::Result<(u32, StateDto)> {
        let id = self.current_review(workspace)?;
        let state = self.comment_edit(id, |_, store| {
            let mut c = store.comment(thread as i64)?.context("no such thread")?;
            anyhow::ensure!(c.thread.is_some(), "that's a note, not a thread");
            let turn = ChatTurn {
                message: String::new(),
                verdict: Some(take.verdict),
                why: Some(take.why),
                question: take.question,
                reply,
            };
            let before = c.body.clone();
            apply_chat(&mut c, turn);
            if c.body != before {
                store.set_comment_body(c.id, &c.body, super::now_ms())?;
            }
            Ok(store.set_thread(c.id, &c.thread)?)
        })?;
        Ok((id, state))
    }

    /// Marks a thread's reply final, for a terminal agent.
    pub fn agent_thread_ready(
        &self,
        workspace: Option<&str>,
        thread: u32,
    ) -> anyhow::Result<(u32, StateDto)> {
        let id = self.current_review(workspace)?;
        Ok((id, self.set_stage(id, thread, Stage::Ready)?))
    }

    pub fn stop_thread_run(&self, id: u32, runs: &Runs) {
        runs.stop(&self.agent_key(&format!("triage-{id}")));
        runs.stop(&self.agent_key(&format!("fix-threads-{id}")));
    }
}

/// A chat turn on a thread. The first carries the change, the rules, and the thread.
pub(super) fn chat_prompt(c: &Comment, ask: &Ask, intro: Option<&(String, String)>) -> String {
    const EDIT: &str = "Edit the files directly and change only what's needed.";
    let asked = match (ask.change, ask.message.trim().is_empty()) {
        (false, _) => ask.message.clone(),
        (true, true) => format!("Make the change this thread asks for. {EDIT}"),
        (true, false) => format!("Make this change: {} {EDIT}", ask.message.trim()),
    };
    let draft = if ask.draft.trim().is_empty() {
        "(empty)"
    } else {
        &ask.draft
    };
    let turn = format!(
        "My reply draft now:\n\n{draft}\n\n{asked}\n\nAnswer in `message`, briefly. When the \
         take or the reply should change, also return the new verdict and why, and the whole \
         new reply."
    );
    match intro {
        None => turn,
        Some((context, rules)) => format!(
            "{context}\n\nYou're in the repository and can read any file.\n\n{rules}\n\nWe're \
             working on this thread together:\n\n{}\n\n{turn}",
            thread_text(c)
        ),
    }
}

/// Applies a chat turn to its thread: a new take and reply replace the old ones. Returns the
/// message to show and whether anything was updated.
pub(super) fn apply_chat(c: &mut Comment, turn: ChatTurn) -> (String, bool) {
    let Some(t) = c.thread.as_mut() else {
        return (turn.message, false);
    };
    let mut updated = false;
    if let (Some(verdict), Some(why)) = (turn.verdict, turn.why) {
        t.take = Some(take(verdict, why, turn.question));
        updated = true;
    }
    if let Some(reply) = turn.reply.filter(|r| !r.trim().is_empty())
        && reply != c.body
    {
        c.body = reply;
        t.stage = super::threads::after_edit(t.stage, c.github.is_none(), &c.body);
        updated = true;
    }
    (turn.message, updated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::ThreadState;

    #[test]
    fn triage_sorts_threads_into_work() {
        use TakeVerdict::*;
        assert_eq!(triaged_stage(true, PushBack, true), Stage::Done);
        assert_eq!(triaged_stage(false, NeedsYou, true), Stage::Decide);
        assert_eq!(triaged_stage(false, Agree, false), Stage::ToFix);
        assert_eq!(triaged_stage(false, AgreeWithChanges, true), Stage::ToFix);
        assert_eq!(triaged_stage(false, Agree, true), Stage::Reply);
        assert_eq!(triaged_stage(false, PushBack, true), Stage::Reply);
    }

    #[test]
    fn a_fix_that_needs_saying_goes_to_its_reply() {
        assert_eq!(fixed_stage(true, false), Stage::Ready);
        assert_eq!(fixed_stage(true, true), Stage::Reply);
        assert_eq!(fixed_stage(false, false), Stage::ToFix);
    }

    #[test]
    fn a_chat_turn_rewrites_take_and_reply_together() {
        let mut c = Comment {
            id: 1,
            path: "a.rs".into(),
            pins: vec![],
            body: "old".into(),
            snippet: String::new(),
            review: String::new(),
            created_at: 0,
            updated_at: 0,
            workshop: Default::default(),
            github: None,
            thread: Some(ThreadState::new("o/n", 1, Stage::Ready)),
            done: false,
        };
        let turn = |reply: Option<&str>| ChatTurn {
            message: "ok".into(),
            verdict: Some(TakeVerdict::PushBack),
            why: Some("2 is enough".into()),
            question: Some(" ".into()),
            reply: reply.map(String::from),
        };
        assert_eq!(apply_chat(&mut c, turn(Some("new"))), ("ok".into(), true));
        let t = c.thread.as_ref().unwrap();
        assert_eq!(c.body, "new");
        assert_eq!(t.take.as_ref().unwrap().question, None);
        // A draft with text stays ready.
        assert_eq!(t.stage, Stage::Ready);
        // Saying nothing new about the reply leaves it.
        let mut quiet = turn(None);
        quiet.verdict = None;
        assert_eq!(apply_chat(&mut c, quiet), ("ok".into(), false));
        assert_eq!(c.body, "new");
    }
}
