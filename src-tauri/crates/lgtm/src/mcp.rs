//! `lgtm mcp` outside the desktop app: tools for the terminal app open in the agent's
//! workspace, asked over its socket at each call, so it can start after the agent does.

use std::path::PathBuf;

use lgtm_core::review::agent as review_agent;
use lgtm_core::turns::{self, StartSnapshot};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};

use crate::agent::{self, CheckFiles, MarkLines, Request, ResolveComment, Response};

const INSTRUCTIONS: &str = "lgtm is the terminal code review app open in this workspace. \
The human reviews a change there and marks hunks reviewed or flagged.";

const MORE_INSTRUCTIONS: &str = "You can check files and mark lines with marks of your \
own, which the human sees apart from theirs. Lines are on the new side of the diff.";

#[derive(Clone)]
pub struct Tui {
    socket: PathBuf,
    tool_router: ToolRouter<Self>,
}

impl Tui {
    async fn ask(&self, req: Request) -> Response {
        let socket = self.socket.clone();
        tokio::task::spawn_blocking(move || agent::ask(&socket, &req))
            .await
            .map_err(|e| e.to_string())?
    }
}

#[tool_router]
impl Tui {
    #[tool(description = review_agent::GET_REVIEW)]
    async fn get_review(&self) -> Response {
        self.ask(Request::GetReview).await
    }

    #[tool(
        description = concat!(
            lgtm_core::get_comments_doc!(),
            " Then the open threads of the review's PR on GitHub (P1, P2, ...), once the \
            human pulled them, with their posts."
        )
    )]
    async fn get_comments(&self) -> Response {
        self.ask(Request::GetComments).await
    }

    #[tool(description = review_agent::RESOLVE_COMMENT)]
    async fn resolve_comment(&self, Parameters(p): Parameters<ResolveComment>) -> Response {
        self.ask(Request::ResolveComment(p)).await
    }

    #[tool(description = review_agent::UNRESOLVE_COMMENT)]
    async fn unresolve_comment(&self, Parameters(p): Parameters<ResolveComment>) -> Response {
        self.ask(Request::UnresolveComment(p)).await
    }

    #[tool(description = review_agent::CHECK_FILES)]
    async fn check_files(&self, Parameters(p): Parameters<CheckFiles>) -> Response {
        self.ask(Request::CheckFiles(p)).await
    }

    #[tool(description = turns::START_SNAPSHOT)]
    async fn start_snapshot(&self, Parameters(p): Parameters<StartSnapshot>) -> Response {
        let label = p.label.lines().next().unwrap_or("").trim().to_string();
        if label.is_empty() {
            return Err("name the snapshot".into());
        }
        let root = std::env::current_dir()
            .ok()
            .and_then(|d| agent::workspace(&d))
            .ok_or("not in a jj workspace")?;
        crate::snap::start(&root, &label)
            .await
            .map_err(|e| format!("{e:#}"))?;
        Ok(format!("Edits from here go into the snapshot \"{label}\"."))
    }

    #[tool(description = review_agent::MARK_LINES)]
    async fn mark_lines(&self, Parameters(p): Parameters<MarkLines>) -> Response {
        self.ask(Request::MarkLines(p)).await
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Tui {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("lgtm", env!("CARGO_PKG_VERSION")))
            .with_instructions(format!(
                "{INSTRUCTIONS} {} {MORE_INSTRUCTIONS}",
                review_agent::COMMENTS_INSTRUCTIONS
            ))
    }
}

/// Serves the tools on stdio for the workspace whose terminal app listens at `socket`.
pub fn serve(socket: PathBuf) -> i32 {
    let tui = Tui {
        socket,
        tool_router: Tui::tool_router(),
    };
    crate::route::serve_stdio(tui)
}
