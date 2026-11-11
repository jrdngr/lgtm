//! Settings, saved as JSON in the config folder.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::agents::Kind;
use crate::fonts::Fonts;
use crate::theme;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub dock_badge: bool,
    pub notify_blocked: bool,
    pub notify_done: bool,
    /// Alerts also go to the system, not only the window.
    pub system_notifications: bool,
    pub sound: bool,
    pub restore: bool,
    pub resume: bool,
    pub font_size: f32,
    pub fonts: Fonts,
    /// The command that starts Claude Code, like the alias `cc`.
    pub claude_command: String,
    pub codex_command: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            dock_badge: true,
            notify_blocked: true,
            notify_done: true,
            system_notifications: true,
            sound: false,
            restore: true,
            resume: true,
            font_size: theme::DEFAULT_FONT_SIZE,
            fonts: Fonts::System,
            claude_command: "claude".into(),
            codex_command: "codex".into(),
        }
    }
}

fn path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("lgterm").join("settings.json"))
}

impl Settings {
    /// The command that starts `kind`: the user's, or its own name.
    pub fn agent_command(&self, kind: Kind) -> &str {
        let set = match kind {
            Kind::Claude => &self.claude_command,
            Kind::Codex => &self.codex_command,
            Kind::Pi => "",
        };
        if set.trim().is_empty() {
            kind.name()
        } else {
            set
        }
    }

    /// Where the user's command for `kind` is kept, if it has one.
    pub fn agent_command_mut(&mut self, kind: Kind) -> Option<&mut String> {
        match kind {
            Kind::Claude => Some(&mut self.claude_command),
            Kind::Codex => Some(&mut self.codex_command),
            Kind::Pi => None,
        }
    }

    pub fn load() -> Self {
        path()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(path) = path() else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_vec_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_take_their_defaults() {
        let s: Settings = serde_json::from_str(r#"{"sound": true}"#).unwrap();
        assert!(s.sound);
        assert!(s.notify_done);
        assert_eq!(s.font_size, theme::DEFAULT_FONT_SIZE);
        assert_eq!(s.fonts, Fonts::System);
    }
}
