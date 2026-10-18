use tauri::WebviewWindow;

use super::{CmdResult, err, repo};
use crate::review::arch::{ArchDto, ArchRange, HistoryDto, IntentKind, LensEdit};

/// The review's architecture diff, or the diff between two of its commits.
#[tauri::command]
#[specta::specta]
pub(crate) async fn review_arch(
    window: WebviewWindow,
    review: u32,
    transitive: bool,
    range: Option<ArchRange>,
) -> CmdResult<ArchDto> {
    repo(&window)?.arch(review, transitive, range).await.map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn arch_history(window: WebviewWindow, review: u32) -> CmdResult<Vec<HistoryDto>> {
    repo(&window)?.arch_history(review).await.map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn arch_lens(window: WebviewWindow, edit: LensEdit) -> CmdResult<()> {
    repo(&window)?.arch_lens(edit).map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn add_intent(window: WebviewWindow, kind: IntentKind) -> CmdResult<u32> {
    repo(&window)?.add_intent(kind).map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn remove_intent(window: WebviewWindow, id: u32) -> CmdResult<()> {
    repo(&window)?.remove_intent(id).map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn check_look(
    window: WebviewWindow,
    review: u32,
    key: String,
    checked: bool,
) -> CmdResult<()> {
    repo(&window)?.check_look(review, &key, checked).map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn intent_prompt(
    window: WebviewWindow,
    review: u32,
    ids: Vec<u32>,
) -> CmdResult<String> {
    repo(&window)?.intent_prompt(review, &ids).await.map_err(err)
}

/// The prompt asking a terminal agent to assess the review's architecture change.
#[tauri::command]
#[specta::specta]
pub(crate) async fn assess_prompt(window: WebviewWindow, review: u32) -> CmdResult<String> {
    repo(&window)?.assess_prompt(review).await.map_err(err)
}
