//! Where a design's spec files are: a revision resolved to one commit, and a hash of the
//! design's files there that says whether its map is still fresh.

use std::collections::HashMap;

use anyhow::{Context, bail};
use serde::Serialize;
use specta::Type;

use crate::jj::Jj;

/// The commit a design's revision resolves to now.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedDto {
    pub commit: String,
    pub change_id: String,
    pub description: String,
    /// Set when the revision is a local bookmark's name.
    pub bookmark: Option<String>,
}

/// Resolves `revision`, any revset of one commit or a bookmark name a revset can't parse.
pub(super) async fn resolve(jj: &Jj, revision: &str) -> anyhow::Result<ResolvedDto> {
    let changes = match jj.changes(revision).await {
        Ok(c) => c,
        Err(_) => jj
            .changes(&serde_json::to_string(revision)?)
            .await
            .with_context(|| format!("no revision `{revision}`"))?,
    };
    let [c] = changes.as_slice() else {
        bail!("`{revision}` is {} commits, not one", changes.len());
    };
    let bookmark = c
        .bookmarks
        .iter()
        .map(|b| b.trim_end_matches('*'))
        .find(|b| *b == revision)
        .map(String::from);
    Ok(ResolvedDto {
        commit: c.commit_id.clone(),
        change_id: c.change_id.clone(),
        description: c.description.clone(),
        bookmark,
    })
}

/// What to store for a revision: a bookmark's name as given, else the change id, which
/// survives rewrites where `@` or a commit id would not.
pub(super) fn stored(r: &ResolvedDto) -> String {
    r.bookmark.clone().unwrap_or_else(|| r.change_id.clone())
}

/// One hash over `files` from their per-file `hashes`; a file missing from `hashes` counts too.
pub(super) fn files_hash(files: &[String], hashes: &HashMap<String, String>) -> String {
    let mut sorted: Vec<&String> = files.iter().collect();
    sorted.sort();
    let mut h = blake3::Hasher::new();
    for f in sorted {
        h.update(f.as_bytes());
        h.update(b"\0");
        h.update(hashes.get(f).map_or(&b"-"[..], |x| x.as_bytes()));
        h.update(b"\n");
    }
    h.finalize().to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hashes(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(p, h)| (p.to_string(), h.to_string()))
            .collect()
    }

    #[test]
    fn files_hash_changes_only_with_the_designs_own_files() {
        let mine = ["phase1.md".to_string(), "shared.md".to_string()];
        let before = hashes(&[("phase1.md", "a"), ("shared.md", "b"), ("phase2.md", "c")]);
        let other_edited = hashes(&[("phase1.md", "a"), ("shared.md", "b"), ("phase2.md", "z")]);
        let mine_edited = hashes(&[("phase1.md", "y"), ("shared.md", "b"), ("phase2.md", "c")]);
        let reordered = [mine[1].clone(), mine[0].clone()];
        assert_eq!(files_hash(&mine, &before), files_hash(&mine, &other_edited));
        assert_eq!(files_hash(&mine, &before), files_hash(&reordered, &before));
        assert_ne!(files_hash(&mine, &before), files_hash(&mine, &mine_edited));
    }
}
