use serde::Serialize;
use specta::Type;
use tauri::ipc::Channel;
use tauri::{State, WebviewWindow};

use super::{CmdResult, err, repo};
use crate::agent::Runs;
use crate::review::StateDto;

#[derive(Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HotspotsFound {
    pub state: StateDto,
    /// Hotspots the agent put outside the review's files.
    pub skipped: u32,
}

/// Runs an agent over the review to find its hotspots, replacing the last ones. Sends each
/// tool call it makes through `activity`. `null` when it was stopped.
#[tauri::command]
#[specta::specta]
pub(crate) async fn find_hotspots(
    window: WebviewWindow,
    runs: State<'_, Runs>,
    review: u32,
    activity: Channel<String>,
) -> CmdResult<Option<HotspotsFound>> {
    let found = repo(&window)?
        .find_hotspots(review, &runs, |t| {
            activity.send(t).ok();
        })
        .await
        .map_err(err)?;
    Ok(found.map(|(state, skipped)| HotspotsFound {
        state,
        skipped: skipped as u32,
    }))
}

#[tauri::command]
#[specta::specta]
pub(crate) fn stop_hotspots(
    window: WebviewWindow,
    runs: State<Runs>,
    review: u32,
) -> CmdResult<()> {
    repo(&window)?.stop_hotspots(review, &runs);
    Ok(())
}
