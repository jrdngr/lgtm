//! Each window reopens at the size and place it last had, by label: one per repo, and the start
//! window. Kept in logical points, the units macOS lays windows out in across displays, and
//! given to the window as it's built so nothing moves it afterwards. A window has a place for
//! each set of displays it was on, so macOS moving it to the laptop when an external display
//! sleeps or goes away doesn't replace where it goes once that display is back.

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
    /// Whether the two are the same display, give or take rounding.
    fn same(&self, other: &Rect) -> bool {
        let close = |a: f64, b: f64| (a - b).abs() < 1.0;
        close(self.x, other.x)
            && close(self.y, other.y)
            && close(self.width, other.width)
            && close(self.height, other.height)
    }

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

/// Whether two sets of displays, each in [`screens`] order, are the same.
fn same_screens(a: &[Rect], b: &[Rect]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| a.same(b))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Placement {
    /// The content area, from when the window was last neither maximized nor full screen.
    content: Rect,
    maximized: bool,
    fullscreen: bool,
    /// The displays there were. Files from before this field have none.
    #[serde(default)]
    screens: Vec<Rect>,
}

/// A window's places, one per set of displays, the latest last.
#[derive(Deserialize)]
#[serde(untagged)]
enum Saved {
    Each(Vec<Placement>),
    /// The single place files kept before there was one per set of displays.
    One(Placement),
}

/// Most sets of displays a window keeps a place for.
const MAX_LAYOUTS: usize = 8;

/// The place to use with `screens`: the one kept for those displays, else the latest.
fn pick<'a>(places: &'a [Placement], screens: &[Rect]) -> Option<&'a Placement> {
    places
        .iter()
        .find(|p| same_screens(&p.screens, screens))
        .or(places.last())
}

/// Keeps `place` as the one for its displays, and the latest.
fn keep(places: &mut Vec<Placement>, place: Placement) {
    places.retain(|p| !same_screens(&p.screens, &place.screens));
    places.push(place);
    if places.len() > MAX_LAYOUTS {
        places.remove(0);
    }
}

/// Where each window was, by label, saved to disk whenever a window goes.
pub struct Placements {
    file: Option<PathBuf>,
    saved: Mutex<HashMap<String, Vec<Placement>>>,
}

impl Placements {
    /// The saved placements. A missing or unreadable file has none.
    pub fn load(app: &AppHandle) -> Self {
        let file = crate::paths::data_dir(app)
            .ok()
            .map(|d| d.join("windows.json"));
        let saved: HashMap<String, Saved> = file
            .as_deref()
            .and_then(|f| std::fs::read(f).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let saved = saved
            .into_iter()
            .map(|(label, s)| match s {
                Saved::Each(places) => (label, places),
                Saved::One(place) => (label, vec![place]),
            })
            .collect();
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
        let screens = screens(window.app_handle());
        let mut saved = self.saved.lock().unwrap();
        let places = saved.entry(window.label().into()).or_default();
        if maximized || fullscreen {
            if let Some(p) = places.iter().find(|p| same_screens(&p.screens, &screens)) {
                let place = Placement {
                    maximized,
                    fullscreen,
                    ..p.clone()
                };
                keep(places, place);
            }
            return;
        }
        let Some(content) = content(window) else {
            return;
        };
        let place = Placement {
            content,
            maximized,
            fullscreen,
            screens,
        };
        keep(places, place);
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

/// The displays, left to right and then top to bottom.
fn screens(app: &AppHandle) -> Vec<Rect> {
    let monitors = app.available_monitors().unwrap_or_default();
    let mut screens: Vec<Rect> = monitors
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
        .collect();
    screens.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    screens
}

/// Gives a new window the size and place it last had with these displays, or else its latest.
/// It keeps the builder's place when that was on a display that's gone.
pub fn place<'a, M: Manager<Wry>>(
    app: &AppHandle,
    label: &str,
    builder: WebviewWindowBuilder<'a, Wry, M>,
) -> WebviewWindowBuilder<'a, Wry, M> {
    let screens = screens(app);
    let placements = app.state::<Placements>();
    let saved = placements.saved.lock().unwrap();
    let Some(p) = saved.get(label).and_then(|places| pick(places, &screens)) else {
        return builder;
    };
    let r = p.content;
    let mut builder = builder.inner_size(r.width, r.height);
    if on_screen(&r, &screens) {
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

    fn at(content: Rect, screens: &[Rect]) -> Placement {
        Placement {
            content,
            maximized: false,
            fullscreen: false,
            screens: screens.to_vec(),
        }
    }

    #[test]
    fn a_move_to_the_laptop_while_undocked_keeps_the_docked_place() {
        let laptop = rect(0.0, 0.0, 1512.0, 982.0);
        let ultrawide = rect(-500.0, -1440.0, 3440.0, 1440.0);
        let docked = [ultrawide, laptop];
        let on_ultrawide = rect(0.0, -1400.0, 2400.0, 1300.0);
        let on_laptop = rect(50.0, 40.0, 1400.0, 900.0);
        let mut places = vec![];
        keep(&mut places, at(on_ultrawide, &docked));
        keep(&mut places, at(on_laptop, &[laptop]));
        assert_eq!(pick(&places, &docked).unwrap().content, on_ultrawide);
        assert_eq!(pick(&places, &[laptop]).unwrap().content, on_laptop);
        // A set of displays it was never on gets the latest place.
        let other = [laptop, rect(1512.0, 0.0, 1920.0, 1080.0)];
        assert_eq!(pick(&places, &other).unwrap().content, on_laptop);
    }

    #[test]
    fn a_file_from_before_display_sets_keeps_its_place() {
        let json = r#"{"start":{"content":{"x":1.0,"y":2.0,"width":3.0,"height":4.0},"maximized":false,"fullscreen":true}}"#;
        let saved: HashMap<String, Saved> = serde_json::from_str(json).unwrap();
        let Some(Saved::One(p)) = saved.get("start") else {
            panic!("not read as a single place");
        };
        assert!(p.fullscreen && p.screens.is_empty());
        assert_eq!(p.content, rect(1.0, 2.0, 3.0, 4.0));
    }
}
