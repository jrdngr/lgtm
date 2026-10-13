//! The app menu: the platform default plus New Window and Open Repository in File, and a Quit
//! that closes windows one by one so each can ask about unsaved edits.

use tauri::menu::{Menu, MenuEvent, MenuItem, MenuItemKind, PredefinedMenuItem};
use tauri::{AppHandle, Manager, Runtime};
use tauri_specta::Event as _;

use crate::events::SwitchRepo;
use crate::repo;

const NEW_WINDOW: &str = "new-window";
const OPEN_REPO: &str = "open-repo";
const QUIT: &str = "quit";

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let menu = Menu::default(app)?;
    for item in menu.items()? {
        let Some(sub) = item.as_submenu() else { continue };
        if sub.text()? == "File" {
            sub.prepend_items(&[
                &MenuItem::with_id(app, NEW_WINDOW, "New Window", true, Some("CmdOrCtrl+N"))?,
                &MenuItem::with_id(app, OPEN_REPO, "Open Repository…", true, Some("CmdOrCtrl+O"))?,
                &PredefinedMenuItem::separator(app)?,
            ])?;
        }
        // The predefined Quit ends the app without asking any window.
        for (i, entry) in sub.items()?.iter().enumerate() {
            if let MenuItemKind::Predefined(p) = entry
                && p.text()?.starts_with("Quit")
            {
                let text = p.text()?;
                sub.remove_at(i)?;
                sub.insert(&MenuItem::with_id(app, QUIT, text, true, Some("CmdOrCtrl+Q"))?, i)?;
            }
        }
    }
    Ok(menu)
}

pub fn handle(app: &AppHandle, event: MenuEvent) {
    let result = match event.id().as_ref() {
        NEW_WINDOW => repo::open_start(app),
        OPEN_REPO => switch_repo(app),
        QUIT => quit(app),
        _ => Ok(()),
    };
    if let Err(e) = result {
        eprintln!("lgtm: {e}");
    }
}

/// Asks the focused repo window to swap itself for the start window.
fn switch_repo(app: &AppHandle) -> tauri::Result<()> {
    let focused = app
        .webview_windows()
        .into_values()
        .find(|w| w.label() != repo::START_LABEL && w.is_focused().unwrap_or(false));
    match focused {
        Some(window) => Ok(SwitchRepo {}.emit_to(app, window.label())?),
        None => repo::open_start(app),
    }
}

/// Requests every window close. The app quits when the last one goes.
fn quit(app: &AppHandle) -> tauri::Result<()> {
    for window in app.webview_windows().into_values() {
        window.close()?;
    }
    Ok(())
}
