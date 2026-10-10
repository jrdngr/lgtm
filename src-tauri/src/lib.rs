pub mod anchor;
mod config;
pub mod diff;
mod events;
mod gitmeta;
mod jj;
mod layout;
mod pty;
mod recent;
mod repo;
mod review;
mod store;
mod syntax;
mod watch;
mod workspace;
mod workspaces;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State, WebviewWindow, WindowEvent};
use tauri_specta::{collect_commands, collect_events};

use pty::{Owner, PtyEvent, Ptys};
use repo::{RepoInfo, RepoWindows};
use review::{ProgressDto, RefreshDto, RepoCtx, ReviewDto, RowRef, StateDto, View};
use store::MarkState;
use workspaces::WorkspaceDto;

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
    Some(RepoInfo::new(&repo_root(&window).ok()?))
}

#[tauri::command]
#[specta::specta]
fn open_repo(app: AppHandle, path: String) -> CmdResult<()> {
    repo::open(&app, Path::new(&path))
}

/// Recently opened repos, most recent first.
#[tauri::command]
#[specta::specta]
fn recent_repos(app: AppHandle) -> CmdResult<Vec<RepoInfo>> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(recent::load(&recent::path(&data_dir))
        .iter()
        .map(|root| RepoInfo::new(root))
        .collect())
}

fn layout_path(window: &WebviewWindow, workspace: Option<&str>) -> CmdResult<PathBuf> {
    let data_dir = window.path().app_data_dir().map_err(|e| e.to_string())?;
    let key = repo::key(&repo_root(window)?);
    Ok(layout::path(&data_dir, &key, workspace))
}

#[tauri::command]
#[specta::specta]
fn load_layout(window: WebviewWindow, workspace: Option<String>) -> CmdResult<Option<String>> {
    layout::load(&layout_path(&window, workspace.as_deref())?).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
fn save_layout(window: WebviewWindow, workspace: Option<String>, layout: String) -> CmdResult<()> {
    layout::save(&layout_path(&window, workspace.as_deref())?, &layout).map_err(|e| e.to_string())
}

/// Starts a login shell in the workspace. With `setup`, the shell first runs its
/// `.lgtm/setup`.
#[tauri::command]
#[specta::specta]
async fn pty_spawn(
    window: WebviewWindow,
    ptys: State<'_, Ptys>,
    workspace: Option<String>,
    cols: u16,
    rows: u16,
    setup: bool,
    on_event: Channel<PtyEvent>,
) -> CmdResult<u32> {
    let ctx = repo(&window)?;
    let ws = ctx.workspace(workspace.as_deref()).await.map_err(err)?;
    let command = setup.then(|| {
        let shell = pty::default_shell();
        let quoted = format!("'{}'", shell.replace('\'', r"'\''"));
        let script = format!(
            "if [ -x {s} ]; then ./{s}; else sh {s}; fi || echo \"lgtm: {s} exited with $?\"; exec {quoted} -l",
            s = workspaces::SETUP
        );
        vec![shell, "-l".into(), "-c".into(), script]
    });
    let root = ctx.root.to_string_lossy();
    let env = [
        ("LGTM_ROOT", root.as_ref()),
        ("LGTM_WORKSPACE", ws.name.as_deref().unwrap_or("")),
    ];
    let owner = Owner {
        window: window.label().to_string(),
        workspace,
    };
    ptys.spawn(owner, &ws.root, (cols, rows), command, &env, move |e| {
        on_event.send(e).ok();
    })
    .map_err(|e| format!("{e:#}"))
}

/// Every workspace, the repo's main one first.
#[tauri::command]
#[specta::specta]
async fn list_workspaces(window: WebviewWindow) -> CmdResult<Vec<WorkspaceDto>> {
    repo(&window)?.list_workspaces().await.map_err(err)
}

/// Adds a workspace on `trunk()`. Returns whether it has a `.lgtm/setup` to run.
#[tauri::command]
#[specta::specta]
async fn create_workspace(window: WebviewWindow, name: String) -> CmdResult<bool> {
    repo(&window)?.create_workspace(&name).await.map_err(err)
}

/// Ends the workspace's terminals, then forgets it and deletes its directory.
#[tauri::command]
#[specta::specta]
async fn delete_workspace(
    window: WebviewWindow,
    ptys: State<'_, Ptys>,
    name: String,
) -> CmdResult<()> {
    ptys.kill_owner(&Owner {
        window: window.label().to_string(),
        workspace: Some(name.clone()),
    });
    repo(&window)?.delete_workspace(&name).await.map_err(err)?;
    layout::remove(&layout_path(&window, Some(&name))?).map_err(|e| e.to_string())
}

/// Review progress of the workspace's latest review, or of its `@`.
#[tauri::command]
#[specta::specta]
async fn workspace_progress(
    window: WebviewWindow,
    workspace: Option<String>,
) -> CmdResult<ProgressDto> {
    repo(&window)?
        .progress(workspace.as_deref())
        .await
        .map_err(err)
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

/// Loads the review for `revset` after snapshotting the working copy. Starts a review
/// session, or with `resume` continues the revset's latest one.
#[tauri::command]
#[specta::specta]
async fn open_review(
    window: WebviewWindow,
    workspace: Option<String>,
    revset: String,
    resume: bool,
) -> CmdResult<ReviewDto> {
    repo(&window)?
        .open(workspace.as_deref(), &revset, resume)
        .await
        .map_err(err)
}

#[tauri::command]
#[specta::specta]
fn close_review(window: WebviewWindow, review: u32) -> CmdResult<()> {
    repo(&window)?.close(review);
    Ok(())
}

/// `jj log` with the user's default revset and template, for picking commits.
#[tauri::command]
#[specta::specta]
async fn log_graph(
    window: WebviewWindow,
    workspace: Option<String>,
) -> CmdResult<Vec<jj::LogLine>> {
    let ws = repo(&window)?
        .workspace(workspace.as_deref())
        .await
        .map_err(err)?;
    ws.jj.log_graph().await.map_err(err)
}

/// Reloads a review after the repo changed, or switches it to `view`.
#[tauri::command]
#[specta::specta]
async fn refresh_review(
    window: WebviewWindow,
    review: u32,
    view: Option<View>,
) -> CmdResult<RefreshDto> {
    repo(&window)?.refresh(review, view).await.map_err(err)
}

/// Marks changed rows, or clears their marks when `state` is null. Files in `check` are
/// checked in the same undo step.
#[tauri::command]
#[specta::specta]
fn mark_rows(
    window: WebviewWindow,
    review: u32,
    rows: Vec<RowRef>,
    state: Option<MarkState>,
    check: Vec<u32>,
) -> CmdResult<StateDto> {
    repo(&window)?
        .mark(review, &rows, state, &check)
        .map_err(err)
}

#[tauri::command]
#[specta::specta]
fn set_files_checked(
    window: WebviewWindow,
    review: u32,
    files: Vec<u32>,
    checked: bool,
) -> CmdResult<StateDto> {
    repo(&window)?
        .set_checked(review, &files, checked)
        .map_err(err)
}

#[tauri::command]
#[specta::specta]
fn undo_marks(window: WebviewWindow, review: u32) -> CmdResult<StateDto> {
    repo(&window)?.undo(review).map_err(err)
}

#[tauri::command]
#[specta::specta]
fn redo_marks(window: WebviewWindow, review: u32) -> CmdResult<StateDto> {
    repo(&window)?.redo(review).map_err(err)
}

/// Removes the marks and file checks this review's revset created.
#[tauri::command]
#[specta::specta]
fn reset_review_marks(window: WebviewWindow, review: u32) -> CmdResult<StateDto> {
    repo(&window)?.reset(review).map_err(err)
}

fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::new()
        .constant("HIGHLIGHT_NAMES", syntax::HIGHLIGHT_NAMES)
        .commands(collect_commands![
            current_repo,
            open_repo,
            recent_repos,
            load_layout,
            save_layout,
            pty_spawn,
            pty_write,
            pty_resize,
            pty_kill,
            list_workspaces,
            create_workspace,
            delete_workspace,
            workspace_progress,
            open_review,
            close_review,
            refresh_review,
            log_graph,
            mark_rows,
            set_files_checked,
            undo_marks,
            redo_marks,
            reset_review_marks,
        ])
        .events(collect_events![
            events::RepoChanged,
            events::WorkingCopyStale
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
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "gc") {
        std::process::exit(workspaces::gc::main(&args[1..]));
    }
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
        .setup(move |app| {
            builder.mount_events(app);
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
