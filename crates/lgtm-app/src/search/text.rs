//! Text search and replace across the workspace.

use std::path::Path;

use anyhow::Context;
use globset::{Glob, GlobSet, GlobSetBuilder};
use regex::{Regex, RegexBuilder};
use serde::Deserialize;
use specta::Type;

use super::{Hit, Hits, MAX_HITS, clip, files, read, spans};

#[derive(Clone, Debug, Default, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TextQuery {
    pub pattern: String,
    pub regex: bool,
    pub case_sensitive: bool,
    pub whole_word: bool,
    /// Comma-separated globs a path must match one of; empty for any.
    pub include: String,
    /// Comma-separated globs no path may match.
    pub exclude: String,
    /// Search only these paths, when set.
    pub paths: Option<Vec<String>>,
}

impl TextQuery {
    fn matcher(&self) -> anyhow::Result<Regex> {
        let body = if self.regex {
            self.pattern.clone()
        } else {
            regex::escape(&self.pattern)
        };
        let body = if self.whole_word {
            format!(r"\b(?:{body})\b")
        } else {
            body
        };
        RegexBuilder::new(&body)
            .case_insensitive(!self.case_sensitive)
            .multi_line(true)
            .build()
            .context("invalid regex")
    }

    /// The files to search, by absolute and relative path.
    fn targets(&self, root: &Path) -> anyhow::Result<Vec<(std::path::PathBuf, String)>> {
        let include = globs(&self.include)?;
        let exclude = globs(&self.exclude)?;
        let keep = |rel: &str| {
            include.as_ref().is_none_or(|g| g.is_match(rel))
                && !exclude.as_ref().is_some_and(|g| g.is_match(rel))
        };
        let all = match &self.paths {
            Some(paths) => paths
                .iter()
                .filter_map(|p| Some((crate::files::resolve(root, p).ok()?, p.clone())))
                .collect(),
            None => files(root),
        };
        Ok(all.into_iter().filter(|(_, rel)| keep(rel)).collect())
    }
}

/// `list` as a glob set, or none when it's blank. A glob without a `/` matches at any depth.
fn globs(list: &str) -> anyhow::Result<Option<GlobSet>> {
    let mut b = GlobSetBuilder::new();
    let mut any = false;
    for g in list.split(',').map(str::trim).filter(|g| !g.is_empty()) {
        let g = g.trim_end_matches('/');
        let pattern = if g.contains('/') {
            g.to_string()
        } else {
            format!("**/{g}")
        };
        b.add(Glob::new(&pattern).with_context(|| format!("invalid glob {g}"))?);
        // A directory matches everything under it too.
        b.add(Glob::new(&format!("{pattern}/**"))?);
        any = true;
    }
    Ok(if any { Some(b.build()?) } else { None })
}

/// The lines under `root` matching `q`, by path then line.
pub fn search(root: &Path, q: &TextQuery) -> anyhow::Result<Hits> {
    if q.pattern.is_empty() {
        return Ok(Hits {
            hits: vec![],
            truncated: false,
        });
    }
    let re = q.matcher()?;
    let targets = q.targets(root)?;
    let per_file = crate::par::map(&targets, |(abs, rel)| {
        let text = read(abs)?;
        if !re.is_match(&text) {
            return None;
        }
        Some(file_hits(rel, &text, &re))
    });
    let mut hits = Vec::new();
    let mut truncated = false;
    for h in per_file.into_iter().flatten() {
        if hits.len() + h.len() > MAX_HITS {
            hits.extend(h.into_iter().take(MAX_HITS - hits.len()));
            truncated = true;
            break;
        }
        hits.extend(h);
    }
    Ok(Hits { hits, truncated })
}

fn file_hits(rel: &str, text: &str, re: &Regex) -> Vec<Hit> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let ranges: Vec<_> = re
            .find_iter(line)
            .filter(|m| !m.is_empty())
            .map(|m| (m.start(), m.end()))
            .collect();
        if ranges.is_empty() {
            continue;
        }
        out.push(Hit {
            path: rel.to_string(),
            line: i as u32 + 1,
            text: clip(line).to_string(),
            spans: spans(line, ranges),
            def: false,
            before: None,
            after: None,
        });
    }
    out
}

/// Replaces every match of `q` under `root` with `with`, where `$1` and `${name}` name regex
/// groups. Returns how many files changed.
pub fn replace(root: &Path, q: &TextQuery, with: &str) -> anyhow::Result<u32> {
    if q.pattern.is_empty() {
        return Ok(0);
    }
    let re = q.matcher()?;
    let with = if q.regex {
        with.to_string()
    } else {
        with.replace('$', "$$")
    };
    let mut changed = 0;
    for (abs, rel) in q.targets(root)? {
        let Some(text) = read(&abs) else { continue };
        // Line by line, as search matches, so a match never spans a line break.
        let next: String = text
            .split_inclusive('\n')
            .map(|piece| {
                let body = piece.trim_end_matches(['\n', '\r']);
                let end = &piece[body.len()..];
                format!("{}{end}", re.replace_all(body, with.as_str()))
            })
            .collect();
        if next != text {
            std::fs::write(&abs, next.as_bytes()).with_context(|| format!("write {rel}"))?;
            changed += 1;
        }
    }
    Ok(changed)
}
