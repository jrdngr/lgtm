use serde::Serialize;
use specta::Type;
use tauri::ipc::Channel;
use tauri::{State, WebviewWindow};

use super::{CmdResult, err, repo};
use crate::agent::Runs;
use crate::review::StateDto;

#[derive(Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WalkthroughDone {
    pub state: StateDto,
    /// Notes the agent put outside the review's files.
    pub skipped: u32,
}

/// Runs an agent over the review to write its walkthrough, replacing the last one. Sends each
/// tool call it makes through `activity`. `null` when it was stopped.
#[tauri::command]
#[specta::specta]
pub(crate) async fn run_walkthrough(
    window: WebviewWindow,
    runs: State<'_, Runs>,
    review: u32,
    activity: Channel<String>,
) -> CmdResult<Option<WalkthroughDone>> {
    let done = repo(&window)?
        .run_walkthrough(review, &runs, |t| {
            activity.send(t).ok();
        })
        .await
        .map_err(err)?;
    Ok(done.map(|(state, skipped)| WalkthroughDone {
        state,
        skipped: skipped as u32,
    }))
}

#[tauri::command]
#[specta::specta]
pub(crate) fn stop_walkthrough(
    window: WebviewWindow,
    runs: State<Runs>,
    review: u32,
) -> CmdResult<()> {
    repo(&window)?.stop_walkthrough(review, &runs);
    Ok(())
}
