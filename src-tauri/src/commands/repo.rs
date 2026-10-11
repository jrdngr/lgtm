use std::path::Path;

use tauri::{AppHandle, WebviewWindow};

use super::{CmdResult, err, layout_path, repo_root};
use crate::jj::init;
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

/// The git repo containing `path` when it isn't a jj repo, so the start window can offer to init.
#[tauri::command]
#[specta::specta]
pub(crate) fn git_root(path: String) -> Option<String> {
    let path = Path::new(&path);
    if repo::find_root(path).is_some() {
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
    Ok(recent::load(&recent::path(&repo::data_dir(&app)?))
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
pub(crate) fn mcp_status() -> CmdResult<crate::mcp::register::McpStatus> {
    crate::mcp::register::status().map_err(err)
}

/// Registers this app's `lgtm mcp` with Claude Code at user scope.
#[tauri::command]
#[specta::specta]
pub(crate) async fn register_mcp() -> CmdResult<()> {
    crate::mcp::register::register().await.map_err(err)
}
