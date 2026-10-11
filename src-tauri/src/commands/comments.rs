use tauri::ipc::Channel;
use tauri::{State, WebviewWindow};

use super::{CmdResult, err, repo};
use crate::agent::Runs;
use crate::review::{Ask, RowRef, StateDto};

/// Comments on the changed `rows` of a selection. `snippet` is the code as shown.
#[tauri::command]
#[specta::specta]
pub(crate) fn add_comment(
    window: WebviewWindow,
    review: u32,
    rows: Vec<RowRef>,
    body: String,
    snippet: String,
) -> CmdResult<StateDto> {
    repo(&window)?
        .add_comment(review, &rows, &body, &snippet)
        .map_err(err)
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

#[tauri::command]
#[specta::specta]
pub(crate) fn delete_comment(
    window: WebviewWindow,
    review: u32,
    comment: u32,
) -> CmdResult<StateDto> {
    repo(&window)?.delete_comment(review, comment).map_err(err)
}

/// Sends `ask` to the comment's agent, which edits the code when it asks for a change. Streams
/// the reply's text through `on_text` and returns once it's saved.
#[tauri::command]
#[specta::specta]
pub(crate) async fn workshop_comment(
    window: WebviewWindow,
    runs: State<'_, Runs>,
    review: u32,
    comment: u32,
    ask: Ask,
    on_text: Channel<String>,
) -> CmdResult<StateDto> {
    repo(&window)?
        .workshop(review, comment, &ask, &runs, |t| {
            on_text.send(t).ok();
        })
        .await
        .map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn stop_workshop(
    window: WebviewWindow,
    runs: State<Runs>,
    comment: u32,
) -> CmdResult<()> {
    repo(&window)?.stop_workshop(comment, &runs);
    Ok(())
}
