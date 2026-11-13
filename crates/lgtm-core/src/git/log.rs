//! `git log --graph` for picking commits, as the rows jj's log gives: the working copy on
//! top, then the branches' commits not on trunk, then trunk.

use std::collections::HashMap;

use super::Git;
use crate::jj::{LogCommit, LogRow};

/// How many of a commit id's characters show bold.
pub const SHORT: u32 = 7;
/// The most commits the log lists.
const MAX: &str = "-n300";

/// Each commit's fields, `\x1f`-separated, then a second line marked `\x1e` so git draws the
/// graph beside it too.
const ROW_FORMAT: &str = "--format=%x1f%H%x1f%T%x1f%P%x1f%D%x1f%ae%x1f%an%x1f%at%x1f%s%n%x1e";

/// A ref in `git log --decorate=full`'s list.
enum Ref<'a> {
    Local(&'a str),
    Remote { name: &'a str, remote: &'a str },
}

fn refs(decorations: &str) -> impl Iterator<Item = Ref<'_>> {
    decorations.split(", ").filter_map(|d| {
        let d = d.strip_prefix("HEAD -> ").unwrap_or(d);
        if let Some(name) = d.strip_prefix("refs/heads/") {
            return Some(Ref::Local(name));
        }
        let (remote, name) = d.strip_prefix("refs/remotes/")?.split_once('/')?;
        (name != "HEAD").then_some(Ref::Remote { name, remote })
    })
}

/// The local branches in a commit's decorations.
pub fn local_branches(decorations: &str) -> Vec<String> {
    refs(decorations)
        .filter_map(|r| match r {
            Ref::Local(name) => Some(name.to_string()),
            Ref::Remote { .. } => None,
        })
        .collect()
}

/// git's graph in jj's characters, `node` for the commit's `*`.
fn graph(line: &str, node: char) -> String {
    line.chars()
        .map(|c| match c {
            '*' => node,
            '|' => '│',
            '/' => '╱',
            '\\' => '╲',
            c => c,
        })
        .collect()
}

impl Git {
    /// The working copy, the branches' commits not on trunk and trunk, newest first, with
    /// git's graph.
    pub async fn log_rows(&self) -> anyhow::Result<Vec<LogRow>> {
        let (wc, trunk, tips) = tokio::join!(
            self.snapshot(),
            self.trunk(),
            self.read(&[
                "for-each-ref",
                "--format=%(refname)%00%(objectname)",
                "refs/heads",
                "refs/remotes",
            ])
        );
        let wc = wc?;
        let tips: HashMap<String, String> = tips?
            .lines()
            .filter_map(|l| l.split_once('\0'))
            .map(|(r, id)| (r.to_string(), id.to_string()))
            .collect();
        let trunk_tip = trunk.as_ref().and_then(|t| tips.get(t)).cloned();
        let mut args = vec![
            "log",
            "--graph",
            "--topo-order",
            "--decorate=full",
            MAX,
            ROW_FORMAT,
            wc.as_str(),
            "--branches",
        ];
        // Trunk's parents and what's below them stay out.
        let below_trunk = trunk.as_ref().map(|t| format!("{t}^@"));
        if let (Some(trunk), Some(below)) = (&trunk, &below_trunk) {
            args.extend([trunk.as_str(), "--not", below.as_str()]);
        }
        args.push("--");
        let out = self.read(&args).await?;
        Ok(parse_rows(&out, &wc, trunk_tip.as_deref(), &tips))
    }
}

/// Rows from `git log --graph` with [`ROW_FORMAT`]: a commit's line and its `\x1e` line make
/// one row, and any other line is graph alone.
fn parse_rows(
    out: &str,
    wc: &str,
    trunk: Option<&str>,
    tips: &HashMap<String, String>,
) -> Vec<LogRow> {
    let mut rows: Vec<LogRow> = Vec::new();
    let mut trees: HashMap<String, String> = HashMap::new();
    let mut parents: Vec<Option<String>> = Vec::new();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64());
    for line in out.lines() {
        if let Some((g, rest)) = line.split_once('\x1f') {
            let f: Vec<&str> = rest.splitn(8, '\x1f').collect();
            let [id, tree, ps, decorations, email, name, time, subject] = f[..] else {
                continue;
            };
            trees.insert(id.to_string(), tree.to_string());
            parents.push(ps.split(' ').next().map(String::from));
            let working_copy = id == wc;
            let node = match () {
                _ if working_copy => '@',
                _ if Some(id) == trunk => '◆',
                _ => '○',
            };
            let locals = local_branches(decorations);
            let mut bookmarks = Vec::new();
            for r in refs(decorations) {
                match r {
                    Ref::Local(b) => {
                        let behind = tips.iter().any(|(r, at)| {
                            r.strip_prefix("refs/remotes/")
                                .and_then(|r| r.split_once('/'))
                                .is_some_and(|(_, n)| n == b)
                                && at != id
                        });
                        bookmarks.push(format!("{}{b}", if behind { "*" } else { "" }));
                    }
                    Ref::Remote { name, remote } if !locals.iter().any(|l| l == name) => {
                        bookmarks.push(format!("{name}@{remote}"));
                    }
                    Ref::Remote { .. } => {}
                }
            }
            bookmarks.sort_by_key(|b| !b.starts_with('*'));
            let author = email
                .split_once('@')
                .map(|(local, _)| local)
                .filter(|l| !l.is_empty())
                .unwrap_or(name);
            rows.push(LogRow {
                graph: vec![graph(g, node)],
                commit: Some(LogCommit {
                    commit_id: id.into(),
                    change_id: id.into(),
                    unique: SHORT,
                    name: match working_copy {
                        true => Some(super::WORKING_COPY.into()),
                        false => locals.first().cloned(),
                    },
                    description: match working_copy {
                        true => String::new(),
                        false => subject.into(),
                    },
                    bookmarks,
                    author: author.into(),
                    // The working copy is as of now.
                    time: match working_copy {
                        true => now,
                        false => time.parse().unwrap_or(0.0),
                    },
                    working_copy,
                    // lgtm doesn't change git repos.
                    immutable: true,
                    conflict: false,
                    divergent: false,
                    empty: false,
                    workspaces: vec![],
                }),
            });
        } else if let Some((g, _)) = line.split_once('\x1e') {
            if let Some(last) = rows.last_mut().filter(|r| r.graph.len() == 1) {
                last.graph.push(graph(g, '│'));
            }
        } else {
            rows.push(LogRow {
                graph: vec![graph(line.trim_end(), '│')],
                commit: None,
            });
            parents.push(None);
        }
    }
    for (row, parent) in rows.iter_mut().zip(parents) {
        if let (Some(c), Some(p)) = (row.commit.as_mut(), parent) {
            c.empty = trees
                .get(&c.commit_id)
                .is_some_and(|t| trees.get(&p).is_some_and(|pt| pt == t));
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{git, repo, write};

    #[tokio::test]
    async fn the_log_shows_the_working_copy_over_a_branch_over_trunk() {
        let (dir, g) = repo();
        let d = dir.path();
        git(d, &["switch", "-q", "-c", "topic"]);
        write(d, "b", "b\n");
        git(d, &["add", "."]);
        git(d, &["commit", "-q", "-m", "add b"]);
        write(d, "c", "c\n");

        let rows = g.log_rows().await.unwrap();
        let commits: Vec<_> = rows.iter().filter_map(|r| r.commit.as_ref()).collect();
        assert_eq!(commits.len(), 3);
        assert!(commits[0].working_copy && !commits[0].empty);
        assert_eq!(commits[0].name.as_deref(), Some("@"));
        assert!(rows[0].graph[0].starts_with('@'));
        assert_eq!(commits[1].description, "add b");
        assert_eq!(commits[1].name.as_deref(), Some("topic"));
        assert_eq!(commits[2].bookmarks, ["main"]);
        assert!(rows.last().unwrap().graph[0].starts_with('◆'));
    }

    #[tokio::test]
    async fn a_clean_working_copy_is_empty() {
        let (_dir, g) = repo();
        let rows = g.log_rows().await.unwrap();
        assert!(rows[0].commit.as_ref().unwrap().empty);
    }
}
