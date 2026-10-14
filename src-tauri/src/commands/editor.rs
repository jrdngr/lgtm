use tauri::ipc::Channel;
use tauri::{State, WebviewWindow};

use super::{CmdResult, err, repo};
use crate::agent::Runs;
use crate::editor_agent::{self, SelectionAsk, SelectionReply};

/// Sends `ask` about lines selected in an editor tab to the agent under `key`, which edits the
/// working copy when it asks for a change. Streams the reply's text through `on_text`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn ask_selection(
    window: WebviewWindow,
    runs: State<'_, Runs>,
    workspace: Option<String>,
    key: String,
    ask: SelectionAsk,
    on_text: Channel<String>,
) -> CmdResult<SelectionReply> {
    let key = run_key(&window, &key)?;
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    editor_agent::ask(&ws, &runs, &key, &ask, |t| {
        on_text.send(t).ok();
    })
    .await
    .map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn stop_selection_ask(
    window: WebviewWindow,
    runs: State<Runs>,
    key: String,
) -> CmdResult<()> {
    runs.stop(&run_key(&window, &key)?);
    Ok(())
}

fn run_key(window: &WebviewWindow, key: &str) -> CmdResult<String> {
    Ok(repo(window)?.agent_key(&format!("editor-{key}")))
}
