use tauri::AppHandle;
use tauri_specta::Event as _;

use super::{CmdResult, err};
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
    let claude_changed = settings.claude_terminal != settings::get().claude_terminal;
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

/// Each prompt and whether its file is in use.
#[tauri::command]
#[specta::specta]
pub(crate) fn prompt_files() -> Vec<PromptInfo> {
    prompts::infos()
}

/// The path of the prompt's file, to open in the editor. Writes the built-in prompt there first
/// when there's no file yet.
#[tauri::command]
#[specta::specta]
pub(crate) fn edit_prompt(prompt: Prompt) -> CmdResult<String> {
    let path = prompts::edit(prompt).map_err(err)?;
    Ok(path.to_string_lossy().into_owned())
}

/// The prompt's text: its file, else the built-in prompt.
#[tauri::command]
#[specta::specta]
pub(crate) fn prompt_text(prompt: Prompt) -> CmdResult<String> {
    prompts::load(prompt).map_err(err)
}

/// Deletes the prompt's file.
#[tauri::command]
#[specta::specta]
pub(crate) fn remove_prompt(prompt: Prompt) -> CmdResult<()> {
    prompts::remove(prompt).map_err(err)
}
