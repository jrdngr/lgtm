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

#[derive(Default)]
pub struct Config {
    pub skip: GlobSet,
    pub matching: MatchConfig,
    /// The file's text, so cached results built under other settings can be told apart.
    pub source: String,
}

impl Config {
    /// Reads the config at the base of `revset`, so the changes under review can't set what
    /// it skips. A missing file gives the defaults.
    pub async fn at_base(jj: &Jj, revset: &str) -> anyhow::Result<Self> {
        let base = jj.resolve(revset).await?.from;
        let Some(bytes) = jj.file_if_any(base.as_deref(), PATH).await? else {
            return Ok(Self::default());
        };
        let text = String::from_utf8(bytes).with_context(|| format!("{PATH} is not UTF-8"))?;
        Self::parse(&text).with_context(|| format!("read {PATH}"))
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
