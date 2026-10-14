//! Pushed bookmarks near the working copy, for finding the PRs it belongs to.

use std::collections::{HashMap, HashSet, VecDeque};

use super::Jj;

/// Where a commit sits relative to `@`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StackSide {
    At,
    /// An ancestor of `@`.
    Below,
    /// A descendant of `@`.
    Above,
    /// On another branch of the stack.
    Beside,
}

/// A bookmark on `@`'s stack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NearBookmark {
    pub name: String,
    /// The remote it was pushed to, or empty for the local bookmark.
    pub remote: String,
    /// Parent edges between it and `@`: 0 on `@`, then counting out both ways.
    pub distance: u32,
    pub side: StackSide,
    /// The commit's change id, shortest unique prefix first.
    pub change: String,
    pub commit: String,
}

impl NearBookmark {
    /// Where it is as a jj revset from `@`, like `@`, `@--` or `@+`. Empty beside `@`.
    pub fn position(&self) -> String {
        let n = self.distance as usize;
        match self.side {
            StackSide::At => "@".into(),
            StackSide::Below => format!("@{}", "-".repeat(n)),
            StackSide::Above => format!("@{}", "+".repeat(n)),
            StackSide::Beside => String::new(),
        }
    }
}

/// `@`'s stack: its ancestors down to trunk and their descendants, so a new change on top
/// of a stack commit (`jj new`, or a PR workspace's `@`) still sees the commits above it.
/// Empty when `@` is in trunk, where the descendants would be every branch.
const STACK: &str = "(trunk()..@)::";

/// `@`, `a` for its ancestors, `d` for its descendants or `o` for the rest, then the commit
/// id, its parents' ids, its change id, its remote bookmarks and its local ones.
const TEMPLATE: &str = r#"if(self.contained_in("@"), "@", if(self.contained_in("::@"), "a", if(self.contained_in("@::"), "d", "o"))) ++ "\t"
  ++ commit_id ++ "\t" ++ parents.map(|p| p.commit_id()).join(",") ++ "\t"
  ++ change_id.shortest(4) ++ "\t"
  ++ remote_bookmarks.filter(|b| b.remote() != "git").map(|b| b.name() ++ "\x1f" ++ b.remote()).join("\x1e") ++ "\t"
  ++ local_bookmarks.map(|b| b.name()).join("\x1e")
  ++ "\n""#;

/// Parses `jj log` lines and measures each commit's distance from `@` through the parent
/// edges between them. Returns the bookmarks nearest first, ancestors before descendants at
/// the same distance.
fn parse_near(out: &str) -> Vec<NearBookmark> {
    let rows: Vec<[&str; 6]> = out
        .lines()
        .filter_map(|line| {
            let mut f = line.splitn(6, '\t');
            Some([f.next()?, f.next()?, f.next()?, f.next()?, f.next()?, f.next()?])
        })
        .collect();
    let ids: HashSet<&str> = rows.iter().map(|r| r[1]).collect();
    let mut edges: HashMap<&str, Vec<&str>> = HashMap::new();
    for &[_, id, parents, ..] in &rows {
        for p in parents.split(',').filter(|p| ids.contains(p)) {
            edges.entry(id).or_default().push(p);
            edges.entry(p).or_default().push(id);
        }
    }
    let mut dist: HashMap<&str, u32> = HashMap::new();
    let mut queue: VecDeque<&str> = rows.iter().filter(|r| r[0] == "@").map(|r| r[1]).collect();
    for &id in &queue {
        dist.insert(id, 0);
    }
    while let Some(id) = queue.pop_front() {
        let d = dist[id] + 1;
        for &next in edges.get(id).into_iter().flatten() {
            if !dist.contains_key(next) {
                dist.insert(next, d);
                queue.push_back(next);
            }
        }
    }
    let mut all: Vec<_> = rows
        .iter()
        .map(|row| (dist.get(row[1]).copied().unwrap_or(u32::MAX), row[0] != "a" && row[0] != "@", row))
        .collect();
    all.sort_by_key(|&(d, off, _)| (d, off));
    all.into_iter()
        .flat_map(|(distance, _, &[side, commit, _, change, remote, local])| {
            let side = match side {
                "@" => StackSide::At,
                "a" => StackSide::Below,
                "d" => StackSide::Above,
                _ => StackSide::Beside,
            };
            let at = move |name: &str, remote: &str| NearBookmark {
                name: name.to_string(),
                remote: remote.to_string(),
                distance,
                side,
                change: change.to_string(),
                commit: commit.to_string(),
            };
            let remote = remote
                .split('\x1e')
                .filter_map(move |r| r.split_once('\x1f').map(|(n, rm)| at(n, rm)));
            let local = local.split('\x1e').filter(|n| !n.is_empty()).map(move |n| at(n, ""));
            remote.chain(local)
        })
        .collect()
}

impl Jj {
    /// Remote bookmarks on `@`'s stack, nearest first.
    pub async fn bookmarks_near(&self) -> anyhow::Result<Vec<NearBookmark>> {
        let out = self
            .read(&["log", "--no-graph", "-r", STACK, "-T", TEMPLATE])
            .await?;
        Ok(parse_near(&out))
    }
}

#[cfg(test)]
mod tests {
    use crate::review::tests::{jj, repo, write};

    fn pushed(got: Vec<super::NearBookmark>) -> Vec<super::NearBookmark> {
        got.into_iter().filter(|b| !b.remote.is_empty()).collect()
    }

    fn names(got: &[super::NearBookmark]) -> Vec<(&str, u32)> {
        got.iter().map(|b| (b.name.as_str(), b.distance)).collect()
    }

    #[test]
    fn measures_distance_along_each_branch() {
        // `@` has two branches above it, b1 -> b2 -> b3 and c1, which jj lists one after the
        // other, and `side` branches off `base`. Lines are `side`, commit id, parents, change
        // id, `name\x1fremote` and local bookmarks.
        let out = [
            "o\tside\tbase\tsi\tside\x1fo\t",
            "d\tc1\tat\tc1\tc1\x1fo\t",
            "d\tb3\tb2\tb3\tb3\x1fo\t",
            "d\tb2\tb1\tb2\t\tb3",
            "d\tb1\tat\tb1\tb1\x1fo\t",
            "@\tat\tbase\tat\t\t",
            "a\tbase\ttrunk\tba\tbase\x1fo\t",
        ]
        .join("\n");
        let got = super::parse_near(&out);
        assert_eq!(
            names(&got),
            [("base", 1), ("c1", 1), ("b1", 1), ("side", 2), ("b3", 2), ("b3", 3)]
        );
        let at: Vec<_> = got.iter().map(|b| (b.remote.as_str(), b.position())).collect();
        assert_eq!(
            at,
            [
                ("o", "@-".into()),
                ("o", "@+".into()),
                ("o", "@+".into()),
                ("o", String::new()),
                ("", "@++".into()),
                ("o", "@+++".into()),
            ]
        );
    }

    #[tokio::test]
    async fn finds_pushed_bookmarks_on_the_stack_nearest_first() {
        let remote = tempfile::tempdir().unwrap();
        let out = std::process::Command::new("git")
            .args(["init", "-q", "--bare"])
            .arg(remote.path())
            .output()
            .unwrap();
        assert!(out.status.success());
        let (dir, ctx) = repo();
        let d = dir.path();
        let url = remote.path().to_string_lossy();
        jj(d, &["git", "remote", "add", "origin", &url]);
        let push = |b: &str| jj(d, &["git", "push", "--remote", "origin", "-b", b]);
        let commit = |file: &str, bookmark: &str| {
            write(d, file, file);
            jj(d, &["commit", "-m", file]);
            jj(d, &["bookmark", "create", bookmark, "-r", "@-"]);
        };

        commit("base", "main");
        push("main");
        jj(
            d,
            &[
                "config",
                "set",
                "--repo",
                "revset-aliases.\"trunk()\"",
                "main@origin",
            ],
        );
        commit("one", "one");
        push("one");
        commit("two", "local-only");
        commit("three", "three");
        commit("four", "four");
        push("three");
        push("four");
        commit("five", "five");
        push("five");

        // A bookmark the remote lacks, or that moved past it, is marked unpushed.
        let marks = async |rev: &str| ctx.main.jj.changes(rev).await.unwrap()[0].bookmarks.clone();
        assert_eq!(marks("one").await, ["one"]);
        assert_eq!(marks("local-only").await, ["local-only*"]);
        jj(d, &["bookmark", "set", "one", "-r", "local-only"]);
        assert_eq!(marks("local-only").await, ["local-only*", "one*"]);
        jj(d, &["bookmark", "set", "one", "-r", "one@origin", "--allow-backwards"]);

        // From the middle of the stack: `three` is on @, `one` two below, `four` and `five`
        // above. `main` is trunk and `local-only` was never pushed.
        jj(d, &["edit", "three"]);
        let got = pushed(ctx.main.jj.bookmarks_near().await.unwrap());
        assert_eq!(
            names(&got),
            [("three", 0), ("four", 1), ("one", 2), ("five", 2)]
        );
        assert!(got.iter().all(|b| b.remote == "origin"));

        // A new change on `three` still sees the stack above it, through `three`.
        jj(d, &["new", "three"]);
        let got = pushed(ctx.main.jj.bookmarks_near().await.unwrap());
        assert_eq!(
            names(&got),
            [("three", 1), ("four", 2), ("one", 3), ("five", 3)]
        );

        // At trunk, other branches aren't near.
        jj(d, &["new", "main"]);
        assert!(pushed(ctx.main.jj.bookmarks_near().await.unwrap()).is_empty());
    }
}
