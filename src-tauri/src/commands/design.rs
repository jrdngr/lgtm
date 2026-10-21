use tauri::WebviewWindow;

use super::{CmdResult, err, repo};
use crate::review::arch::design::{DesignCanvas, DesignDto, DesignInput, DesignItemDto};

/// Every design of the repo, with its state.
#[tauri::command]
#[specta::specta]
pub(crate) async fn designs(
    window: WebviewWindow,
    workspace: Option<String>,
) -> CmdResult<Vec<DesignItemDto>> {
    repo(&window)?
        .designs(workspace.as_deref())
        .await
        .map_err(err)
}

/// Creates a design, or with `id` changes one.
#[tauri::command]
#[specta::specta]
pub(crate) async fn save_design(
    window: WebviewWindow,
    workspace: Option<String>,
    id: Option<String>,
    design: DesignInput,
) -> CmdResult<DesignDto> {
    repo(&window)?
        .save_design(workspace.as_deref(), id.as_deref(), design)
        .await
        .map_err(err)
}

/// Stores the user's drawing over a design's map.
#[tauri::command]
#[specta::specta]
pub(crate) fn save_design_canvas(
    window: WebviewWindow,
    id: String,
    canvas: DesignCanvas,
) -> CmdResult<DesignDto> {
    repo(&window)?.save_design_canvas(&id, canvas).map_err(err)
}

/// Undoes the last agent pass over a design's canvas.
#[tauri::command]
#[specta::specta]
pub(crate) fn undo_design_agent(window: WebviewWindow, id: String) -> CmdResult<DesignDto> {
    repo(&window)?.undo_design_agent(&id).map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn delete_design(window: WebviewWindow, id: String) -> CmdResult<()> {
    repo(&window)?.delete_design(&id).map_err(err)
}

/// The prompt asking a terminal agent to write a design's map.
#[tauri::command]
#[specta::specta]
pub(crate) async fn design_prompt(
    window: WebviewWindow,
    workspace: Option<String>,
    id: String,
) -> CmdResult<String> {
    repo(&window)?
        .design_prompt(workspace.as_deref(), &id)
        .await
        .map_err(err)
}

/// Every file at a revision.
#[tauri::command]
#[specta::specta]
pub(crate) async fn revision_files(
    window: WebviewWindow,
    workspace: Option<String>,
    revision: String,
) -> CmdResult<Vec<String>> {
    repo(&window)?
        .revision_files(workspace.as_deref(), &revision)
        .await
        .map_err(err)
}

/// Local bookmarks, those on `@`'s stack first.
#[tauri::command]
#[specta::specta]
pub(crate) async fn design_bookmarks(
    window: WebviewWindow,
    workspace: Option<String>,
) -> CmdResult<Vec<String>> {
    repo(&window)?
        .design_bookmarks(workspace.as_deref())
        .await
        .map_err(err)
}
