//! Workspace commands. Adding and forgetting workspaces are writes on the repo, so they take
//! the writer lock of the `Jj` they run through.

use std::path::{Path, PathBuf};

use super::Jj;

/// A workspace as `jj workspace list` reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceInfo {
    pub name: String,
    pub root: PathBuf,
    /// The change and first description line of the workspace's `@`.
    pub change_id: String,
    pub description: String,
}

const LIST_TEMPLATE: &str = r#"name ++ "\t" ++ self.root() ++ "\t" ++ target.change_id().short(8) ++ "\t" ++ target.description().first_line() ++ "\n""#;

fn parse_workspaces(out: &str) -> Vec<WorkspaceInfo> {
    out.lines()
        .filter_map(|l| {
            let mut parts = l.splitn(4, '\t');
            Some(WorkspaceInfo {
                name: parts.next()?.to_string(),
                root: PathBuf::from(parts.next()?),
                change_id: parts.next()?.to_string(),
                description: parts.next().unwrap_or_default().to_string(),
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

    /// Adds workspace `name` at `path` with a new working-copy commit on `trunk()`.
    pub async fn workspace_add(&self, name: &str, path: &Path) -> anyhow::Result<()> {
        let _guard = self.writer.lock().await;
        let path = path.to_string_lossy();
        self.exec(
            &["workspace", "add", "--name", name, "-r", "trunk()", &path],
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

/// `jj workspace list` for the repo at `root`, outside the async runtime (for `lgtm gc`).
pub fn workspaces_blocking(root: &Path) -> anyhow::Result<Vec<WorkspaceInfo>> {
    let out = std::process::Command::new("jj")
        .arg("-R")
        .arg(root)
        .args(["--no-pager", "--color=never", "--ignore-working-copy"])
        .args(["workspace", "list", "-T", LIST_TEMPLATE])
        .stdin(std::process::Stdio::null())
        .output()?;
    anyhow::ensure!(
        out.status.success(),
        "jj workspace list: {}",
        String::from_utf8_lossy(&out.stderr).trim()
    );
    Ok(parse_workspaces(&String::from_utf8_lossy(&out.stdout)))
}

/// `jj workspace forget`, outside the async runtime (for `lgtm gc`).
pub fn forget_blocking(root: &Path, name: &str) -> anyhow::Result<()> {
    let out = std::process::Command::new("jj")
        .arg("-R")
        .arg(root)
        .args(["--no-pager", "--color=never", "--ignore-working-copy"])
        .args(["workspace", "forget", name])
        .stdin(std::process::Stdio::null())
        .output()?;
    anyhow::ensure!(
        out.status.success(),
        "jj workspace forget {name}: {}",
        String::from_utf8_lossy(&out.stderr).trim()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_workspace_list() {
        let got = parse_workspaces("default\t/r/main\tabc\t\nfix\t/ws/r/fix\txyz\tFix:\ta\n");
        assert_eq!(got[0].description, "");
        assert_eq!(got[1].root, PathBuf::from("/ws/r/fix"));
        assert_eq!(got[1].description, "Fix:\ta");
    }
}
