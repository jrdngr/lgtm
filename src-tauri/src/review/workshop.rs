//! Workshopping a comment with an agent, which can also make the change it asks for.

use anyhow::Context;
use serde::Deserialize;
use specta::Type;

use super::{RepoCtx, StateDto};
use crate::agent::{self, Runs};
use crate::store::{Author, Comment, Message};

/// A message for a comment's agent.
#[derive(Clone, Deserialize, Type)]
pub struct Ask {
    pub message: String,
    /// The comment's text as it is in the editor, which may not be saved.
    pub draft: String,
    /// Have the agent edit the working copy.
    pub change: bool,
}

pub const NOT_AT_WORKING_COPY: &str = "The agent edits the working copy (@), which this review \
doesn't include, so its changes wouldn't show here. Review a revset that includes @ first.";

const INSTRUCTIONS: &str = "Keep replies short. When you suggest a version of the comment, \
put its full text in a fenced code block tagged `comment`.";

const EDIT: &str = "\n\nEdit the files directly and change only what's needed. Then say in a \
sentence or two what you changed.";

/// One workshop turn. A change asks the agent to edit the code, as the message says or, when
/// it's empty, as the comment says.
fn prompt(review: &str, c: &Comment, ask: &Ask) -> String {
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
    if c.workshop.session.is_some() {
        return turn;
    }
    format!(
        "You're helping me with a code review comment. I'm reviewing `{review}` in this \
         repository. The comment is on this code:\n\n{}\n\n{turn}\n\n{INSTRUCTIONS}",
        c.snippet
    )
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
        let (ws, revset) = {
            let review = self.review(id)?;
            let review = review.lock().unwrap();
            (review.ws.clone(), review.revset.clone())
        };
        if ask.change && !ws.jj.includes_working_copy(&revset).await? {
            anyhow::bail!(NOT_AT_WORKING_COPY);
        }
        let (prompt, resume) = {
            let store = self.store.lock().unwrap();
            let mut c = store.comment(cid)?.context("the comment is gone")?;
            let prompt = prompt(&revset, &c, ask);
            c.workshop.messages.push(Message {
                author: Author::User,
                text: ask.message.clone(),
                change: ask.change,
            });
            store.set_workshop(cid, &c.workshop)?;
            (prompt, c.workshop.session)
        };
        let reply = agent::run(
            runs,
            &self.workshop_key(comment),
            agent::Task {
                cwd: &ws.root,
                prompt: &prompt,
                resume: resume.as_deref(),
                edit: ask.change,
                schema: None,
            },
            |p| {
                if let agent::Progress::Text(t) = p {
                    on_text(t)
                }
            },
        )
        .await;
        let text = match &reply {
            Ok(r) if r.stopped => format!("{} (stopped)", r.text.trim_end()),
            Ok(r) => r.text.clone(),
            Err(e) => format!("{e:#}"),
        };
        let session = reply.ok().and_then(|r| r.session);
        if ask.change {
            // Pick up the edits now rather than waiting on the watcher.
            ws.snapshot().await.ok();
        }
        // Re-read the comment, since an agent may have replied on it during the run.
        self.comment_edit(id, |_, store| {
            let mut c = store.comment(cid)?.context("the comment is gone")?;
            if let Some(s) = session {
                c.workshop.session = Some(s);
            }
            c.workshop.messages.push(Message {
                author: Author::Agent,
                text,
                change: false,
            });
            Ok(store.set_workshop(cid, &c.workshop)?)
        })
    }

    pub fn stop_workshop(&self, comment: u32, runs: &Runs) {
        runs.stop(&self.workshop_key(comment));
    }

    fn workshop_key(&self, comment: u32) -> String {
        self.agent_key(&format!("workshop-{comment}"))
    }
}
