use tauri::WebviewWindow;

use super::{CmdResult, err, repo};
use crate::review::arch::design::{DesignDto, DesignInput, DesignItemDto};

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
pub(crate) fn save_design(
    window: WebviewWindow,
    id: Option<String>,
    design: DesignInput,
) -> CmdResult<DesignDto> {
    repo(&window)?
        .save_design(id.as_deref(), design)
        .map_err(err)
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

/// Every file at a bookmark.
#[tauri::command]
#[specta::specta]
pub(crate) async fn bookmark_files(
    window: WebviewWindow,
    workspace: Option<String>,
    bookmark: String,
) -> CmdResult<Vec<String>> {
    repo(&window)?
        .bookmark_files(workspace.as_deref(), &bookmark)
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
