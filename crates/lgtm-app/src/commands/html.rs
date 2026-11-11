use tauri::Manager;

use super::CmdResult;
use crate::frame::Frame;
use crate::html::HtmlRoots;

/// A root id for a workspace's files on the `lgtmhtml:` scheme, for one HTML tab.
#[tauri::command]
#[specta::specta]
pub(crate) async fn html_root(window: Frame, workspace: Option<String>) -> CmdResult<String> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    Ok(window
        .state::<HtmlRoots>()
        .add(window.label(), ws.root.clone()))
}

/// Whether a file the HTML tab's root served changed since, so the tab reloads.
#[tauri::command]
#[specta::specta]
pub(crate) fn html_changed(window: Frame, root: String) -> bool {
    window.state::<HtmlRoots>().changed(window.label(), &root)
}
