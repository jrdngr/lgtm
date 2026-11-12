//! The review's toggles, kept between runs in the data folder.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub hide_reviewed: bool,
    pub split: bool,
    pub hide_pr: bool,
    pub show_resolved: bool,
    pub wrap: bool,
    /// The file list in the review and the file tree in the editor.
    pub files: bool,
    /// Heat on the file trees: rows glow as their files are written.
    pub heat: bool,
    /// Widths `<` and `>` chose: the review's file list, the editor's file tree, the home
    /// screen's revisions side. `None` is the default.
    pub review_files_width: Option<u16>,
    pub editor_files_width: Option<u16>,
    pub home_revisions_width: Option<u16>,
    /// The file trees and lists on the left, not the right.
    pub files_left: bool,
    /// Whether the editor wraps long lines.
    pub editor_wrap: bool,
    /// Whether markdown files open rendered, as the last one was left.
    pub render_markdown: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hide_reviewed: true,
            split: false,
            hide_pr: false,
            show_resolved: false,
            wrap: false,
            files: true,
            heat: true,
            review_files_width: None,
            editor_files_width: None,
            home_revisions_width: None,
            files_left: false,
            editor_wrap: false,
            render_markdown: false,
        }
    }
}

fn path() -> Option<PathBuf> {
    Some(lgtm_core::paths::data_dir()?.join("tui-settings.json"))
}

/// The settings last saved, or the defaults. The files' side takes effect.
pub fn load() -> Settings {
    let settings = load_file();
    super::side::set_files_left(settings.files_left);
    settings
}

/// The settings last saved, or the defaults.
pub fn load_file() -> Settings {
    path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Saves `settings`, with the files on the side they're on now.
pub fn save(settings: &Settings) {
    let Some(p) = path() else { return };
    let settings = Settings {
        files_left: super::side::files_left(),
        ..settings.clone()
    };
    if let (Some(dir), Ok(text)) = (p.parent(), serde_json::to_string_pretty(&settings)) {
        let _ = std::fs::create_dir_all(dir);
        let _ = std::fs::write(p, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_settings_take_their_defaults() {
        let s: Settings = serde_json::from_str(r#"{"split": true}"#).unwrap();
        assert!(s.split && s.hide_reviewed && !s.hide_pr);
    }
}
