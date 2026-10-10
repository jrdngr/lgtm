pub mod anchor;
pub mod diff;
mod jj;
mod layout;
mod pty;
mod repo;
mod review;
mod store;
mod syntax;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State, WebviewWindow, WindowEvent};
use tauri_specta::collect_commands;

use pty::{PtyEvent, Ptys};
use repo::{RepoInfo, RepoWindows};
use review::{MarkDto, RepoCtx, ReviewDto, RowRef};
use store::MarkState;

type CmdResult<T> = Result<T, String>;

fn repo(window: &WebviewWindow) -> CmdResult<Arc<RepoCtx>> {
    window
        .state::<RepoWindows>()
        .get(window.label())
        .ok_or_else(|| "window has no repo".into())
}

fn repo_root(window: &WebviewWindow) -> CmdResult<PathBuf> {
    Ok(repo(window)?.root.clone())
}

fn err(e: anyhow::Error) -> String {
    format!("{e:#}")
}

#[tauri::command]
#[specta::specta]
fn current_repo(window: WebviewWindow) -> Option<RepoInfo> {
    let root = repo_root(&window).ok()?;
    let name = root.file_name()?.to_string_lossy().into_owned();
    Some(RepoInfo {
        root: root.display().to_string(),
        name,
    })
}

#[tauri::command]
#[specta::specta]
fn open_repo(app: AppHandle, path: String) -> CmdResult<()> {
    repo::open(&app, Path::new(&path))
}

fn layout_path(window: &WebviewWindow) -> CmdResult<PathBuf> {
    let data_dir = window.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(layout::path(&data_dir, &repo::key(&repo_root(window)?)))
}

#[tauri::command]
#[specta::specta]
fn load_layout(window: WebviewWindow) -> CmdResult<Option<String>> {
    layout::load(&layout_path(&window)?).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
fn save_layout(window: WebviewWindow, layout: String) -> CmdResult<()> {
    layout::save(&layout_path(&window)?, &layout).map_err(|e| e.to_string())
}

/// Starts a PTY in the window's repo root. `command` defaults to the login shell.
#[tauri::command]
#[specta::specta]
fn pty_spawn(
    window: WebviewWindow,
    ptys: State<Ptys>,
    cols: u16,
    rows: u16,
    command: Option<Vec<String>>,
    on_event: Channel<PtyEvent>,
) -> CmdResult<u32> {
    let cwd = repo_root(&window)?;
    ptys.spawn(window.label(), &cwd, (cols, rows), command, move |e| {
        on_event.send(e).ok();
    })
    .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
#[specta::specta]
fn pty_write(ptys: State<Ptys>, id: u32, data: String) -> CmdResult<()> {
    ptys.write(id, data.as_bytes())
        .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
#[specta::specta]
fn pty_resize(ptys: State<Ptys>, id: u32, cols: u16, rows: u16) -> CmdResult<()> {
    ptys.resize(id, cols, rows).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
#[specta::specta]
fn pty_kill(ptys: State<Ptys>, id: u32) {
    ptys.kill(id);
}

/// Loads the review for `revset` after snapshotting the working copy.
#[tauri::command]
#[specta::specta]
async fn open_review(window: WebviewWindow, revset: String) -> CmdResult<ReviewDto> {
    repo(&window)?.open(&revset).await.map_err(err)
}

#[tauri::command]
#[specta::specta]
fn close_review(window: WebviewWindow, review: u32) -> CmdResult<()> {
    repo(&window)?.close(review);
    Ok(())
}

/// Marks changed rows, or clears their marks when `state` is null. Returns every mark in the
/// review.
#[tauri::command]
#[specta::specta]
fn mark_rows(
    window: WebviewWindow,
    review: u32,
    rows: Vec<RowRef>,
    state: Option<MarkState>,
) -> CmdResult<Vec<MarkDto>> {
    repo(&window)?.mark(review, &rows, state).map_err(err)
}

#[tauri::command]
#[specta::specta]
fn undo_marks(window: WebviewWindow, review: u32) -> CmdResult<Vec<MarkDto>> {
    repo(&window)?.undo(review).map_err(err)
}

#[tauri::command]
#[specta::specta]
fn redo_marks(window: WebviewWindow, review: u32) -> CmdResult<Vec<MarkDto>> {
    repo(&window)?.redo(review).map_err(err)
}

/// Removes the marks this review's revset created.
#[tauri::command]
#[specta::specta]
fn reset_review_marks(window: WebviewWindow, review: u32) -> CmdResult<Vec<MarkDto>> {
    repo(&window)?.reset(review).map_err(err)
}

fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::new()
        .constant("HIGHLIGHT_NAMES", syntax::HIGHLIGHT_NAMES)
        .commands(collect_commands![
            current_repo,
            open_repo,
            load_layout,
            save_layout,
            pty_spawn,
            pty_write,
            pty_resize,
            pty_kill,
            open_review,
            close_review,
            mark_rows,
            undo_marks,
            redo_marks,
            reset_review_marks,
        ])
}

#[cfg(any(debug_assertions, test))]
const BINDINGS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/bindings.ts");

#[cfg(any(debug_assertions, test))]
fn export_bindings(builder: &tauri_specta::Builder<tauri::Wry>, path: &str) {
    builder
        .export(specta_typescript::Typescript::default(), path)
        .expect("export TypeScript bindings");
}

/// Apps opened from Finder get a bare PATH without Homebrew or ~/.cargo/bin, so jj and the
/// user's tools wouldn't resolve. Adopt the PATH their login shell sets up.
fn adopt_login_shell_path() {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    let Ok(out) = std::process::Command::new(shell)
        .args(["-l", "-i", "-c", "printf '\\n__LGTM_PATH__%s' \"$PATH\""])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
    else {
        return;
    };
    let out = String::from_utf8_lossy(&out.stdout);
    if let Some(path) = out.rsplit_once("__LGTM_PATH__").map(|(_, p)| p.trim())
        && !path.is_empty()
    {
        // SAFETY: runs during startup before any other thread reads the environment.
        unsafe { std::env::set_var("PATH", path) };
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    adopt_login_shell_path();
    let builder = specta_builder();
    #[cfg(debug_assertions)]
    export_bindings(&builder, BINDINGS_PATH);

    tauri::Builder::default()
        // Must be registered first. Later launches hand their arguments to this process.
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            repo::open_from_args(app, argv.get(1..).unwrap_or_default(), Path::new(&cwd));
        }))
        .plugin(tauri_plugin_dialog::init())
        .manage(Ptys::default())
        .manage(RepoWindows::default())
        .invoke_handler(builder.invoke_handler())
        .setup(|app| {
            let args: Vec<String> = std::env::args().skip(1).collect();
            let cwd = std::env::current_dir()?;
            repo::open_from_args(app.handle(), &args, &cwd);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::Destroyed = event {
                window.state::<Ptys>().kill_window(window.label());
                window.state::<RepoWindows>().remove(window.label());
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    #[test]
    fn bindings_match_commands() {
        let dir = tempfile::tempdir().unwrap();
        let fresh = dir.path().join("bindings.ts");
        super::export_bindings(&super::specta_builder(), fresh.to_str().unwrap());
        let fresh = std::fs::read_to_string(fresh).unwrap();
        if std::env::var_os("UPDATE_BINDINGS").is_some() {
            std::fs::write(super::BINDINGS_PATH, &fresh).unwrap();
        }
        let committed = std::fs::read_to_string(super::BINDINGS_PATH).unwrap_or_default();
        assert!(
            committed == fresh,
            "src/bindings.ts is stale. Rerun with UPDATE_BINDINGS=1."
        );
    }
}
