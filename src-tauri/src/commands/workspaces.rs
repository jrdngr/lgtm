use tauri::{State, WebviewWindow};

use super::{CmdResult, err, layout_path, repo};
use crate::layout;
use crate::pty::{Owner, Ptys};
use crate::review::ProgressDto;
use crate::workspaces::WorkspaceDto;

/// Every workspace, the repo's main one first.
#[tauri::command]
#[specta::specta]
pub(crate) async fn list_workspaces(window: WebviewWindow) -> CmdResult<Vec<WorkspaceDto>> {
    repo(&window)?.list_workspaces().await.map_err(err)
}

/// Adds a workspace on `trunk()`. Returns whether it has a `.lgtm/setup` to run.
#[tauri::command]
#[specta::specta]
pub(crate) async fn create_workspace(window: WebviewWindow, name: String) -> CmdResult<bool> {
    repo(&window)?.create_workspace(&name).await.map_err(err)
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

/// Review progress of the workspace's latest review, or of its `@`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn workspace_progress(
    window: WebviewWindow,
    workspace: Option<String>,
) -> CmdResult<ProgressDto> {
    repo(&window)?
        .progress(workspace.as_deref())
        .await
        .map_err(err)
}
