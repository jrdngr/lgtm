//! The revisions log tool: which commits the human's revisions log lists.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};
use tauri_specta::Event as _;

use super::inputs;
use super::server::{Lgtm, Out, failed};
use crate::events::LogRevsetChanged;

#[tool_router(router = revisions_router, vis = "pub(super)")]
impl Lgtm {
    #[tool(
        description = "Set the jj revset the human's revisions log lists, in this terminal's \
        workspace, and show it to them. An empty revset goes back to jj's default log. jj's \
        error comes back for a revset it can't take, and the log stays as it was. This doesn't \
        change the review; set_review_revset does."
    )]
    async fn set_log_revset(&self, Parameters(p): Parameters<inputs::Revset>) -> Out {
        let (label, ctx) = self.repo().map_err(failed)?;
        ctx.set_log_revset(self.workspace(), &p.revset)
            .await
            .map_err(failed)?;
        LogRevsetChanged {
            workspace: self.hello.workspace.clone(),
        }
        .emit_to(&self.app, &label)
        .map_err(|e| e.to_string())?;
        Ok("The revisions log shows it.".into())
    }
}
