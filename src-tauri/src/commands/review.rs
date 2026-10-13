use tauri::WebviewWindow;

use super::{CmdResult, err, repo};
use crate::jj::{JjAction, LogLine};
use crate::review::{GraphDto, RefreshDto, ReviewDto, RowRef, StateDto, View};
use crate::store::MarkState;

/// Loads the review for `revset` after snapshotting the working copy. Starts a review
/// session, or with `resume` continues the revset's latest one.
#[tauri::command]
#[specta::specta]
pub(crate) async fn open_review(
    window: WebviewWindow,
    workspace: Option<String>,
    revset: String,
    resume: bool,
) -> CmdResult<ReviewDto> {
    repo(&window)?
        .open(workspace.as_deref(), &revset, resume)
        .await
        .map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn close_review(window: WebviewWindow, review: u32) -> CmdResult<()> {
    repo(&window)?.close(review);
    Ok(())
}

/// `jj log` with the user's default revset and template, for picking commits.
#[tauri::command]
#[specta::specta]
pub(crate) async fn log_graph(
    window: WebviewWindow,
    workspace: Option<String>,
) -> CmdResult<Vec<LogLine>> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    ws.jj.log_graph().await.map_err(err)
}

/// The local bookmark `trunk()` resolves to, as a revset symbol, for the revisions panel's
/// presets. `None` when trunk has no bookmark.
#[tauri::command]
#[specta::specta]
pub(crate) async fn trunk_bookmark(
    window: WebviewWindow,
    workspace: Option<String>,
) -> CmdResult<Option<String>> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    ws.jj.trunk_bookmark().await.map_err(err)
}

/// Runs a revisions panel action in the workspace. Returns how many jj operations it took.
#[tauri::command]
#[specta::specta]
pub(crate) async fn jj_act(
    window: WebviewWindow,
    workspace: Option<String>,
    action: JjAction,
) -> CmdResult<u32> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    ws.jj.act(&action).await.map_err(err)
}

/// Reloads a review after the repo changed, or switches it to `view`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn refresh_review(
    window: WebviewWindow,
    review: u32,
    view: Option<View>,
) -> CmdResult<RefreshDto> {
    repo(&window)?.refresh(review, view).await.map_err(err)
}

/// Marks changed rows, or clears their marks when `state` is null. Files in `check` are
/// checked in the same undo step.
#[tauri::command]
#[specta::specta]
pub(crate) fn mark_rows(
    window: WebviewWindow,
    review: u32,
    rows: Vec<RowRef>,
    state: Option<MarkState>,
    check: Vec<u32>,
) -> CmdResult<StateDto> {
    repo(&window)?
        .mark(review, &rows, state, &check)
        .map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn set_files_checked(
    window: WebviewWindow,
    review: u32,
    files: Vec<u32>,
    checked: bool,
) -> CmdResult<StateDto> {
    repo(&window)?
        .set_checked(review, &files, checked)
        .map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn undo_marks(window: WebviewWindow, review: u32) -> CmdResult<StateDto> {
    repo(&window)?.undo(review).map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn redo_marks(window: WebviewWindow, review: u32) -> CmdResult<StateDto> {
    repo(&window)?.redo(review).map_err(err)
}

/// Removes the marks and file checks this review's revset created.
#[tauri::command]
#[specta::specta]
pub(crate) fn reset_review_marks(window: WebviewWindow, review: u32) -> CmdResult<StateDto> {
    repo(&window)?.reset(review).map_err(err)
}

/// The review's symbol graph: changed symbols, their links, and unchanged callers.
#[tauri::command]
#[specta::specta]
pub(crate) async fn review_graph(window: WebviewWindow, review: u32) -> CmdResult<GraphDto> {
    repo(&window)?.graph(review).await.map_err(err)
}

/// Records the review a workspace's active review pane shows, for agents in its terminals.
#[tauri::command]
#[specta::specta]
pub(crate) fn focus_review(
    window: WebviewWindow,
    workspace: Option<String>,
    review: u32,
) -> CmdResult<()> {
    repo(&window)?.focus(workspace.as_deref(), review);
    Ok(())
}
