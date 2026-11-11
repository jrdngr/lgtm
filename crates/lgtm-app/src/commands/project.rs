use std::path::Path;

use tauri::{AppHandle, WebviewWindow};

use super::CmdResult;
use crate::project::{self, ProjectDto, ProjectRepo, ProjectSummary};

/// The project this window shows, `null` in the start window.
#[tauri::command]
#[specta::specta]
pub(crate) fn current_project(app: AppHandle, window: WebviewWindow) -> Option<ProjectDto> {
    project::current(&app, window.label())
}

/// Adds the repo holding `path` to this window's project. `null` when another window has it,
/// which then shows it.
#[tauri::command]
#[specta::specta]
pub(crate) fn add_repo(
    app: AppHandle,
    window: WebviewWindow,
    path: String,
) -> CmdResult<Option<ProjectRepo>> {
    project::add(&app, window.label(), Path::new(&path))
}

/// Ends everything of repo `label` and takes it out of the project.
#[tauri::command]
#[specta::specta]
pub(crate) fn remove_repo(app: AppHandle, window: WebviewWindow, label: String) -> CmdResult<()> {
    project::remove(&app, window.label(), &label)
}

/// Ends everything of repo `label`, which stays in the project asleep.
#[tauri::command]
#[specta::specta]
pub(crate) fn sleep_repo(app: AppHandle, window: WebviewWindow, label: String) -> CmdResult<()> {
    project::sleep(&app, window.label(), &label)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn wake_repo(
    app: AppHandle,
    window: WebviewWindow,
    label: String,
) -> CmdResult<ProjectRepo> {
    project::wake(&app, window.label(), &label)
}

/// The window shows repo `label`, which the menu's Settings and Keyboard Shortcuts open in.
#[tauri::command]
#[specta::specta]
pub(crate) fn set_active_repo(app: AppHandle, window: WebviewWindow, label: String) {
    use tauri::Manager;
    app.state::<project::Projects>()
        .set_active(window.label(), Some(label));
}

#[tauri::command]
#[specta::specta]
pub(crate) fn rename_project(app: AppHandle, window: WebviewWindow, name: String) -> CmdResult<()> {
    project::rename(&app, window.label(), &name)
}

/// Saves the window's own state for its repo list, which `current_project` hands back.
#[tauri::command]
#[specta::specta]
pub(crate) fn save_project_ui(app: AppHandle, window: WebviewWindow, ui: String) -> CmdResult<()> {
    project::save_ui(&app, window.label(), ui)
}

/// Saved projects no window has open, most recently used first.
#[tauri::command]
#[specta::specta]
pub(crate) fn closed_projects(app: AppHandle) -> CmdResult<Vec<ProjectSummary>> {
    project::closed_projects(&app)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn open_project(app: AppHandle, id: String) -> CmdResult<()> {
    project::open_project(&app, &id, None)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn delete_project(app: AppHandle, id: String) -> CmdResult<()> {
    project::delete(&app, &id)
}
