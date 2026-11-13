//! Workspace commands. Adding and forgetting workspaces are writes on the repo, so they take
//! the writer lock of the `Jj` they run through.

use std::path::PathBuf;

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
