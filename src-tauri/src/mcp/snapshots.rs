//! The snapshot tool: splitting an agent's turn into named snapshots in the revisions log.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};
use tauri_specta::Event as _;

use super::inputs;
use super::server::{Lgtm, Out, failed};
use crate::events::TurnsChanged;
use crate::turns::Checkpoint;

#[tool_router(router = snapshots_router, vis = "pub(super)")]
impl Lgtm {
    #[tool(
        description = "Start a new lgtm snapshot named `label`. lgtm records the files you \
        change for each prompt as one snapshot under the working copy in the human's revisions \
        log, which they can review, undo or move to its own commit on its own. Your edits after \
        this call go into the new snapshot, until you call it again or your turn ends. When the \
        human asks for work in separate snapshots, call it before each part, naming the part."
    )]
    async fn start_snapshot(&self, Parameters(p): Parameters<inputs::Snapshot>) -> Out {
        let (label, ctx) = self.repo().map_err(failed)?;
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
        TurnsChanged {
            workspace: self.hello.workspace.clone(),
        }
        .emit_to(&self.app, &label)
        .map_err(|e| e.to_string())?;
        Ok(format!("Edits from here go into the snapshot \"{name}\"."))
    }
}
