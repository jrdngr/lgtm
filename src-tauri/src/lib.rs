mod layout;
mod pty;
mod repo;

use std::path::{Path, PathBuf};

use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State, WebviewWindow, WindowEvent};
use tauri_specta::collect_commands;

use pty::{PtyEvent, Ptys};
use repo::{RepoInfo, RepoWindows};

type CmdResult<T> = Result<T, String>;

fn repo_root(window: &WebviewWindow) -> CmdResult<PathBuf> {
    window
        .state::<RepoWindows>()
        .root(window.label())
        .ok_or_else(|| "window has no repo".into())
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

fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::new().commands(collect_commands![
        current_repo,
        open_repo,
        load_layout,
        save_layout,
        pty_spawn,
        pty_write,
        pty_resize,
        pty_kill,
    ])
}

const BINDINGS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/bindings.ts");

fn export_bindings(builder: &tauri_specta::Builder<tauri::Wry>, path: &str) {
    builder
        .export(specta_typescript::Typescript::default(), path)
        .expect("export TypeScript bindings");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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
