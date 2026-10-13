//! The agent steps of a design: drafting it, applying each answer to the open questions,
//! discussing or re-asking a question, revising the design, applying comments on its docs,
//! and making docs, diagrams and HTML. Every step continues one conversation per design, and
//! may only write inside the design's folder.

use anyhow::Context;
use serde::{Deserialize, Serialize};
use specta::Type;

use super::queue::{self, NewQuestion, QState, Rewrite, Turn, Update};
use super::{CommentKind, CommentState, DESIGN, DesignDto, HtmlSource, edit_queue, hash, inside};
use crate::agent::{self, Progress, Runs};
use crate::prompts::{self, Prompt};
use crate::review::RepoCtx;
use crate::workspace::Workspace;

const CHOICE: &str = r#"{
  "type": "object",
  "properties": {
    "label": { "type": "string" },
    "bullets": { "type": "array", "items": { "type": "string" } }
  },
  "required": ["label", "bullets"],
  "additionalProperties": false
}"#;

fn question_props() -> String {
    format!(
        r#""subject": {{ "type": "string" }},
        "context": {{ "type": "array", "items": {{ "type": "string" }} }},
        "options": {{ "type": "array", "items": {CHOICE}, "minItems": 1 }},
        "why": {{ "type": "string" }}"#
    )
}

fn new_question() -> String {
    format!(
        r#"{{
      "type": "object",
      "properties": {{
        "sub": {{ "type": "boolean" }},
        {},
        "sources": {{ "type": "array", "items": {{ "type": "string" }} }}
      }},
      "required": ["sub", "subject", "context", "options", "why", "sources"],
      "additionalProperties": false
    }}"#,
        question_props()
    )
}

fn rewrite() -> String {
    format!(
        r#"{{
      "type": "object",
      "properties": {{
        "id": {{ "type": "string" }},
        "what": {{ "type": "string" }},
        {}
      }},
      "required": ["id", "what", "subject", "context", "options", "why"],
      "additionalProperties": false
    }}"#,
        question_props()
    )
}

fn questions_schema() -> String {
    format!(
        r#"{{
  "type": "object",
  "properties": {{ "questions": {{ "type": "array", "items": {} }} }},
  "required": ["questions"],
  "additionalProperties": false
}}"#,
        new_question()
    )
}

fn update_schema() -> String {
    format!(
        r#"{{
  "type": "object",
  "properties": {{
    "drop": {{
      "type": "array",
      "items": {{
        "type": "object",
        "properties": {{ "id": {{ "type": "string" }}, "why": {{ "type": "string" }} }},
        "required": ["id", "why"],
        "additionalProperties": false
      }}
    }},
    "rewrite": {{ "type": "array", "items": {} }},
    "add": {{ "type": "array", "items": {} }}
  }},
  "required": ["drop", "rewrite", "add"],
  "additionalProperties": false
}}"#,
        rewrite(),
        new_question()
    )
}

fn comments_schema() -> String {
    format!(
        r#"{{
  "type": "object",
  "properties": {{
    "comments": {{
      "type": "array",
      "items": {{
        "type": "object",
        "properties": {{ "id": {{ "type": "integer" }}, "note": {{ "type": "string" }} }},
        "required": ["id", "note"],
        "additionalProperties": false
      }}
    }},
    "questions": {{ "type": "array", "items": {} }}
  }},
  "required": ["comments", "questions"],
  "additionalProperties": false
}}"#,
        new_question()
    )
}

const PATH_SCHEMA: &str = r#"{
  "type": "object",
  "properties": { "path": { "type": "string" } },
  "required": ["path"],
  "additionalProperties": false
}"#;

#[derive(Deserialize)]
struct Questions {
    questions: Vec<NewQuestion>,
}

#[derive(Deserialize)]
struct Applied {
    id: u32,
    note: String,
}

#[derive(Deserialize)]
struct CommentsDone {
    comments: Vec<Applied>,
    questions: Vec<NewQuestion>,
}

#[derive(Deserialize)]
struct Written {
    path: String,
}

/// What to make for the design.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ArtifactKind {
    Doc,
    Diagram,
    HtmlPage,
}

/// Where an HTML page made from `source` goes.
fn html_path(source: &str) -> String {
    let name = source.rsplit('/').next().unwrap_or(source);
    match (name.strip_suffix(".md"), name.strip_suffix(".mmd")) {
        (Some(stem), _) => format!("html/{stem}.html"),
        (_, Some(stem)) => format!("html/{stem}-diagram.html"),
        _ => format!("html/{name}.html"),
    }
}

fn parse<T: serde::de::DeserializeOwned>(reply: &agent::Reply, what: &str) -> anyhow::Result<T> {
    let value = reply
        .structured
        .clone()
        .with_context(|| format!("the {what} returned no result"))?;
    serde_json::from_value(value)
        .with_context(|| format!("the {what} returned an unexpected result"))
}

/// One step of the design's conversation.
struct Step<'a> {
    turn: String,
    edit: bool,
    schema: Option<String>,
    /// Starts a new conversation instead of continuing the design's.
    fresh: bool,
    /// Keeps the conversation this step ends in as the design's.
    keep: bool,
    key: &'a str,
}

impl RepoCtx {
    fn design_key(&self, ws: &Workspace, slug: &str, step: &str) -> String {
        self.agent_key(&format!(
            "design-{}-{slug}-{step}",
            ws.name.as_deref().unwrap_or("")
        ))
    }

    /// The opening of the design's conversation: where things are and the rules.
    fn design_intro(&self, ws: &Workspace, slug: &str) -> anyhow::Result<String> {
        let (rel, _) = self.design_dir(ws, slug)?;
        let qq = prompts::load(Prompt::Qq, &ws.root)?;
        let draft = prompts::load(Prompt::DesignDraft, &ws.root)?;
        Ok(format!(
            "We're working out a design together. Its folder is `{rel}/`: my requirements are \
             in `{rel}/brief.md` and the design goes in `{rel}/design.md`. lgtm keeps the \
             question queue; don't edit `{rel}/questions.json`. You're in the repository and \
             can read any file. Only create or change files inside `{rel}/`. You can't run \
             commands.\n\n{qq}\n\n{draft}"
        ))
    }

    async fn design_step(
        &self,
        ws: &Workspace,
        slug: &str,
        runs: &Runs,
        step: Step<'_>,
        on: impl Fn(Progress),
    ) -> anyhow::Result<agent::Reply> {
        let session = if step.fresh {
            None
        } else {
            self.design_local(ws, slug)?.session
        };
        let first = |turn: &str| -> anyhow::Result<String> {
            Ok(format!("{}\n\n{turn}", self.design_intro(ws, slug)?))
        };
        let prompt = match session {
            Some(_) => step.turn.clone(),
            None => first(&step.turn)?,
        };
        let key = self.design_key(ws, slug, step.key);
        let _busy = if step.edit { Some(ws.claim()?) } else { None };
        let mut reply = agent::run(
            runs,
            &key,
            agent::Task {
                cwd: &ws.root,
                prompt: &prompt,
                resume: session.as_deref(),
                edit: step.edit,
                schema: step.schema.as_deref(),
                purpose: agent::Purpose::Design,
            },
            &on,
        )
        .await;
        // A session Claude Code no longer has starts over with the intro.
        if session.is_some() && reply.is_err() {
            let prompt = first(&step.turn)?;
            reply = agent::run(
                runs,
                &key,
                agent::Task {
                    cwd: &ws.root,
                    prompt: &prompt,
                    resume: None,
                    edit: step.edit,
                    schema: step.schema.as_deref(),
                    purpose: agent::Purpose::Design,
                },
                &on,
            )
            .await;
        }
        if step.edit {
            ws.snapshot().await.ok();
        }
        let reply = reply?;
        if step.keep && !reply.stopped && reply.session.is_some() {
            self.edit_design_local(ws, slug, |l| {
                l.session = reply.session.clone();
                Ok(())
            })?;
        }
        Ok(reply)
    }

    /// Has the agent write the design from the brief and list its open questions. Replaces
    /// the queue.
    pub async fn draft_design(
        &self,
        ws: &Workspace,
        slug: &str,
        runs: &Runs,
        on_tool: impl Fn(String),
    ) -> anyhow::Result<DesignDto> {
        let (rel, dir) = self.design_dir(ws, slug)?;
        let attach = self.design_local(ws, slug)?.attach;
        let read_first = if attach.is_empty() {
            String::new()
        } else {
            format!(" and these first: {}", attach.join(", "))
        };
        let turn = format!(
            "Read the brief, `{rel}/brief.md`,{read_first} and whatever in the repo it touches. \
             Then write the design to `{rel}/{DESIGN}`, to the best of your ability. Return \
             every question I need to answer, in the order to ask them."
        );
        let reply = self
            .design_step(
                ws,
                slug,
                runs,
                Step {
                    turn,
                    edit: true,
                    schema: Some(questions_schema()),
                    fresh: true,
                    keep: true,
                    key: "main",
                },
                tool_only(on_tool),
            )
            .await?;
        if !reply.stopped {
            let Questions { questions } = parse(&reply, "draft")?;
            edit_queue(&dir, |q| {
                *q = queue::Queue::default();
                q.add_round(questions);
                Ok(())
            })?;
        }
        self.design(ws, slug)
    }

    /// Applies the answer to `id` to the open questions: drops, rewrites and new ones.
    pub async fn apply_answer(
        &self,
        ws: &Workspace,
        slug: &str,
        id: &str,
        runs: &Runs,
    ) -> anyhow::Result<DesignDto> {
        let (_, dir) = self.design_dir(ws, slug)?;
        let q = super::read_queue(&dir)?;
        let answered = q.get(id).with_context(|| format!("no question {id}"))?;
        let open: Vec<String> = q
            .questions
            .iter()
            .filter(|o| o.state == QState::Open && o.id != id)
            .map(queue::question_text)
            .collect();
        if open.is_empty() {
            return self.design(ws, slug);
        }
        let turn = format!(
            "I answered {id} ({}): {}.\n\nApply that to the open questions below. Drop the ones \
             it settles, with why in a few words (\"settled by {id}\"). Rewrite the ones it \
             reshapes, saying in a few words what changed. Add any new decision it raises. \
             Leave out questions it doesn't affect. Don't edit files.\n\n{}",
            answered.text.subject,
            queue::answer_text(answered),
            open.join("\n\n---\n\n")
        );
        let reply = self
            .design_step(
                ws,
                slug,
                runs,
                Step {
                    turn,
                    edit: false,
                    schema: Some(update_schema()),
                    fresh: false,
                    keep: true,
                    key: "main",
                },
                |_| {},
            )
            .await?;
        if !reply.stopped {
            let update: Update = parse(&reply, "update")?;
            edit_queue(&dir, |q| {
                q.apply(id, update);
                Ok(())
            })?;
        }
        self.design(ws, slug)
    }

    /// A discussion turn on question `id`, which isn't a decision. Streams the reply.
    pub async fn discuss_question(
        &self,
        ws: &Workspace,
        slug: &str,
        id: &str,
        message: &str,
        runs: &Runs,
        on_text: impl Fn(String),
    ) -> anyhow::Result<DesignDto> {
        let (_, dir) = self.design_dir(ws, slug)?;
        let q = super::read_queue(&dir)?;
        let question = q.get(id).with_context(|| format!("no question {id}"))?;
        let turn = if question.discussion.is_empty() {
            format!(
                "Let's discuss this question. It's a discussion, not a decision: answer briefly \
                 and don't edit files.\n\n{}\n\n{message}",
                queue::question_text(question)
            )
        } else {
            format!("About {id}: {message}")
        };
        edit_queue(&dir, |q| {
            q.discuss(
                id,
                Turn {
                    you: true,
                    text: message.to_string(),
                },
            )
        })?;
        let reply = self
            .design_step(
                ws,
                slug,
                runs,
                Step {
                    turn,
                    edit: false,
                    schema: None,
                    fresh: false,
                    keep: true,
                    key: "main",
                },
                |p| {
                    if let Progress::Text(t) = p {
                        on_text(t)
                    }
                },
            )
            .await?;
        edit_queue(&dir, |q| {
            q.discuss(
                id,
                Turn {
                    you: false,
                    text: reply.text.trim().to_string(),
                },
            )
        })?;
        self.design(ws, slug)
    }

    /// Rewrites question `id` with what came out of its discussion.
    pub async fn reask_question(
        &self,
        ws: &Workspace,
        slug: &str,
        id: &str,
        runs: &Runs,
    ) -> anyhow::Result<DesignDto> {
        let (_, dir) = self.design_dir(ws, slug)?;
        let turn = format!(
            "Rewrite {id} with what came out of our discussion, by the same rules, keeping its \
             id. Say in a few words what changed. Don't edit files."
        );
        let reply = self
            .design_step(
                ws,
                slug,
                runs,
                Step {
                    turn,
                    edit: false,
                    schema: Some(rewrite()),
                    fresh: false,
                    keep: true,
                    key: "main",
                },
                |_| {},
            )
            .await?;
        if !reply.stopped {
            let mut r: Rewrite = parse(&reply, "re-ask")?;
            r.id = id.to_string();
            edit_queue(&dir, |q| q.reask(r))?;
        }
        self.design(ws, slug)
    }

    /// Folds the decisions not yet in the design into it. New questions start a round.
    pub async fn revise_design(
        &self,
        ws: &Workspace,
        slug: &str,
        runs: &Runs,
        on_tool: impl Fn(String),
    ) -> anyhow::Result<DesignDto> {
        let (rel, dir) = self.design_dir(ws, slug)?;
        let q = super::read_queue(&dir)?;
        let decided: Vec<String> = q
            .unrevised()
            .map(|d| format!("- {} ({}): {}", d.id, d.text.subject, queue::answer_text(d)))
            .collect();
        anyhow::ensure!(!decided.is_empty(), "there are no new decisions to fold in");
        let turn = format!(
            "I decided these:\n\n{}\n\nFold them into `{rel}/{DESIGN}` by the revising rules, \
             and update its Open questions section. Return any new questions the decisions \
             raise; none is fine.",
            decided.join("\n")
        );
        let round = q.round();
        let reply = self
            .design_step(
                ws,
                slug,
                runs,
                Step {
                    turn,
                    edit: true,
                    schema: Some(questions_schema()),
                    fresh: false,
                    keep: true,
                    key: "main",
                },
                tool_only(on_tool),
            )
            .await?;
        if !reply.stopped {
            let Questions { questions } = parse(&reply, "revision")?;
            edit_queue(&dir, |q| {
                q.revised = q.revised.max(round);
                if !questions.is_empty() {
                    q.add_round(questions);
                }
                Ok(())
            })?;
        }
        self.design(ws, slug)
    }

    /// Has the agent apply the queued change comments to the docs, in one turn.
    pub async fn apply_design_comments(
        &self,
        ws: &Workspace,
        slug: &str,
        runs: &Runs,
        on_tool: impl Fn(String),
    ) -> anyhow::Result<DesignDto> {
        let (rel, dir) = self.design_dir(ws, slug)?;
        let queued: Vec<_> = self
            .design_local(ws, slug)?
            .comments
            .into_iter()
            .filter(|c| c.kind == CommentKind::Change && c.state == CommentState::Queued)
            .collect();
        anyhow::ensure!(!queued.is_empty(), "there are no comments to apply");
        let list: Vec<String> = queued
            .iter()
            .map(|c| {
                format!(
                    "Comment {} on `{rel}/{}` lines {}-{}, on \"{}\":\n{}",
                    c.id,
                    c.path,
                    c.start,
                    c.end,
                    c.quote.trim(),
                    c.body.trim()
                )
            })
            .collect();
        let turn = format!(
            "Apply my comments below by editing those files directly, changing only what they \
             ask. When a comment needs a decision only I can make, don't guess: leave the text \
             and return a question for it. Return what you did for each comment in a \
             sentence.\n\n{}",
            list.join("\n\n")
        );
        let reply = self
            .design_step(
                ws,
                slug,
                runs,
                Step {
                    turn,
                    edit: true,
                    schema: Some(comments_schema()),
                    fresh: false,
                    keep: true,
                    key: "main",
                },
                tool_only(on_tool),
            )
            .await?;
        if !reply.stopped {
            let done: CommentsDone = parse(&reply, "comments")?;
            self.edit_design_local(ws, slug, |l| {
                for a in &done.comments {
                    if let Some(c) = l.comments.iter_mut().find(|c| c.id == a.id) {
                        c.state = CommentState::Applied;
                        c.reply = Some(a.note.clone());
                    }
                }
                Ok(())
            })?;
            if !done.questions.is_empty() {
                edit_queue(&dir, |q| {
                    q.add_round(done.questions);
                    Ok(())
                })?;
            }
        }
        self.design(ws, slug)
    }

    /// Answers an ask comment about its passage. Nothing changes.
    pub async fn answer_design_comment(
        &self,
        ws: &Workspace,
        slug: &str,
        id: u32,
        runs: &Runs,
        on_text: impl Fn(String),
    ) -> anyhow::Result<DesignDto> {
        let (rel, _) = self.design_dir(ws, slug)?;
        let c = self
            .design_local(ws, slug)?
            .comments
            .into_iter()
            .find(|c| c.id == id)
            .context("no such comment")?;
        let turn = format!(
            "A question about `{rel}/{}` lines {}-{}, on \"{}\". Answer briefly and don't edit \
             files:\n\n{}",
            c.path,
            c.start,
            c.end,
            c.quote.trim(),
            c.body.trim()
        );
        let key = format!("ask-{id}");
        let reply = self
            .design_step(
                ws,
                slug,
                runs,
                Step {
                    turn,
                    edit: false,
                    schema: None,
                    fresh: false,
                    keep: false,
                    key: &key,
                },
                |p| {
                    if let Progress::Text(t) = p {
                        on_text(t)
                    }
                },
            )
            .await?;
        if !reply.stopped {
            self.edit_design_local(ws, slug, |l| {
                if let Some(c) = l.comments.iter_mut().find(|c| c.id == id) {
                    c.state = CommentState::Answered;
                    c.reply = Some(reply.text.trim().to_string());
                }
                Ok(())
            })?;
        }
        self.design(ws, slug)
    }

    /// Makes a doc, a diagram or an HTML page for the design from `about`.
    pub async fn make_design_artifact(
        &self,
        ws: &Workspace,
        slug: &str,
        kind: ArtifactKind,
        about: &str,
        runs: &Runs,
        on_tool: impl Fn(String),
    ) -> anyhow::Result<(DesignDto, String)> {
        let (rel, dir) = self.design_dir(ws, slug)?;
        let (rules, what) = match kind {
            ArtifactKind::Doc => (
                prompts::load(Prompt::DesignDocs, &ws.root)?,
                format!("a markdown document in `{rel}/docs/`, with a short file name"),
            ),
            ArtifactKind::Diagram => (
                prompts::load(Prompt::DesignDocs, &ws.root)?,
                format!(
                    "a Mermaid diagram as a `.mmd` file in `{rel}/docs/`, with a short file name"
                ),
            ),
            ArtifactKind::HtmlPage => (
                prompts::load(Prompt::DesignHtml, &ws.root)?,
                format!("an HTML page in `{rel}/html/`, with a short file name"),
            ),
        };
        let turn = format!(
            "{rules}\n\nFrom the design, write {what}, for: {about}\n\nReturn the file's path \
             relative to `{rel}/`."
        );
        let reply = self
            .design_step(
                ws,
                slug,
                runs,
                Step {
                    turn,
                    edit: true,
                    schema: Some(PATH_SCHEMA.into()),
                    fresh: false,
                    keep: true,
                    key: "main",
                },
                tool_only(on_tool),
            )
            .await?;
        let path = if reply.stopped {
            String::new()
        } else {
            let Written { path } = parse(&reply, "artifact")?;
            let path = path.trim_start_matches(&format!("{rel}/")).to_string();
            anyhow::ensure!(
                inside(&dir, &path)?.is_file(),
                "the agent said it wrote {path}, but it isn't there"
            );
            path
        };
        Ok((self.design(ws, slug)?, path))
    }

    /// Makes an HTML page from one of the design's markdown or Mermaid files, remembering
    /// what it was made from so a later edit marks it stale.
    pub async fn make_design_html(
        &self,
        ws: &Workspace,
        slug: &str,
        source: &str,
        runs: &Runs,
        on_tool: impl Fn(String),
    ) -> anyhow::Result<(DesignDto, String)> {
        let (rel, dir) = self.design_dir(ws, slug)?;
        let text = std::fs::read_to_string(inside(&dir, source)?)
            .with_context(|| format!("can't read {source}"))?;
        let out = html_path(source);
        let rules = prompts::load(Prompt::DesignHtml, &ws.root)?;
        let kind = if source.ends_with(".mmd") {
            "This is a Mermaid diagram: draw it as an inline SVG diagram."
        } else {
            ""
        };
        let turn = format!(
            "{rules}\n\nMake an HTML page from `{rel}/{source}` and write it to `{rel}/{out}`, \
             replacing what's there. {kind} Return the path relative to `{rel}/`."
        );
        let reply = self
            .design_step(
                ws,
                slug,
                runs,
                Step {
                    turn,
                    edit: true,
                    schema: Some(PATH_SCHEMA.into()),
                    fresh: false,
                    keep: true,
                    key: "main",
                },
                tool_only(on_tool),
            )
            .await?;
        if !reply.stopped {
            anyhow::ensure!(dir.join(&out).is_file(), "the agent didn't write {out}");
            self.edit_design_local(ws, slug, |l| {
                l.html.insert(
                    out.clone(),
                    HtmlSource {
                        source: source.to_string(),
                        hash: hash(&text),
                    },
                );
                Ok(())
            })?;
        }
        Ok((self.design(ws, slug)?, out))
    }

    /// Stops whatever the agent is doing on the design.
    pub fn stop_design(&self, ws: &Workspace, slug: &str, runs: &Runs) {
        runs.stop_all(&self.design_key(ws, slug, ""));
    }
}

fn tool_only(on_tool: impl Fn(String)) -> impl Fn(Progress) {
    move |p| {
        if let Progress::Tool(t) = p {
            on_tool(t)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_pages_are_named_after_their_source() {
        assert_eq!(html_path("design.md"), "html/design.html");
        assert_eq!(html_path("docs/overview.md"), "html/overview.html");
        assert_eq!(html_path("docs/flow.mmd"), "html/flow-diagram.html");
    }

    #[test]
    fn the_schemas_are_json() {
        for s in [
            questions_schema(),
            update_schema(),
            comments_schema(),
            rewrite(),
        ] {
            serde_json::from_str::<serde_json::Value>(&s).unwrap();
        }
    }
}
