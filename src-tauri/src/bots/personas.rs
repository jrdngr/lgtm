//! Personas: markdown files with a `description` and `off_lens` front matter, and the profile
//! as the body. They live only in the user's config dir, so a change under review can't rewrite
//! its own reviewers.

use std::path::{Path, PathBuf};

use anyhow::{Context, bail, ensure};
use serde::{Deserialize, Serialize};
use specta::Type;

/// What a run needs of a persona, copied into the run when it starts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Persona {
    pub name: String,
    pub description: String,
    /// Whether it also reports serious issues outside its lens.
    pub off_lens: bool,
    pub profile: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PersonaFile {
    #[serde(flatten)]
    pub persona: Persona,
    pub path: String,
}

const NEW_PERSONA: &str = "---
description: What this persona cares about most, in a few words
off_lens: true
---

- What it looks for, one priority per line.
- What it flags, and what it leaves alone.
";

/// lgtm's config: `$XDG_CONFIG_HOME/lgtm`, falling back to `~/.config`.
pub fn config_dir() -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::home_dir().map(|h| h.join(".config")))?;
    Some(config.join("lgtm"))
}

/// The personas: `personas` in the config dir.
pub fn dir() -> anyhow::Result<PathBuf> {
    Ok(config_dir().context("no home directory")?.join("personas"))
}

/// Parses a persona file. Front matter is optional; without it the persona reports off-lens
/// issues and has no description.
pub fn parse(name: &str, text: &str) -> anyhow::Result<Persona> {
    let mut persona = Persona {
        name: name.to_string(),
        description: String::new(),
        off_lens: true,
        profile: text.trim().to_string(),
    };
    let Some(rest) = text.strip_prefix("---\n") else {
        return Ok(persona);
    };
    let (front, body) = rest
        .split_once("\n---")
        .context("front matter has no closing `---`")?;
    for line in front.lines().filter(|l| !l.trim().is_empty()) {
        let (key, value) = line
            .split_once(':')
            .with_context(|| format!("front matter line `{line}` isn't `key: value`"))?;
        let value = value.trim().trim_matches('"');
        match key.trim() {
            "description" => persona.description = value.to_string(),
            "off_lens" => {
                persona.off_lens = value
                    .parse()
                    .with_context(|| format!("off_lens is `{value}`, not true or false"))?
            }
            other => bail!("unknown front matter key `{other}`"),
        }
    }
    persona.profile = body.trim().to_string();
    Ok(persona)
}

fn read_dir(dir: &Path) -> Vec<(PersonaFile, Option<String>)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return vec![];
    };
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .filter_map(|path| {
            let name = path.file_stem()?.to_str()?.to_string();
            let read = std::fs::read_to_string(&path).map_err(anyhow::Error::from);
            let (persona, error) = match read.and_then(|t| parse(&name, &t)) {
                Ok(p) => (p, None),
                Err(e) => (parse(&name, "").ok()?, Some(format!("{e:#}"))),
            };
            let path = path.to_string_lossy().into_owned();
            Some((PersonaFile { persona, path }, error))
        })
        .collect()
}

/// A persona file that couldn't be read, by path, with why.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
pub struct Broken {
    pub path: String,
    pub error: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Type)]
pub struct Personas {
    /// By name.
    pub personas: Vec<PersonaFile>,
    pub broken: Vec<Broken>,
}

pub fn load() -> Personas {
    let files = dir().map_or_else(|_| Vec::new(), |d| read_dir(&d));
    let mut out = Personas::default();
    for (file, error) in files {
        match error {
            Some(error) => out.broken.push(Broken {
                path: file.path,
                error,
            }),
            None => out.personas.push(file),
        }
    }
    out.personas
        .sort_by(|a, b| a.persona.name.cmp(&b.persona.name));
    out
}

/// Names are file names: letters, digits, `.`, `_` and `-`.
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
}

/// Writes a starter file for a new persona and returns its path.
pub fn create(name: &str) -> anyhow::Result<PathBuf> {
    ensure!(
        valid_name(name),
        "persona names use letters, digits, `.`, `_` and `-`"
    );
    let dir = dir()?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{name}.md"));
    ensure!(
        !path.exists(),
        "there's already a persona named `{name}` there"
    );
    std::fs::write(&path, NEW_PERSONA)?;
    Ok(path)
}

/// A file that isn't a symlink.
pub fn is_plain_file(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.is_file())
}

/// Whether `path` is a persona file, the only files the app opens for editing. Symlinks aren't,
/// so Edit can't be pointed at some other file through one.
pub fn is_persona_file(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "md")
        && is_plain_file(path)
        && dir().is_ok_and(|d| path.parent() == Some(&d))
}

/// Copies persona file `path` to `name` beside it, adding `.md` when `name` has no extension.
/// Returns the copy's path.
pub fn duplicate(path: &Path, name: &str) -> anyhow::Result<PathBuf> {
    anyhow::ensure!(is_persona_file(path), "{} isn't a persona", path.display());
    let name = name.trim();
    anyhow::ensure!(
        !name.is_empty() && !name.starts_with('.') && !name.contains(['/', '\\']),
        "{name} isn't a file name"
    );
    let file = if Path::new(name).extension().is_some() {
        name.to_string()
    } else {
        format!("{name}.md")
    };
    let to = path.with_file_name(file);
    anyhow::ensure!(!to.exists(), "{} already exists", to.display());
    std::fs::copy(path, &to)?;
    Ok(to)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bots::install::{Installed, defaults, install};

    #[test]
    fn parses_front_matter_and_the_profile() {
        let p = parse(
            "x",
            "---\ndescription: \"Be terse\"\noff_lens: false\n---\n\n- one\n",
        )
        .unwrap();
        assert_eq!(p.description, "Be terse");
        assert!(!p.off_lens);
        assert_eq!(p.profile, "- one");
        assert!(parse("x", "- just a profile").unwrap().off_lens);
        assert!(parse("x", "---\nmodel: big\n---\n").is_err());
        assert!(parse("x", "---\noff_lens: maybe\n---\n").is_err());
    }

    #[test]
    fn installs_track_edits_and_only_plain_files_in_the_dir_are_personas() {
        let config = tempfile::tempdir().unwrap();
        // SAFETY: no other test reads XDG_CONFIG_HOME.
        unsafe { std::env::set_var("XDG_CONFIG_HOME", config.path()) };
        install(&["security-bot".into(), "general-bot".into()]).unwrap();
        let dir = dir().unwrap();
        std::fs::write(
            dir.join("security-bot.md"),
            "---\ndescription: mine\n---\nbody",
        )
        .unwrap();
        std::fs::write(dir.join("bad.md"), "---\nnope\n---").unwrap();

        let loaded = load();
        let names: Vec<_> = loaded
            .personas
            .iter()
            .map(|p| p.persona.name.as_str())
            .collect();
        assert_eq!(names, ["general-bot", "security-bot"]);
        assert_eq!(loaded.personas[1].persona.description, "mine");
        assert_eq!(loaded.broken.len(), 1);

        let sec = defaults()
            .into_iter()
            .find(|d| d.name == "security-bot")
            .unwrap();
        assert_eq!(sec.installed, Installed::Changed);

        let made = create("docs-bot").unwrap();
        assert!(is_persona_file(&made));
        assert!(create("docs-bot").is_err());
        assert!(create("../x").is_err());
        let outside = config.path().join("x.md");
        std::fs::write(&outside, "").unwrap();
        assert!(!is_persona_file(&outside));

        #[cfg(unix)]
        {
            let link = dir.join("link.md");
            std::fs::write(config.path().join("run.command"), "").unwrap();
            std::os::unix::fs::symlink(config.path().join("run.command"), &link).unwrap();
            assert!(!is_persona_file(&link));
        }
    }
}
