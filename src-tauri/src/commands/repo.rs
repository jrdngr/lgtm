use std::path::Path;

use tauri::{AppHandle, Manager, WebviewWindow};

use super::{CmdResult, err, layout_path, repo_root};
use crate::jj::init;
use crate::menu::WordWrapItem;
use crate::paths::{data_dir, find_root};
use crate::repo::{self, RepoInfo};
use crate::{layout, recent};

#[tauri::command]
#[specta::specta]
pub(crate) fn current_repo(window: WebviewWindow) -> Option<RepoInfo> {
    Some(RepoInfo::new(&repo_root(&window).ok()?))
}

#[tauri::command]
#[specta::specta]
pub(crate) fn open_repo(app: AppHandle, path: String) -> CmdResult<()> {
    repo::open(&app, Path::new(&path))
}

/// Swaps this window for the start window.
#[tauri::command]
#[specta::specta]
pub(crate) fn switch_repo(app: AppHandle, window: WebviewWindow) -> CmdResult<()> {
    // Start first, so closing the last repo window doesn't quit the app.
    repo::open_start(&app).map_err(|e| e.to_string())?;
    window.destroy().map_err(|e| e.to_string())
}

/// Shows the window's saved Word Wrap setting as View > Word Wrap's check.
#[tauri::command]
#[specta::specta]
pub(crate) fn set_word_wrap(app: AppHandle, on: bool) -> CmdResult<()> {
    match app.try_state::<WordWrapItem<tauri::Wry>>() {
        Some(item) => item.0.set_checked(on).map_err(|e| e.to_string()),
        None => Ok(()),
    }
}

/// Closes this window without another close request, once the window has confirmed it.
/// While quitting, it stays open until every window has.
#[tauri::command]
#[specta::specta]
pub(crate) fn close_window(window: WebviewWindow) -> CmdResult<()> {
    #[cfg(target_os = "macos")]
    if crate::quit::confirmed(window.label()) {
        return Ok(());
    }
    window.destroy().map_err(|e| e.to_string())
}

/// The window cancelled its close, which stops a quit.
#[tauri::command]
#[specta::specta]
pub(crate) fn keep_window(window: WebviewWindow) {
    #[cfg(target_os = "macos")]
    crate::quit::cancelled(window.label());
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

/// The git repo containing `path` when it isn't a jj repo, so the start window can offer to init.
#[tauri::command]
#[specta::specta]
pub(crate) fn git_root(path: String) -> Option<String> {
    let path = Path::new(&path);
    if find_root(path).is_some() {
        return None;
    }
    Some(init::git_root(path)?.display().to_string())
}

/// Colocates jj with the git repo at `root`, then opens it.
#[tauri::command]
#[specta::specta]
pub(crate) async fn init_jj(app: AppHandle, root: String) -> CmdResult<()> {
    let path = std::path::PathBuf::from(&root);
    let dir = path.clone();
    tauri::async_runtime::spawn_blocking(move || init::init_colocated(&dir))
        .await
        .map_err(|e| e.to_string())?
        .map_err(err)?;
    repo::open(&app, &path)
}

/// Recently opened repos, most recent first.
#[tauri::command]
#[specta::specta]
pub(crate) fn recent_repos(app: AppHandle) -> CmdResult<Vec<RepoInfo>> {
    Ok(recent::load(&recent::path(&data_dir(&app)?))
        .iter()
        .map(|root| RepoInfo::new(root))
        .collect())
}

#[tauri::command]
#[specta::specta]
pub(crate) fn load_layout(
    window: WebviewWindow,
    workspace: Option<String>,
) -> CmdResult<Option<String>> {
    layout::load(&layout_path(&window, workspace.as_deref())?).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) fn save_layout(
    window: WebviewWindow,
    workspace: Option<String>,
    layout: String,
) -> CmdResult<()> {
    layout::save(&layout_path(&window, workspace.as_deref())?, &layout).map_err(|e| e.to_string())
}

/// The command-line tools that are missing or too old.
#[tauri::command]
#[specta::specta]
pub(crate) async fn check_tools() -> Vec<crate::tools::ToolProblem> {
    crate::tools::check().await
}

/// Whether agents in terminals can reach the app: `lgtm mcp` registered with Claude Code.
#[tauri::command]
#[specta::specta]
pub(crate) async fn mcp_status() -> CmdResult<crate::mcp::register::McpStatus> {
    // Resolving the Claude command can ask the login shell.
    tokio::task::spawn_blocking(crate::mcp::register::status)
        .await
        .map_err(|e| e.to_string())?
        .map_err(err)
}

/// Registers this app's `lgtm mcp` with Claude Code at user scope.
#[tauri::command]
#[specta::specta]
pub(crate) async fn register_mcp() -> CmdResult<()> {
    crate::mcp::register::register().await.map_err(err)
}
