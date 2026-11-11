//! Heat on the file trees: a file glows when it's written and fades over a few seconds. While
//! it's out of sight, the closed folder that holds it glows instead.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use ratatui::style::Color;

/// How long a write glows.
pub const FADE: Duration = Duration::from_secs(3);
/// How much of the tint a fresh write lays over the row.
const STRENGTH: f32 = 0.38;

/// When each file, and each folder above one, was last written.
#[derive(Clone, Debug, Default)]
pub struct Heat {
    files: HashMap<String, Instant>,
    dirs: HashMap<String, Instant>,
}

impl Heat {
    pub fn written(&mut self, paths: &[String]) {
        let now = Instant::now();
        self.files.retain(|_, at| now - *at < FADE);
        self.dirs.retain(|_, at| now - *at < FADE);
        for path in paths {
            self.files.insert(path.clone(), now);
            for (i, _) in path.match_indices('/') {
                self.dirs.insert(path[..i].to_string(), now);
            }
        }
    }

    /// How much a file glows, from 1 just written to 0 once faded.
    pub fn file(&self, path: &str) -> f32 {
        level(self.files.get(path))
    }

    pub fn dir(&self, path: &str) -> f32 {
        level(self.dirs.get(path))
    }
}

fn level(at: Option<&Instant>) -> f32 {
    at.map_or(0.0, |at| {
        1.0 - (at.elapsed().as_secs_f32() / FADE.as_secs_f32()).min(1.0)
    })
}

/// Marks a Files title that ends at `x` with an amber dot while heat is on.
pub fn mark(c: &mut crate::tui::paint::Canvas, x: u16, y: u16, on: bool) {
    use crate::tui::theme::{self, AMBER, BG};
    if on {
        c.put(x + 1, y, "●", theme::fg(AMBER).bg(BG));
    }
}

/// `bg` with `tint` laid over it as much as `level` glows.
pub fn glow(bg: Color, tint: Color, level: f32) -> Color {
    mix(bg, tint, STRENGTH * level)
}

/// `bg` toward `tint` by `a`, from 0 to 1.
pub fn mix(bg: Color, tint: Color, a: f32) -> Color {
    let (Color::Rgb(br, bgn, bb), Color::Rgb(tr, tg, tb)) = (bg, tint) else {
        return bg;
    };
    let a = a.clamp(0.0, 1.0);
    let mix = |b: u8, t: u8| (f32::from(b) * (1.0 - a) + f32::from(t) * a).round() as u8;
    Color::Rgb(mix(br, tr), mix(bgn, tg), mix(bb, tb))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_write_lights_its_file_and_every_folder_above_it() {
        let mut h = Heat::default();
        h.written(&["src/review/a.rs".into()]);
        assert!(h.file("src/review/a.rs") > 0.99);
        assert!(h.dir("src") > 0.99);
        assert!(h.dir("src/review") > 0.99);
        assert_eq!(h.file("src/review"), 0.0);
        assert_eq!(h.dir("src/rev"), 0.0);
    }

    #[test]
    fn glow_mixes_toward_the_tint_and_leaves_the_background_when_cold() {
        let bg = Color::Rgb(30, 30, 30);
        assert_eq!(glow(bg, Color::Rgb(230, 30, 30), 0.0), bg);
        assert_eq!(
            glow(bg, Color::Rgb(230, 30, 30), 1.0),
            Color::Rgb(106, 30, 30)
        );
    }
}
