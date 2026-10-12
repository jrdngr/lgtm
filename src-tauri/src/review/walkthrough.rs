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

/// What the agent is asked.
const INSTRUCTIONS: &str = "Write a walkthrough of this change for the human reviewing \
it: notes pinned to the code that tell them where to look, in what order, and what to check. \
The reviewer reads the diff anyway, so don't narrate it. Each note should tell them something \
they'd otherwise have to work out: why the code is the way it is, what could go wrong, how \
the pieces connect, what isn't there. Read the surrounding code when the diff isn't enough.

Return the notes in the order the reviewer should read them. Start with a context note on \
the change's entry point saying in two or three sentences what the change does and how it's \
organized. Then follow the change the way it runs, not file by file. Usually 5 to 20 notes; \
fewer for a small change. Skip anything routine.

Each note has a repository-relative path, the first and last lines on the new side of the \
diff (the old side for a deleted file), a category, a title, and a body.

- title: one line, under 80 characters, that makes sense on its own in a list.
- body: one to four sentences of Markdown. Concrete: name the variable, the case, the caller.
- Keep ranges tight: a function or a few lines, not a whole file.

Categories:
- important: needs a careful look. Subtle logic, edge cases, concurrency, error handling, \
security boundaries, data and schema changes, public interfaces, behavior tests don't cover. \
Say what to check. Give a level: high (likely wrong, or a mistake would do serious damage), \
medium (easy to get wrong), or low (routine, but worth knowing).
- flow: a step in a path through several places that's hard to follow from the diff, like a \
request going through handlers or a value threaded through layers. Give each flow a short \
name and number its steps from 1, so the reviewer can step through it alone. Only for flows \
of three or more steps; one change can have several flows.
- context: intent or background the diff doesn't show, like why an approach was chosen or \
what a removed piece used to do.
- question: something you couldn't settle from the code and the reviewer or author should, \
like a behavior change that may or may not be intended.";

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
        let reply = agent::run(runs, &self.agent_key(&run_key(id)), task, |p| {
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
        self.save_notes(id, &reply.notes, Source::Headless)
            .map(Some)
    }

    pub fn stop_walkthrough(&self, id: u32, runs: &Runs) {
        runs.stop(&self.agent_key(&run_key(id)));
    }
}
