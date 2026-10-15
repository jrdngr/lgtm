//! Workshopping a comment with an agent, which can also make the change it asks for.

use anyhow::Context;
use serde::Deserialize;
use specta::Type;

use super::{RepoCtx, StateDto, proposal, replies};
use crate::agent::{self, Runs};
use crate::store::{Author, Comment, GhLink, Message};

/// A message for a comment's agent.
#[derive(Clone, Deserialize, Type)]
pub struct Ask {
    pub message: String,
    /// The comment's text as it is in the editor, which may not be saved.
    pub draft: String,
    /// Have the agent edit the working copy.
    pub change: bool,
    /// Ask for a first take and reply, with no message of the user's.
    #[serde(default)]
    pub compose: bool,
}

pub const NOT_AT_WORKING_COPY: &str = "The agent edits the working copy (@), which this review \
doesn't include, so its changes wouldn't show here. Review a revset that includes @ first.";

const INSTRUCTIONS: &str = "Keep replies short. Suggest text for the comment only when I ask \
for it, and then put its full text in a fenced code block tagged `comment`.";

const EDIT: &str = "\n\nEdit the files directly and change only what's needed. Then say in a \
sentence or two what you changed.";

/// One workshop turn, with the intro when it's `first` in its session. A change asks the agent
/// to edit the code, as the message says or, when it's empty, as the comment says.
fn prompt(review: &str, c: &Comment, ask: &Ask, first: bool) -> String {
    let (message, change) = (&ask.message, ask.change);
    let draft = if ask.draft.trim().is_empty() {
        "(empty)"
    } else {
        &ask.draft
    };
    let ask = match (change, message.trim().is_empty()) {
        (false, _) => message.clone(),
        (true, true) => format!("Make the change this comment asks for.{EDIT}"),
        (true, false) => format!("Make this change: {message}{EDIT}"),
    };
    let turn = format!("Current draft of the comment:\n\n{draft}\n\n{ask}");
    if !first {
        return turn;
    }
    let thread = match (&c.github, c.quote()) {
        (Some(g), _) => format!(
            "\n\nIt's my reply to this GitHub review thread:\n\n{}",
            thread_text(g)
        ),
        (None, Some(q)) => format!(
            "\n\nIt's my reply to this part of a comment {} wrote on the PR:\n\n> {}",
            q.author,
            q.text.trim().replace('\n', "\n> ")
        ),
        (None, None) => String::new(),
    };
    format!(
        "You're helping me with a code review comment. I'm reviewing `{review}` in this \
         repository. The comment is on this code:\n\n{}{thread}\n\n{turn}\n\n{INSTRUCTIONS}",
        c.snippet
    )
}

/// A GitHub thread's posts, quoted.
fn thread_text(g: &GhLink) -> String {
    g.posts
        .iter()
        .map(|p| format!("{} wrote:\n> {}", p.author, p.body.replace('\n', "\n> ")))
        .collect::<Vec<_>>()
        .join("\n\n")
}

impl RepoCtx {
    /// Sends `ask` to the comment's agent in the review's workspace. Streams the reply through
    /// `on_text` and saves both sides of the exchange. After a change, snapshots the working copy.
    pub async fn workshop(
        &self,
        id: u32,
        comment: u32,
        ask: &Ask,
        runs: &Runs,
        on_text: impl Fn(String),
    ) -> anyhow::Result<StateDto> {
        let cid = comment as i64;
        // Held for the whole run, so the reply saves even if the review closes meanwhile.
        let review = self.review(id)?;
        let (ws, revset) = {
            let review = review.lock().unwrap();
            (review.ws.clone(), review.revset.clone())
        };
        if ask.change && !ws.jj.includes_working_copy(&revset).await? {
            anyhow::bail!(NOT_AT_WORKING_COPY);
        }
        // A change edits the working copy, which mustn't move under it.
        let _busy = ask.change.then(|| ws.claim()).transpose()?;
        let (thread, resume) = {
            let store = self.store.lock().unwrap();
            let c = store.comment(cid)?.context("the comment is gone")?;
            (c.thread.is_some(), c.workshop.session)
        };
        let prompt = self
            .workshop_prompt(id, cid, &revset, ask, resume.is_none())
            .await?;
        {
            let store = self.store.lock().unwrap();
            let mut c = store.comment(cid)?.context("the comment is gone")?;
            c.workshop.messages.push(Message {
                author: Author::User,
                text: ask.message.clone(),
                change: ask.change,
                updated: false,
                compose: ask.compose,
                before: None,
            });
            store.set_workshop(cid, &c.workshop)?;
        }
        let schema = thread.then(replies::chat_schema);
        let key = self.workshop_key(comment);
        let task = |prompt, resume| agent::Task {
            cwd: &ws.root,
            prompt,
            resume,
            edit: ask.change,
            schema: schema.as_deref(),
            purpose: match thread {
                true => agent::Purpose::Replies,
                false => agent::Purpose::Ask,
            },
        };
        let on = |p| {
            if let agent::Progress::Text(t) = p {
                on_text(t)
            }
        };
        let mut reply = agent::run(runs, &key, task(&prompt, resume.as_deref()), &on).await;
        // A session Claude Code no longer has starts over with the intro.
        if resume.is_some() && reply.as_ref().err().is_some_and(agent::session_missing) {
            reply = match self.workshop_prompt(id, cid, &revset, ask, true).await {
                Ok(prompt) => agent::run(runs, &key, task(&prompt, None), &on).await,
                Err(e) => Err(e),
            };
        }
        let text = match &reply {
            Ok(r) if r.stopped => format!("{} (stopped)", r.text.trim_end()),
            Ok(r) => r.text.clone(),
            Err(e) => format!("{e:#}"),
        };
        let mut turn: Option<replies::ChatTurn> = match &reply {
            Ok(r) if !r.stopped => r
                .structured
                .clone()
                .and_then(|v| serde_json::from_value(v).ok()),
            _ => None,
        };
        // Edits from a turn that can't write files become the thread's proposal.
        let proposed = turn
            .as_mut()
            .and_then(|t| t.edits.take())
            .filter(|_| !ask.change)
            .map(|edits| proposal::build(&ws.root, &edits));
        let session = reply.ok().and_then(|r| r.session);
        // Review of a reply suggests a new one rather than replacing it.
        let review_reply = ask.compose && !ask.draft.trim().is_empty();
        if ask.change {
            // Pick up the edits now rather than waiting on the watcher.
            ws.snapshot().await.ok();
        }
        // Re-read the comment, since an agent may have replied on it during the run.
        self.comment_edit_on(&review, |_, store| {
            let mut c = store.comment(cid)?.context("the comment is gone")?;
            if let Some(s) = session {
                c.workshop.session = Some(s);
            }
            let (text, updated, replaced) = match turn {
                Some(turn) => {
                    let before = c.body.clone();
                    let (mut message, mut updated) =
                        replies::apply_chat(&mut c, turn, review_reply);
                    if let (Some((files, problems)), Some(t)) = (proposed, c.thread.as_mut()) {
                        updated |= t.proposal != files;
                        t.proposal = files;
                        for p in problems {
                            message = format!("{message}\n\n{p}");
                        }
                    }
                    if let (true, Some(t)) = (ask.change, c.thread.as_mut()) {
                        let was = t.proposal.clone();
                        proposal::settle(&ws.root, &mut t.proposal);
                        updated |= t.proposal != was;
                    }
                    let replaced = c.body != before;
                    if replaced {
                        store.set_comment_body(cid, &c.body, super::now_ms())?;
                    }
                    store.set_thread(cid, &c.thread)?;
                    (message, updated, replaced.then_some(before))
                }
                None => (text, false, None),
            };
            c.workshop.messages.push(Message {
                author: Author::Agent,
                text,
                change: false,
                updated,
                compose: false,
                before: replaced,
            });
            Ok(store.set_workshop(cid, &c.workshop)?)
        })
    }

    /// The prompt for `ask` on comment `cid`. A thread's chat works on its take and reply,
    /// under the reply rules.
    async fn workshop_prompt(
        &self,
        id: u32,
        cid: i64,
        revset: &str,
        ask: &Ask,
        first: bool,
    ) -> anyhow::Result<String> {
        let thread = {
            let store = self.store.lock().unwrap();
            store
                .comment(cid)?
                .context("the comment is gone")?
                .thread
                .is_some()
        };
        let intro = match thread && first {
            true => {
                let (_, context, rules) = self.replies_context(id).await?;
                Some((context, rules))
            }
            false => None,
        };
        let store = self.store.lock().unwrap();
        let c = store.comment(cid)?.context("the comment is gone")?;
        Ok(match thread {
            true => replies::chat_prompt(&c, ask, intro.as_ref()),
            false => prompt(revset, &c, ask, first),
        })
    }

    pub fn stop_workshop(&self, comment: u32, runs: &Runs) {
        runs.stop(&self.workshop_key(comment));
    }

    fn workshop_key(&self, comment: u32) -> String {
        self.agent_key(&format!("workshop-{comment}"))
    }
}
