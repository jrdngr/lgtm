//! The app menu: the platform default plus New Window and Open Repository in File.

use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::{AppHandle, Manager, Runtime};

use crate::repo;

const NEW_WINDOW: &str = "new-window";
const OPEN_REPO: &str = "open-repo";

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let menu = Menu::default(app)?;
    let file = menu.items()?.into_iter().find_map(|item| {
        item.as_submenu()
            .filter(|s| s.text().is_ok_and(|t| t == "File"))
            .cloned()
    });
    if let Some(file) = file {
        file.prepend_items(&[
            &MenuItem::with_id(app, NEW_WINDOW, "New Window", true, Some("CmdOrCtrl+N"))?,
            &MenuItem::with_id(app, OPEN_REPO, "Open Repository…", true, Some("CmdOrCtrl+O"))?,
            &PredefinedMenuItem::separator(app)?,
        ])?;
    }
    Ok(menu)
}

pub fn handle(app: &AppHandle, event: MenuEvent) {
    let result = match event.id().as_ref() {
        NEW_WINDOW => repo::open_start(app),
        OPEN_REPO => switch_repo(app),
        _ => Ok(()),
    };
    if let Err(e) = result {
        eprintln!("lgtm: {e}");
    }
}

/// Swaps the focused repo window for the start window.
fn switch_repo(app: &AppHandle) -> tauri::Result<()> {
    let focused = app
        .webview_windows()
        .into_values()
        .find(|w| w.label() != repo::START_LABEL && w.is_focused().unwrap_or(false));
    // Start first, so closing the last repo window doesn't quit the app.
    repo::open_start(app)?;
    match focused {
        Some(window) => window.close(),
        None => Ok(()),
    }
}
