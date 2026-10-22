//! The Symbol Information tool: an agent's notes on a symbol for its callers.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};
use tauri_specta::Event as _;

use super::server::{Lgtm, Out, failed};
use crate::events::SymbolInfoChanged;
use crate::symbol_info::{self, InfoReply};

#[tool_router(router = symbol_info_router, vis = "pub(super)")]
impl Lgtm {
    #[tool(
        description = "Store your Symbol Information on a symbol in lgtm, replacing the last: \
        what it does, its callers with paths and lines, what a caller must assume or keep \
        true, and how it fails. lgtm shows it under the symbol's definition. Only when lgtm's \
        prompt asks for it."
    )]
    async fn set_symbol_info(&self, Parameters(p): Parameters<InfoReply>) -> Out {
        let (label, ctx) = self.repo().map_err(failed)?;
        let ws = ctx.workspace(self.workspace()).await.map_err(failed)?;
        let name = p.name.clone();
        symbol_info::set(&ctx.store.lock().unwrap(), &ws.root, p).map_err(failed)?;
        SymbolInfoChanged.emit_to(self.app(), &label).ok();
        Ok(format!("Stored Symbol Information on `{name}`."))
    }
}
