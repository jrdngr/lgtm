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
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hide_reviewed: true,
            split: false,
            hide_pr: false,
            show_resolved: false,
        }
    }
}

fn path() -> Option<PathBuf> {
    Some(lgtm_core::paths::data_dir()?.join("tui-settings.json"))
}

/// The settings last saved, or the defaults.
pub fn load() -> Settings {
    path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save(settings: &Settings) {
    let Some(p) = path() else { return };
    if let (Some(dir), Ok(text)) = (p.parent(), serde_json::to_string_pretty(settings)) {
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
