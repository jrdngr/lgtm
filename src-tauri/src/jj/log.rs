//! `jj log` as the user sees it, for picking commits.

use super::Jj;

/// One line of `jj log` output, with ANSI colors.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    pub text: String,
    /// Set on the first line of each commit.
    pub commit_id: Option<String>,
    pub change_id: Option<String>,
    /// The commit's whole description, on the first line of each commit.
    pub description: Option<String>,
    /// Whether this line starts the workspace's working-copy commit.
    pub working_copy: bool,
}

/// Splits out the `\x1f`-separated ids, working-copy flag and JSON-escaped description that
/// `log_graph` puts in front of each commit's template.
fn parse_log(out: &str) -> Vec<LogLine> {
    out.lines()
        .map(|line| {
            let mut parts = line.splitn(6, '\x1f');
            let prefix = parts.next().unwrap_or_default();
            let fields = (parts.next(), parts.next(), parts.next(), parts.next(), parts.next());
            match fields {
                (Some(commit), Some(change), Some(wc), Some(desc), Some(rest)) => LogLine {
                    text: format!("{prefix}{rest}"),
                    commit_id: Some(commit.to_string()),
                    change_id: Some(change.to_string()),
                    description: serde_json::from_str(desc).ok(),
                    working_copy: wc == "1",
                },
                _ => LogLine {
                    text: line.to_string(),
                    commit_id: None,
                    change_id: None,
                    description: None,
                    working_copy: false,
                },
            }
        })
        .collect()
}

/// Names `trunk()` usually resolves to, preferred over other bookmarks on the same commit.
const TRUNK_NAMES: [&str; 3] = ["main", "master", "trunk"];

/// Picks the trunk bookmark from the newline-separated local bookmarks on `trunk()`, as a
/// revset symbol: quoted when it has characters a bare symbol can't.
fn pick_trunk(out: &str) -> Option<String> {
    let names: Vec<&str> = out.lines().filter(|l| !l.is_empty()).collect();
    let name = TRUNK_NAMES
        .into_iter()
        .find(|t| names.contains(t))
        .or_else(|| names.first().copied())?;
    let bare = name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "_./-+".contains(c));
    Some(if bare {
        name.to_string()
    } else {
        format!("\"{}\"", name.replace('\\', "\\\\").replace('"', "\\\""))
    })
}

impl Jj {
    /// `jj log` as the user sees it: their default revset and log template, with the graph
    /// and colors. Lines that start a commit carry its ids.
    pub async fn log_graph(&self) -> anyhow::Result<Vec<LogLine>> {
        let template = self
            .read(&["config", "get", "templates.log"])
            .await
            .map(|t| t.trim().to_string())
            .unwrap_or_else(|_| "builtin_log_compact".into());
        let template = format!(
            r#""\x1f" ++ stringify(commit_id) ++ "\x1f" ++ stringify(change_id.shortest(8)) ++ "\x1f" ++ if(current_working_copy, "1", "0") ++ "\x1f" ++ description.escape_json() ++ "\x1f" ++ ({template})"#
        );
        let out = self
            .exec_with(&["log", "-T", &template], true, "always")
            .await?;
        Ok(parse_log(&String::from_utf8_lossy(&out)))
    }

    /// The local bookmark `trunk()` resolves to, as a revset symbol, or `None` when it has
    /// none.
    pub async fn trunk_bookmark(&self) -> anyhow::Result<Option<String>> {
        let out = self
            .read(&[
                "log",
                "--no-graph",
                "-r",
                "trunk()",
                "-T",
                r#"local_bookmarks.map(|b| b.name()).join("\n") ++ "\n""#,
            ])
            .await?;
        Ok(pick_trunk(&out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_lines_carry_ids_only_where_a_commit_starts() {
        let got = parse_log("@  \x1fc1\x1fab\x1f1\x1f\"fix\\n\\nbody\"\x1fab desc\n│  body\n");
        assert_eq!(got[0].text, "@  ab desc");
        assert_eq!(got[0].description.as_deref(), Some("fix\n\nbody"));
        assert!(got[0].working_copy);
        assert_eq!(got[0].commit_id.as_deref(), Some("c1"));
        assert_eq!(
            (got[1].text.as_str(), got[1].change_id.as_deref()),
            ("│  body", None)
        );
    }

    #[test]
    fn trunk_prefers_the_usual_names_and_quotes_odd_ones() {
        assert_eq!(pick_trunk("release\nmain\n").as_deref(), Some("main"));
        assert_eq!(pick_trunk("jg/v1.2\n").as_deref(), Some("jg/v1.2"));
        assert_eq!(pick_trunk("a\"b c\n").as_deref(), Some(r#""a\"b c""#));
        assert_eq!(pick_trunk("\n"), None);
    }
}
