//! The app menu: the platform default plus New Window and Open Repository in File, and Word
//! Wrap in View.

use tauri::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::{AppHandle, Manager, Runtime};
use tauri_specta::Event as _;

use crate::events::{SwitchRepo, WordWrap};
use crate::repo;

const NEW_WINDOW: &str = "new-window";
const OPEN_REPO: &str = "open-repo";
const WORD_WRAP: &str = "word-wrap";

/// View > Word Wrap. Windows keep the setting and tell the menu what it is.
pub struct WordWrapItem<R: Runtime>(pub CheckMenuItem<R>);

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let menu = Menu::default(app)?;
    for item in menu.items()? {
        let Some(sub) = item.as_submenu() else { continue };
        match sub.text()?.as_str() {
            "File" => sub.prepend_items(&[
                &MenuItem::with_id(app, NEW_WINDOW, "New Window", true, Some("CmdOrCtrl+N"))?,
                &MenuItem::with_id(app, OPEN_REPO, "Open Repository…", true, Some("CmdOrCtrl+O"))?,
                &PredefinedMenuItem::separator(app)?,
            ])?,
            "View" => {
                let wrap =
                    CheckMenuItem::with_id(app, WORD_WRAP, "Word Wrap", true, false, Some("Alt+Z"))?;
                sub.prepend_items(&[&wrap, &PredefinedMenuItem::separator(app)?])?;
                app.manage(WordWrapItem(wrap));
            }
            _ => {}
        }
    }
    Ok(menu)
}

pub fn handle(app: &AppHandle, event: MenuEvent) {
    let result = match event.id().as_ref() {
        NEW_WINDOW => repo::open_start(app),
        OPEN_REPO => switch_repo(app),
        WORD_WRAP => word_wrap(app),
        _ => Ok(()),
    };
    if let Err(e) = result {
        eprintln!("lgtm: {e}");
    }
}

/// Clicking the item has already flipped its check, so every window takes that.
fn word_wrap(app: &AppHandle) -> tauri::Result<()> {
    let Some(item) = app.try_state::<WordWrapItem<tauri::Wry>>() else { return Ok(()) };
    Ok(WordWrap { on: item.0.is_checked()? }.emit(app)?)
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
