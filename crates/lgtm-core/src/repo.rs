//! A repo lgtm reviews: jj's, or git's where there's no jj. What both can do goes through
//! here; what only jj does is on [`Repo::jj`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::git::Git;
use crate::jj::{Change, Containing, DiffPath, FileDiff, Jj, LogRow, Range, Version};

pub enum Repo {
    Jj(Jj),
    Git(Git),
}

/// The workspace or working tree holding `dir`: the nearest folder above it, `dir` too,
/// with a `.jj` directory or a `.git` directory or file.
pub fn root_of(dir: &Path) -> Option<PathBuf> {
    dir.ancestors()
        .find(|p| p.join(".jj").is_dir() || p.join(".git").exists())
        .map(Path::to_path_buf)
}

impl Repo {
    /// The repo at `root`, a workspace or working tree root: jj's when it has a `.jj`.
    pub fn open(root: &Path) -> Self {
        match root.join(".jj").is_dir() {
            true => Repo::Jj(Jj::new(root)),
            false => Repo::Git(Git::new(root)),
        }
    }

    /// jj, when the repo is jj's.
    pub fn jj(&self) -> Option<&Jj> {
        match self {
            Repo::Jj(jj) => Some(jj),
            Repo::Git(_) => None,
        }
    }

    pub fn is_git(&self) -> bool {
        matches!(self, Repo::Git(_))
    }

    /// The log to pick reviews from: `revset`'s or the default in jj, git's own pick in git.
    pub async fn log_rows(&self, revset: Option<&str>) -> anyhow::Result<Vec<LogRow>> {
        match self {
            Repo::Jj(jj) => jj.log_rows(revset).await,
            Repo::Git(git) => git.log_rows().await,
        }
    }

    /// The diff endpoints of the review `name`.
    pub async fn resolve(&self, name: &str) -> anyhow::Result<Range> {
        match self {
            Repo::Jj(jj) => jj.resolve(name).await,
            Repo::Git(git) => git.resolve(name).await,
        }
    }

    /// The commits of the review `name`, oldest first.
    pub async fn changes(&self, name: &str) -> anyhow::Result<Vec<Change>> {
        match self {
            Repo::Jj(jj) => jj.changes(name).await,
            Repo::Git(git) => git.changes(name).await,
        }
    }

    pub async fn diff(&self, range: &Range) -> anyhow::Result<Vec<FileDiff>> {
        match self {
            Repo::Jj(jj) => jj.diff(range).await,
            Repo::Git(git) => git.diff(range).await,
        }
    }

    pub async fn diff_paths(&self, range: &Range) -> anyhow::Result<Vec<DiffPath>> {
        match self {
            Repo::Jj(jj) => jj.diff_paths(range).await,
            Repo::Git(git) => git.diff_paths(range).await,
        }
    }

    pub async fn files_at(
        &self,
        commit: &str,
        paths: &[String],
    ) -> anyhow::Result<HashMap<String, Vec<u8>>> {
        match self {
            Repo::Jj(jj) => jj.files_at(commit, paths).await,
            Repo::Git(git) => git.files_at(commit, paths).await,
        }
    }

    pub async fn file_if_any(
        &self,
        commit: Option<&str>,
        path: &str,
    ) -> anyhow::Result<Option<Vec<u8>>> {
        match self {
            Repo::Jj(jj) => jj.file_if_any(commit, path).await,
            Repo::Git(git) => git.file_if_any(commit, path).await,
        }
    }

    pub async fn containing(&self, rev: &str) -> anyhow::Result<Containing> {
        match self {
            Repo::Jj(jj) => jj.containing(rev).await,
            Repo::Git(git) => git.containing(rev).await,
        }
    }

    pub async fn commit_ids(&self, rev: &str) -> anyhow::Result<Vec<String>> {
        match self {
            Repo::Jj(jj) => jj.commit_ids(rev).await,
            Repo::Git(git) => git.commit_ids(rev).await,
        }
    }

    pub async fn remotes(&self) -> anyhow::Result<Vec<(String, String)>> {
        match self {
            Repo::Jj(jj) => jj.remotes().await,
            Repo::Git(git) => git.remotes().await,
        }
    }

    /// The versions of the change `rev`, newest first. git keeps none.
    pub async fn versions(&self, rev: &str) -> anyhow::Result<Vec<Version>> {
        match self {
            Repo::Jj(jj) => jj.versions(rev).await,
            Repo::Git(_) => Ok(vec![]),
        }
    }

    /// Takes in the working copy's edits, as the watcher does. git's working copy is read
    /// fresh each time, so there's nothing to take.
    pub async fn background_snapshot(&self) {
        if let Repo::Jj(jj) = self {
            let _ = jj.background_snapshot().await;
        }
    }

    /// The files with conflicts in the working copy.
    pub async fn conflicted_files(&self) -> anyhow::Result<Vec<String>> {
        match self {
            Repo::Jj(jj) => {
                let _ = jj.snapshot().await;
                jj.conflicted_files("@").await
            }
            Repo::Git(git) => git.conflicted_files().await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_nearest_jj_or_git_folder_holds_a_dir() {
        let dir = tempfile::tempdir().unwrap();
        let outer = dir.path().join("outer");
        let inner = outer.join("inner");
        std::fs::create_dir_all(outer.join(".jj")).unwrap();
        std::fs::create_dir_all(inner.join("src")).unwrap();
        std::fs::write(inner.join(".git"), "gitdir: elsewhere\n").unwrap();
        assert_eq!(root_of(&inner.join("src")), Some(inner.clone()));
        assert_eq!(root_of(&outer), Some(outer.clone()));
        assert!(matches!(Repo::open(&inner), Repo::Git(_)));
        assert!(matches!(Repo::open(&outer), Repo::Jj(_)));
    }
}
