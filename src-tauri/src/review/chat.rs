//! A thread's chat: what the user and the linked terminal agent said while it was the current
//! thread, and the reply the agent suggests.

use anyhow::Context;

use super::threads::after_edit;
use super::{RepoCtx, StateDto, now_ms};
use crate::store::{Author, Message};

impl RepoCtx {
    /// Adds a message to a comment's chat. Blank text adds nothing.
    pub fn add_chat_message(
        &self,
        id: u32,
        comment: u32,
        author: Author,
        text: &str,
    ) -> anyhow::Result<StateDto> {
        self.comment_edit(id, |_, store| {
            let mut c = store
                .comment(comment as i64)?
                .context("the comment is gone")?;
            let text = text.trim();
            if text.is_empty() {
                return Ok(());
            }
            c.conversation.messages.push(Message {
                author,
                text: text.to_string(),
            });
            Ok(store.set_conversation(c.id, &c.conversation)?)
        })
    }

    /// Sets a thread's suggested reply, for a terminal agent. It replaces any earlier one.
    pub fn agent_suggest_reply(
        &self,
        workspace: Option<&str>,
        thread: u32,
        text: &str,
    ) -> anyhow::Result<(u32, StateDto)> {
        let id = self.current_review(workspace)?;
        let state = self.comment_edit(id, |_, store| {
            let mut c = store.comment(thread as i64)?.context("no such thread")?;
            anyhow::ensure!(c.thread.is_some(), "that's a note, not a thread");
            anyhow::ensure!(!text.trim().is_empty(), "the reply is empty");
            c.conversation.suggestion = Some(text.trim().to_string());
            Ok(store.set_conversation(c.id, &c.conversation)?)
        })?;
        Ok((id, state))
    }

    /// Drops a thread's suggested reply.
    pub fn dismiss_suggestion(&self, id: u32, comment: u32) -> anyhow::Result<StateDto> {
        self.comment_edit(id, |_, store| {
            let mut c = store
                .comment(comment as i64)?
                .context("the thread is gone")?;
            c.conversation.suggestion = None;
            Ok(store.set_conversation(c.id, &c.conversation)?)
        })
    }

    /// Makes a thread's suggested reply its reply, moving it along as an edit would.
    pub fn take_suggestion(&self, id: u32, comment: u32) -> anyhow::Result<StateDto> {
        self.comment_edit(id, |_, store| {
            let cid = comment as i64;
            let mut c = store.comment(cid)?.context("the thread is gone")?;
            let body = c
                .conversation
                .suggestion
                .take()
                .context("no suggested reply")?;
            let draft = c.is_draft();
            if let Some(t) = c.thread.as_mut()
                && c.body != body
            {
                t.stage = after_edit(t.stage, draft, &body);
                store.set_thread(cid, &c.thread)?;
            }
            store.set_comment_body(cid, &body, now_ms())?;
            Ok(store.set_conversation(cid, &c.conversation)?)
        })
    }
}
