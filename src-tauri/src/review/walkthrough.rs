//! Walkthrough: the prompt that has a terminal agent guide the reviewer through a review with
//! notes, written through lgtm's MCP tools.

use super::RepoCtx;
use crate::prompts::{self, Prompt};

impl RepoCtx {
    /// What to send a terminal agent so it writes review `id`'s walkthrough.
    pub fn walkthrough_prompt(&self, id: u32) -> anyhow::Result<String> {
        let revset = self.review(id)?.lock().unwrap().revset.clone();
        let instructions = prompts::load(Prompt::Walkthrough)?;
        Ok(format!(
            "Write a walkthrough of the review of `{revset}` open in lgtm. Read it with lgtm's \
             get_review MCP tool and save the notes with its set_notes tool, which replaces the \
             last walkthrough. Don't edit files.\n\n{instructions}"
        ))
    }
}
