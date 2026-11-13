//! Diffs between commits and the commits in a review, from git.

use anyhow::bail;

use super::Git;
use crate::diff::{self, FileStatus};
use crate::jj::{Change, DiffPath, FileDiff, Range};

/// What `git diff --raw` says of one file.
#[derive(Debug, PartialEq, Eq)]
struct Raw {
    status: FileStatus,
    old_exec: bool,
    new_exec: bool,
    old_path: Option<String>,
    path: String,
}

/// Parses `git diff --raw -z` output.
fn parse_raw(out: &str) -> Vec<Raw> {
    let mut fields = out.split('\0');
    let mut found = Vec::new();
    while let Some(head) = fields.next() {
        let Some(head) = head.strip_prefix(':') else {
            continue;
        };
        let parts: Vec<&str> = head.split(' ').collect();
        let [old_mode, new_mode, _, _, letter] = parts[..] else {
            continue;
        };
        let status = match letter.chars().next() {
            Some('A') => FileStatus::Added,
            Some('D') => FileStatus::Removed,
            Some('R') => FileStatus::Renamed,
            Some('C') => FileStatus::Copied,
            _ => FileStatus::Modified,
        };
        let moved = matches!(status, FileStatus::Renamed | FileStatus::Copied);
        let Some(first) = fields.next() else { break };
        let (old_path, path) = match moved {
            true => match fields.next() {
                Some(second) => (Some(first.to_string()), second.to_string()),
                None => break,
            },
            false => (None, first.to_string()),
        };
        found.push(Raw {
            status,
            old_exec: old_mode == "100755",
            new_exec: new_mode == "100755",
            old_path,
            path,
        });
    }
    found
}

/// Format of `git log` for [`Change`]s: the fields are `\x1f`-separated, one commit a line.
const CHANGE_FORMAT: &str = "--format=%H%x1f%D%x1f%s";

impl Git {
    /// `range`'s two sides as git names them, the empty tree standing in for no base.
    async fn sides(&self, range: &Range) -> anyhow::Result<(String, String)> {
        let from = match &range.from {
            Some(from) => from.clone(),
            None => self.empty_tree().await?,
        };
        Ok((from, range.to.clone()))
    }

    async fn empty_tree(&self) -> anyhow::Result<String> {
        let out = self
            .exec_with_input(&["hash-object", "-t", "tree", "--stdin"], b"")
            .await?;
        Ok(String::from_utf8_lossy(&out).trim().to_string())
    }

    async fn raw(&self, from: &str, to: &str) -> anyhow::Result<Vec<Raw>> {
        let out = self
            .read(&["diff", "--raw", "-z", "-M", "--no-abbrev", from, to, "--"])
            .await?;
        Ok(parse_raw(&out))
    }

    pub async fn diff(&self, range: &Range) -> anyhow::Result<Vec<FileDiff>> {
        let (from, to) = self.sides(range).await?;
        let patch_args = [
            "diff",
            "-M",
            "--no-ext-diff",
            "--src-prefix=a/",
            "--dst-prefix=b/",
            &from,
            &to,
            "--",
        ];
        let (raw, patch) = tokio::try_join!(self.raw(&from, &to), self.read(&patch_args))?;
        let patch = diff::parse_git(&patch)?;
        if raw.len() != patch.len() {
            bail!(
                "git diff listed {} files and patched {}",
                raw.len(),
                patch.len()
            );
        }
        // Both list the files in the same order.
        Ok(raw
            .into_iter()
            .zip(patch)
            .map(|(r, g)| FileDiff {
                old_path: r.old_path,
                path: r.path,
                status: r.status,
                binary: g.binary,
                conflict: false,
                old_exec: r.old_exec,
                new_exec: r.new_exec,
                blobs: g.extended.iter().find_map(|l| {
                    let ids = l.strip_prefix("index ")?;
                    Some(ids.split(' ').next().unwrap_or(ids).to_string())
                }),
                hunks: g.hunks,
            })
            .collect())
    }

    /// The files `range` changes, without reading their patches.
    pub async fn diff_paths(&self, range: &Range) -> anyhow::Result<Vec<DiffPath>> {
        let (from, to) = self.sides(range).await?;
        Ok(self
            .raw(&from, &to)
            .await?
            .into_iter()
            .map(|r| DiffPath {
                old_path: r.old_path,
                path: r.path,
                status: r.status,
                conflict: false,
            })
            .collect())
    }

    /// The commits of the review `name`, oldest first.
    pub async fn changes(&self, name: &str) -> anyhow::Result<Vec<Change>> {
        let range = self.resolve(name).await?;
        let span = match &range.from {
            Some(from) => format!("{from}..{}", range.to),
            None => range.to.clone(),
        };
        let wc = self.rev(super::WORKING_COPY).await.ok();
        let out = self
            .read(&[
                "log",
                "--reverse",
                "--decorate=full",
                CHANGE_FORMAT,
                &span,
                "--",
            ])
            .await?;
        let mut changes = Vec::new();
        for line in out.lines() {
            let f: Vec<&str> = line.splitn(3, '\x1f').collect();
            let [id, refs, subject] = f[..] else { continue };
            let working_copy = wc.as_deref() == Some(id);
            changes.push(Change {
                change_id: id.to_string(),
                commit_id: id.to_string(),
                unique: super::log::SHORT,
                working_copy,
                conflict: false,
                empty: self.is_empty(id).await,
                divergent: false,
                bookmarks: super::log::local_branches(refs),
                description: match working_copy {
                    true => String::new(),
                    false => subject.to_string(),
                },
            });
        }
        Ok(changes)
    }

    /// Whether `commit` changes nothing from its first parent.
    async fn is_empty(&self, commit: &str) -> bool {
        let tree = |rev: String| async move {
            let spec = format!("{rev}^{{tree}}");
            let out = self
                .read(&["rev-parse", "--verify", "--quiet", &spec])
                .await;
            out.ok().map(|t| t.trim().to_string())
        };
        let (mine, parent) = tokio::join!(tree(commit.to_string()), tree(format!("{commit}^")));
        mine.is_some() && mine == parent
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{git, repo, write};
    use super::*;

    #[test]
    fn raw_records_give_renames_both_paths_and_modes_the_exec_bit() {
        let out = ":100644 100755 a b M\0x.sh\0:100644 100644 a b R090\0old\0new\0:000000 100644 0 b A\0n\0";
        let got = parse_raw(out);
        assert_eq!(got.len(), 3);
        assert!(!got[0].old_exec && got[0].new_exec);
        assert_eq!(
            (
                got[1].old_path.as_deref(),
                got[1].path.as_str(),
                got[1].status
            ),
            (Some("old"), "new", FileStatus::Renamed)
        );
        assert_eq!(got[2].status, FileStatus::Added);
    }

    #[tokio::test]
    async fn diffs_the_working_copy_with_unusual_names_and_non_utf8_content() {
        let (dir, g) = repo();
        std::fs::write(dir.path().join("üni"), b"caf\xe9\n").unwrap();
        write(dir.path(), "quo\"te", "x\n");
        write(dir.path(), "a.txt", "a\nb\n");
        let mut files = g.diff(&g.resolve("@").await.unwrap()).await.unwrap();
        files.sort_by(|a, b| a.path.cmp(&b.path));
        let paths: Vec<_> = files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["a.txt", "quo\"te", "üni"]);
        assert_eq!(files[0].hunks[0].rows.len(), 2);
        assert_eq!(files[1].status, FileStatus::Added);
    }

    #[tokio::test]
    async fn a_root_commit_diffs_against_nothing_and_renames_keep_the_old_path() {
        let (dir, g) = repo();
        let first = g.diff(&g.resolve("main").await.unwrap()).await.unwrap();
        assert_eq!(first[0].status, FileStatus::Added);
        git(dir.path(), &["mv", "a.txt", "b.txt"]);
        let moved = g.diff_paths(&g.resolve("@").await.unwrap()).await.unwrap();
        assert_eq!(moved[0].old_path.as_deref(), Some("a.txt"));
        assert_eq!(moved[0].status, FileStatus::Renamed);
    }

    #[tokio::test]
    async fn changes_list_a_branch_oldest_first_with_its_name_and_the_working_copy_last() {
        let (dir, g) = repo();
        let d = dir.path();
        git(d, &["switch", "-q", "-c", "topic"]);
        write(d, "b", "b\n");
        git(d, &["add", "."]);
        git(d, &["commit", "-q", "-m", "add b"]);
        let changes = g.changes("topic").await.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].description, "add b");
        assert_eq!(changes[0].bookmarks, ["topic"]);
        let wc = g.changes("@").await.unwrap();
        assert!(wc[0].working_copy && wc[0].empty);
    }
}
