//! The MCP server agents in lgtm's terminals use to read the review and add to it, served
//! over the socket `agents` listens on.

mod arch;
mod canvas;
mod design;
mod guided;
mod inputs;
mod notes;
mod qa;
mod qa_inputs;
mod reports;
mod revisions;
mod server;
mod snapshots;
mod symbol_info;

use serde::{Deserialize, Serialize};

pub use server::Lgtm;

/// Who's calling, from the terminal's environment. The bridge sends it as the first line.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Hello {
    /// The repo's main workspace root (`LGTM_ROOT`).
    pub root: String,
    /// The workspace, absent for the main one (`LGTM_WORKSPACE`).
    pub workspace: Option<String>,
    /// The Claude Code session that started the bridge (`CLAUDE_CODE_SESSION_ID`).
    #[serde(default)]
    pub claude_session: Option<String>,
    /// The terminal panel the bridge runs in (`LGTM_TERMINAL`).
    #[serde(default)]
    pub terminal: Option<String>,
}
