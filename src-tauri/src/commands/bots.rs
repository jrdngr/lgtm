use tauri::WebviewWindow;

use super::{CmdResult, err, repo};
use crate::store::BotRun;

#[tauri::command]
#[specta::specta]
pub(crate) fn delete_bots(window: WebviewWindow, run: u32) -> CmdResult<()> {
    repo(&window)?.delete_bots(run).map_err(err)
}

/// The workspace's reports, newest first.
#[tauri::command]
#[specta::specta]
pub(crate) fn bot_runs(window: WebviewWindow, workspace: Option<String>) -> CmdResult<Vec<BotRun>> {
    repo(&window)?.bot_runs(workspace.as_deref()).map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn bot_run(window: WebviewWindow, run: u32) -> CmdResult<BotRun> {
    repo(&window)?.bot_run(run).map_err(err)
}

/// Renames a report, or with a blank `name`, gives it back its `R<n>` id.
#[tauri::command]
#[specta::specta]
pub(crate) fn rename_report(window: WebviewWindow, run: u32, name: String) -> CmdResult<BotRun> {
    repo(&window)?.rename_report(run, &name).map_err(err)
}

/// Dismisses findings of a report, or with `dismissed` false, brings them back.
#[tauri::command]
#[specta::specta]
pub(crate) fn dismiss_findings(
    window: WebviewWindow,
    run: u32,
    findings: Vec<String>,
    dismissed: bool,
) -> CmdResult<BotRun> {
    repo(&window)?
        .dismiss_findings(run, &findings, dismissed)
        .map_err(err)
}
