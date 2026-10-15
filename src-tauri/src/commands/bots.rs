use std::sync::Arc;

use tauri::{Manager, State, WebviewWindow};
use tauri_specta::Event as _;

use super::{CmdResult, err, repo};
use crate::agent::Runs;
use crate::bots::install::{self, DefaultPersona};
use crate::bots::personas::{self, Personas};
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
            Event::Changed { run } => BotRunChanged {
                run,
                show: false,
                workspace: None,
            }
            .emit_to(&app, &label),
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
    context: String,
) -> CmdResult<BotRun> {
    repo(&window)?
        .start_bots(
            workspace.as_deref(),
            &revset,
            &personas,
            options,
            context,
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

/// Renames a report, or with a blank `name`, gives it back its `R<n>` id.
#[tauri::command]
#[specta::specta]
pub(crate) fn rename_report(window: WebviewWindow, run: u32, name: String) -> CmdResult<BotRun> {
    repo(&window)?.rename_report(run, &name).map_err(err)
}

/// The personas a review can use, and files that couldn't be read.
#[tauri::command]
#[specta::specta]
pub(crate) fn list_personas() -> Personas {
    personas::load()
}

/// The default personas, and whether each is installed.
#[tauri::command]
#[specta::specta]
pub(crate) fn default_personas() -> Vec<DefaultPersona> {
    install::defaults()
}

/// Writes the named default personas.
#[tauri::command]
#[specta::specta]
pub(crate) fn install_personas(names: Vec<String>) -> CmdResult<()> {
    install::install(&names).map_err(err)
}

/// Writes a starter file for a new persona. Returns its path, to open in the editor.
#[tauri::command]
#[specta::specta]
pub(crate) fn new_persona(name: String) -> CmdResult<String> {
    let path = personas::create(&name).map_err(err)?;
    Ok(path.to_string_lossy().into_owned())
}

/// Copies a persona file beside itself as `name`. Returns the copy's path.
#[tauri::command]
#[specta::specta]
pub(crate) fn duplicate_persona(path: String, name: String) -> CmdResult<String> {
    let to = personas::duplicate(std::path::Path::new(&path), &name).map_err(err)?;
    Ok(to.to_string_lossy().into_owned())
}

/// Dismisses findings of a run, or with `dismissed` false, brings them back.
#[tauri::command]
#[specta::specta]
pub(crate) fn dismiss_findings(
    window: WebviewWindow,
    run: u32,
    findings: Vec<String>,
    dismissed: bool,
) -> CmdResult<BotRun> {
    repo(&window)?
        .dismiss_findings(run, &findings, dismissed)
        .map_err(err)
}
