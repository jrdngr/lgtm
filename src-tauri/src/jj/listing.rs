//! File listings kept while what they list can't change: a commit's for good, `@`'s until
//! the next operation.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use super::{Jj, checkout};

/// Past this many listings the cache starts over.
const MAX_LISTINGS: usize = 64;

/// Listings by commit id, or for `@` by workspace and the repo's operation heads.
static LISTINGS: LazyLock<Mutex<HashMap<String, Arc<Vec<String>>>>> =
    LazyLock::new(Mutex::default);

impl Jj {
    /// The paths of the files in `commit`, a commit id.
    pub async fn commit_files(&self, commit: &str) -> anyhow::Result<Arc<Vec<String>>> {
        self.listing(commit.to_string(), commit).await
    }

    /// The paths of the files in `@`, as of the last snapshot.
    pub async fn working_copy_files(&self) -> anyhow::Result<Arc<Vec<String>>> {
        let Some(repo) = checkout::repo_dir(&self.root) else {
            return Ok(Arc::new(self.files("@").await?));
        };
        let heads = checkout::op_heads(&repo);
        if heads.is_empty() {
            return Ok(Arc::new(self.files("@").await?));
        }
        let key = format!("{}@{}:{}", repo.display(), self.name(), heads.join(","));
        self.listing(key, "@").await
    }

    async fn listing(&self, key: String, rev: &str) -> anyhow::Result<Arc<Vec<String>>> {
        if let Some(found) = LISTINGS.lock().unwrap().get(&key) {
            return Ok(found.clone());
        }
        let files = Arc::new(self.files(rev).await?);
        let mut cache = LISTINGS.lock().unwrap();
        if cache.len() >= MAX_LISTINGS {
            cache.clear();
        }
        cache.insert(key, files.clone());
        Ok(files)
    }
}
