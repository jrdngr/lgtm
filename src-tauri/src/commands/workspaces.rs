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

/// Adds a workspace on `trunk()`. Returns whether it has a `.lgtm/setup` the user let run.
#[tauri::command]
#[specta::specta]
pub(crate) async fn create_workspace(window: WebviewWindow, name: String) -> CmdResult<Setup> {
    repo(&window)?.create_workspace(&name).await.map_err(err)
}

/// Lets the workspace's `.lgtm/setup` run, and others with the same contents after it.
#[tauri::command]
#[specta::specta]
pub(crate) async fn approve_setup(window: WebviewWindow, workspace: String) -> CmdResult<()> {
    let ctx = repo(&window)?;
    let ws = ctx.workspace(Some(&workspace)).await.map_err(err)?;
    ctx.approve_setup(&ws.root).map_err(err)
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

