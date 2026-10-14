//! The tools an agent in a terminal gets. Each acts on the review open in the terminal's
//! workspace. Agents read everything and add comments, walkthrough notes and findings, but can't mark
//! code reviewed: that's the human's record.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use anyhow::Context;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_specta::Event as _;

use super::{Hello, inputs};
use crate::bots::output::Output;
use crate::events::{AgentRevset, AgentShow, BotRunChanged, QaChanged, ReviewStateChanged};
use crate::qa::queue::{Choice, NewQuestion, QState, answer_text};
use crate::repo::RepoWindows;
use crate::review::{NoteInput, RepoCtx, StateDto};
use crate::store::Source;
use crate::store::{Take, TakeVerdict};

const INSTRUCTIONS: &str = "lgtm is the code review app this terminal runs in. The human \
reviews a change there and marks rows with colors they label, like reviewed or flagged. These tools act on the review open in \
this terminal's workspace: read its state, comments, walkthrough, symbol graph and Reviewer \
findings; add comments; write a walkthrough; submit your own review findings; and point the \
review at \
code or a revset. You can't mark code. Lines are on the new side of the diff. To work \
through decisions with the human, ask through lgtm's Q&A queue (qa_ask) instead of in the \
chat, then read their decisions with qa_answers.";

type Out = Result<String, String>;

#[derive(Clone)]
pub struct Lgtm {
    app: AppHandle,
    hello: Hello,
    tool_router: ToolRouter<Self>,
}

fn json(value: &impl Serialize) -> Out {
    serde_json::to_string_pretty(value).map_err(|e| e.to_string())
}

/// `path`, absolute or relative, as the workspace-relative path the review shows it by.
fn workspace_path(root: &Path, path: &str) -> anyhow::Result<String> {
    let p = Path::new(path);
    let rel = if p.is_absolute() {
        p.strip_prefix(root)
            .with_context(|| format!("{path} isn't in the workspace"))?
    } else {
        p
    };
    let rel: PathBuf = rel
        .components()
        .filter(|c| !matches!(c, Component::CurDir))
        .collect();
    let rel = rel
        .to_str()
        .with_context(|| format!("{path} isn't UTF-8"))?;
    crate::files::resolve(root, rel)?;
    Ok(rel.to_string())
}

fn failed(e: anyhow::Error) -> String {
    format!("{e:#}")
}

impl Lgtm {
    pub(super) fn new(app: AppHandle, hello: Hello) -> Self {
        Self {
            app,
            hello,
            tool_router: Self::tool_router(),
        }
    }

    fn workspace(&self) -> Option<&str> {
        self.hello.workspace.as_deref()
    }

    /// The repo's state and its window's label.
    fn repo(&self) -> anyhow::Result<(String, Arc<RepoCtx>)> {
        self.app
            .state::<RepoWindows>()
            .find(Path::new(&self.hello.root))
            .context("lgtm doesn't have this repo open")
    }

    fn changed(&self, label: &str, review: u32, state: StateDto) {
        ReviewStateChanged { review, state }
            .emit_to(&self.app, label)
            .ok();
    }
}

#[tool_router]
impl Lgtm {
    #[tool(
        description = "The open review: its revset and commits, and per file the changed \
        rows the human has marked (by mark label), how many count as done, and the line \
        ranges still left."
    )]
    async fn get_review(&self) -> Out {
        let (_, ctx) = self.repo().map_err(failed)?;
        json(&ctx.summary(self.workspace()).map_err(failed)?)
    }

    #[tool(
        description = "The review's comments, with their lines, author and any \
        conversation. Comments whose code is gone have no lines."
    )]
    async fn list_comments(&self) -> Out {
        let (_, ctx) = self.repo().map_err(failed)?;
        json(&ctx.agent_comments(self.workspace()).map_err(failed)?)
    }

    #[tool(
        description = "Comment on lines of a file in the review. The comment shows in \
        the review marked as from a terminal agent. It anchors to the changed lines in the \
        range, so the range must include at least one changed line."
    )]
    async fn add_comment(&self, Parameters(p): Parameters<inputs::AddComment>) -> Out {
        let (label, ctx) = self.repo().map_err(failed)?;
        let (review, state) = ctx
            .agent_comment(self.workspace(), &p.path, p.start_line, p.end_line, &p.body)
            .map_err(failed)?;
        self.changed(&label, review, state);
        Ok("Added the comment.".into())
    }

    #[tool(description = "Reply to a comment. The reply joins the comment's conversation.")]
    async fn reply_to_comment(&self, Parameters(p): Parameters<inputs::Reply>) -> Out {
        let (label, ctx) = self.repo().map_err(failed)?;
        let (review, state) = ctx
            .agent_reply(self.workspace(), p.comment_id, &p.text)
            .map_err(failed)?;
        self.changed(&label, review, state);
        Ok("Replied.".into())
    }

    #[tool(
        description = "In a PR workspace, the PR's GitHub threads: where each is, its posts, \
        its stage (new, decide, toFix, reply, ready, done), the take on it, and the reply \
        drafted. Nothing here is posted until the human sends it."
    )]
    async fn list_threads(&self) -> Out {
        let (_, ctx) = self.repo().map_err(failed)?;
        json(&ctx.agent_threads(self.workspace()).map_err(failed)?)
    }

    #[tool(
        description = "Set a thread's take (verdict and why) and, optionally, its whole reply. \
        This never posts to GitHub."
    )]
    async fn set_thread_take(&self, Parameters(p): Parameters<inputs::SetTake>) -> Out {
        let (label, ctx) = self.repo().map_err(failed)?;
        let verdict = match p.verdict {
            inputs::Verdict::Agree => TakeVerdict::Agree,
            inputs::Verdict::AgreeWithChanges => TakeVerdict::AgreeWithChanges,
            inputs::Verdict::PushBack => TakeVerdict::PushBack,
            inputs::Verdict::NeedsYou => TakeVerdict::NeedsYou,
        };
        let take = Take {
            verdict,
            why: p.why,
            question: p.question,
        };
        let (review, state) = ctx
            .agent_set_take(self.workspace(), p.thread_id, take, p.reply)
            .map_err(failed)?;
        self.changed(&label, review, state);
        Ok("Set the take.".into())
    }

    #[tool(
        description = "Mark a thread's reply final, so it's ready for the human to send. Use \
        it only when the human said the reply is good. This never posts to GitHub."
    )]
    async fn mark_thread_ready(&self, Parameters(p): Parameters<inputs::ThreadId>) -> Out {
        let (label, ctx) = self.repo().map_err(failed)?;
        let (review, state) = ctx
            .agent_thread_ready(self.workspace(), p.thread_id)
            .map_err(failed)?;
        self.changed(&label, review, state);
        Ok("Marked it ready.".into())
    }

    #[tool(
        description = "The review's walkthrough: notes on the code in reading order, each \
        important (with a level), a step in a named flow, context, or a question."
    )]
    async fn list_notes(&self) -> Out {
        let (_, ctx) = self.repo().map_err(failed)?;
        json(&ctx.agent_notes(self.workspace()).map_err(failed)?)
    }

    #[tool(
        description = "Replace the review's walkthrough with yours: notes pinned to the code, \
        in the order the reviewer should read them, starting with a context note on the entry \
        point that says what the change does. Tell the reviewer what they'd otherwise have to \
        work out; don't narrate the diff. important: needs a careful look (subtle logic, edge \
        cases, error handling, security, data changes, untested behavior), with a level: high \
        (likely wrong or a mistake does serious damage), medium, or low. flow: a numbered step \
        in a named path through three or more places. context: intent or background the diff \
        doesn't show. question: something the reviewer or author should settle. Notes outside \
        the review's files are left out."
    )]
    async fn set_notes(&self, Parameters(p): Parameters<inputs::Notes>) -> Out {
        let (label, ctx) = self.repo().map_err(failed)?;
        let inputs: Vec<NoteInput> =
            serde_json::from_value(serde_json::to_value(&p.notes).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        let review = ctx.current_review(self.workspace()).map_err(failed)?;
        let (state, skipped) = ctx
            .save_notes(review, &inputs, Source::Terminal)
            .map_err(failed)?;
        let n = state.notes.len();
        self.changed(&label, review, state);
        Ok(format!(
            "Set {n} notes; left out {skipped} outside the review's files."
        ))
    }

    #[tool(
        description = "The review's changed symbols (functions, types) and unchanged \
        callers of them. `uses` holds [user, used] id pairs. Symbols link by name, so a link \
        can be wrong for common names."
    )]
    async fn get_symbol_graph(&self) -> Out {
        let (_, ctx) = self.repo().map_err(failed)?;
        let review = ctx.current_review(self.workspace()).map_err(failed)?;
        let graph = ctx.graph(review).await.map_err(failed)?;
        let nodes: Vec<_> = graph
            .nodes
            .iter()
            .map(|n| {
                serde_json::json!({
                    "id": n.id, "name": n.name, "parent": n.parent, "status": n.status,
                    "path": n.path, "line": n.line,
                })
            })
            .collect();
        let uses: Vec<_> = graph.edges.iter().map(|e| [e.from, e.to]).collect();
        json(&serde_json::json!({ "symbols": nodes, "uses": uses }))
    }

    #[tool(
        description = "Findings from the latest Reviewers run in this workspace: the \
        synthesis when there is one, otherwise each reviewer's own. Each has a state: open, or \
        fixed or dismissed by the human, which are settled and to be left alone."
    )]
    async fn list_findings(&self) -> Out {
        let (_, ctx) = self.repo().map_err(failed)?;
        let runs = ctx.bot_runs(self.workspace()).map_err(failed)?;
        let Some(run) = runs.first() else {
            return Ok("No Reviewers runs in this workspace.".into());
        };
        let outputs = run.parts.iter().filter_map(|p| p.output.as_ref());
        let synthesis = outputs.clone().find_map(|o| match o {
            Output::Synthesis(s) => Some(s.findings.clone()),
            _ => None,
        });
        let findings = synthesis.unwrap_or_else(|| {
            outputs
                .filter_map(|o| match o {
                    Output::Review(r) => Some(r.findings.clone()),
                    _ => None,
                })
                .flatten()
                .collect()
        });
        let mut listed = Vec::new();
        for f in &findings {
            let mut value = serde_json::to_value(f).map_err(|e| e.to_string())?;
            value["state"] = run.finding_state(&f.id).into();
            listed.push(value);
        }
        json(&serde_json::json!({ "revset": run.revset, "findings": listed }))
    }

    #[tool(
        description = "Submit your own review of the open change as a Reviewers run, so \
        the human sees your findings as cards they can comment on, send to an agent, or fix. \
        Rate severity by real impact: critical (exploitable or loses data in normal use), high \
        (a core flow is broken), medium (a real bug off the main path), low (hygiene), info."
    )]
    async fn submit_findings(&self, Parameters(p): Parameters<inputs::Findings>) -> Out {
        let (label, ctx) = self.repo().map_err(failed)?;
        let reviewer = p.reviewer.as_deref().unwrap_or("terminal-agent");
        let review = serde_json::json!({ "verdict": p.verdict, "findings": p.findings });
        let run = ctx
            .submit_findings(self.workspace(), reviewer, review)
            .map_err(failed)?;
        BotRunChanged { run: run.id }
            .emit_to(&self.app, &label)
            .ok();
        Ok(format!("Submitted {} findings.", p.findings.len()))
    }

    #[tool(
        description = "Show a file at a line in lgtm: in the review pane when the review \
        changes it, otherwise in an editor tab."
    )]
    async fn show(&self, Parameters(p): Parameters<inputs::Show>) -> Out {
        let (label, ctx) = self.repo().map_err(failed)?;
        let ws = ctx.workspace(self.workspace()).await.map_err(failed)?;
        let path = workspace_path(&ws.root, &p.path).map_err(failed)?;
        AgentShow {
            workspace: self.hello.workspace.clone(),
            path,
            line: p.line,
        }
        .emit_to(&self.app, &label)
        .map_err(|e| e.to_string())?;
        Ok("Shown.".into())
    }

    #[tool(description = "Change what the review pane reviews to a jj revset.")]
    async fn set_revset(&self, Parameters(p): Parameters<inputs::Revset>) -> Out {
        let (label, ctx) = self.repo().map_err(failed)?;
        let ws = ctx.workspace(self.workspace()).await.map_err(failed)?;
        ws.jj.resolve(&p.revset).await.map_err(failed)?;
        AgentRevset {
            workspace: self.hello.workspace.clone(),
            revset: p.revset,
        }
        .emit_to(&self.app, &label)
        .map_err(|e| e.to_string())?;
        Ok("The review pane is loading it.".into())
    }

    #[tool(
        description = "The workspace's active Q&A sessions (not yet implemented), with how many \
        of their questions are open and decided."
    )]
    async fn list_qa(&self) -> Out {
        let (_, ctx) = self.repo().map_err(failed)?;
        let ws = ctx.workspace(self.workspace()).await.map_err(failed)?;
        let sessions = ctx.qa_sessions(&ws).map_err(failed)?;
        let list: Vec<_> = sessions
            .iter()
            .filter(|s| !s.archived)
            .map(|s| {
                let count = |st| s.queue.questions.iter().filter(|q| q.state == st).count();
                serde_json::json!({
                    "session": s.id,
                    "title": s.title,
                    "open": count(QState::Open),
                    "decided": count(QState::Decided),
                })
            })
            .collect();
        json(&list)
    }

    #[tool(
        description = "A Q&A session's prompt and question queue: every question, its \
        state and answer."
    )]
    async fn qa_questions(&self, Parameters(p): Parameters<inputs::QaSession>) -> Out {
        let (_, ctx) = self.repo().map_err(failed)?;
        let ws = ctx.workspace(self.workspace()).await.map_err(failed)?;
        let s = ctx.qa_session(&ws, &p.session).map_err(failed)?;
        json(&serde_json::json!({ "prompt": s.prompt, "queue": s.queue }))
    }

    #[tool(
        description = "Ask the human questions through lgtm's Q&A. They show one at a time in \
        the Q&A tab, in the qqueue format: context bullets, options with the recommended one \
        first, and why. Returns the session and the new questions' ids in the order given; \
        read the decisions later with qa_answers."
    )]
    async fn qa_ask(&self, Parameters(p): Parameters<inputs::QaAsk>) -> Out {
        let (label, ctx) = self.repo().map_err(failed)?;
        let ws = ctx.workspace(self.workspace()).await.map_err(failed)?;
        let id = match p.session {
            Some(id) => id,
            None => {
                let title = p.title.unwrap_or_default();
                ctx.create_qa(&ws, &title, vec![]).map_err(failed)?.id
            }
        };
        let questions = p
            .questions
            .into_iter()
            .map(|q| NewQuestion {
                sub: q.sub,
                subject: q.subject,
                context: q.context,
                options: q
                    .options
                    .into_iter()
                    .map(|o| Choice {
                        label: o.label,
                        means: o.means,
                        pros: o.pros,
                        cons: o.cons,
                        risks: o.risks,
                        bullets: vec![],
                    })
                    .collect(),
                why: q.why,
                sources: q.sources,
                depends: q.depends,
            })
            .collect();
        let ids = ctx
            .edit_qa(&ws, &id, |s| Ok(s.queue.add_round(questions)))
            .map_err(failed)?;
        QaChanged {
            workspace: self.hello.workspace.clone(),
            id: id.clone(),
        }
        .emit_to(&self.app, &label)
        .ok();
        Ok(format!(
            "Queued {} (in the order given) in session {id}. The human answers them in lgtm.",
            ids.join(", ")
        ))
    }

    #[tool(description = "The decisions on a Q&A session's questions so far, in ask order.")]
    async fn qa_answers(&self, Parameters(p): Parameters<inputs::QaSession>) -> Out {
        let (_, ctx) = self.repo().map_err(failed)?;
        let ws = ctx.workspace(self.workspace()).await.map_err(failed)?;
        let queue = ctx.qa_session(&ws, &p.session).map_err(failed)?.queue;
        let list: Vec<_> = queue
            .questions
            .iter()
            .filter(|q| q.state != QState::Open)
            .map(|q| {
                serde_json::json!({
                    "id": q.id,
                    "subject": q.text.subject,
                    "answer": match q.state {
                        QState::Dropped => format!("dropped: {}", q.dropped.as_deref().unwrap_or("")),
                        QState::Deferred => "deferred".into(),
                        _ => answer_text(q),
                    },
                })
            })
            .collect();
        let open = queue.open_count();
        json(&serde_json::json!({ "decided": list, "stillOpen": open }))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Lgtm {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("lgtm", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_takes_paths_the_way_agents_write_them() {
        let root = Path::new("/repo");
        assert_eq!(workspace_path(root, "/repo/src/a.rs").unwrap(), "src/a.rs");
        assert_eq!(workspace_path(root, "./src/a.rs").unwrap(), "src/a.rs");
        assert_eq!(workspace_path(root, "src/a.rs").unwrap(), "src/a.rs");
        assert!(workspace_path(root, "/other/a.rs").is_err());
        assert!(workspace_path(root, "../a.rs").is_err());
    }
}
