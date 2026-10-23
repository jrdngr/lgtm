//! The guided review tool: storing an agent's pick of the review's code a person should read.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};
use tauri_specta::Event as _;

use super::server::{Lgtm, Out, failed};
use crate::events::GuidedChanged;
use crate::review::guided::reply::GuidedReply;

#[tool_router(router = guided_router, vis = "pub(super)")]
impl Lgtm {
    #[tool(
        description = "Store your guided review of the review's change in lgtm, replacing the \
        last one: the code a person should read as chunks rated high, medium or low, the code \
        left out, and the ADRs and dev logs it relates to. Its Guided tab shows it. Ranges on \
        files the review doesn't change are dropped. It's refused once the review has moved \
        off the from and to commits the prompt named."
    )]
    async fn set_guided_review(&self, Parameters(p): Parameters<GuidedReply>) -> Out {
        let (label, ctx) = self.repo().map_err(failed)?;
        let review = ctx.current_review(self.workspace()).map_err(failed)?;
        let (revset, from, to) = (p.revset.clone(), p.from.clone(), p.to.clone());
        let (chunks, excluded) = ctx
            .save_guided(review, &revset, (&from, &to), p)
            .map_err(failed)?;
        GuidedChanged {
            workspace: self.workspace().map(str::to_string),
            review,
        }
        .emit_to(self.app(), &label)
        .ok();
        Ok(format!(
            "Stored it: {chunks} chunks, {excluded} excluded parts."
        ))
    }
}
