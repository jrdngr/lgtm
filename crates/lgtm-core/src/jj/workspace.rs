//! Workspace commands. Adding and forgetting workspaces are writes on the repo, so they take
//! the writer lock of the `Jj` they run through.

use std::path::{Path, PathBuf};

use super::Jj;

/// A workspace as `jj workspace list` reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceInfo {
    pub name: String,
    pub root: PathBuf,
    /// The change of the workspace's `@`.
    pub change_id: String,
    /// Length of the shortest prefix of `change_id` that's unique in the repo.
    pub unique: u32,
    /// The whole description of the workspace's `@`.
    pub description: String,
}

const LIST_TEMPLATE: &str = r#"name ++ "\t" ++ self.root() ++ "\t" ++ target.change_id().short(8) ++ "\t"
  ++ target.change_id().shortest().prefix().len() ++ "\t" ++ target.description().escape_json() ++ "\n""#;

fn parse_workspaces(out: &str) -> Vec<WorkspaceInfo> {
    out.lines()
        .filter_map(|l| {
            let mut parts = l.splitn(5, '\t');
            Some(WorkspaceInfo {
                name: parts.next()?.to_string(),
                root: PathBuf::from(parts.next()?),
                change_id: parts.next()?.to_string(),
                unique: parts.next()?.parse().ok()?,
                description: serde_json::from_str(parts.next()?).ok()?,
            })
        })
        .collect()
}

impl Jj {
    pub async fn workspaces(&self) -> anyhow::Result<Vec<WorkspaceInfo>> {
        Ok(parse_workspaces(
            &self
                .read(&["workspace", "list", "-T", LIST_TEMPLATE])
                .await?,
        ))
    }

    /// Adds workspace `name` at `path` with a new working-copy commit on `rev`.
    pub async fn workspace_add(&self, name: &str, path: &Path, rev: &str) -> anyhow::Result<()> {
        let _guard = self.writer.lock().await;
        let path = path.to_string_lossy();
        self.exec(
            &["workspace", "add", "--name", name, "-r", rev, &path],
            false,
        )
        .await
        .map(drop)
    }

    /// Stops tracking workspace `name`. Its working-copy commit stays in the repo.
    pub async fn workspace_forget(&self, name: &str) -> anyhow::Result<()> {
        let _guard = self.writer.lock().await;
        self.exec(&["workspace", "forget", name], false)
            .await
            .map(drop)
    }
}

/// Runs jj on the repo at `root` outside the async runtime (for `lgtm gc`).
fn exec_blocking(root: &Path, args: &[&str], what: &str) -> anyhow::Result<String> {
    let out = std::process::Command::new("jj")
        .arg("-R")
        .arg(root)
        .args(["--no-pager", "--color=never", "--ignore-working-copy"])
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()?;
    anyhow::ensure!(
        out.status.success(),
        "jj {what}: {}",
        String::from_utf8_lossy(&out.stderr).trim()
    );
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// `jj workspace list`, outside the async runtime.
pub fn workspaces_blocking(root: &Path) -> anyhow::Result<Vec<WorkspaceInfo>> {
    let out = exec_blocking(
        root,
        &["workspace", "list", "-T", LIST_TEMPLATE],
        "workspace list",
    )?;
    Ok(parse_workspaces(&out))
}

/// `jj workspace forget`, outside the async runtime.
pub fn forget_blocking(root: &Path, name: &str) -> anyhow::Result<()> {
    let what = format!("workspace forget {name}");
    exec_blocking(root, &["workspace", "forget", name], &what).map(drop)
}

/// Local bookmarks matching `glob`, outside the async runtime.
pub fn bookmarks_blocking(root: &Path, glob: &str) -> anyhow::Result<Vec<String>> {
    let pattern = format!("glob:{glob}");
    let args = ["bookmark", "list", "-T", r#"name ++ "\n""#, &pattern];
    let mut names: Vec<String> = exec_blocking(root, &args, "bookmark list")?
        .lines()
        .map(str::to_string)
        .collect();
    names.dedup();
    Ok(names)
}

pub fn bookmark_delete_blocking(root: &Path, name: &str) -> anyhow::Result<()> {
    let pattern = format!("exact:{name}");
    let what = format!("bookmark delete {name}");
    exec_blocking(root, &["bookmark", "delete", &pattern], &what).map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_workspace_list() {
        let got = parse_workspaces(
            "default\t/r/main\tabc\t1\t\"\"\nfix\t/ws/r/fix\txyz\t2\t\"Fix:\\ta\\n\\nWhy.\\n\"\n",
        );
        assert_eq!(got[0].description, "");
        assert_eq!(got[1].root, PathBuf::from("/ws/r/fix"));
        assert_eq!(got[1].unique, 2);
        assert_eq!(got[1].description, "Fix:\ta\n\nWhy.\n");
    }
}
