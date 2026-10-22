use tauri::{Manager, WebviewWindow};
use tauri_specta::Event as _;

use super::{CmdResult, err, repo};
use crate::events::LogRevsetChanged;
use crate::jj::{BookmarkSpot, JjAction, WorkingCopyDto};
use crate::revisions::LogView;
use crate::review::{ContextLine, RefreshDto, ReviewDto, RowRef, StateDto, View};
use crate::store::MarkColor;

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

/// Unchanged lines `from` to `to` of a file's new side, for showing code between hunks.
#[tauri::command]
#[specta::specta]
pub(crate) async fn context_lines(
    window: WebviewWindow,
    review: u32,
    file: u32,
    from: u32,
    to: u32,
) -> CmdResult<Vec<ContextLine>> {
    repo(&window)?
        .context_lines(review, file, from, to)
        .await
        .map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn close_review(window: WebviewWindow, review: u32) -> CmdResult<()> {
    repo(&window)?.close(review);
    Ok(())
}

/// The workspace's revisions log: its revset and the rows of `jj log` with its graph.
#[tauri::command]
#[specta::specta]
pub(crate) async fn log_view(
    window: WebviewWindow,
    workspace: Option<String>,
) -> CmdResult<LogView> {
    repo(&window)?.log_view(workspace.as_deref()).await.map_err(err)
}

/// Sets the workspace's log revset, blank for jj's default, failing with jj's error.
#[tauri::command]
#[specta::specta]
pub(crate) async fn set_log_revset(
    window: WebviewWindow,
    workspace: Option<String>,
    revset: String,
) -> CmdResult<()> {
    let ctx = repo(&window)?;
    ctx.set_log_revset(workspace.as_deref(), &revset)
        .await
        .map_err(err)?;
    LogRevsetChanged { workspace }
        .emit_to(window.app_handle(), window.label())
        .map_err(|e| e.to_string())
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

/// The commit ids in `revset`, so the revisions panel can show what a range will review.
#[tauri::command]
#[specta::specta]
pub(crate) async fn revset_commits(
    window: WebviewWindow,
    workspace: Option<String>,
    revset: String,
) -> CmdResult<Vec<String>> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    ws.jj.commit_ids(&revset).await.map_err(err)
}

/// The workspace's working copy, and where it sits against the review of `revset`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn working_copy(
    window: WebviewWindow,
    workspace: Option<String>,
    revset: String,
) -> CmdResult<WorkingCopyDto> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    ws.jj.working_copy(&revset).await.map_err(err)
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

/// The bookmarks closest to `rev` on either side, for moving one onto it.
#[tauri::command]
#[specta::specta]
pub(crate) async fn nearest_bookmarks(
    window: WebviewWindow,
    workspace: Option<String>,
    rev: String,
) -> CmdResult<Vec<BookmarkSpot>> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    ws.jj.nearest_bookmarks(&rev).await.map_err(err)
}

/// What a push from the workspace would leave out without asking.
#[tauri::command]
#[specta::specta]
pub(crate) async fn push_plan(
    window: WebviewWindow,
    workspace: Option<String>,
) -> CmdResult<crate::jj::PushPlan> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    ws.jj.push_plan().await.map_err(err)
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

/// Marks changed rows, or clears their marks when `color` is null. Files in `check` are
/// checked in the same undo step.
#[tauri::command]
#[specta::specta]
pub(crate) fn mark_rows(
    window: WebviewWindow,
    review: u32,
    rows: Vec<RowRef>,
    color: Option<MarkColor>,
    check: Vec<u32>,
) -> CmdResult<StateDto> {
    repo(&window)?
        .mark(review, &rows, color, &check)
        .map_err(err)
}

/// Clears a file's marks and unchecks it, in one undo step.
#[tauri::command]
#[specta::specta]
pub(crate) fn clear_file_marks(
    window: WebviewWindow,
    review: u32,
    file: u32,
) -> CmdResult<StateDto> {
    repo(&window)?.clear_file(review, file).map_err(err)
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
