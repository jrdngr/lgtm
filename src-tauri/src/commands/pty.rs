use tauri::ipc::Channel;
use tauri::{Manager, State, WebviewWindow};

use super::{CmdResult, err, repo};
use crate::pty::{Owner, PtyEvent, Ptys};
use crate::workspaces;

/// Starts a login shell in the workspace for terminal panel `terminal`. With `setup`, the
/// shell first runs its `.lgtm/setup`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn pty_spawn(
    window: WebviewWindow,
    ptys: State<'_, Ptys>,
    workspace: Option<String>,
    terminal: String,
    cols: u16,
    rows: u16,
    setup: bool,
    run: Option<String>,
    on_event: Channel<PtyEvent>,
) -> CmdResult<u32> {
    let ctx = repo(&window)?;
    let ws = ctx.workspace(workspace.as_deref()).await.map_err(err)?;
    let command = match run {
        Some(line) => Some(crate::pty::shell_running(&line)),
        None => setup.then(workspaces::setup_command),
    };
    let root = ctx.root.to_string_lossy();
    let socket = crate::repo::data_dir(window.app_handle())
        .map(|d| crate::mcp::socket_path(&d).to_string_lossy().into_owned())
        .unwrap_or_default();
    let env = [
        ("LGTM_ROOT", root.as_ref()),
        ("LGTM_WORKSPACE", ws.name.as_deref().unwrap_or("")),
        ("LGTM_SOCKET", socket.as_str()),
        ("LGTM_TERMINAL", terminal.as_str()),
    ];
    let owner = Owner {
        window: window.label().to_string(),
        workspace,
    };
    ptys.spawn(owner, &ws.root, (cols, rows), command, &env, move |e| {
        on_event.send(e).ok();
    })
    .map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn pty_write(ptys: State<Ptys>, id: u32, data: String) -> CmdResult<()> {
    ptys.write(id, data.as_bytes()).map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn pty_resize(ptys: State<Ptys>, id: u32, cols: u16, rows: u16) -> CmdResult<()> {
    ptys.resize(id, cols, rows).map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn pty_kill(ptys: State<Ptys>, id: u32) {
    ptys.kill(id);
}

/// The program running in the foreground of PTY `id`, when it isn't the shell itself.
#[tauri::command]
#[specta::specta]
pub(crate) fn pty_running(ptys: State<Ptys>, id: u32) -> Option<String> {
    ptys.running(id)
}
