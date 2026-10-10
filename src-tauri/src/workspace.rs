//! A jj workspace at runtime: its own `Jj` (so `@` is its working copy), snapshot
//! state, and watcher.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::jj::Jj;
use crate::watch::Watch;

pub struct Workspace {
    /// `None` for the repo's main workspace.
    pub name: Option<String>,
    pub root: PathBuf,
    pub jj: Arc<Jj>,
    /// jj's message while the working copy is stale.
    pub stale: Arc<Mutex<Option<String>>>,
    pub watch: Mutex<Option<Watch>>,
}

impl Workspace {
    pub fn new(name: Option<String>, root: PathBuf, jj: Jj) -> Self {
        Self {
            name,
            root,
            jj: Arc::new(jj),
            stale: Arc::default(),
            watch: Mutex::default(),
        }
    }

    /// Stored with review sessions: empty for the main workspace.
    pub fn key(&self) -> &str {
        self.name.as_deref().unwrap_or("")
    }

    /// What marks made in a review of `revset` here record as their source review.
    pub fn label(&self, revset: &str) -> String {
        match &self.name {
            None => revset.to_string(),
            Some(name) => format!("{revset} in {name}"),
        }
    }

    /// Snapshots the working copy, recording rather than failing when it's stale.
    pub async fn snapshot(&self) -> anyhow::Result<()> {
        *self.stale.lock().unwrap() = self.jj.snapshot_or_stale().await?;
        Ok(())
    }
}
