use std::path::Path;
use std::sync::Arc;

use tauri::{Manager, State, WebviewWindow};
use tauri_specta::Event as _;

use super::{CmdResult, err, repo};
use crate::agent::Runs;
use crate::bots::personas::{self, DefaultPersona, Personas, Scope};
use crate::bots::{BotOptions, BotRun, Event, Notify};
use crate::events::{BotFixText, BotRunActivity, BotRunChanged};

/// Sends a run's events to `window`.
fn notify(window: &WebviewWindow) -> Notify {
    let (app, label) = (window.app_handle().clone(), window.label().to_string());
    Arc::new(move |e| {
        match e {
            Event::Activity {
                run,
                part,
                activity,
            } => BotRunActivity {
                run,
                part,
                activity,
            }
            .emit_to(&app, &label),
            Event::FixText { run, fix, text } => {
                BotFixText { run, fix, text }.emit_to(&app, &label)
            }
            Event::Changed { run } => BotRunChanged { run }.emit_to(&app, &label),
        }
        .ok();
    })
}

/// Starts the named personas reviewing `revset` in the background. Progress comes as events.
#[tauri::command]
#[specta::specta]
pub(crate) async fn start_bots(
    window: WebviewWindow,
    runs: State<'_, Runs>,
    workspace: Option<String>,
    revset: String,
    personas: Vec<String>,
    options: BotOptions,
) -> CmdResult<BotRun> {
    repo(&window)?
        .start_bots(
            workspace.as_deref(),
            &revset,
            &personas,
            options,
            runs.inner().clone(),
            notify(&window),
        )
        .await
        .map_err(err)
}

/// Adds `options` to a finished or halted run and runs what it's missing.
#[tauri::command]
#[specta::specta]
pub(crate) fn continue_bots(
    window: WebviewWindow,
    runs: State<Runs>,
    run: u32,
    options: BotOptions,
) -> CmdResult<()> {
    repo(&window)?
        .continue_bots(run, options, runs.inner().clone(), notify(&window))
        .map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn stop_bots(window: WebviewWindow, runs: State<Runs>, run: u32) -> CmdResult<()> {
    repo(&window)?.stop_bots(run, &runs);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub(crate) fn delete_bots(window: WebviewWindow, run: u32) -> CmdResult<()> {
    repo(&window)?.delete_bots(run).map_err(err)
}

/// Starts an agent fixing the picked findings in the run's workspace. `text` is them as
/// markdown. Its reply streams as events.
#[tauri::command]
#[specta::specta]
pub(crate) async fn fix_findings(
    window: WebviewWindow,
    runs: State<'_, Runs>,
    run: u32,
    findings: Vec<String>,
    text: String,
) -> CmdResult<()> {
    repo(&window)?
        .fix_findings(run, findings, text, runs.inner().clone(), notify(&window))
        .await
        .map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn stop_fix(
    window: WebviewWindow,
    runs: State<Runs>,
    run: u32,
    fix: u32,
) -> CmdResult<()> {
    repo(&window)?.stop_fix(run, fix, &runs);
    Ok(())
}

/// The workspace's persona reviews, newest first.
#[tauri::command]
#[specta::specta]
pub(crate) fn bot_runs(window: WebviewWindow, workspace: Option<String>) -> CmdResult<Vec<BotRun>> {
    repo(&window)?.bot_runs(workspace.as_deref()).map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn bot_run(window: WebviewWindow, run: u32) -> CmdResult<BotRun> {
    repo(&window)?.bot_run(run).map_err(err)
}

/// The personas a review in the workspace can use, and files that couldn't be read.
#[tauri::command]
#[specta::specta]
pub(crate) async fn list_personas(
    window: WebviewWindow,
    workspace: Option<String>,
) -> CmdResult<Personas> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    Ok(personas::load(&ws.root))
}

/// The default personas, and where each is installed.
#[tauri::command]
#[specta::specta]
pub(crate) async fn default_personas(
    window: WebviewWindow,
    workspace: Option<String>,
) -> CmdResult<Vec<DefaultPersona>> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    Ok(personas::defaults(&ws.root))
}

/// Writes the named default personas globally or into the workspace's repo.
#[tauri::command]
#[specta::specta]
pub(crate) async fn install_personas(
    window: WebviewWindow,
    workspace: Option<String>,
    names: Vec<String>,
    scope: Scope,
) -> CmdResult<()> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    personas::install(&ws.root, &names, scope).map_err(err)
}

/// Writes a starter file for a new persona and opens it in the default editor.
#[tauri::command]
#[specta::specta]
pub(crate) async fn new_persona(
    window: WebviewWindow,
    workspace: Option<String>,
    name: String,
    scope: Scope,
) -> CmdResult<()> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    let path = personas::create(&ws.root, &name, scope).map_err(err)?;
    open(&path)
}

/// Opens a persona file in the default editor.
#[tauri::command]
#[specta::specta]
pub(crate) async fn open_persona(
    window: WebviewWindow,
    workspace: Option<String>,
    path: String,
) -> CmdResult<()> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    let path = Path::new(&path);
    if !personas::is_persona_file(&ws.root, path) {
        return Err("that isn't a persona file".into());
    }
    open(path)
}

fn open(path: &Path) -> CmdResult<()> {
    open::that_detached(path).map_err(|e| format!("couldn't open {}: {e}", path.display()))
}
