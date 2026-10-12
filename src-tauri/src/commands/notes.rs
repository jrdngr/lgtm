use serde::Serialize;
use specta::Type;
use tauri::ipc::Channel;
use tauri::{State, WebviewWindow};

use super::{CmdResult, err, repo};
use crate::agent::Runs;
use crate::bots::personas::Scope;
use crate::review::StateDto;
use crate::review::walkthrough::prompt::{self, PromptFile};

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

/// The walkthrough prompt file the workspace's reviews use, `null` for the built-in prompt.
#[tauri::command]
#[specta::specta]
pub(crate) async fn walkthrough_prompt(
    window: WebviewWindow,
    workspace: Option<String>,
) -> CmdResult<Option<PromptFile>> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    Ok(prompt::in_use(&ws.root))
}

/// The path of `scope`'s walkthrough prompt, to open in the editor. Writes the built-in prompt
/// there first when there's no file yet.
#[tauri::command]
#[specta::specta]
pub(crate) async fn edit_walkthrough_prompt(
    window: WebviewWindow,
    workspace: Option<String>,
    scope: Scope,
) -> CmdResult<String> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    let path = prompt::edit(&ws.root, scope).map_err(err)?;
    Ok(path.to_string_lossy().into_owned())
}
