use tauri::Manager;
use tauri_specta::Event;

use super::{CmdResult, err, repo};
use crate::events::ReviewStateChanged;
use crate::frame::Frame;
use crate::review::{RowRef, StateDto};
use crate::store::{Author, Stage};

/// Starts a draft for the workspace's PR on GitHub on the changed `rows` of a selection.
/// `snippet` is the code as shown.
#[tauri::command]
#[specta::specta]
pub(crate) fn add_comment(
    window: Frame,
    review: u32,
    rows: Vec<RowRef>,
    body: String,
    snippet: String,
) -> CmdResult<StateDto> {
    repo(&window)?
        .add_comment(review, &rows, &body, &snippet)
        .map_err(err)
}

/// Moves a thread to `stage`.
#[tauri::command]
#[specta::specta]
pub(crate) fn set_thread_stage(
    window: Frame,
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
pub(crate) fn reset_thread(window: Frame, review: u32, comment: u32) -> CmdResult<StateDto> {
    repo(&window)?.reset_thread(review, comment).map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn edit_comment(
    window: Frame,
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
pub(crate) fn add_quote(window: Frame, review: u32, of: u32, text: String) -> CmdResult<StateDto> {
    repo(&window)?.add_quote(review, of, &text).map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn delete_comment(window: Frame, review: u32, comment: u32) -> CmdResult<StateDto> {
    repo(&window)?.delete_comment(review, comment).map_err(err)
}

/// Adds a message from a synced terminal to a comment's chat. The review shows it as it
/// would an agent's change, wherever the review is open.
#[tauri::command]
#[specta::specta]
pub(crate) fn add_chat_message(
    window: Frame,
    review: u32,
    comment: u32,
    author: Author,
    text: String,
) -> CmdResult<()> {
    let state = repo(&window)?
        .add_chat_message(review, comment, author, &text)
        .map_err(err)?;
    ReviewStateChanged { review, state }
        .emit_to(window.app_handle(), window.label())
        .map_err(|e| e.to_string())
}

/// Drops a thread's suggested reply.
#[tauri::command]
#[specta::specta]
pub(crate) fn dismiss_suggestion(window: Frame, review: u32, comment: u32) -> CmdResult<StateDto> {
    repo(&window)?
        .dismiss_suggestion(review, comment)
        .map_err(err)
}

/// Links terminal `terminal`'s agent to the review, with `brief` telling it how to work on the
/// PR's threads and `thread` as the thread the user is on, or with `linked` false, unlinks it.
#[tauri::command]
#[specta::specta]
pub(crate) fn set_agent_link(
    window: Frame,
    terminal: String,
    linked: bool,
    brief: Option<String>,
    thread: Option<crate::review::LinkThread>,
) -> CmdResult<()> {
    repo(&window)?.set_agent_link(&terminal, linked, brief, thread);
    Ok(())
}

/// Makes a thread's suggested reply its reply.
#[tauri::command]
#[specta::specta]
pub(crate) fn take_suggestion(window: Frame, review: u32, comment: u32) -> CmdResult<StateDto> {
    repo(&window)?.take_suggestion(review, comment).map_err(err)
}
