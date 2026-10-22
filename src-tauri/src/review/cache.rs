//! Built files from recent loads, reused when a file's diff hasn't changed. A refresh after
//! an edit then only reloads and re-parses the files that changed.

use std::collections::HashMap;

use super::build::{FileDto, ReviewFile, Sides};
use crate::config::Config;
use crate::jj::FileDiff;

pub type Key = [u8; 32];

/// Past this many files the cache starts over, so loads of many reviews (every workspace's
/// progress) can share it without it growing forever.
const MAX_FILES: usize = 4096;

#[derive(Default)]
pub struct FileCache(HashMap<Key, (ReviewFile, FileDto)>);

/// Same contents, same patch, same settings: the same build. Blob ids name both sides'
/// contents, so the side commits can differ, as after an edit elsewhere or a rebase. Without
/// them (a mode-only change) the base commit stands in for the old side. An interdiff's old
/// side isn't a commit, so its base is the new commit and its builds are only reused while
/// that stays the same.
pub fn key(sides: &Sides, interdiff: bool, d: &FileDiff, config: &Config) -> Key {
    let mut h = blake3::Hasher::new();
    let mut field = |bytes: &[u8]| {
        h.update(&(bytes.len() as u64).to_le_bytes());
        h.update(bytes);
    };
    let base = match (&sides.old, interdiff) {
        (_, true) => &sides.new,
        _ if d.blobs.is_some() => "",
        (Some(old), false) => old,
        (None, false) => "",
    };
    field(base.as_bytes());
    field(d.path.as_bytes());
    field(d.old_path.as_deref().unwrap_or("").as_bytes());
    field(config.source.as_bytes());
    field(
        format!(
            "{:?} {} {} {} {} {:?}",
            d.status, d.binary, d.conflict, d.old_exec, d.new_exec, d.blobs
        )
        .as_bytes(),
    );
    field(&serde_json::to_vec(&d.hunks).unwrap_or_default());
    *h.finalize().as_bytes()
}

impl FileCache {
    /// Takes out a cached build, pointing its sides at `sides`' commits.
    pub fn take(&mut self, key: &Key, sides: &Sides) -> Option<(ReviewFile, FileDto)> {
        let (mut file, dto) = self.0.remove(key)?;
        file.old.commit = sides.old.clone();
        file.new.commit = Some(sides.new.clone());
        Some((file, dto))
    }

    pub fn put(&mut self, entries: impl IntoIterator<Item = (Key, (ReviewFile, FileDto))>) {
        if self.0.len() > MAX_FILES {
            self.0.clear();
        }
        self.0.extend(entries);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::FileStatus;

    fn diff(blobs: Option<&str>) -> FileDiff {
        FileDiff {
            path: "a.rs".into(),
            old_path: None,
            status: FileStatus::Modified,
            binary: false,
            conflict: false,
            old_exec: false,
            new_exec: false,
            blobs: blobs.map(str::to_string),
            hunks: vec![],
        }
    }

    fn sides(old: &str, new: &str) -> Sides {
        Sides {
            old: Some(old.into()),
            new: new.into(),
        }
    }

    #[test]
    fn a_rebase_keeps_the_key_only_when_blob_ids_name_the_contents() {
        let config = Config::default();
        let before = sides("base1", "head1");
        let rebased = sides("base2", "head2");
        let with = diff(Some("abc..def"));
        assert_eq!(
            key(&before, false, &with, &config),
            key(&rebased, false, &with, &config)
        );
        let without = diff(None);
        assert_ne!(
            key(&before, false, &without, &config),
            key(&rebased, false, &without, &config)
        );
        // An interdiff's builds hold only for its own new commit.
        assert_ne!(
            key(&before, true, &with, &config),
            key(&rebased, true, &with, &config)
        );
    }
}
