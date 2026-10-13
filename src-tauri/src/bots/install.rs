//! The default personas, which ship only as templates for the installer, and where each is
//! installed. An installed file that matches any text lgtm has shipped is the user's to update;
//! one that matches none holds their edits.

use std::path::Path;

use anyhow::Context;
use serde::Serialize;
use specta::Type;

use super::personas::{Scope, dir, global_dir, parse, repo_dir};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Installed {
    No,
    Yes,
    /// Installed as an older shipped version, untouched since.
    Outdated,
    /// Installed, and edited since.
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

struct Template {
    name: &'static str,
    text: &'static str,
    /// `hash` of every text shipped for it, the current one included. When a default changes,
    /// add its new hash here; `every_default_lists_its_current_hash` prints it.
    shipped: &'static [&'static str],
}

const DEFAULTS: [Template; 7] = [
    Template {
        name: "simplicity-bot",
        text: include_str!("defaults/simplicity-bot.md"),
        shipped: &["dba7168467a49144"],
    },
    Template {
        name: "security-bot",
        text: include_str!("defaults/security-bot.md"),
        shipped: &["87a0c291b60a0269"],
    },
    Template {
        name: "correctness-bot",
        text: include_str!("defaults/correctness-bot.md"),
        shipped: &["b078bc6fd2a7cf80"],
    },
    Template {
        name: "performance-bot",
        text: include_str!("defaults/performance-bot.md"),
        shipped: &["81e967bc5122381f"],
    },
    Template {
        name: "maintainability-bot",
        text: include_str!("defaults/maintainability-bot.md"),
        shipped: &["86977d285b005c2e", "9a87d847bb99cba5"],
    },
    Template {
        name: "architecture-bot",
        text: include_str!("defaults/architecture-bot.md"),
        shipped: &["d3bb07c06d10d0fb"],
    },
    Template {
        name: "general-bot",
        text: include_str!("defaults/general-bot.md"),
        shipped: &["9fa9714de57280da", "103e8223040bd882"],
    },
];

/// A text's fingerprint: the first 16 hex digits of its BLAKE3 hash.
fn hash(text: &str) -> String {
    blake3::hash(text.as_bytes()).to_hex()[..16].to_string()
}

/// How an installed file, if there is one, compares with the `current` text and the hashes of
/// every `shipped` one.
fn classify(current: &str, shipped: &[&str], installed: Option<&str>) -> Installed {
    match installed {
        None => Installed::No,
        Some(text) if text == current => Installed::Yes,
        Some(text) if shipped.contains(&hash(text).as_str()) => Installed::Outdated,
        Some(_) => Installed::Changed,
    }
}

fn installed(dir: Option<&Path>, d: &Template) -> Installed {
    let text = dir.and_then(|dir| std::fs::read_to_string(dir.join(format!("{}.md", d.name))).ok());
    classify(d.text, d.shipped, text.as_deref())
}

pub fn defaults(root: &Path) -> Vec<DefaultPersona> {
    let global = global_dir();
    let repo = repo_dir(root);
    DEFAULTS
        .iter()
        .map(|d| DefaultPersona {
            name: d.name.to_string(),
            description: parse(d.name, d.text)
                .map(|p| p.description)
                .unwrap_or_default(),
            global: installed(global.as_deref(), d),
            repo: installed(Some(&repo), d),
        })
        .collect()
}

/// Writes the named default personas into `scope`'s directory, replacing what's there.
pub fn install(root: &Path, names: &[String], scope: Scope) -> anyhow::Result<()> {
    let dir = dir(scope, root)?;
    std::fs::create_dir_all(&dir)?;
    for name in names {
        let d = DEFAULTS
            .iter()
            .find(|d| d.name == name)
            .with_context(|| format!("no default persona named `{name}`"))?;
        std::fs::write(dir.join(format!("{name}.md")), d.text)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_default_parses_and_lists_its_current_hash() {
        for d in &DEFAULTS {
            let p = parse(d.name, d.text).unwrap();
            assert!(
                !p.description.is_empty() && p.profile.starts_with("- "),
                "{}",
                d.name
            );
            let h = hash(d.text);
            assert!(
                d.shipped.contains(&h.as_str()),
                "add \"{h}\" to {}'s shipped hashes",
                d.name
            );
        }
    }

    #[test]
    fn an_untouched_older_default_is_outdated_and_an_edited_one_changed() {
        let (old, new) = ("- old\n", "- new\n");
        let hashes = [hash(old), hash(new)];
        let shipped: Vec<&str> = hashes.iter().map(String::as_str).collect();
        let class = |text| classify(new, &shipped, text);
        assert_eq!(class(None), Installed::No);
        assert_eq!(class(Some(new)), Installed::Yes);
        assert_eq!(class(Some(old)), Installed::Outdated);
        assert_eq!(class(Some("- old, edited\n")), Installed::Changed);
    }
}
