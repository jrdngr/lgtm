//! Tauri commands, grouped by what they act on.

pub mod bots;
pub mod comments;
pub mod hotspots;
pub mod pty;
pub mod repo;
pub mod review;
pub mod workspaces;

use std::path::PathBuf;
use std::sync::Arc;

use tauri::{Manager, WebviewWindow};

use crate::repo::RepoWindows;
use crate::review::RepoCtx;
use crate::workspace::Workspace;

type CmdResult<T> = Result<T, String>;

fn repo(window: &WebviewWindow) -> CmdResult<Arc<RepoCtx>> {
    window
        .state::<RepoWindows>()
        .get(window.label())
        .ok_or_else(|| "window has no repo".into())
}

async fn workspace(window: &WebviewWindow, name: Option<&str>) -> CmdResult<Arc<Workspace>> {
    repo(window)?.workspace(name).await.map_err(err)
}

fn repo_root(window: &WebviewWindow) -> CmdResult<PathBuf> {
    Ok(repo(window)?.root.clone())
}

fn err(e: anyhow::Error) -> String {
    format!("{e:#}")
}

fn layout_path(window: &WebviewWindow, workspace: Option<&str>) -> CmdResult<PathBuf> {
    let repo_dir = crate::repo::repo_dir(&crate::repo::data_dir(window)?, &repo_root(window)?);
    Ok(crate::layout::path(&repo_dir, workspace))
}
