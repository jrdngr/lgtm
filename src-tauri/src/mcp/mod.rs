//! The MCP server agents in lgtm's terminals use to read the review and add to it. The app
//! listens on a Unix socket, and terminals get its path as `LGTM_SOCKET`. Claude Code starts
//! `lgtm mcp`, which bridges its stdio to that socket. Its hooks run `lgtm hook`, which reports
//! the terminal agent's state over the same socket.

mod bridge;
pub mod hook;
mod inputs;
pub mod register;
mod reports;
mod server;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::events::TerminalAgent;
use crate::repo::RepoWindows;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

pub use bridge::main as bridge_main;

pub fn socket_path(data_dir: &Path) -> PathBuf {
    data_dir.join("mcp.sock")
}

/// Who's calling, from the terminal's environment. The bridge sends it as the first line.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Hello {
    /// The repo's main workspace root (`LGTM_ROOT`).
    root: String,
    /// The workspace, absent for the main one (`LGTM_WORKSPACE`).
    workspace: Option<String>,
    /// The Claude Code session that started the bridge (`CLAUDE_CODE_SESSION_ID`).
    #[serde(default)]
    claude_session: Option<String>,
    /// The terminal panel the bridge runs in (`LGTM_TERMINAL`).
    #[serde(default)]
    terminal: Option<String>,
}

/// Starts serving agents on `path`, replacing a socket left by an earlier run.
pub fn listen(app: AppHandle, path: PathBuf) {
    tauri::async_runtime::spawn(async move {
        std::fs::remove_file(&path).ok();
        let listener = match UnixListener::bind(&path) {
            Ok(l) => l,
            Err(e) => return eprintln!("lgtm: MCP socket {}: {e}", path.display()),
        };
        loop {
            let (stream, _) = match listener.accept().await {
                Ok(s) => s,
                Err(e) => {
                    // Errors like running out of file descriptors persist, so don't spin.
                    eprintln!("lgtm: MCP accept: {e}");
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    continue;
                }
            };
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = serve(app, stream).await {
                    eprintln!("lgtm: MCP connection: {e:#}");
                }
            });
        }
    });
}

async fn serve(app: AppHandle, stream: UnixStream) -> anyhow::Result<()> {
    use rmcp::ServiceExt;
    let (read, write) = stream.into_split();
    let mut read = BufReader::new(read);
    let mut line = String::new();
    read.read_line(&mut line).await?;
    let first: serde_json::Value = serde_json::from_str(&line)?;
    if let Some(hook) = first.get("hook") {
        let report: hook::HookReport = serde_json::from_value(hook.clone())?;
        if let Some((label, _)) = app.state::<RepoWindows>().find(Path::new(&report.root)) {
            TerminalAgent {
                terminal: report.terminal,
                state: report.state,
            }
            .emit_to(&app, &label)
            .ok();
        }
        return Ok(());
    }
    let hello: Hello = serde_json::from_value(first)?;
    server::Lgtm::new(app, hello)
        .serve((read, write))
        .await?
        .waiting()
        .await?;
    Ok(())
}
