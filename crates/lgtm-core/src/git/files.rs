//! Reading files out of commits, and the branches above one.

use std::collections::HashMap;

use super::Git;
use crate::jj::Containing;

impl Git {
    /// The contents of each of `paths` at `commit`. Paths not there are left out.
    pub async fn files_at(
        &self,
        commit: &str,
        paths: &[String],
    ) -> anyhow::Result<HashMap<String, Vec<u8>>> {
        if paths.is_empty() {
            return Ok(HashMap::new());
        }
        let commit = self.rev(commit).await?;
        let mut input = Vec::new();
        for path in paths {
            input.extend_from_slice(format!("{commit}:{path}\n").as_bytes());
        }
        let out = self
            .exec_with_input(&["cat-file", "--batch"], &input)
            .await?;
        Ok(paths
            .iter()
            .cloned()
            .zip(split_batch(&out))
            .filter_map(|(p, b)| Some((p, b?)))
            .collect())
    }

    /// File contents at `commit`, or `None` when the file isn't there or there's no commit.
    pub async fn file_if_any(
        &self,
        commit: Option<&str>,
        path: &str,
    ) -> anyhow::Result<Option<Vec<u8>>> {
        let Some(commit) = commit else {
            return Ok(None);
        };
        let mut found = self.files_at(commit, &[path.to_string()]).await?;
        Ok(found.remove(path))
    }

    /// The local branches on the closest commits at or above `rev` that have any.
    pub async fn containing(&self, rev: &str) -> anyhow::Result<Containing> {
        let rev = self.rev(rev).await?;
        let out = self
            .read(&[
                "for-each-ref",
                "--format=%(refname:short)%00%(objectname)",
                "--contains",
                &rev,
                "refs/heads",
            ])
            .await?;
        let mut best: Vec<String> = Vec::new();
        let mut best_ahead = usize::MAX;
        for line in out.lines() {
            let Some((name, tip)) = line.split_once('\0') else {
                continue;
            };
            let count = self
                .read(&["rev-list", "--count", &format!("{rev}..{tip}")])
                .await?;
            let ahead: usize = count.trim().parse().unwrap_or(usize::MAX);
            if ahead < best_ahead {
                (best, best_ahead) = (vec![], ahead);
            }
            if ahead == best_ahead {
                best.push(name.to_string());
            }
        }
        Ok(Containing {
            bookmarks: best,
            workspaces: vec![],
        })
    }
}

/// Each object of `git cat-file --batch` output, in order, `None` where it's missing.
fn split_batch(out: &[u8]) -> Vec<Option<Vec<u8>>> {
    let mut found = Vec::new();
    let mut at = 0;
    while at < out.len() {
        let Some(end) = out[at..].iter().position(|&b| b == b'\n') else {
            break;
        };
        let header = String::from_utf8_lossy(&out[at..at + end]).into_owned();
        at += end + 1;
        let mut parts = header.split(' ');
        let size = match (parts.next(), parts.next(), parts.next()) {
            (Some(_), Some("blob"), Some(size)) => size.parse::<usize>().ok(),
            _ => None,
        };
        match size {
            Some(size) if at + size <= out.len() => {
                found.push(Some(out[at..at + size].to_vec()));
                // The content and its closing newline.
                at += size + 1;
            }
            _ => found.push(None),
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{git, repo, write};
    use super::*;

    #[test]
    fn batch_output_splits_into_files_and_missing_ones() {
        let out = b"abc blob 3\nx\ny\nfoo:bar missing\ndef blob 0\n\n";
        assert_eq!(
            split_batch(out),
            [Some(b"x\ny".to_vec()), None, Some(vec![])]
        );
    }

    #[tokio::test]
    async fn reads_files_at_the_working_copy_and_leaves_out_missing_ones() {
        let (dir, g) = repo();
        write(dir.path(), "a.txt", "now\n");
        let paths = ["a.txt".to_string(), "gone".to_string()];
        let found = g.files_at("@", &paths).await.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found["a.txt"], b"now\n");
        let base = g.file_if_any(Some("@-"), "a.txt").await.unwrap();
        assert_eq!(base.as_deref(), Some(&b"a\n"[..]));
        assert_eq!(g.file_if_any(Some("@-"), "gone").await.unwrap(), None);
    }

    #[tokio::test]
    async fn containing_names_the_closest_branch_above() {
        let (dir, g) = repo();
        let d = dir.path();
        let first = git(d, &["rev-parse", "HEAD"]);
        git(d, &["switch", "-q", "-c", "near"]);
        write(d, "b", "b\n");
        git(d, &["add", "."]);
        git(d, &["commit", "-q", "-m", "b"]);
        git(d, &["switch", "-q", "-c", "far"]);
        write(d, "c", "c\n");
        git(d, &["add", "."]);
        git(d, &["commit", "-q", "-m", "c"]);
        let found = g.containing(&first).await.unwrap();
        assert_eq!(found.bookmarks, ["main"]);
        let near = git(d, &["rev-parse", "near"]);
        assert_eq!(g.containing(&near).await.unwrap().bookmarks, ["near"]);
    }
}
