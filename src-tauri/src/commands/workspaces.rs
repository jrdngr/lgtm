use tauri::{State, WebviewWindow};

use super::{CmdResult, err, layout_path, repo};
use crate::layout;
use crate::pty::{Owner, Ptys};
use crate::workspaces::{Setup, WorkspaceDto};

/// Every workspace, the repo's main one first.
#[tauri::command]
#[specta::specta]
pub(crate) async fn list_workspaces(window: WebviewWindow) -> CmdResult<Vec<WorkspaceDto>> {
    repo(&window)?.list_workspaces().await.map_err(err)
}

/// Remembers where workspace `name`'s files are, when they're there.
#[tauri::command]
#[specta::specta]
pub(crate) async fn locate_workspace(
    window: WebviewWindow,
    name: String,
    root: String,
) -> CmdResult<()> {
    repo(&window)?
        .locate_workspace(&name, std::path::Path::new(&root))
        .await
        .map_err(err)
}

/// The UI opened the workspace: watches it, and starts its paused background snapshots again.
#[tauri::command]
#[specta::specta]
pub(crate) async fn open_workspace(
    window: WebviewWindow,
    workspace: Option<String>,
) -> CmdResult<()> {
    repo(&window)?
        .open_workspace(workspace.as_deref())
        .await
        .map_err(err)
}

/// Ends the workspace's terminals and puts it to sleep until it's opened again.
#[tauri::command]
#[specta::specta]
pub(crate) async fn sleep_workspace(
    window: WebviewWindow,
    ptys: State<'_, Ptys>,
    workspace: Option<String>,
) -> CmdResult<()> {
    ptys.kill_owner(&Owner {
        window: window.label().to_string(),
        workspace: workspace.clone(),
    });
    repo(&window)?
        .sleep_workspace(workspace.as_deref())
        .await
        .map_err(err)
}

/// Adds a workspace on `trunk()`. Returns whether it has a `.lgtm/setup` the user let run.
#[tauri::command]
#[specta::specta]
pub(crate) async fn create_workspace(window: WebviewWindow, name: String) -> CmdResult<Setup> {
    repo(&window)?.create_workspace(&name).await.map_err(err)
}

/// Lets the workspace's `.lgtm/setup` run, and others with the same contents after it, if it
/// still has `hash`, the hash of the script the user was shown.
#[tauri::command]
#[specta::specta]
pub(crate) async fn approve_setup(
    window: WebviewWindow,
    workspace: String,
    hash: String,
) -> CmdResult<()> {
    let ctx = repo(&window)?;
    let ws = ctx.workspace(Some(&workspace)).await.map_err(err)?;
    ctx.approve_setup(&ws.root, &hash).map_err(err)
}

/// Ends the workspace's terminals, then forgets it and deletes its directory.
#[tauri::command]
#[specta::specta]
pub(crate) async fn delete_workspace(
    window: WebviewWindow,
    ptys: State<'_, Ptys>,
    name: String,
) -> CmdResult<()> {
    let owner = Owner {
        window: window.label().to_string(),
        workspace: Some(name.clone()),
    };
    repo(&window)?
        .delete_workspace(&name, || ptys.kill_owner(&owner))
        .await
        .map_err(err)?;
    layout::remove(&layout_path(&window, Some(&name))?).map_err(|e| e.to_string())
}
