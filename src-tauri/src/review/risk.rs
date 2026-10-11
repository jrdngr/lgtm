//! Highlight risks: a headless agent run that finds the parts of a review most likely to need
//! a careful human look.

use anyhow::Context;
use serde::Deserialize;

use super::hotspots::HotspotInput;
use super::{RepoCtx, StateDto};
use crate::agent::{self, Progress, Runs};
use crate::store::Source;

const SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "hotspots": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "path": { "type": "string" },
          "start": { "type": "integer" },
          "end": { "type": "integer" },
          "level": { "enum": ["high", "medium", "low"] },
          "reason": { "type": "string" }
        },
        "required": ["path", "start", "end", "level", "reason"],
        "additionalProperties": false
      }
    }
  },
  "required": ["hotspots"],
  "additionalProperties": false
}"#;

/// What the agent is asked. Also the instructions an agent in a terminal gets.
pub const INSTRUCTIONS: &str = "Find the parts of this change that most need a careful human \
review, so the reviewer knows where to spend attention. Look for subtle logic, edge cases, \
concurrency, error handling, security boundaries, data and schema changes, public interfaces, \
and changed behavior that tests don't cover. Ignore style and naming. Read the surrounding code \
when the diff isn't enough.

Return at most 15 hotspots, each with a repository-relative path, the first and last lines on \
the new side of the diff (the old side for a deleted file), a level, and a one-line reason that \
says what to check. Keep ranges tight: a function or a few lines, not a whole file.

- high: likely wrong, or a mistake here would do serious damage.
- medium: worth a close look; easy to get wrong.
- low: routine, but the reviewer should know it's there.";

#[derive(Deserialize)]
struct Reply {
    hotspots: Vec<HotspotInput>,
}

fn run_key(id: u32) -> String {
    format!("hotspots-{id}")
}

impl RepoCtx {
    /// Runs an agent over review `id` and replaces its hotspots with what it finds. `on_tool`
    /// hears each tool call, like "Read src/a.rs". `None` when it was stopped.
    pub async fn find_hotspots(
        &self,
        id: u32,
        runs: &Runs,
        on_tool: impl Fn(String),
    ) -> anyhow::Result<Option<(StateDto, usize)>> {
        let (ws, revset, from, to) = {
            let review = self.review(id)?;
            let r = review.lock().unwrap();
            (r.ws.clone(), r.revset.clone(), r.from.clone(), r.to.clone())
        };
        let context = crate::bots::describe(&ws, &revset, from, to).await?;
        let prompt = format!(
            "{context}\n\nYou're in the repository and can read any file. Don't edit files.\n\n\
             {INSTRUCTIONS}"
        );
        let task = agent::Task {
            cwd: &ws.root,
            prompt: &prompt,
            resume: None,
            edit: false,
            schema: Some(SCHEMA),
        };
        let reply = agent::run(runs, &run_key(id), task, |p| {
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
                .context("the agent didn't return hotspots")?,
        )
        .context("the agent's hotspots don't match the format")?;
        self.save_hotspots(id, &reply.hotspots, Source::Headless)
            .map(Some)
    }

    pub fn stop_hotspots(&self, id: u32, runs: &Runs) {
        runs.stop(&run_key(id));
    }
}
