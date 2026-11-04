use super::{CmdResult, err, repo};
use crate::frame::Frame;
use crate::store::ReportRun;

#[tauri::command]
#[specta::specta]
pub(crate) fn delete_report(window: Frame, run: u32) -> CmdResult<()> {
    repo(&window)?.delete_report(run).map_err(err)
}

/// The workspace's reports, newest first.
#[tauri::command]
#[specta::specta]
pub(crate) fn report_runs(window: Frame, workspace: Option<String>) -> CmdResult<Vec<ReportRun>> {
    repo(&window)?
        .report_runs(workspace.as_deref())
        .map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn report_run(window: Frame, run: u32) -> CmdResult<ReportRun> {
    repo(&window)?.report_run(run).map_err(err)
}

/// Renames a report, or with a blank `name`, gives it back its `R<n>` id.
#[tauri::command]
#[specta::specta]
pub(crate) fn rename_report(window: Frame, run: u32, name: String) -> CmdResult<ReportRun> {
    repo(&window)?.rename_report(run, &name).map_err(err)
}

/// Dismisses findings of a report, or with `dismissed` false, brings them back.
#[tauri::command]
#[specta::specta]
pub(crate) fn dismiss_findings(
    window: Frame,
    run: u32,
    findings: Vec<String>,
    dismissed: bool,
) -> CmdResult<ReportRun> {
    repo(&window)?
        .dismiss_findings(run, &findings, dismissed)
        .map_err(err)
}
