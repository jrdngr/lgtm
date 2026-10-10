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
}

/// Splits out the `\x1f`-separated ids `log_graph` puts in front of each commit's template.
fn parse_log(out: &str) -> Vec<LogLine> {
    out.lines()
        .map(|line| {
            let mut parts = line.splitn(4, '\x1f');
            let prefix = parts.next().unwrap_or_default();
            match (parts.next(), parts.next(), parts.next()) {
                (Some(commit), Some(change), Some(rest)) => LogLine {
                    text: format!("{prefix}{rest}"),
                    commit_id: Some(commit.to_string()),
                    change_id: Some(change.to_string()),
                },
                _ => LogLine {
                    text: line.to_string(),
                    commit_id: None,
                    change_id: None,
                },
            }
        })
        .collect()
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
            r#""\x1f" ++ stringify(commit_id) ++ "\x1f" ++ stringify(change_id.shortest(8)) ++ "\x1f" ++ ({template})"#
        );
        let out = self
            .exec_with(&["log", "-T", &template], true, "always")
            .await?;
        Ok(parse_log(&String::from_utf8_lossy(&out)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_lines_carry_ids_only_where_a_commit_starts() {
        let got = parse_log("@  \x1fc1\x1fab\x1fab desc\n│  body\n");
        assert_eq!(got[0].text, "@  ab desc");
        assert_eq!(got[0].commit_id.as_deref(), Some("c1"));
        assert_eq!(
            (got[1].text.as_str(), got[1].change_id.as_deref()),
            ("│  body", None)
        );
    }
}
