//! App-wide settings from `settings.toml` in lgtm's config dir, edited on the Settings tab.
//!
//! ```toml
//! claude = "cc"
//! terminalFontSize = 13
//!
//! [models]
//! walkthrough = "sonnet"
//! ```

pub mod claude;
pub mod marks;

use std::path::{Path, PathBuf};
use std::sync::RwLock;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::bots::personas::config_dir;

const FILE: &str = "settings.toml";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    /// How to start Claude Code: a command line, or an alias or function from the login shell.
    pub claude: String,
    pub models: Models,
    pub terminal_font_size: u16,
    /// Every mark color, the ones in use first in key order.
    pub marks: Vec<marks::MarkSlot>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            claude: "claude".into(),
            models: Models::default(),
            terminal_font_size: 13,
            marks: marks::defaults(),
        }
    }
}

/// The model for each kind of agent run, passed to `--model`. Empty is Claude Code's default.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Models {
    /// Asks and changes on notes, Fix headless, and conflict resolution.
    pub ask: String,
    pub walkthrough: String,
    pub reviewers: String,
    /// Triage and replies on PR threads.
    pub replies: String,
    /// Every step of a Q&A session: questions, plan, apply.
    pub qa: String,
}

/// What an agent run is for, which picks its model.
#[derive(Clone, Copy, Debug)]
pub enum Purpose {
    Ask,
    Walkthrough,
    Reviewers,
    /// Triage, fixes and the reply chat on GitHub threads.
    Replies,
    Qa,
}

impl Models {
    pub fn for_purpose(&self, purpose: Purpose) -> Option<&str> {
        let model = match purpose {
            Purpose::Ask => &self.ask,
            Purpose::Walkthrough => &self.walkthrough,
            Purpose::Reviewers => &self.reviewers,
            Purpose::Replies => &self.replies,
            Purpose::Qa => &self.qa,
        };
        Some(model.trim()).filter(|m| !m.is_empty())
    }
}

static CURRENT: RwLock<Option<Settings>> = RwLock::new(None);
/// Why the file couldn't be read at startup, until a save replaces it.
static LOAD_ERROR: RwLock<Option<String>> = RwLock::new(None);

fn path() -> Option<PathBuf> {
    Some(config_dir()?.join(FILE))
}

/// Lays `file` over `base`, table by table, so a file with only some keys keeps the rest.
fn merge(base: &mut toml::Table, file: toml::Table) {
    for (key, value) in file {
        match (base.get_mut(&key), value) {
            (Some(toml::Value::Table(b)), toml::Value::Table(f)) => merge(b, f),
            (_, value) => {
                base.insert(key, value);
            }
        }
    }
}

fn parse(text: &str) -> anyhow::Result<Settings> {
    let mut settings = toml::Table::try_from(Settings::default())?;
    merge(&mut settings, toml::from_str(text)?);
    let mut settings: Settings = settings.try_into()?;
    marks::normalize(&mut settings.marks);
    Ok(settings)
}

fn load_from(path: &Path) -> anyhow::Result<Settings> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse(&text).with_context(|| format!("read {}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(e) => Err(e).with_context(|| format!("read {}", path.display())),
    }
}

/// Reads the settings file. A file that can't be read is reported and the defaults used.
pub fn load() {
    let settings = path()
        .map(|p| load_from(&p))
        .unwrap_or(Ok(Settings::default()));
    let settings = settings.unwrap_or_else(|e| {
        eprintln!("lgtm: {e:#}");
        *LOAD_ERROR.write().unwrap() = Some(format!("{e:#}"));
        Settings::default()
    });
    *CURRENT.write().unwrap() = Some(settings);
}

/// Why the file couldn't be read, while the defaults stand in for it.
pub fn load_error() -> Option<String> {
    LOAD_ERROR.read().unwrap().clone()
}

/// The current settings.
pub fn get() -> Settings {
    CURRENT.read().unwrap().clone().unwrap_or_default()
}

/// Writes `settings` to the file and makes them current.
pub fn save(mut settings: Settings) -> anyhow::Result<()> {
    marks::normalize(&mut settings.marks);
    let path = path().context("no home directory")?;
    write(&path, &settings, load_error().is_some())?;
    *LOAD_ERROR.write().unwrap() = None;
    *CURRENT.write().unwrap() = Some(settings);
    Ok(())
}

/// Writes `settings` to `path`. With `backup`, copies the file there to `settings.toml.bak`
/// first, so a file that couldn't be read isn't lost.
fn write(path: &Path, settings: &Settings, backup: bool) -> anyhow::Result<()> {
    std::fs::create_dir_all(path.parent().context("settings path has no parent")?)?;
    if backup && path.exists() {
        let bak = path.with_extension("toml.bak");
        std::fs::copy(path, &bak).with_context(|| format!("back up to {}", bak.display()))?;
    }
    std::fs::write(path, toml::to_string(settings)?)
        .with_context(|| format!("write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_partial_file_keeps_the_other_defaults_and_saves_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(FILE);
        assert_eq!(load_from(&file).unwrap(), Settings::default());

        std::fs::write(
            &file,
            "claude = \"cc\"\n[models]\nwalkthrough = \"sonnet\"\n",
        )
        .unwrap();
        let s = load_from(&file).unwrap();
        assert_eq!(s.claude, "cc");
        assert_eq!(s.terminal_font_size, 13);
        assert_eq!(s.models.for_purpose(Purpose::Walkthrough), Some("sonnet"));
        assert_eq!(s.models.for_purpose(Purpose::Ask), None);

        std::fs::write(&file, toml::to_string(&s).unwrap()).unwrap();
        assert_eq!(load_from(&file).unwrap(), s);

        std::fs::write(&file, "claud = \"cc\"\n").unwrap();
        assert!(load_from(&file).is_err());
    }

    #[test]
    fn saving_over_a_file_that_failed_to_load_backs_it_up() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(FILE);
        std::fs::write(&file, "claud = \"cc\"\n").unwrap();
        write(&file, &Settings::default(), true).unwrap();
        let bak = std::fs::read_to_string(dir.path().join("settings.toml.bak")).unwrap();
        assert_eq!(bak, "claud = \"cc\"\n");
        assert_eq!(load_from(&file).unwrap(), Settings::default());
    }
}
