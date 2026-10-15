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
/// which live outside the repo.
fn locate(ws: &Workspace, path: &str) -> CmdResult<PathBuf> {
    let abs = Path::new(path);
    if !abs.is_absolute() {
        return files::resolve(&ws.root, path).map_err(err);
    }
    if !personas::is_persona_file(abs) && !prompts::is_prompt_file(abs) {
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
    files::read(&contained(&ws, &path)?, &path).map_err(err)
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
    files::write(&contained(&ws, &path)?, &path, &text).map_err(err)?;
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

/// A workspace path that a symlinked folder can't lead out of, or an absolute persona or
/// prompt file as `locate` allows.
fn confined(ws: &Workspace, path: &str) -> CmdResult<PathBuf> {
    if Path::new(path).is_absolute() {
        return locate(ws, path);
    }
    let abs = files::resolve(&ws.root, path).map_err(err)?;
    files::confine(&ws.root, &abs).map_err(err)?;
    Ok(abs)
}

/// Like `confined`, but a symlinked file can't lead out of the workspace either.
fn contained(ws: &Workspace, path: &str) -> CmdResult<PathBuf> {
    if Path::new(path).is_absolute() {
        return locate(ws, path);
    }
    let abs = files::resolve(&ws.root, path).map_err(err)?;
    files::contain(&ws.root, &abs).map_err(err)?;
    Ok(abs)
}

/// Makes an empty file, or a folder when `dir`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn create_path(
    window: WebviewWindow,
    workspace: Option<String>,
    path: String,
    dir: bool,
) -> CmdResult<()> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    files::create(&confined(&ws, &path)?, &path, dir).map_err(err)
}

/// A file dropped into the workspace: where it goes and its contents as base64.
#[derive(serde::Deserialize, specta::Type)]
pub(crate) struct DroppedFile {
    path: String,
    data: String,
}

/// Writes dropped files, over files already there when `replace`, then snapshots. Returns the
/// paths that were taken and left alone.
#[tauri::command]
#[specta::specta]
pub(crate) async fn add_files(
    window: WebviewWindow,
    workspace: Option<String>,
    files: Vec<DroppedFile>,
    replace: bool,
) -> CmdResult<Vec<String>> {
    use base64::Engine as _;
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    let mut taken = vec![];
    for f in files {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&f.data)
            .map_err(|e| format!("can't read {}: {e}", f.path))?;
        if !files::put(&confined(&ws, &f.path)?, &f.path, &bytes, replace).map_err(err)? {
            taken.push(f.path);
        }
    }
    ws.snapshot().await.map_err(err)?;
    Ok(taken)
}

/// Drags workspace paths out of the window, as files for Finder and other apps. Returns once
/// the drag ends.
#[tauri::command]
#[specta::specta]
pub(crate) async fn drag_out(
    window: WebviewWindow,
    workspace: Option<String>,
    paths: Vec<String>,
) -> CmdResult<()> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    let files = paths
        .iter()
        .map(|p| confined(&ws, p))
        .collect::<CmdResult<Vec<_>>>()?;
    let (done, ended) = tokio::sync::oneshot::channel();
    start_drag(window, files, done)?;
    ended.await.ok();
    Ok(())
}

#[cfg(target_os = "macos")]
fn start_drag(
    window: WebviewWindow,
    files: Vec<PathBuf>,
    done: tokio::sync::oneshot::Sender<()>,
) -> CmdResult<()> {
    let icon = include_bytes!("../../icons/32x32.png").to_vec();
    let done = std::sync::Mutex::new(Some(done));
    let w = window.clone();
    window
        .run_on_main_thread(move || {
            let item = drag::DragItem::Files(files);
            let image = drag::Image::Raw(icon);
            let ended = move |_, _| {
                if let Some(done) = done.lock().unwrap().take() {
                    done.send(()).ok();
                }
            };
            if let Err(e) = drag::start_drag(&w, item, image, ended, Default::default()) {
                eprintln!("lgtm: drag out: {e}");
            }
        })
        .map_err(err)
}

#[cfg(not(target_os = "macos"))]
fn start_drag(
    _: WebviewWindow,
    _: Vec<PathBuf>,
    _: tokio::sync::oneshot::Sender<()>,
) -> CmdResult<()> {
    Err("dragging files out needs macOS".into())
}

/// Renames or moves a file or folder, then snapshots so reviews of `@` pick it up.
#[tauri::command]
#[specta::specta]
pub(crate) async fn rename_path(
    window: WebviewWindow,
    workspace: Option<String>,
    from: String,
    to: String,
) -> CmdResult<()> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    files::rename(&confined(&ws, &from)?, &confined(&ws, &to)?, &to).map_err(err)?;
    ws.snapshot().await.map_err(err)
}

/// Moves a file or folder to the Trash, then snapshots.
#[tauri::command]
#[specta::specta]
pub(crate) async fn trash_path(
    window: WebviewWindow,
    workspace: Option<String>,
    path: String,
) -> CmdResult<()> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    let abs = confined(&ws, &path)?;
    if abs == ws.root {
        return Err("can't trash the workspace".into());
    }
    trash::delete(&abs).map_err(|e| format!("can't move {path} to the Trash: {e}"))?;
    ws.snapshot().await.map_err(err)
}

/// Shows a path in Finder. `""` is the workspace's root.
#[tauri::command]
#[specta::specta]
pub(crate) async fn reveal_path(
    window: WebviewWindow,
    workspace: Option<String>,
    path: String,
) -> CmdResult<()> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    let abs = locate(&ws, &path)?;
    let status = tokio::process::Command::new("open")
        .arg("-R")
        .arg(&abs)
        .status()
        .await
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("can't open {}", abs.display()));
    }
    Ok(())
}
