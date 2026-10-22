//! Each window reopens at the size and place it last had, by label: one per repo, and the start
//! window. Kept in logical points, the units macOS lays windows out in across displays, and
//! given to the window as it's built so nothing moves it afterwards.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, WebviewWindowBuilder, Window, Wry};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
struct Rect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl Rect {
    fn overlaps(&self, other: &Rect) -> bool {
        self.x < other.x + other.width
            && other.x < self.x + self.width
            && self.y < other.y + other.height
            && other.y < self.y + self.height
    }
}

/// Whether any of `rect` shows on one of `screens`.
fn on_screen(rect: &Rect, screens: &[Rect]) -> bool {
    screens.iter().any(|s| rect.overlaps(s))
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct Placement {
    /// The content area, from when the window was last neither maximized nor full screen.
    content: Rect,
    maximized: bool,
    fullscreen: bool,
}

/// Where each window was, by label, saved to disk whenever a window goes.
pub struct Placements {
    file: Option<PathBuf>,
    saved: Mutex<HashMap<String, Placement>>,
}

impl Placements {
    /// The saved placements. A missing or unreadable file has none.
    pub fn load(app: &AppHandle) -> Self {
        let file = crate::paths::data_dir(app)
            .ok()
            .map(|d| d.join("windows.json"));
        let saved = file
            .as_deref()
            .and_then(|f| std::fs::read(f).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self {
            file,
            saved: Mutex::new(saved),
        }
    }

    /// Notes where the window is now, after it moved or resized.
    pub fn record(&self, window: &Window) {
        if window.is_minimized().unwrap_or(true) {
            return;
        }
        let maximized = window.is_maximized().unwrap_or(false);
        let fullscreen = window.is_fullscreen().unwrap_or(false);
        let mut saved = self.saved.lock().unwrap();
        if maximized || fullscreen {
            if let Some(p) = saved.get_mut(window.label()) {
                (p.maximized, p.fullscreen) = (maximized, fullscreen);
            }
            return;
        }
        let Some(content) = content(window) else {
            return;
        };
        let placement = Placement {
            content,
            maximized,
            fullscreen,
        };
        saved.insert(window.label().into(), placement);
    }

    pub fn save(&self) {
        let Some(file) = &self.file else { return };
        let json = serde_json::to_vec_pretty(&*self.saved.lock().unwrap()).unwrap_or_default();
        let written = file
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(file, json));
        if let Err(e) = written {
            tracing::warn!("save window placements: {e}");
        }
    }
}

/// The window's content area in logical points.
fn content(window: &Window) -> Option<Rect> {
    let scale = window.scale_factor().ok()?;
    let pos = window.inner_position().ok()?.to_logical::<f64>(scale);
    let size = window.inner_size().ok()?.to_logical::<f64>(scale);
    (size.width > 0.0 && size.height > 0.0).then_some(Rect {
        x: pos.x,
        y: pos.y,
        width: size.width,
        height: size.height,
    })
}

fn screens(app: &AppHandle) -> Vec<Rect> {
    let monitors = app.available_monitors().unwrap_or_default();
    monitors
        .iter()
        .map(|m| {
            let pos = m.position().to_logical::<f64>(m.scale_factor());
            let size = m.size().to_logical::<f64>(m.scale_factor());
            Rect {
                x: pos.x,
                y: pos.y,
                width: size.width,
                height: size.height,
            }
        })
        .collect()
}

/// Gives a new window the size and place it last had. It keeps the builder's place when
/// that was on a display that's gone.
pub fn place<'a, M: Manager<Wry>>(
    app: &AppHandle,
    label: &str,
    builder: WebviewWindowBuilder<'a, Wry, M>,
) -> WebviewWindowBuilder<'a, Wry, M> {
    let Some(p) = app
        .state::<Placements>()
        .saved
        .lock()
        .unwrap()
        .get(label)
        .copied()
    else {
        return builder;
    };
    let r = p.content;
    let mut builder = builder.inner_size(r.width, r.height);
    if on_screen(&r, &screens(app)) {
        builder = builder.position(r.x, r.y);
    }
    builder.maximized(p.maximized).fullscreen(p.fullscreen)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn a_window_left_on_an_unplugged_display_is_off_screen() {
        let laptop = rect(0.0, 0.0, 1512.0, 982.0);
        let external = rect(1512.0, -200.0, 2560.0, 1440.0);
        let window = rect(2000.0, 100.0, 1400.0, 900.0);
        assert!(on_screen(&window, &[laptop, external]));
        assert!(!on_screen(&window, &[laptop]));
        // Touching the display's edge shows nothing of the window.
        assert!(!on_screen(&rect(1512.0, 0.0, 800.0, 600.0), &[laptop]));
        assert!(on_screen(&rect(-700.0, 900.0, 800.0, 600.0), &[laptop]));
    }
}
