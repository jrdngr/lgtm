//! `jj log` for picking commits, as rows lgtm draws.

use super::Jj;

/// One commit in the log, with the graph drawn beside its two lines, or a line of graph
/// alone where branches join or are elided.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogRow {
    /// The graph left of each of the row's lines: jj's own, without colors.
    pub graph: Vec<String>,
    pub commit: Option<LogCommit>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogCommit {
    pub commit_id: String,
    pub change_id: String,
    /// How many of the change id's first characters tell it apart.
    pub unique: u32,
    /// The whole description.
    pub description: String,
    /// Local bookmarks, `*` first when out of sync with their remote, then remote ones as
    /// `name@remote` where they aren't where the local one is.
    pub bookmarks: Vec<String>,
    /// The author's email up to the `@`, else their name.
    pub author: String,
    /// When it was authored, in seconds since the epoch.
    pub time: f64,
    pub working_copy: bool,
    pub immutable: bool,
    pub conflict: bool,
    pub divergent: bool,
    pub empty: bool,
    /// The workspaces whose working copy it is, when the repo has more than one.
    pub workspaces: Vec<String>,
}

/// Each commit's fields, `\x1f`-separated, then a second line marked `\x1e` so jj draws the
/// graph beside it too.
const ROW_TEMPLATE: &str = r#""\x1f" ++ commit_id ++ "\x1f" ++ change_id ++ "\x1f"
  ++ change_id.shortest().prefix().len() ++ "\x1f" ++ description.escape_json() ++ "\x1f"
  ++ bookmarks.map(|b| if(b.remote(), b.name() ++ "@" ++ b.remote(),
       if(b.synced(), "", "*") ++ b.name())).join(" ")
  ++ "\x1f"
  ++ coalesce(author.email().local(), author.name()) ++ "\x1f"
  ++ author.timestamp().format("%s") ++ "\x1f"
  ++ if(current_working_copy, "w") ++ if(immutable, "i") ++ if(conflict, "c")
  ++ if(divergent, "d") ++ if(empty, "e") ++ "\x1f"
  ++ working_copies.map(|w| w.name()).join(" ")
  ++ "\n\x1e\n""#;

/// A line of graph alone, without the text jj puts after it, like `(elided revisions)`.
fn graph_only(line: &str) -> &str {
    let end = line
        .find(|c: char| c == '(' || c.is_ascii_alphanumeric())
        .unwrap_or(line.len());
    line[..end].trim_end()
}

/// Rows from `jj log -T ROW_TEMPLATE`: a commit's line and its `\x1e` line make one row, and
/// any other line is graph alone.
fn parse_rows(out: &str) -> Vec<LogRow> {
    let mut rows: Vec<LogRow> = Vec::new();
    for line in out.lines() {
        if let Some((graph, rest)) = line.split_once('\x1f') {
            let f: Vec<&str> = rest.split('\x1f').collect();
            if f.len() < 8 {
                continue;
            }
            let flags = f[7];
            rows.push(LogRow {
                graph: vec![graph.to_string()],
                commit: Some(LogCommit {
                    commit_id: f[0].into(),
                    change_id: f[1].into(),
                    unique: f[2].parse().unwrap_or(4),
                    description: serde_json::from_str(f[3]).unwrap_or_default(),
                    bookmarks: f[4].split_whitespace().map(String::from).collect(),
                    author: f[5].into(),
                    time: f[6].parse().unwrap_or(0.0),
                    working_copy: flags.contains('w'),
                    immutable: flags.contains('i'),
                    conflict: flags.contains('c'),
                    divergent: flags.contains('d'),
                    empty: flags.contains('e'),
                    workspaces: f
                        .get(8)
                        .map(|w| w.split_whitespace().map(String::from).collect())
                        .unwrap_or_default(),
                }),
            });
        } else if let Some((graph, _)) = line.split_once('\x1e') {
            if let Some(last) = rows.last_mut().filter(|r| r.graph.len() == 1) {
                last.graph.push(graph.to_string());
            }
        } else {
            rows.push(LogRow {
                graph: vec![graph_only(line).to_string()],
                commit: None,
            });
        }
    }
    rows
}

impl Jj {
    /// The commits of `revset`, or `jj log`'s default revset, newest first, with jj's graph.
    /// Fails with jj's own error, without the command line.
    pub async fn log_rows(&self, revset: Option<&str>) -> anyhow::Result<Vec<LogRow>> {
        let mut args = vec!["log", "-T", ROW_TEMPLATE];
        if let Some(r) = revset {
            args.extend(["-r", r]);
        }
        let out = self.exec_with(&args, true, "never").await.map_err(|e| {
            let text = format!("{e:#}");
            let command = format!("jj {}: ", args.join(" "));
            anyhow::anyhow!("{}", text.strip_prefix(&command).unwrap_or(&text))
        })?;
        Ok(parse_rows(&String::from_utf8_lossy(&out)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_pair_each_commit_with_its_graph_and_keep_graph_only_lines_without_their_text() {
        let out = "@  \x1fc1\x1fkx\x1f2\x1f\"fix\\n\\nbody\"\x1fmain feat\x1fjo\x1f1700000000\x1fwe\x1fdefault ws\n\
                   │  \x1e\n\
                   │ ○  \x1fc2\x1fpq\x1f1\x1f\"\"\x1f\x1fjo\x1f1\x1fc\n\
                   ├─╯  \x1e\n\
                   │ ~  (elided revisions)\n\
                   ~\n";
        let rows = parse_rows(out);
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].graph, ["@  ", "│  "]);
        let c = rows[0].commit.as_ref().unwrap();
        assert_eq!(c.description, "fix\n\nbody");
        assert_eq!(c.bookmarks, ["main", "feat"]);
        assert_eq!(c.workspaces, ["default", "ws"]);
        assert!(c.working_copy && c.empty && !c.conflict);
        assert_eq!((c.unique, c.time), (2, 1_700_000_000.0));
        assert_eq!(rows[1].graph, ["│ ○  ", "├─╯  "]);
        assert!(rows[1].commit.as_ref().unwrap().conflict);
        assert_eq!(
            (rows[2].graph[0].as_str(), rows[2].commit.is_none()),
            ("│ ~", true)
        );
        assert_eq!(rows[3].graph, ["~"]);
    }
}
