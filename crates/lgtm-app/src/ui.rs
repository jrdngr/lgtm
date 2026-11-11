//! How features tell the UI their state changed: to the repo's window, or to every window
//! for state they all show. Features notify after they change something, so the commands and
//! MCP tools that call them don't.

use serde::Serialize;
use tauri::AppHandle;
use tauri_specta::Event;

use crate::ctx::RepoCtx;

/// The repo's window, once it exists.
pub struct Ui {
    pub app: AppHandle,
    /// The repo's label, which its window's events go to.
    pub label: String,
}

impl RepoCtx {
    /// Sends `event` to the repo's window.
    pub fn notify<E: Event + Serialize + Clone>(&self, event: E) {
        if let Some(ui) = self.ui.lock().unwrap().as_ref() {
            event.emit_to(&ui.app, &ui.label).ok();
        }
    }

    /// Sends `event` to every window.
    pub fn notify_all<E: Event + Serialize + Clone>(&self, event: E) {
        if let Some(ui) = self.ui.lock().unwrap().as_ref() {
            event.emit(&ui.app).ok();
        }
    }
}
