use super::{CmdResult, err, repo};
use crate::frame::Frame;
use crate::turns::TurnDto;

/// The agent turns and checkpoints on the workspace's working-copy change, newest first.
#[tauri::command]
#[specta::specta]
pub(crate) async fn agent_turns(
    window: Frame,
    workspace: Option<String>,
) -> CmdResult<Vec<TurnDto>> {
    repo(&window)?
        .turns(workspace.as_deref())
        .await
        .map_err(err)
}

/// Takes a checkpoint of the workspace's working copy.
#[tauri::command]
#[specta::specta]
pub(crate) async fn set_checkpoint(window: Frame, workspace: Option<String>) -> CmdResult<()> {
    repo(&window)?
        .checkpoint(workspace.as_deref())
        .await
        .map_err(err)
}

/// Puts the files an agent turn changed back as they were before it.
#[tauri::command]
#[specta::specta]
pub(crate) async fn undo_turn(
    window: Frame,
    workspace: Option<String>,
    turn: u32,
) -> CmdResult<()> {
    repo(&window)?
        .undo_turn(workspace.as_deref(), turn)
        .await
        .map_err(err)
}

/// Splits the working-copy change by time around agent turns, keeping their order.
#[tauri::command]
#[specta::specta]
pub(crate) async fn split_at_turns(
    window: Frame,
    workspace: Option<String>,
    turns: Vec<u32>,
) -> CmdResult<()> {
    repo(&window)?
        .split_at_turns(workspace.as_deref(), &turns)
        .await
        .map_err(err)
}

/// Forgets an agent turn or checkpoint.
#[tauri::command]
#[specta::specta]
pub(crate) fn remove_turn(window: Frame, turn: u32) -> CmdResult<()> {
    repo(&window)?.remove_turn(turn).map_err(err)
}
