//! Reading a diff's files at both of its commits, for building their review state.

use std::collections::HashMap;

use super::file::Sides;
use crate::diff::FileStatus;
use crate::jj::FileDiff;
use crate::repo::Repo;

/// Files larger than this are left unread, as if binary.
pub const MAX_FILE_BYTES: usize = 4 << 20;

/// Old and new contents per file, `None` where a side is absent, binary, or too large.
/// Each side is one batched read.
pub async fn read(
    repo: &Repo,
    sides: &Sides,
    diffs: &[&FileDiff],
) -> anyhow::Result<Vec<(Option<String>, Option<String>)>> {
    let old_path = |d: &FileDiff| d.old_path.clone().unwrap_or_else(|| d.path.clone());
    let old_paths: Vec<String> = diffs
        .iter()
        .filter(|d| !d.binary && d.status != FileStatus::Added)
        .map(|d| old_path(d))
        .collect();
    let new_paths: Vec<String> = diffs
        .iter()
        .filter(|d| !d.binary && d.status != FileStatus::Removed)
        .map(|d| d.path.clone())
        .collect();
    let old = async {
        match sides.old.as_deref() {
            Some(commit) if !old_paths.is_empty() => repo.files_at(commit, &old_paths).await,
            // The root commit has no files.
            _ => Ok(HashMap::new()),
        }
    };
    let new = async {
        if new_paths.is_empty() {
            return Ok(HashMap::new());
        }
        repo.files_at(&sides.new, &new_paths).await
    };
    let (old, mut new) = tokio::try_join!(old, new)?;
    let text = |bytes: Option<Vec<u8>>| {
        bytes
            .filter(|b| b.len() <= MAX_FILE_BYTES)
            .and_then(|b| String::from_utf8(b).ok())
    };
    Ok(diffs
        .iter()
        .map(|d| {
            let o = (!d.binary && d.status != FileStatus::Added)
                // A file copied twice shares its old side, so it's cloned, not taken.
                .then(|| old.get(&old_path(d)).cloned())
                .flatten();
            let n = (!d.binary && d.status != FileStatus::Removed)
                .then(|| new.remove(&d.path))
                .flatten();
            (text(o), text(n))
        })
        .collect())
}
