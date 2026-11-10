//! The font choice, System or Geist, and the bundled Geist files.

use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, Ordering};

use gpui::App;
use serde::{Deserialize, Serialize};

use crate::theme;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fonts {
    #[default]
    System,
    Geist,
}

impl Fonts {
    pub const ALL: [Fonts; 2] = [Fonts::System, Fonts::Geist];

    pub fn name(self) -> &'static str {
        match self {
            Fonts::System => "System",
            Fonts::Geist => "Geist",
        }
    }
}

static GEIST: AtomicBool = AtomicBool::new(false);

/// Registers the bundled Geist files with the text system.
pub fn load(cx: &App) {
    let files: [&'static [u8]; 8] = [
        include_bytes!("../fonts/Geist-Regular.ttf"),
        include_bytes!("../fonts/Geist-Medium.ttf"),
        include_bytes!("../fonts/Geist-SemiBold.ttf"),
        include_bytes!("../fonts/Geist-Bold.ttf"),
        include_bytes!("../fonts/GeistMono-Regular.ttf"),
        include_bytes!("../fonts/GeistMono-Bold.ttf"),
        include_bytes!("../fonts/GeistMono-Italic.ttf"),
        include_bytes!("../fonts/GeistMono-BoldItalic.ttf"),
    ];
    let files = files.into_iter().map(Cow::Borrowed).collect();
    if let Err(err) = cx.text_system().add_fonts(files) {
        eprintln!("lgterm: can't load Geist: {err:#}");
    }
}

pub fn set(fonts: Fonts) {
    GEIST.store(fonts == Fonts::Geist, Ordering::Relaxed);
}

pub fn current() -> Fonts {
    if GEIST.load(Ordering::Relaxed) {
        Fonts::Geist
    } else {
        Fonts::System
    }
}

/// The family for interface text.
pub fn ui() -> &'static str {
    match current() {
        Fonts::System => theme::UI_FONT,
        Fonts::Geist => "Geist",
    }
}

/// The family for terminals and code.
pub fn mono() -> &'static str {
    match current() {
        Fonts::System => theme::MONO_FONT,
        Fonts::Geist => "Geist Mono",
    }
}
