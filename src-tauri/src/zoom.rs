//! View > Zoom In / Zoom Out / Actual Size: one zoom level for every window, remembered.

use std::path::{Path, PathBuf};

use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Manager, WebviewWindow, WebviewWindowBuilder, Wry};

const LEVELS: [f64; 11] = [0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0];
const DEFAULT: usize = 5;

fn path(data_dir: &Path) -> PathBuf {
    data_dir.join("zoom.json")
}

/// The saved step in `LEVELS`. A missing, unreadable or out-of-range file is the default.
fn load(file: &Path) -> usize {
    std::fs::read(file)
        .ok()
        .and_then(|b| serde_json::from_slice::<usize>(&b).ok())
        .filter(|&i| i < LEVELS.len())
        .unwrap_or(DEFAULT)
}

fn save(file: &Path, step: usize) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(file, serde_json::to_vec(&step)?)
}

/// The step after moving `by` from `step`, staying in range. `None` resets.
fn next(step: usize, by: Option<isize>) -> usize {
    match by {
        Some(by) => step.saturating_add_signed(by).min(LEVELS.len() - 1),
        None => DEFAULT,
    }
}

fn file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|d| path(&d))
}

/// Gives a window the saved zoom once its page has loaded. WKWebView zoomed before the first
/// load lays the page out for the wrong size, overscaled and cut off at the bottom. Going
/// through 1.0 makes the change a real one even if WebKit already holds the level.
pub fn apply(window: &WebviewWindow) {
    let Some(file) = file(window.app_handle()) else {
        return;
    };
    let step = load(&file);
    if step != DEFAULT {
        window.set_zoom(1.0).ok();
        window.set_zoom(LEVELS[step]).ok();
    }
}

/// `builder` with the saved zoom applied each time its page finishes loading.
pub fn on_load<'a, M: Manager<Wry>>(
    builder: WebviewWindowBuilder<'a, Wry, M>,
) -> WebviewWindowBuilder<'a, Wry, M> {
    builder.on_page_load(|window, payload| {
        if payload.event() == PageLoadEvent::Finished {
            apply(&window);
        }
    })
}

/// Steps the zoom `by` (or resets it with `None`) in every window and saves it.
pub fn step(app: &AppHandle, by: Option<isize>) -> tauri::Result<()> {
    let Some(file) = file(app) else {
        return Ok(());
    };
    let step = next(load(&file), by);
    for window in app.webview_windows().into_values() {
        window.set_zoom(LEVELS[step])?;
    }
    save(&file, step)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_stop_at_the_ends_reset_and_round_trip() {
        assert_eq!(next(0, Some(-1)), 0);
        assert_eq!(next(LEVELS.len() - 1, Some(1)), LEVELS.len() - 1);
        assert_eq!(next(2, None), DEFAULT);

        let dir = tempfile::tempdir().unwrap();
        let file = path(dir.path());
        assert_eq!(load(&file), DEFAULT);
        save(&file, 8).unwrap();
        assert_eq!(load(&file), 8);
        std::fs::write(&file, "99").unwrap();
        assert_eq!(load(&file), DEFAULT);
    }
}
