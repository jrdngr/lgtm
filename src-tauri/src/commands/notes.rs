use tauri::WebviewWindow;

use super::{CmdResult, err, repo};

/// The prompt that has a terminal agent write the review's walkthrough.
#[tauri::command]
#[specta::specta]
pub(crate) fn walkthrough_prompt(window: WebviewWindow, review: u32) -> CmdResult<String> {
    repo(&window)?.walkthrough_prompt(review).map_err(err)
}
