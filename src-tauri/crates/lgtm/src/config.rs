//! The terminal app's settings: `default.toml`'s, with the user's `config.toml` over them.

use std::path::PathBuf;

use anyhow::Context;
use serde::Deserialize;
use toml::Table;

const DEFAULT: &str = include_str!("../default.toml");

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// The command that opens a file at a line, with `{file}` and `{line}` in it.
    pub editor: String,
}

/// The user's config file.
pub fn path() -> Option<PathBuf> {
    Some(lgtm_core::paths::config_dir()?.join("config.toml"))
}

/// The settings, or why the user's config file can't be read.
pub fn load() -> anyhow::Result<Config> {
    let user = match path() {
        Some(p) if p.exists() => {
            let text = std::fs::read_to_string(&p)
                .with_context(|| format!("couldn't read {}", p.display()))?;
            Some((p, text))
        }
        _ => None,
    };
    let (path, text) = user.unzip();
    parse(text.as_deref()).with_context(|| match path {
        Some(p) => format!("{} isn't valid", p.display()),
        None => "default.toml isn't valid".into(),
    })
}

/// The defaults with `user`'s settings over them.
fn parse(user: Option<&str>) -> anyhow::Result<Config> {
    let mut config: Table = DEFAULT.parse()?;
    if let Some(user) = user {
        merge(&mut config, user.parse()?);
    }
    Ok(config.try_into()?)
}

/// Puts `over`'s values into `base`, table by table.
fn merge(base: &mut Table, over: Table) {
    for (key, value) in over {
        match (base.get_mut(&key), value) {
            (Some(toml::Value::Table(b)), toml::Value::Table(o)) => merge(b, o),
            (_, value) => {
                base.insert(key, value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_users_settings_go_over_the_defaults() {
        assert_eq!(parse(None).unwrap().editor, "hx {file}:{line}");
        let c = parse(Some("editor = \"nvim +{line} {file}\"")).unwrap();
        assert_eq!(c.editor, "nvim +{line} {file}");
        assert!(parse(Some("editr = \"vi\"")).is_err());
        assert!(parse(Some("editor = 3")).is_err());
    }
}
