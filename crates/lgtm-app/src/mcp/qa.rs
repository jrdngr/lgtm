//! Q&A tools: the agent asks the human questions in lgtm's Q&A tab, then works through the
//! steps the tab sends it as prompts starting with `lgtm Q&A`.

use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};

use super::qa_inputs as inputs;
use super::server::{Lgtm, Out, failed, json};
use crate::ctx::RepoCtx;
use crate::prompts::{self, Prompt};
use crate::qa::discuss::{self, Decision};
use crate::qa::queue::Rewrite;
use crate::qa::update::dependents_text;
use crate::qa::{AskIn, Session};
use crate::workspace::Workspace;

impl Lgtm {
    /// The repo and this terminal's workspace.
    async fn qa_repo(&self) -> Result<(Arc<RepoCtx>, Arc<Workspace>), String> {
        let (_, ctx) = self.repo().map_err(failed)?;
        let ws = ctx.workspace(self.workspace()).await.map_err(failed)?;
        Ok((ctx, ws))
    }

    async fn session(&self, id: &str) -> Result<Session, String> {
        let (ctx, ws) = self.qa_repo().await?;
        ctx.qa_session(&ws, id).map_err(failed)
    }
}

#[tool_router(router = qa_router, vis = "pub(super)")]
impl Lgtm {
    #[tool(
        description = "The workspace's active Q&A sessions (not yet implemented), with how many \
        of their questions are open and decided."
    )]
    async fn list_qa(&self) -> Out {
        let (ctx, ws) = self.qa_repo().await?;
        let sessions = ctx.qa_sessions(&ws).map_err(failed)?;
        let list: Vec<_> = sessions
            .iter()
            .filter(|s| !s.archived)
            .map(Session::listing)
            .collect();
        json(&list)
    }

    #[tool(
        description = "A Q&A session's prompt, the files to read first, and its question \
        queue: every question, its state and answer."
    )]
    async fn qa_questions(&self, Parameters(p): Parameters<inputs::QaSession>) -> Out {
        let s = self.session(&p.session).await?;
        json(&serde_json::json!({
            "prompt": s.prompt,
            "readFirst": s.attach,
            "queue": s.queue,
        }))
    }

    #[tool(
        description = "The rules for building and updating Q&A questions. Read them before \
        qa_ask."
    )]
    async fn qa_rules(&self) -> Out {
        prompts::load(Prompt::Qq).map_err(failed)
    }

    #[tool(
        description = "Ask the human questions through lgtm's Q&A, built by the rules qa_rules \
        returns. They show one at a time in the Q&A tab, in the qqueue format: context bullets, \
        options with the recommended one first, and why. A question with `auto` is answered \
        with option 1 and skipped unless the human reopens it. The Q&A tab opens on the \
        session. Returns the session and the new questions' ids in the order given. As the \
        human answers, the tab sends this terminal prompts starting with `lgtm Q&A` to update \
        and discuss questions. The human sends the decisions back to this terminal; \
        qa_answers reads them too."
    )]
    async fn qa_ask(&self, Parameters(p): Parameters<inputs::QaAsk>) -> Out {
        let (ctx, ws) = self.qa_repo().await?;
        let into = match p.session {
            Some(id) => AskIn::Session(id),
            None => {
                let title = p.title.unwrap_or_default();
                let prompt = match p.brief {
                    Some(b) => format!("{title}\n\n{b}"),
                    None => title,
                };
                AskIn::New {
                    prompt,
                    attach: p.read_first,
                }
            }
        };
        let questions = p.questions.into_iter().map(Into::into).collect();
        let terminal = self.hello.terminal.clone();
        let (id, ids) = ctx.ask_qa(&ws, into, terminal, questions).map_err(failed)?;
        Ok(format!(
            "Queued {} (in the order given) in session {id}, now showing in lgtm's Q&A tab. \
             Stop here: the human answers there. lgtm sends you prompts starting with `lgtm Q&A` \
             as they do, and the decisions when they're done.",
            ids.join(", ")
        ))
    }

    #[tool(
        description = "A queued question, its discussion so far and how to discuss it. Read it \
        when a `lgtm Q&A` prompt asks about a question."
    )]
    async fn qa_question(&self, Parameters(p): Parameters<inputs::QaQuestionRef>) -> Out {
        let s = self.session(&p.session).await?;
        discuss::question_text(&s, &p.id).map_err(failed)
    }

    #[tool(
        description = "Rewrite a queued question when talking it over changes its context, \
        options or recommendation. lgtm's Q&A tab shows the new version; a decided question \
        opens again, with what it said before, unless `auto` keeps its auto answer."
    )]
    async fn qa_rewrite(&self, Parameters(p): Parameters<inputs::QaRewrite>) -> Out {
        let r: Rewrite = p.question.into();
        let qid = r.id.clone();
        let (ctx, ws) = self.qa_repo().await?;
        ctx.reask_qa(&ws, &p.session, r).map_err(failed)?;
        Ok(format!("Rewrote {qid}; lgtm's Q&A tab shows it."))
    }

    #[tool(
        description = "Record the human's decision on an open question when they settle it while \
        discussing it. lgtm then asks you to update the questions that depend on it."
    )]
    async fn qa_decide(&self, Parameters(p): Parameters<inputs::QaDecide>) -> Out {
        let (ctx, ws) = self.qa_repo().await?;
        let decision = Decision {
            option: p.option,
            note: p.note,
            other: p.other,
        };
        ctx.decide_in_discussion(&ws, &p.session, &p.id, decision)
            .map_err(failed)?;
        Ok(format!("Decided {}.", p.id))
    }

    #[tool(
        description = "The answer to a question, the questions that depend on it, and how to \
        update them with qa_update."
    )]
    async fn qa_dependents(&self, Parameters(p): Parameters<inputs::QaQuestionRef>) -> Out {
        let s = self.session(&p.session).await?;
        dependents_text(&s, &p.id).map_err(failed)
    }

    #[tool(
        description = "Apply an answer to the questions that depend on it, as qa_dependents \
        describes: drop the open ones it settles, rewrite the ones it reshapes, and add the \
        decisions it raises. Questions it leaves alone stay out."
    )]
    async fn qa_update(&self, Parameters(p): Parameters<inputs::QaUpdate>) -> Out {
        let (session, after) = (p.session.clone(), p.after.clone());
        let (ctx, ws) = self.qa_repo().await?;
        ctx.update_qa(&ws, &session, &after, p.into())
            .map_err(failed)?;
        Ok("Updated; lgtm's Q&A tab shows it.".into())
    }

    #[tool(description = "The decisions on a Q&A session's questions so far, in ask order.")]
    async fn qa_answers(&self, Parameters(p): Parameters<inputs::QaSession>) -> Out {
        let s = self.session(&p.session).await?;
        json(&s.answers())
    }
}
