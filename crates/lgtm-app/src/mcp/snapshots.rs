//! The snapshot tool: splitting an agent's turn into named snapshots in the revisions log.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};

use super::server::{Lgtm, Out, failed};
use crate::turns::Checkpoint;
use lgtm_core::turns::StartSnapshot;

#[tool_router(router = snapshots_router, vis = "pub(super)")]
impl Lgtm {
    #[tool(description = lgtm_core::turns::START_SNAPSHOT)]
    async fn start_snapshot(&self, Parameters(p): Parameters<StartSnapshot>) -> Out {
        let (_, ctx) = self.repo().map_err(failed)?;
        let session = self
            .agent_session(&ctx)
            .ok_or("lgtm doesn't know this agent's session")?;
        let name = p.label.lines().next().unwrap_or("").trim();
        if name.is_empty() {
            return Err("name the snapshot".into());
        }
        ctx.turn_checkpoint(
            self.workspace(),
            self.hello.terminal.as_deref(),
            &session,
            None,
            Checkpoint::Start,
            Some(name),
        )
        .await
        .map_err(failed)?;
        Ok(format!("Edits from here go into the snapshot \"{name}\"."))
    }
}
