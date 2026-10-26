//! App-wide settings from `settings.toml` in lgtm's config dir, edited on the Settings tab.
//!
//! ```toml
//! claudeTerminal = "cc"
//! terminalFontSize = 13
//! backgroundAutoOpen = true
//! backgroundName = "bg agent"
//! ```

pub mod marks;

use std::path::{Path, PathBuf};
use std::sync::RwLock;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use specta::Type;

const FILE: &str = "settings.toml";

/// lgtm's config: `$XDG_CONFIG_HOME/lgtm`, falling back to `~/.config`.
pub fn config_dir() -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::home_dir().map(|h| h.join(".config")))?;
    Some(config.join("lgtm"))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    /// What a terminal runs to start Claude Code, typed into its shell. lgtm also runs it to
    /// check Claude Code and register its MCP server.
    pub claude_terminal: String,
    pub terminal_font_size: u16,
    /// A background send with no background terminal opens a Claude terminal and pins it.
    pub background_auto_open: bool,
    /// That terminal's tab title.
    pub background_name: String,
    /// Every mark color, the ones in use first in key order.
    pub marks: Vec<marks::MarkSlot>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            claude_terminal: "claude".into(),
            terminal_font_size: 13,
            background_auto_open: true,
            background_name: "bg agent".into(),
            marks: marks::defaults(),
        }
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

/// Keys earlier versions wrote, as table path and key. They're dropped on read, and the next
/// save leaves them out. Any other unknown key is an error, to catch typos.
const RETIRED: &[(&[&str], &str)] = &[(&[], "designDir"), (&[], "claude"), (&[], "models")];

fn drop_retired(file: &mut toml::Table) {
    for (tables, key) in RETIRED {
        let mut at = Some(&mut *file);
        for t in *tables {
            at = at
                .and_then(|a| a.get_mut(*t))
                .and_then(toml::Value::as_table_mut);
        }
        if let Some(table) = at {
            table.remove(*key);
        }
    }
}

fn parse(text: &str) -> anyhow::Result<Settings> {
    let mut settings = toml::Table::try_from(Settings::default())?;
    let mut file = toml::from_str(text)?;
    drop_retired(&mut file);
    merge(&mut settings, file);
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
        tracing::error!("{e:#}");
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

        std::fs::write(&file, "claudeTerminal = \"cc\"\n").unwrap();
        let s = load_from(&file).unwrap();
        assert_eq!(s.claude_terminal, "cc");
        assert_eq!(s.terminal_font_size, 13);

        std::fs::write(&file, toml::to_string(&s).unwrap()).unwrap();
        assert_eq!(load_from(&file).unwrap(), s);

        std::fs::write(&file, "claud = \"cc\"\n").unwrap();
        assert!(load_from(&file).is_err());
    }

    #[test]
    fn keys_from_earlier_versions_are_dropped() {
        let s = parse(
            "designDir = \"docs\"\nclaude = \"cc\"\nclaudeTerminal = \"ct\"\n[models]\nqa = \"opus\"\n",
        )
        .unwrap();
        assert_eq!(s.claude_terminal, "ct");
        let saved = toml::to_string(&s).unwrap();
        assert!(!saved.contains("design") && !saved.contains("models"));
        assert!(!saved.contains("claude ="));
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
