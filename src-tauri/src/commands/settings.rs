use tauri::{AppHandle, WebviewWindow};
use tauri_specta::Event as _;

use super::{CmdResult, err};
use crate::bots::personas::Scope;
use crate::events::SettingsChanged;
use crate::prompts::{self, Prompt, PromptInfo};
use crate::settings::claude::{self, ClaudeCommand};
use crate::settings::{self, Settings};
use crate::tools::{self, ToolStatus};

#[tauri::command]
#[specta::specta]
pub(crate) fn get_settings() -> Settings {
    settings::get()
}

/// Why `settings.toml` couldn't be read, while the defaults stand in for it.
#[tauri::command]
#[specta::specta]
pub(crate) fn settings_load_error() -> Option<String> {
    settings::load_error()
}

/// Saves the settings and tells every window.
#[tauri::command]
#[specta::specta]
pub(crate) async fn save_settings(app: AppHandle, settings: Settings) -> CmdResult<()> {
    let claude_changed = settings.claude != settings::get().claude;
    settings::save(settings.clone()).map_err(err)?;
    if claude_changed {
        tokio::task::spawn_blocking(claude::refresh)
            .await
            .map_err(|e| e.to_string())?;
    }
    SettingsChanged { settings }
        .emit(&app)
        .map_err(|e| e.to_string())
}

/// How the Claude command resolves. `refresh` asks the login shell again.
#[tauri::command]
#[specta::specta]
pub(crate) async fn claude_command(refresh: bool) -> CmdResult<ClaudeCommand> {
    tokio::task::spawn_blocking(move || {
        if refresh {
            claude::refresh();
        }
        claude::status()
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn tool_status() -> Vec<ToolStatus> {
    tools::status().await
}

/// Each prompt and which of its files the window's repo uses.
#[tauri::command]
#[specta::specta]
pub(crate) async fn prompt_files(
    window: WebviewWindow,
    workspace: Option<String>,
) -> CmdResult<Vec<PromptInfo>> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    Ok(prompts::infos(&ws.root))
}

/// The path of `scope`'s prompt file, to open in the editor. Writes the built-in prompt there
/// first when there's no file yet.
#[tauri::command]
#[specta::specta]
pub(crate) async fn edit_prompt(
    window: WebviewWindow,
    workspace: Option<String>,
    prompt: Prompt,
    scope: Scope,
) -> CmdResult<String> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    let path = prompts::edit(prompt, &ws.root, scope).map_err(err)?;
    Ok(path.to_string_lossy().into_owned())
}

/// Deletes `scope`'s prompt file.
#[tauri::command]
#[specta::specta]
pub(crate) async fn remove_prompt(
    window: WebviewWindow,
    workspace: Option<String>,
    prompt: Prompt,
    scope: Scope,
) -> CmdResult<()> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    prompts::remove(prompt, &ws.root, scope).map_err(err)
}
