//! Pushed bookmarks near the working copy, for finding the PRs it belongs to.

use super::Jj;

/// A remote bookmark on `@`'s stack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NearBookmark {
    pub name: String,
    pub remote: String,
    /// Commits between it and `@`: 0 on `@`, then counting out both ways.
    pub distance: u32,
}

/// `@`'s stack: its ancestors down to trunk, and its descendants unless `@` is in trunk,
/// where they'd be every branch.
const STACK: &str = "(trunk()..@) | ((@ ~ ::trunk())::)";

/// `a` for `@` and its ancestors, `d` for descendants, then the commit's remote bookmarks.
const TEMPLATE: &str = r#"if(self.contained_in("::@"), "a", "d") ++ "\t"
  ++ remote_bookmarks.filter(|b| b.remote() != "git").map(|b| b.name() ++ "\x1f" ++ b.remote()).join("\x1e")
  ++ "\n""#;

/// Parses `jj log` lines in its order, which lists descendants farthest first, then `@` and
/// its ancestors nearest first. Returns the bookmarks nearest first, ancestors before
/// descendants at the same distance.
fn parse_near(out: &str) -> Vec<NearBookmark> {
    let (mut desc, mut anc) = (Vec::new(), Vec::new());
    for line in out.lines() {
        let Some((side, refs)) = line.split_once('\t') else {
            continue;
        };
        if side == "a" {
            anc.push(refs);
        } else {
            desc.push(refs);
        }
    }
    let anc = anc.into_iter().enumerate().map(|(i, r)| (i, false, r));
    let desc = desc
        .into_iter()
        .rev()
        .enumerate()
        .map(|(i, r)| (i + 1, true, r));
    let mut all: Vec<_> = anc.chain(desc).collect();
    all.sort_by_key(|&(d, is_desc, _)| (d, is_desc));
    all.into_iter()
        .flat_map(|(d, _, refs)| {
            refs.split('\x1e').filter_map(move |r| {
                let (name, remote) = r.split_once('\x1f')?;
                Some(NearBookmark {
                    name: name.to_string(),
                    remote: remote.to_string(),
                    distance: d as u32,
                })
            })
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

    fn names(got: &[super::NearBookmark]) -> Vec<(&str, u32)> {
        got.iter().map(|b| (b.name.as_str(), b.distance)).collect()
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

        // From the middle of the stack: `three` is on @, `one` two below, `four` and `five`
        // above. `main` is trunk and `local-only` was never pushed.
        jj(d, &["edit", "three"]);
        let got = ctx.main.jj.bookmarks_near().await.unwrap();
        assert_eq!(
            names(&got),
            [("three", 0), ("four", 1), ("one", 2), ("five", 2)]
        );
        assert!(got.iter().all(|b| b.remote == "origin"));

        // At trunk, other branches aren't near.
        jj(d, &["new", "main"]);
        assert!(ctx.main.jj.bookmarks_near().await.unwrap().is_empty());
    }
}
