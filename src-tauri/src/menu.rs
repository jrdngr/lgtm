//! The app menu: the platform default plus Settings in the app menu, New Window, Open
//! Repository and Close Window in File, Word Wrap and zoom in View, and Keyboard Shortcuts in
//! Help. Close Window is on ⇧⌘W, leaving ⌘W to the webview, which closes the active tab.

use tauri::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Manager, Runtime};
use tauri_specta::Event as _;

use crate::events::{OpenSettings, OpenShortcuts, SwitchRepo, WordWrap};
use crate::{repo, zoom};

const NEW_WINDOW: &str = "new-window";
const OPEN_REPO: &str = "open-repo";
const CLOSE_WINDOW: &str = "close-window";
const WORD_WRAP: &str = "word-wrap";
const SETTINGS: &str = "settings";
const SHORTCUTS: &str = "shortcuts";
const ZOOM_IN: &str = "zoom-in";
const ZOOM_OUT: &str = "zoom-out";
const ZOOM_RESET: &str = "zoom-reset";

/// View > Word Wrap. Windows keep the setting and tell the menu what it is.
pub struct WordWrapItem<R: Runtime>(pub CheckMenuItem<R>);

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let menu = Menu::default(app)?;
    let app_name = app.package_info().name.clone();
    let close_window = PredefinedMenuItem::close_window(app, None)?.text()?;
    for item in menu.items()? {
        let Some(sub) = item.as_submenu() else {
            continue;
        };
        match sub.text()?.as_str() {
            // macOS's app menu: Settings goes after About, like other apps.
            name if name == app_name => sub.insert_items(
                &[
                    &PredefinedMenuItem::separator(app)?,
                    &MenuItem::with_id(app, SETTINGS, "Settings…", true, Some("CmdOrCtrl+,"))?,
                ],
                1,
            )?,
            "File" => {
                remove_predefined(sub, &close_window)?;
                sub.prepend_items(&[
                    &MenuItem::with_id(app, NEW_WINDOW, "New Window", true, Some("CmdOrCtrl+N"))?,
                    &MenuItem::with_id(
                        app,
                        OPEN_REPO,
                        "Open Repository…",
                        true,
                        Some("CmdOrCtrl+O"),
                    )?,
                    &PredefinedMenuItem::separator(app)?,
                    &MenuItem::with_id(
                        app,
                        CLOSE_WINDOW,
                        "Close Window",
                        true,
                        Some("CmdOrCtrl+Shift+W"),
                    )?,
                ])?
            }
            "Window" => remove_predefined(sub, &close_window)?,
            "View" => {
                let wrap = CheckMenuItem::with_id(
                    app,
                    WORD_WRAP,
                    "Word Wrap",
                    true,
                    false,
                    Some("Alt+Z"),
                )?;
                sub.prepend_items(&[
                    &wrap,
                    &PredefinedMenuItem::separator(app)?,
                    &MenuItem::with_id(app, ZOOM_IN, "Zoom In", true, Some("CmdOrCtrl+="))?,
                    &MenuItem::with_id(app, ZOOM_OUT, "Zoom Out", true, Some("CmdOrCtrl+-"))?,
                    &MenuItem::with_id(app, ZOOM_RESET, "Actual Size", true, Some("CmdOrCtrl+0"))?,
                    &PredefinedMenuItem::separator(app)?,
                ])?;
                app.manage(WordWrapItem(wrap));
            }
            "Help" => sub.prepend(&MenuItem::with_id(
                app,
                SHORTCUTS,
                "Keyboard Shortcuts",
                true,
                None::<&str>,
            )?)?,
            _ => {}
        }
    }
    Ok(menu)
}

/// Drops the submenu's predefined items titled `text`.
fn remove_predefined<R: Runtime>(sub: &Submenu<R>, text: &str) -> tauri::Result<()> {
    for item in sub.items()? {
        if let Some(predefined) = item.as_predefined_menuitem()
            && predefined.text()? == text
        {
            sub.remove(predefined)?;
        }
    }
    Ok(())
}

pub fn handle(app: &AppHandle, event: MenuEvent) {
    let result = match event.id().as_ref() {
        NEW_WINDOW => repo::open_start(app),
        OPEN_REPO => switch_repo(app),
        CLOSE_WINDOW => close_window(app),
        WORD_WRAP => word_wrap(app),
        SETTINGS => open_settings(app),
        SHORTCUTS => open_shortcuts(app),
        ZOOM_IN => zoom::step(app, Some(1)),
        ZOOM_OUT => zoom::step(app, Some(-1)),
        ZOOM_RESET => zoom::step(app, None),
        _ => Ok(()),
    };
    if let Err(e) = result {
        tracing::error!("{e}");
    }
}

/// Clicking the item has already flipped its check, so every window takes that.
fn word_wrap(app: &AppHandle) -> tauri::Result<()> {
    let Some(item) = app.try_state::<WordWrapItem<tauri::Wry>>() else {
        return Ok(());
    };
    Ok(WordWrap {
        on: item.0.is_checked()?,
    }
    .emit(app)?)
}

/// The focused repo window.
fn focused(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    app.webview_windows()
        .into_values()
        .find(|w| w.label() != repo::START_LABEL && w.is_focused().unwrap_or(false))
}

/// Asks the focused window to close, which a repo window confirms first.
fn close_window(app: &AppHandle) -> tauri::Result<()> {
    let focused = app
        .webview_windows()
        .into_values()
        .find(|w| w.is_focused().unwrap_or(false));
    match focused {
        Some(window) => window.close(),
        None => Ok(()),
    }
}

/// Opens the Settings tab in the focused repo window.
fn open_settings(app: &AppHandle) -> tauri::Result<()> {
    match focused(app) {
        Some(window) => Ok(OpenSettings {}.emit_to(app, window.label())?),
        None => Ok(()),
    }
}

/// Opens the Keyboard Shortcuts tab in the focused repo window.
fn open_shortcuts(app: &AppHandle) -> tauri::Result<()> {
    match focused(app) {
        Some(window) => Ok(OpenShortcuts {}.emit_to(app, window.label())?),
        None => Ok(()),
    }
}

/// Asks the focused repo window to swap itself for the start window.
fn switch_repo(app: &AppHandle) -> tauri::Result<()> {
    match focused(app) {
        Some(window) => Ok(SwitchRepo {}.emit_to(app, window.label())?),
        None => repo::open_start(app),
    }
}
