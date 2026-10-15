//! Walkthrough: a headless agent run that guides the reviewer through a review with notes:
//! what needs a careful look, how complex flows run, background the diff doesn't show, and
//! open questions, in a reading order.

use anyhow::Context;
use serde::Deserialize;

use super::notes::NoteInput;
use super::{RepoCtx, StateDto};
use crate::agent::{self, Progress, Runs};
use crate::store::Source;

const SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "notes": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "path": { "type": "string" },
          "start": { "type": "integer" },
          "end": { "type": "integer" },
          "category": { "enum": ["important", "flow", "context", "question"] },
          "level": { "enum": ["high", "medium", "low"] },
          "title": { "type": "string" },
          "body": { "type": "string" },
          "flow": {
            "type": "object",
            "properties": {
              "flow": { "type": "string" },
              "step": { "type": "integer" }
            },
            "required": ["flow", "step"],
            "additionalProperties": false
          }
        },
        "required": ["path", "start", "end", "category", "title", "body"],
        "additionalProperties": false
      }
    }
  },
  "required": ["notes"],
  "additionalProperties": false
}"#;

#[derive(Deserialize)]
struct Reply {
    notes: Vec<NoteInput>,
}

fn run_key(id: u32) -> String {
    format!("walkthrough-{id}")
}

impl RepoCtx {
    /// Runs an agent over review `id` and replaces its notes with its walkthrough. `on_tool`
    /// hears each tool call, like "Read src/a.rs". `None` when it was stopped.
    pub async fn run_walkthrough(
        &self,
        id: u32,
        runs: &Runs,
        on_tool: impl Fn(String),
    ) -> anyhow::Result<Option<(StateDto, usize)>> {
        // Held for the whole run, so the notes save even if the review closes meanwhile.
        let review = self.review(id)?;
        let run = runs.register(&self.agent_key(&run_key(id)))?;
        let (ws, revset, from, to) = {
            let r = review.lock().unwrap();
            (r.ws.clone(), r.revset.clone(), r.from.clone(), r.to.clone())
        };
        let instructions = crate::prompts::load(crate::prompts::Prompt::Walkthrough)?;
        let context = crate::bots::describe(&ws, &revset, from, to).await?;
        let prompt = format!(
            "{context}\n\nYou're in the repository and can read any file. Don't edit files.\n\n\
             {instructions}"
        );
        let task = agent::Task {
            cwd: &ws.root,
            prompt: &prompt,
            resume: None,
            fork: false,
            edit: false,
            schema: Some(SCHEMA),
            purpose: agent::Purpose::Walkthrough,
        };
        let reply = agent::run_registered(run, task, |p| {
            if let Progress::Tool(t) = p {
                on_tool(t)
            }
        })
        .await?;
        if reply.stopped {
            return Ok(None);
        }
        let reply: Reply = serde_json::from_value(
            reply
                .structured
                .context("the agent didn't return a walkthrough")?,
        )
        .context("the agent's walkthrough doesn't match the format")?;
        self.save_notes_to(&review, &reply.notes, Source::Headless)
            .map(Some)
    }

    pub fn stop_walkthrough(&self, id: u32, runs: &Runs) {
        runs.stop(&self.agent_key(&run_key(id)));
    }
}
