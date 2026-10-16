use tauri::ipc::Channel;
use tauri::{State, WebviewWindow};

use super::{CmdResult, err, repo};
use crate::agent::Runs;
use crate::review::{RowRef, StateDto};
use crate::store::Stage;

/// Comments on the changed `rows` of a selection. `snippet` is the code as shown. `draft`
/// makes it a draft for the workspace's PR on GitHub instead of a note.
#[tauri::command]
#[specta::specta]
pub(crate) fn add_comment(
    window: WebviewWindow,
    review: u32,
    rows: Vec<RowRef>,
    body: String,
    snippet: String,
    draft: bool,
) -> CmdResult<StateDto> {
    repo(&window)?
        .add_comment(review, &rows, &body, &snippet, draft)
        .map_err(err)
}

/// Moves a thread to `stage`.
#[tauri::command]
#[specta::specta]
pub(crate) fn set_thread_stage(
    window: WebviewWindow,
    review: u32,
    comment: u32,
    stage: Stage,
) -> CmdResult<StateDto> {
    repo(&window)?
        .set_stage(review, comment, stage)
        .map_err(err)
}

/// Clears a GitHub thread's take, reply and agent conversation, as if it were new.
#[tauri::command]
#[specta::specta]
pub(crate) fn reset_thread(
    window: WebviewWindow,
    review: u32,
    comment: u32,
) -> CmdResult<StateDto> {
    repo(&window)?.reset_thread(review, comment).map_err(err)
}

/// Marks a note done, or open again.
#[tauri::command]
#[specta::specta]
pub(crate) fn set_note_done(
    window: WebviewWindow,
    review: u32,
    comment: u32,
    done: bool,
) -> CmdResult<StateDto> {
    repo(&window)?
        .set_note_done(review, comment, done)
        .map_err(err)
}

/// Starts a GitHub draft on a note's code, with its text.
#[tauri::command]
#[specta::specta]
pub(crate) fn draft_from_note(
    window: WebviewWindow,
    review: u32,
    comment: u32,
) -> CmdResult<StateDto> {
    repo(&window)?.draft_from_note(review, comment).map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn edit_comment(
    window: WebviewWindow,
    review: u32,
    comment: u32,
    body: String,
) -> CmdResult<StateDto> {
    repo(&window)?
        .edit_comment(review, comment, &body)
        .map_err(err)
}

/// Starts a thread on `text`, quoted from comment `of` on the PR.
#[tauri::command]
#[specta::specta]
pub(crate) fn add_quote(
    window: WebviewWindow,
    review: u32,
    of: u32,
    text: String,
) -> CmdResult<StateDto> {
    repo(&window)?.add_quote(review, of, &text).map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn delete_comment(
    window: WebviewWindow,
    review: u32,
    comment: u32,
) -> CmdResult<StateDto> {
    repo(&window)?.delete_comment(review, comment).map_err(err)
}

/// Triages the new threads on the review's PR, reporting each tool call through `on_tool`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn triage_threads(
    window: WebviewWindow,
    runs: State<'_, Runs>,
    review: u32,
    on_tool: Channel<String>,
) -> CmdResult<StateDto> {
    repo(&window)?
        .triage(review, &runs, |t| {
            on_tool.send(t).ok();
        })
        .await
        .map_err(err)
}

/// Stops the review's triage run.
#[tauri::command]
#[specta::specta]
pub(crate) fn stop_thread_run(
    window: WebviewWindow,
    runs: State<Runs>,
    review: u32,
) -> CmdResult<()> {
    repo(&window)?.stop_thread_run(review, &runs);
    Ok(())
}
