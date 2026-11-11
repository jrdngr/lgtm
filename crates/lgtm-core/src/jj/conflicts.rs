//! What a conflict's sides are in jj: their commits' bookmarks, and whether they removed or
//! moved a file.

use super::Jj;

/// A commit a conflict marker names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkerCommit {
    pub commit_id: String,
    /// Length of the shortest unique prefix of its change id.
    pub unique: u32,
    pub bookmarks: Vec<String>,
}

impl Jj {
    /// The commits whose ids start with `ids`. Ids jj can't find are left out.
    pub async fn marker_commits(&self, ids: &[String]) -> anyhow::Result<Vec<MarkerCommit>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let revset = ids
            .iter()
            .map(|id| format!("present(commit_id({id}))"))
            .collect::<Vec<_>>()
            .join("|");
        let template = r#"commit_id ++ "\t" ++ change_id.shortest().prefix().len() ++ "\t"
          ++ local_bookmarks.map(|b| b.name()).join(" ") ++ "\n""#;
        let out = self
            .exec(&["log", "--no-graph", "-r", &revset, "-T", template], true)
            .await?;
        Ok(String::from_utf8_lossy(&out)
            .lines()
            .filter_map(|line| {
                let mut parts = line.split('\t');
                Some(MarkerCommit {
                    commit_id: parts.next()?.to_string(),
                    unique: parts.next()?.parse().ok()?,
                    bookmarks: parts
                        .next()
                        .unwrap_or_default()
                        .split_whitespace()
                        .map(String::from)
                        .collect(),
                })
            })
            .collect())
    }

    /// The repo-relative paths of the files with conflicts in `rev`, as of the last snapshot.
    pub async fn conflicted_files(&self, rev: &str) -> anyhow::Result<Vec<String>> {
        let template = r#"if(conflict, path ++ "\n")"#;
        let out = self
            .read_lossy(&["file", "list", "-r", rev, "-T", template])
            .await?;
        Ok(out.lines().map(str::to_string).collect())
    }

    /// Whether `path` is in `commit`.
    pub async fn has_file(&self, commit: &str, path: &str) -> anyhow::Result<bool> {
        let found = self.files_at(commit, &[path.to_string()]).await?;
        Ok(found.contains_key(path))
    }

    /// Where `path` went between commits `from` and `to`, when jj sees it renamed.
    pub async fn moved_to(
        &self,
        from: &str,
        to: &str,
        path: &str,
    ) -> anyhow::Result<Option<String>> {
        let template = r#"if(status == "renamed", source.path() ++ "\t" ++ target.path() ++ "\n")"#;
        let out = self
            .exec(&["diff", "--from", from, "--to", to, "-T", template], true)
            .await?;
        Ok(String::from_utf8_lossy(&out).lines().find_map(|line| {
            let (source, target) = line.split_once('\t')?;
            (source == path).then(|| target.to_string())
        }))
    }
}
