//! Quotes: threads that each answer one part of a comment on the PR itself, so a comment that
//! asks several things gets an answer per part.

use anyhow::Context;

use super::{RepoCtx, StateDto, now_ms};
use crate::store::{Comment, Quote, Stage, Workshop};

impl RepoCtx {
    /// Starts a thread on `text`, quoted from comment `of` on the PR.
    pub fn add_quote(&self, id: u32, of: u32, text: &str) -> anyhow::Result<StateDto> {
        let text = text.trim();
        anyhow::ensure!(!text.is_empty(), "select the text to quote first");
        self.comment_edit(id, |_, store| {
            let parent = store.comment(of as i64)?.context("the comment is gone")?;
            let (Some(link), Some(state)) = (&parent.github, &parent.thread) else {
                anyhow::bail!("only comments on the PR can be quoted");
            };
            anyhow::ensure!(
                parent.path.is_empty(),
                "only comments on the PR can be quoted"
            );
            let author = link
                .posts
                .first()
                .map_or(String::new(), |p| p.author.clone());
            let mut thread = crate::store::ThreadState::new(&state.repo, state.pr, Stage::New);
            thread.quote = Some(Quote {
                of,
                author,
                text: text.to_string(),
            });
            let now = now_ms();
            store.add_comment(&Comment {
                id: 0,
                path: String::new(),
                pins: vec![],
                body: String::new(),
                snippet: String::new(),
                review: parent.review,
                created_at: now,
                updated_at: now,
                workshop: Workshop::default(),
                github: None,
                thread: Some(thread),
                done: false,
            })?;
            Ok(())
        })
    }
}

/// A reply that opens with the text it answers, quoted.
pub fn quoted_reply(quote: &str, reply: &str) -> String {
    let quote = quote.trim().replace('\n', "\n> ");
    format!("> {quote}\n\n{}", reply.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reply_quotes_every_line_it_answers() {
        assert_eq!(
            quoted_reply("Why 500?\nAnd the counter?", " Fixed both. "),
            "> Why 500?\n> And the counter?\n\nFixed both."
        );
    }
}
