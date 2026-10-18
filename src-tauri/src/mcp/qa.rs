//! Q&A tools: the agent asks the human questions in lgtm's Q&A tab, then works through the
//! steps the tab sends it as prompts starting with `lgtm Q&A`.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};
use tauri_specta::Event as _;

use super::qa_inputs as inputs;
use super::server::{Lgtm, Out, failed, json};
use crate::events::QaChanged;
use crate::prompts::{self, Prompt};
use crate::qa::discuss::{self, Decision};
use crate::qa::queue::{QState, Rewrite, Update};
use crate::qa::text::answer_text;
use crate::qa::update::dependents_text;
use crate::qa::{Session, queue};

impl Lgtm {
    /// Changes session `id` and has the Q&A tab show it.
    async fn edit_session<T>(
        &self,
        id: &str,
        f: impl FnOnce(&mut Session) -> anyhow::Result<T>,
    ) -> Result<T, String> {
        let (label, ctx) = self.repo().map_err(failed)?;
        let ws = ctx.workspace(self.workspace()).await.map_err(failed)?;
        let out = ctx.edit_qa(&ws, id, f).map_err(failed)?;
        QaChanged {
            workspace: self.hello.workspace.clone(),
            id: id.to_string(),
            show: true,
        }
        .emit_to(&self.app, &label)
        .ok();
        Ok(out)
    }

    async fn session(&self, id: &str) -> Result<Session, String> {
        let (_, ctx) = self.repo().map_err(failed)?;
        let ws = ctx.workspace(self.workspace()).await.map_err(failed)?;
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

    #[tool(description = "The rules for building and updating Q&A questions. Read them before \
        qa_ask.")]
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
        let (_, ctx) = self.repo().map_err(failed)?;
        let ws = ctx.workspace(self.workspace()).await.map_err(failed)?;
        let (id, new) = match p.session {
            Some(id) => (id, false),
            None => {
                let title = p.title.unwrap_or_default();
                let prompt = match p.brief {
                    Some(b) => format!("{title}\n\n{b}"),
                    None => title,
                };
                let s = ctx.create_qa(&ws, &prompt, p.read_first).map_err(failed)?;
                (s.id, true)
            }
        };
        let questions = p.questions.into_iter().map(Into::into).collect();
        let terminal = self.hello.terminal.clone();
        let ids = self
            .edit_session(&id, |s| {
                s.from_terminal |= new;
                // The agent that asked runs the session's steps.
                if terminal.is_some() {
                    s.terminal = terminal;
                }
                Ok(s.queue.add_round(questions))
            })
            .await?;
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
        self.edit_session(&p.session, |s| s.queue.reask(r)).await?;
        Ok(format!("Rewrote {qid}; lgtm's Q&A tab shows it."))
    }

    #[tool(
        description = "Record the human's decision on an open question when they settle it while \
        discussing it. lgtm then asks you to update the questions that depend on it."
    )]
    async fn qa_decide(&self, Parameters(p): Parameters<inputs::QaDecide>) -> Out {
        let (label, ctx) = self.repo().map_err(failed)?;
        let ws = ctx.workspace(self.workspace()).await.map_err(failed)?;
        let decision = Decision {
            option: p.option,
            note: p.note,
            other: p.other,
        };
        ctx.decide_in_discussion(&ws, &p.session, &p.id, decision)
            .map_err(failed)?;
        QaChanged {
            workspace: self.hello.workspace.clone(),
            id: p.session,
            show: true,
        }
        .emit_to(&self.app, &label)
        .ok();
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
        let update = Update {
            drop: p
                .drop
                .into_iter()
                .map(|d| queue::Drop {
                    id: d.id,
                    why: d.why,
                })
                .collect(),
            rewrite: p.rewrite.into_iter().map(Into::into).collect(),
            add: p.add.into_iter().map(Into::into).collect(),
        };
        self.edit_session(&p.session, |s| {
            anyhow::ensure!(s.queue.get(&p.after).is_some(), "no question {}", p.after);
            s.queue.apply(&p.after, update);
            Ok(())
        })
        .await?;
        Ok("Updated; lgtm's Q&A tab shows it.".into())
    }

    #[tool(description = "The decisions on a Q&A session's questions so far, in ask order.")]
    async fn qa_answers(&self, Parameters(p): Parameters<inputs::QaSession>) -> Out {
        let s = self.session(&p.session).await?;
        let queue = s.queue;
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
                    "auto": q.answer.as_ref().is_some_and(|a| a.is_auto()),
                })
            })
            .collect();
        let open = queue.open_count();
        json(&serde_json::json!({ "decided": list, "stillOpen": open }))
    }
}
