//! Where the window was, saved so it opens there again.

use std::path::PathBuf;

use gpui::{App, Bounds, Pixels, WindowBounds, point, px, size};
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Saved {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    maximized: bool,
}

fn path() -> Option<PathBuf> {
    Some(dirs::data_dir()?.join("lgterm").join("window.json"))
}

/// The window's last place, when it's still on a display; else centered at the default size.
pub fn load(cx: &App) -> WindowBounds {
    let saved = path()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice::<Saved>(&b).ok());
    let on_screen = |b: &Bounds<Pixels>| cx.displays().iter().any(|d| d.bounds().intersects(b));
    match saved.map(|s| (from_saved(&s), s.maximized)) {
        Some((b, true)) if on_screen(&b) => WindowBounds::Maximized(b),
        Some((b, false)) if on_screen(&b) => WindowBounds::Windowed(b),
        _ => WindowBounds::Windowed(Bounds::centered(None, size(px(1440.), px(900.)), cx)),
    }
}

/// Saves the window's place. A full screen window saves the place it leaves full screen to.
pub fn save(bounds: WindowBounds) {
    let Some(path) = path() else { return };
    let Ok(json) = serde_json::to_vec(&to_saved(bounds)) else {
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
}
