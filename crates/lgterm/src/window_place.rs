//! Where the window was, saved so it opens there again. Window bounds are relative to the
//! window's display, so the display is saved with them, by its UUID, which outlasts restarts.

use std::path::PathBuf;

use gpui::{App, Bounds, DisplayId, Pixels, Window, WindowBounds, point, px, size};
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Saved {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    maximized: bool,
    /// The display's UUID. Files from before this field have none.
    #[serde(default)]
    display: Option<String>,
}

fn path() -> Option<PathBuf> {
    Some(dirs::data_dir()?.join("lgterm").join("window.json"))
}

/// The window's last place and its display, when that display is still there and shows some
/// of it; else centered at the default size on the main display.
pub fn load(cx: &App) -> (WindowBounds, Option<DisplayId>) {
    let saved = path()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice::<Saved>(&b).ok());
    let placed = saved.and_then(|s| {
        let display = cx.displays().into_iter().find(|d| {
            d.uuid()
                .is_ok_and(|u| s.display.as_deref() == Some(&u.to_string()))
        })?;
        let b = from_saved(&s);
        display.bounds().intersects(&b).then(|| {
            let bounds = if s.maximized {
                WindowBounds::Maximized(b)
            } else {
                WindowBounds::Windowed(b)
            };
            (bounds, Some(display.id()))
        })
    });
    placed.unwrap_or_else(|| {
        let b = Bounds::centered(None, size(px(1440.), px(900.)), cx);
        (WindowBounds::Windowed(b), None)
    })
}

/// Saves the window's place. A full screen window saves the place it leaves full screen to.
pub fn save(window: &Window, cx: &App) {
    let Some(path) = path() else { return };
    let mut saved = to_saved(window.window_bounds());
    saved.display = window
        .display(cx)
        .and_then(|d| d.uuid().ok())
        .map(|u| u.to_string());
    let Ok(json) = serde_json::to_vec(&saved) else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, json);
}

fn to_saved(bounds: WindowBounds) -> Saved {
    let (b, maximized) = match bounds {
        WindowBounds::Windowed(b) | WindowBounds::Fullscreen(b) => (b, false),
        WindowBounds::Maximized(b) => (b, true),
    };
    Saved {
        x: b.origin.x.as_f32(),
        y: b.origin.y.as_f32(),
        w: b.size.width.as_f32(),
        h: b.size.height.as_f32(),
        maximized,
        display: None,
    }
}

fn from_saved(s: &Saved) -> Bounds<Pixels> {
    Bounds::new(point(px(s.x), px(s.y)), size(px(s.w), px(s.h)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_screen_saves_its_windowed_place() {
        let b = Bounds::new(point(px(10.), px(20.)), size(px(800.), px(600.)));
        let saved = to_saved(WindowBounds::Fullscreen(b));
        assert!(!saved.maximized);
        assert_eq!(from_saved(&saved), b);
        assert!(to_saved(WindowBounds::Maximized(b)).maximized);
    }

    #[test]
    fn a_file_from_before_displays_were_saved_still_reads() {
        let json = br#"{"x":1.0,"y":2.0,"w":3.0,"h":4.0,"maximized":true}"#;
        let saved: Saved = serde_json::from_slice(json).unwrap();
        assert!(saved.maximized && saved.display.is_none());
    }
}
