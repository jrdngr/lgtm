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

/// Same base, same patch, same settings: the same file contents, so the same build. The new
/// side's commit can differ, since an edit elsewhere makes a new working-copy commit. An
/// interdiff's old side isn't a commit, so its base is the new commit and its builds are only
/// reused while that stays the same.
pub fn key(sides: &Sides, interdiff: bool, d: &FileDiff, config: &Config) -> Key {
    let mut h = blake3::Hasher::new();
    let mut field = |bytes: &[u8]| {
        h.update(&(bytes.len() as u64).to_le_bytes());
        h.update(bytes);
    };
    let base = match (&sides.old, interdiff) {
        (_, true) => &sides.new,
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
    /// Takes out a cached build, pointing its new side at `new_commit`.
    pub fn take(&mut self, key: &Key, new_commit: &str) -> Option<(ReviewFile, FileDto)> {
        let (mut file, dto) = self.0.remove(key)?;
        file.new.commit = Some(new_commit.to_string());
        Some((file, dto))
    }

    pub fn put(&mut self, entries: impl IntoIterator<Item = (Key, (ReviewFile, FileDto))>) {
        if self.0.len() > MAX_FILES {
            self.0.clear();
        }
        self.0.extend(entries);
    }
}
