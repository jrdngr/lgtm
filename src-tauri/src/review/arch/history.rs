//! Architecture history entries by commit. A commit's contents never change, so its entry
//! holds while its parent, the working copy it reads other files from, and the review's files
//! are the same.

use std::collections::HashMap;
use std::sync::Mutex;

use super::HistoryDto;

pub type Key = [u8; 32];

/// Past this many commits the cache starts over.
const MAX_COMMITS: usize = 1024;

/// Each commit's entry, `None` for one that didn't change the architecture.
#[derive(Default)]
pub struct HistoryCache(Mutex<HashMap<Key, Option<HistoryDto>>>);

pub fn key(commit: &str, parent: Option<&str>, seen_from: &str, reviewed: &[String]) -> Key {
    let mut h = blake3::Hasher::new();
    for part in [commit, parent.unwrap_or_default(), seen_from]
        .into_iter()
        .chain(reviewed.iter().map(String::as_str))
    {
        h.update(&(part.len() as u64).to_le_bytes());
        h.update(part.as_bytes());
    }
    *h.finalize().as_bytes()
}

impl HistoryCache {
    pub fn get(&self, key: &Key) -> Option<Option<HistoryDto>> {
        self.0.lock().unwrap().get(key).cloned()
    }

    pub fn put(&self, key: Key, entry: Option<HistoryDto>) {
        let mut cache = self.0.lock().unwrap();
        if cache.len() >= MAX_COMMITS {
            cache.clear();
        }
        cache.insert(key, entry);
    }
}
