//! The MCP server agents in lgtm's terminals use to read the review and add to it. The app
//! listens on a Unix socket, and terminals get its path as `LGTM_SOCKET`. Claude Code starts
//! `lgtm mcp`, which bridges its stdio to that socket. Its hooks run `lgtm hook`, which reports
//! the terminal agent's state over the same socket and gets back what to add to a prompt.

mod arch;
mod bridge;
mod canvas;
mod design;
mod guided;
pub mod hook;
mod inputs;
mod notes;
mod qa;
mod qa_inputs;
pub mod register;
mod reports;
mod revisions;
mod server;
mod snapshots;
mod symbol_info;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::events::{AgentHookState, TerminalAgent, TurnsChanged};
use crate::repo::Repos;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
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
            Err(e) => return tracing::error!("MCP socket {}: {e}", path.display()),
        };
        loop {
            let (stream, _) = match listener.accept().await {
                Ok(s) => s,
                Err(e) => {
                    // Errors like running out of file descriptors persist, so don't spin.
                    tracing::error!("MCP accept: {e}");
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    continue;
                }
            };
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = serve(app, stream).await {
                    tracing::error!("MCP connection: {e:#}");
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
        tracing::debug!(terminal = report.terminal, state = ?report.state, "agent hook");
        if let Some((label, ctx)) = app.state::<Repos>().find(Path::new(&report.root)) {
            if report.state == AgentHookState::Idle {
                ctx.new_agent_session(&report.terminal);
            }
            if let Some(session) = &report.session {
                let edit = report
                    .edit
                    .as_ref()
                    .map(|e| (e.path.as_str(), e.text.as_str()));
                ctx.record_hook(session, report.prompt.as_deref(), edit);
            }
            if let (Some(point), Some(session)) = (report.checkpoint, &report.session) {
                let workspace = report.workspace.as_deref();
                let (tool, prompt) = (report.tool_use.as_deref(), report.prompt.as_deref());
                match ctx
                    .turn_checkpoint(
                        workspace,
                        Some(&report.terminal),
                        session,
                        tool,
                        point,
                        prompt,
                    )
                    .await
                {
                    Ok(true) => {
                        let workspace = report.workspace.clone();
                        TurnsChanged { workspace }.emit_to(&app, &label).ok();
                    }
                    Ok(false) => {}
                    Err(e) => tracing::warn!(?point, "agent turn snapshot: {e:#}"),
                }
            }
            if report.prompt.is_some() || report.checkpoint.is_some() {
                let context = report
                    .prompt
                    .is_some()
                    .then(|| ctx.prompt_context(&report.terminal))
                    .flatten();
                let line = serde_json::json!({ "context": context });
                let mut write = write;
                write.write_all(format!("{line}\n").as_bytes()).await.ok();
            }
            TerminalAgent {
                terminal: report.terminal,
                state: report.state,
                prompt: report.prompt,
                reply: report.reply,
                session: report.session,
                ended: report.ended,
            }
            .emit_to(&app, &label)
            .ok();
        }
        return Ok(());
    }
    let hello: Hello = serde_json::from_value(first)?;
    let (workspace, terminal) = (hello.workspace.clone(), hello.terminal.clone());
    tracing::info!(?workspace, ?terminal, "agent connected over MCP");
    let served = server::Lgtm::new(app, hello).serve((read, write)).await;
    let ended = match served {
        Ok(running) => running
            .waiting()
            .await
            .map(drop)
            .map_err(anyhow::Error::from),
        Err(e) => Err(e.into()),
    };
    tracing::info!(?workspace, ?terminal, "agent disconnected from MCP");
    ended
}
