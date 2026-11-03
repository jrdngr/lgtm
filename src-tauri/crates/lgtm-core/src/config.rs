//! Per-repo settings from `.lgtm/config.toml`, read at a review's base.
//!
//! ```toml
//! skip = ["*.lock", "**/generated/**"]
//!
//! [anchor]
//! window = 2
//! threshold = 0.5
//! ```

use anyhow::Context;
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Deserialize;

use crate::anchor::MatchConfig;
use crate::jj::Jj;

pub const PATH: &str = ".lgtm/config.toml";

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    #[serde(default)]
    skip: Vec<String>,
    #[serde(default)]
    anchor: RawAnchor,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAnchor {
    window: Option<usize>,
    threshold: Option<f64>,
}

/// The config file's text at each base commit read so far, `None` where it's missing.
#[derive(Default)]
pub struct ConfigTexts(std::sync::Mutex<std::collections::HashMap<String, Option<String>>>);

/// Past this many bases the cache starts over.
const MAX_CACHED: usize = 256;

#[derive(Default)]
pub struct Config {
    pub skip: GlobSet,
    pub matching: MatchConfig,
    /// The file's text, so cached results built under other settings can be told apart.
    pub source: String,
}

impl Config {
    /// Reads the config at `base`, a review's base commit, so the changes under review can't
    /// set what it skips. A missing file gives the defaults. Each base's file is read once,
    /// since a commit's contents never change.
    pub async fn at_base(jj: &Jj, cache: &ConfigTexts, base: Option<&str>) -> anyhow::Result<Self> {
        let key = base.unwrap_or_default().to_string();
        let found = cache.0.lock().unwrap().get(&key).cloned();
        let text = match found {
            Some(text) => text,
            None => {
                let text = match jj.file_if_any(base, PATH).await? {
                    Some(bytes) => Some(
                        String::from_utf8(bytes).with_context(|| format!("{PATH} is not UTF-8"))?,
                    ),
                    None => None,
                };
                let mut cache = cache.0.lock().unwrap();
                if cache.len() >= MAX_CACHED {
                    cache.clear();
                }
                cache.insert(key, text.clone());
                text
            }
        };
        match text {
            Some(text) => Self::parse(&text).with_context(|| format!("read {PATH}")),
            None => Ok(Self::default()),
        }
    }

    pub fn parse(text: &str) -> anyhow::Result<Self> {
        let raw: Raw = toml::from_str(text)?;
        let mut skip = GlobSetBuilder::new();
        for pattern in &raw.skip {
            skip.add(Glob::new(pattern).with_context(|| format!("bad skip glob `{pattern}`"))?);
        }
        let defaults = MatchConfig::default();
        let matching = MatchConfig {
            window: raw.anchor.window.unwrap_or(defaults.window),
            threshold: raw.anchor.threshold.unwrap_or(defaults.threshold),
        };
        anyhow::ensure!(matching.window > 0, "anchor.window must be at least 1");
        anyhow::ensure!(
            (0.0..=1.0).contains(&matching.threshold),
            "anchor.threshold must be between 0 and 1"
        );
        Ok(Self {
            skip: skip.build()?,
            matching,
            source: text.to_string(),
        })
    }

    pub fn skips(&self, path: &str) -> bool {
        self.skip.is_match(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_skip_globs_and_anchor_settings() {
        let cfg = Config::parse(
            "skip = [\"*.lock\", \"**/generated/**\"]\n[anchor]\nwindow = 3\nthreshold = 0.75\n",
        )
        .unwrap();
        assert!(cfg.skips("Cargo.lock"));
        assert!(cfg.skips("web/pnpm.lock"));
        assert!(cfg.skips("generated/a.rs"));
        assert!(cfg.skips("src/generated/deep/a.rs"));
        assert!(!cfg.skips("src/lib.rs"));
        assert_eq!(cfg.matching.window, 3);
        assert_eq!(cfg.matching.threshold, 0.75);
    }

    #[test]
    fn missing_values_use_defaults_and_bad_values_are_rejected() {
        let cfg = Config::parse("").unwrap();
        assert_eq!(cfg.matching.window, 2);
        assert!(!cfg.skips("a.lock"));
        assert!(Config::parse("[anchor]\nthreshold = 2.0").is_err());
        assert!(Config::parse("skp = []").is_err());
        assert!(Config::parse("skip = [\"a[\"]").is_err());
    }
}
