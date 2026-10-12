//! Personas: markdown files with a `description` and `off_lens` front matter, and the profile
//! as the body. Global ones live in the user's config dir, and a repo's in `.lgtm/personas`,
//! where they override global ones of the same name. The defaults ship only as templates for
//! the installer.

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Scope {
    Global,
    Repo,
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PersonaFile {
    #[serde(flatten)]
    pub persona: Persona,
    pub scope: Scope,
    pub path: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Installed {
    No,
    Yes,
    /// Installed, but the file differs from the template.
    Changed,
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DefaultPersona {
    pub name: String,
    pub description: String,
    pub global: Installed,
    pub repo: Installed,
}

const DEFAULTS: [(&str, &str); 6] = [
    ("simplicity-bot", include_str!("defaults/simplicity-bot.md")),
    ("security-bot", include_str!("defaults/security-bot.md")),
    (
        "correctness-bot",
        include_str!("defaults/correctness-bot.md"),
    ),
    (
        "performance-bot",
        include_str!("defaults/performance-bot.md"),
    ),
    (
        "maintainability-bot",
        include_str!("defaults/maintainability-bot.md"),
    ),
    ("general-bot", include_str!("defaults/general-bot.md")),
];

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

/// Global personas: `personas` in the config dir.
pub fn global_dir() -> Option<PathBuf> {
    Some(config_dir()?.join("personas"))
}

pub fn repo_dir(root: &Path) -> PathBuf {
    root.join(".lgtm").join("personas")
}

fn dir(scope: Scope, root: &Path) -> anyhow::Result<PathBuf> {
    match scope {
        Scope::Global => global_dir().context("no home directory"),
        Scope::Repo => Ok(repo_dir(root)),
    }
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

fn read_dir(dir: &Path, scope: Scope) -> Vec<(PersonaFile, Option<String>)> {
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
            Some((
                PersonaFile {
                    persona,
                    scope,
                    path,
                },
                error,
            ))
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
    /// By name. A repo persona hides a global one of the same name.
    pub personas: Vec<PersonaFile>,
    pub broken: Vec<Broken>,
}

pub fn load(root: &Path) -> Personas {
    let global = global_dir().map_or_else(Vec::new, |d| read_dir(&d, Scope::Global));
    let repo = read_dir(&repo_dir(root), Scope::Repo);
    let mut out = Personas::default();
    for (file, error) in global.into_iter().chain(repo) {
        if let Some(error) = error {
            out.broken.push(Broken {
                path: file.path,
                error,
            });
            continue;
        }
        out.personas.retain(|p| p.persona.name != file.persona.name);
        out.personas.push(file);
    }
    out.personas
        .sort_by(|a, b| a.persona.name.cmp(&b.persona.name));
    out
}

fn installed(dir: Option<&Path>, name: &str, template: &str) -> Installed {
    match dir.and_then(|d| std::fs::read_to_string(d.join(format!("{name}.md"))).ok()) {
        None => Installed::No,
        Some(text) if text == template => Installed::Yes,
        Some(_) => Installed::Changed,
    }
}

pub fn defaults(root: &Path) -> Vec<DefaultPersona> {
    let global = global_dir();
    let repo = repo_dir(root);
    DEFAULTS
        .iter()
        .map(|(name, template)| DefaultPersona {
            name: name.to_string(),
            description: parse(name, template)
                .map(|p| p.description)
                .unwrap_or_default(),
            global: installed(global.as_deref(), name, template),
            repo: installed(Some(&repo), name, template),
        })
        .collect()
}

/// Writes the named default personas into `scope`'s directory, replacing what's there.
pub fn install(root: &Path, names: &[String], scope: Scope) -> anyhow::Result<()> {
    let dir = dir(scope, root)?;
    std::fs::create_dir_all(&dir)?;
    for name in names {
        let (_, template) = DEFAULTS
            .iter()
            .find(|(n, _)| n == name)
            .with_context(|| format!("no default persona named `{name}`"))?;
        std::fs::write(dir.join(format!("{name}.md")), template)?;
    }
    Ok(())
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
pub fn create(root: &Path, name: &str, scope: Scope) -> anyhow::Result<PathBuf> {
    ensure!(
        valid_name(name),
        "persona names use letters, digits, `.`, `_` and `-`"
    );
    let dir = dir(scope, root)?;
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
/// so a repo can't have Edit open some other file by its own type.
pub fn is_persona_file(root: &Path, path: &Path) -> bool {
    let in_dir = |d: &Path| path.parent() == Some(d);
    path.extension().is_some_and(|e| e == "md")
        && is_plain_file(path)
        && (in_dir(&repo_dir(root)) || global_dir().is_some_and(|d| in_dir(&d)))
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn every_default_parses() {
        for (name, text) in DEFAULTS {
            let p = parse(name, text).unwrap();
            assert!(
                !p.description.is_empty() && p.profile.starts_with("- "),
                "{name}"
            );
        }
    }

    #[test]
    fn repo_personas_override_and_installs_track_edits() {
        let repo = tempfile::tempdir().unwrap();
        let config = tempfile::tempdir().unwrap();
        // SAFETY: no other test reads XDG_CONFIG_HOME.
        unsafe { std::env::set_var("XDG_CONFIG_HOME", config.path()) };
        let root = repo.path();
        install(
            root,
            &["security-bot".into(), "general-bot".into()],
            Scope::Global,
        )
        .unwrap();
        install(root, &["security-bot".into()], Scope::Repo).unwrap();
        let path = repo_dir(root).join("security-bot.md");
        std::fs::write(&path, "---\ndescription: mine\n---\nbody").unwrap();
        std::fs::write(repo_dir(root).join("bad.md"), "---\nnope\n---").unwrap();

        let loaded = load(root);
        let names: Vec<_> = loaded
            .personas
            .iter()
            .map(|p| p.persona.name.as_str())
            .collect();
        assert_eq!(names, ["general-bot", "security-bot"]);
        assert_eq!(loaded.personas[1].scope, Scope::Repo);
        assert_eq!(loaded.personas[1].persona.description, "mine");
        assert_eq!(loaded.broken.len(), 1);

        let sec = defaults(root)
            .into_iter()
            .find(|d| d.name == "security-bot")
            .unwrap();
        assert_eq!((sec.global, sec.repo), (Installed::Yes, Installed::Changed));

        let made = create(root, "docs-bot", Scope::Repo).unwrap();
        assert!(is_persona_file(root, &made));
        assert!(create(root, "docs-bot", Scope::Repo).is_err());
        assert!(create(root, "../x", Scope::Repo).is_err());
        assert!(!is_persona_file(root, &root.join("README.md")));

        #[cfg(unix)]
        {
            let link = repo_dir(root).join("link.md");
            std::fs::write(root.join("run.command"), "").unwrap();
            std::os::unix::fs::symlink(root.join("run.command"), &link).unwrap();
            assert!(!is_persona_file(root, &link));
        }
    }
}
