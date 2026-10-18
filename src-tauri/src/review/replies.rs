//! Agent steps on a PR's threads: triage gives each new thread a take, a summary and a draft
//! reply. The rules for takes and replies are the user's `reply.md` prompt. Agents never post anything.

use std::collections::HashSet;
use std::sync::Arc;

use anyhow::Context;
use serde::Deserialize;

use super::{RepoCtx, StateDto, ThreadDto};
use crate::agent::{self, Progress, Runs};
use crate::prompts::{self, Prompt};
use crate::store::{Comment, Stage, Take, TakeVerdict};
use crate::workspace::Workspace;

const VERDICTS: &str = r#"{ "enum": ["agree", "change", "disagree", "decide", "answer"] }"#;

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
struct List<T> {
    threads: Vec<T>,
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
        (_, TakeVerdict::Decide, _) => Stage::Decide,
        (_, TakeVerdict::Agree, false) | (_, TakeVerdict::Change, _) => Stage::ToFix,
        _ => Stage::Reply,
    }
}

/// A thread for a prompt: its id, where it is, the code, what was said, and where it stands.
pub(super) fn thread_text(c: &Comment) -> String {
    let mut out = match c.quote() {
        Some(q) => format!(
            "Thread {} on part of a comment {} wrote on the PR itself:\n> {}",
            c.id,
            q.author,
            q.text.trim().replace('\n', "\n> ")
        ),
        None => github_text(c),
    };
    if let Some(t) = c.thread.as_ref().and_then(|t| t.take.as_ref()) {
        out.push_str(&format!("\nThe take so far: {:?}: {}", t.verdict, t.why));
    }
    if !c.body.trim().is_empty() {
        out.push_str(&format!("\nMy reply so far:\n{}", c.body.trim()));
    }
    out
}

/// Where a GitHub thread or draft is, its posts, and its code.
fn github_text(c: &Comment) -> String {
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

    /// Threads on review `id`'s PR at `stage`. A comment on the PR that has quotes is left to
    /// them.
    pub(super) fn threads_at(&self, id: u32, stage: Stage) -> anyhow::Result<Vec<Comment>> {
        let ws = self.review_workspace(id)?;
        let pr = self
            .pr_record(ws.name.as_deref())?
            .context("this workspace isn't a PR's")?;
        let all = self
            .store
            .lock()
            .unwrap()
            .pr_threads(&pr.repo.to_string(), pr.number)?;
        let quoted: HashSet<i64> = all
            .iter()
            .filter_map(|c| c.quote())
            .map(|q| q.of as i64)
            .collect();
        Ok(all
            .into_iter()
            .filter(|c| !quoted.contains(&c.id))
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
             I'd fix it exactly as asked), and for decide the question I have to answer.\n\n{}",
            threads.join("\n\n---\n\n")
        );
        let schema = triage_schema();
        let reply = agent::run_registered(
            run,
            agent::Task {
                cwd: &ws.root,
                prompt: &prompt,
                resume: None,
                fork: false,
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

    /// The threads of the review open in `workspace`, for a terminal agent.
    pub fn agent_threads(&self, workspace: Option<&str>) -> anyhow::Result<Vec<ThreadDto>> {
        let (_, threads) = self.with_current(workspace, |r| r.threads.clone())?;
        anyhow::ensure!(
            !threads.is_empty(),
            "this workspace's PR has no threads in lgtm"
        );
        Ok(threads)
    }

    /// Sets a thread's take, and its reply when given, for a terminal agent. The thread moves
    /// to the stage triage would put it in.
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
            let before = c.body.clone();
            let verdict = take.verdict;
            set_take(&mut c, take, reply);
            // Sorted like triage, unless the reply is already final or sent.
            let reply = !c.body.trim().is_empty();
            if let Some(t) = c.thread.as_mut()
                && !matches!(t.stage, Stage::Ready | Stage::Done)
            {
                t.stage = triaged_stage(false, verdict, reply);
            }
            if c.body != before {
                store.set_comment_body(c.id, &c.body, super::now_ms())?;
            }
            store.set_conversation(c.id, &c.conversation)?;
            Ok(store.set_thread(c.id, &c.thread)?)
        })?;
        Ok((id, state))
    }

    /// Moves a thread to `stage`, for a terminal agent.
    pub fn agent_set_stage(
        &self,
        workspace: Option<&str>,
        thread: u32,
        stage: Stage,
    ) -> anyhow::Result<(u32, StateDto)> {
        let id = self.current_review(workspace)?;
        Ok((id, self.set_stage(id, thread, stage)?))
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
    }
}

/// Gives a thread a new take, and a new reply when one is given. A reply on a thread that has
/// one becomes its suggested reply, so the agent never overwrites mine, and a done thread keeps
/// the reply it has.
fn set_take(c: &mut Comment, new: Take, reply: Option<String>) {
    let draft = c.is_draft();
    let Some(t) = c.thread.as_mut() else {
        return;
    };
    t.take = Some(take(new.verdict, new.why, new.question));
    let Some(reply) = reply.filter(|r| !r.trim().is_empty() && r.trim() != c.body.trim()) else {
        return;
    };
    if t.stage == Stage::Done {
        return;
    }
    if c.body.trim().is_empty() {
        c.body = reply;
        t.stage = super::threads::after_edit(t.stage, draft, &c.body);
    } else {
        c.conversation.suggestion = Some(reply.trim().to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::ThreadState;

    #[test]
    fn triage_sorts_threads_into_work() {
        use TakeVerdict::*;
        assert_eq!(triaged_stage(true, Disagree, true), Stage::Done);
        assert_eq!(triaged_stage(false, Decide, true), Stage::Decide);
        assert_eq!(triaged_stage(false, Agree, false), Stage::ToFix);
        assert_eq!(triaged_stage(false, Change, true), Stage::ToFix);
        assert_eq!(triaged_stage(false, Agree, true), Stage::Reply);
        assert_eq!(triaged_stage(false, Disagree, true), Stage::Reply);
    }

    #[test]
    fn a_new_take_suggests_a_reply_instead_of_overwriting_mine() {
        let mut c = Comment {
            id: 1,
            path: "a.rs".into(),
            pins: vec![],
            body: "old".into(),
            snippet: String::new(),
            review: String::new(),
            created_at: 0,
            updated_at: 0,
            conversation: Default::default(),
            github: None,
            thread: Some(ThreadState::new("o/n", 1, Stage::Ready)),
            done: false,
        };
        let new = || Take {
            verdict: TakeVerdict::Disagree,
            why: "2 is enough".into(),
            question: Some(" ".into()),
        };
        set_take(&mut c, new(), Some("new\n".into()));
        let t = c.thread.as_ref().unwrap();
        assert_eq!(c.body, "old");
        assert_eq!(c.conversation.suggestion.as_deref(), Some("new"));
        assert_eq!(t.take.as_ref().unwrap().question, None);
        assert_eq!(t.stage, Stage::Ready);
        // No reply, or the same one, leaves the suggestion there is.
        set_take(&mut c, new(), None);
        set_take(&mut c, new(), Some("old".into()));
        assert_eq!(c.conversation.suggestion.as_deref(), Some("new"));
        // With no reply yet, it's the reply.
        c.body = String::new();
        c.conversation.suggestion = None;
        set_take(&mut c, new(), Some("first".into()));
        assert_eq!(c.body, "first");
        assert_eq!(c.conversation.suggestion, None);
    }
}
