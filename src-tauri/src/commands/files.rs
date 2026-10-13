use std::path::{Path, PathBuf};

use tauri::WebviewWindow;

use super::{CmdResult, err};
use crate::bots::personas;
use crate::files::{self, FileEntry};
use crate::prompts;
use crate::syntax::{self, Segment};
use crate::workspace::Workspace;

/// A directory of the workspace's working copy, `""` for its root.
#[tauri::command]
#[specta::specta]
pub(crate) async fn list_files(
    window: WebviewWindow,
    workspace: Option<String>,
    dir: String,
) -> CmdResult<Vec<FileEntry>> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    files::list(&ws.root, &dir).map_err(err)
}

/// A workspace-relative path, or an absolute one only for persona and prompt files,
/// which can live outside the repo.
fn locate(ws: &Workspace, path: &str) -> CmdResult<PathBuf> {
    let abs = Path::new(path);
    if !abs.is_absolute() {
        return files::resolve(&ws.root, path).map_err(err);
    }
    if !personas::is_persona_file(&ws.root, abs) && !prompts::is_prompt_file(&ws.root, abs) {
        return Err(format!("{path} isn't in the workspace"));
    }
    Ok(abs.to_path_buf())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn read_file(
    window: WebviewWindow,
    workspace: Option<String>,
    path: String,
) -> CmdResult<String> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    files::read(&locate(&ws, &path)?, &path).map_err(err)
}

/// Saves a file, then snapshots the working copy so reviews of `@` pick it up.
#[tauri::command]
#[specta::specta]
pub(crate) async fn save_file(
    window: WebviewWindow,
    workspace: Option<String>,
    path: String,
    text: String,
) -> CmdResult<()> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    files::write(&locate(&ws, &path)?, &path, &text).map_err(err)?;
    ws.snapshot().await.map_err(err)
}

/// Highlight segments per line, empty when `path` has no grammar.
#[tauri::command]
#[specta::specta]
pub(crate) async fn highlight_text(path: String, text: String) -> Vec<Vec<Segment>> {
    tauri::async_runtime::spawn_blocking(move || syntax::highlight_file(&path, &text))
        .await
        .unwrap_or_default()
}
