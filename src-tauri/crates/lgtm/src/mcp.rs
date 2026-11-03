//! `lgtm mcp` outside the desktop app: tools for the terminal app open in the agent's
//! workspace, asked over its socket at each call, so it can start after the agent does.

use std::path::PathBuf;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};

use crate::agent::{self, CheckFiles, MarkLines, Request, ResolveComment, Response};

const INSTRUCTIONS: &str = "lgtm is the terminal code review app open in this workspace. \
The human reviews a change there, marks hunks reviewed or flagged, and leaves numbered \
comments for you (C1, C2, ...). Read them with get_comments, and once you've dealt with one, \
mark it done with resolve_comment. You can check files and mark lines with marks of your \
own, which the human sees apart from theirs. Lines are on the new side of the diff. Before \
each step of your work, run `lgtm snap \"<what the step will do>\"` in the workspace. The \
step's edits, until the next snap, show under that name so the human can review the step on \
its own.";

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
    #[tool(
        description = "The open review as the human has reviewed it so far: its revset and \
        commits, how much is done, and per file its state, how many changed rows count as \
        done, and the line ranges still left."
    )]
    async fn get_review(&self) -> Response {
        self.ask(Request::GetReview).await
    }

    #[tool(
        description = "The human's open comments for you, by number (C1, C2, ...): the file, \
        the line it's on now, that line's code, and what they wrote. Then the open threads of \
        the review's PR on GitHub (P1, P2, ...), once the human pulled them, with their posts."
    )]
    async fn get_comments(&self) -> Response {
        self.ask(Request::GetComments).await
    }

    #[tool(
        description = "Mark a comment done once you've dealt with it. The human no longer \
        sees it."
    )]
    async fn resolve_comment(&self, Parameters(p): Parameters<ResolveComment>) -> Response {
        self.ask(Request::ResolveComment(p)).await
    }

    #[tool(
        description = "Check or uncheck files in the open review as reviewed, as yourself: \
        the human sees your checks apart from theirs. Their own checks stay as they are."
    )]
    async fn check_files(&self, Parameters(p): Parameters<CheckFiles>) -> Response {
        self.ask(Request::CheckFiles(p)).await
    }

    #[tool(
        description = "Mark changed lines of a file in the open review with your own marks: \
        reviewed (counts as done) or flagged (doesn't). The human sees your marks apart from \
        theirs, and lines they marked keep their marks. Leave mark out to clear yours."
    )]
    async fn mark_lines(&self, Parameters(p): Parameters<MarkLines>) -> Response {
        self.ask(Request::MarkLines(p)).await
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Tui {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("lgtm", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
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
