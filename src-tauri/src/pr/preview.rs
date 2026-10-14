//! A read-only look at a PR straight from GitHub: its conversation, then its diff with review
//! threads at their lines. Nothing is fetched into the repo.

use serde::Serialize;
use specta::Type;

use super::discussion::{PostDto, posts};
use super::pull::pulled;
use crate::anchor::Side;
use crate::diff::{GitFileDiff, Hunk, parse_git};
use crate::github::{self, GhRepo, gh, remote};
use crate::review::{PulledThread, RepoCtx};
use crate::store::GhLink;

#[derive(Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PreviewThreadDto {
    pub link: GhLink,
    pub path: String,
    /// GitHub's diff hunk ending at the thread's line, for threads not on the diff.
    pub hunk: String,
}

/// A thread after row `row` of hunk `hunk`.
#[derive(Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlacedThreadDto {
    pub hunk: u32,
    pub row: u32,
    pub thread: PreviewThreadDto,
}

#[derive(Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PreviewFileDto {
    pub path: String,
    /// Source path for renames.
    pub old_path: Option<String>,
    pub binary: bool,
    pub hunks: Vec<Hunk>,
    /// In diff order.
    pub threads: Vec<PlacedThreadDto>,
}

#[derive(Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PrPreviewDto {
    pub number: u32,
    /// `owner/name`.
    pub repo: String,
    pub title: String,
    pub url: String,
    pub author: String,
    /// OPEN, CLOSED or MERGED.
    pub state: String,
    pub base_ref: String,
    pub head_ref: String,
    /// Conversation comments and review summaries, oldest first.
    pub posts: Vec<PostDto>,
    pub files: Vec<PreviewFileDto>,
    /// Threads whose line isn't on the diff, like outdated ones.
    pub elsewhere: Vec<PreviewThreadDto>,
}

/// A file's path and, when it moved, where from. Deleted files keep their old path.
fn paths(f: &GitFileDiff) -> (String, Option<String>) {
    let ext = |p: &str| {
        f.extended
            .iter()
            .find_map(|l| l.strip_prefix(p))
            .map(str::to_string)
    };
    let old = ext("rename from ").or_else(|| ext("--- a/"));
    let new = ext("rename to ").or_else(|| ext("+++ b/"));
    // Binary and mode-only changes have no `---`/`+++`, just the header `a/x b/y`.
    let header = || {
        f.header
            .strip_prefix("a/")
            .and_then(|h| h.split_once(" b/"))
            .map(|(a, b)| (a.to_string(), b.to_string()))
    };
    let (old, new) = match (old, new) {
        (o, Some(n)) => (o, n),
        (Some(o), None) => (None, o),
        (None, None) => header().map_or((None, f.header.clone()), |(a, b)| (Some(a), b)),
    };
    let moved = old.filter(|o| *o != new);
    (new, moved)
}

/// The row a thread's last line is on: its old line on the base side, else its new line.
fn place(hunks: &[Hunk], side: Side, line: u32) -> Option<(u32, u32)> {
    hunks.iter().enumerate().find_map(|(h, hunk)| {
        let row = hunk.rows.iter().position(|r| {
            let n = if side == Side::Old {
                r.old_line
            } else {
                r.new_line
            };
            n == Some(line)
        })?;
        Some((h as u32, row as u32))
    })
}

/// Sorts the threads onto the files' rows, leaving the rest elsewhere.
fn layout(
    diff: Vec<GitFileDiff>,
    threads: Vec<PulledThread>,
) -> (Vec<PreviewFileDto>, Vec<PreviewThreadDto>) {
    let mut files: Vec<PreviewFileDto> = diff
        .into_iter()
        .map(|f| {
            let (path, old_path) = paths(&f);
            PreviewFileDto {
                path,
                old_path,
                binary: f.binary,
                hunks: f.hunks,
                threads: Vec::new(),
            }
        })
        .collect();
    let mut elsewhere = Vec::new();
    for t in threads {
        let thread = PreviewThreadDto {
            link: t.link,
            path: t.path,
            hunk: t.hunk,
        };
        let spot = files.iter_mut().find_map(|f| {
            if f.path != thread.path {
                return None;
            }
            let at = place(&f.hunks, t.side, t.line?)?;
            Some((f, at))
        });
        match spot {
            Some((f, (hunk, row))) => f.threads.push(PlacedThreadDto { hunk, row, thread }),
            None => elsewhere.push(thread),
        }
    }
    for f in &mut files {
        f.threads.sort_by_key(|t| (t.hunk, t.row));
    }
    (files, elsewhere)
}

impl RepoCtx {
    /// The PR `input` names (a number or a URL), read from GitHub without fetching it.
    pub async fn pr_preview(&self, input: &str) -> anyhow::Result<PrPreviewDto> {
        let (repo, number) = remote::parse_pr(input)?;
        let repo: GhRepo = match repo {
            Some(r) => r,
            None => {
                remote::pick(
                    &crate::github::canonical::remotes(&self.main.jj).await?,
                    None,
                )?
                .2
            }
        };
        let (info, d, diff) = tokio::try_join!(
            github::pr::info(&repo, number),
            github::threads::discussion(&repo, number),
            gh::pr_diff(&repo, number),
        )?;
        let name = repo.to_string();
        let threads = d
            .threads
            .into_iter()
            .map(|t| pulled(t, &name, number, &d.viewer))
            .filter(|t| !t.link.posts.is_empty())
            .collect();
        let (files, elsewhere) = layout(parse_git(&diff)?, threads);
        Ok(PrPreviewDto {
            number,
            repo: name,
            title: info.title,
            url: info.url,
            author: github::login(&info.author),
            state: info.state,
            base_ref: info.base_ref_name,
            head_ref: info.head_ref_name,
            posts: posts(d.comments, d.reviews),
            files,
            elsewhere,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thread(path: &str, side: Side, line: Option<u32>) -> PulledThread {
        PulledThread {
            link: GhLink {
                repo: "o/r".into(),
                pr: 1,
                thread: format!("{path}:{line:?}"),
                url: String::new(),
                resolved: false,
                outdated: line.is_none(),
                posts: Vec::new(),
                line: None,
            },
            path: path.into(),
            side,
            start: None,
            line,
            hunk: String::new(),
            answered: false,
        }
    }

    #[test]
    fn threads_land_on_their_side_of_the_diff_or_elsewhere() {
        let diff = parse_git(
            "\
diff --git a/old.rs b/new.rs
rename from old.rs
rename to new.rs
--- a/old.rs
+++ b/new.rs
@@ -10,3 +10,3 @@
 keep
-gone
+came
 keep
@@ -40,1 +40,2 @@
 tail
+end
diff --git a/img.png b/img.png
Binary files a/img.png and b/img.png differ
diff --git a/dead.rs b/dead.rs
deleted file mode 100644
--- a/dead.rs
+++ /dev/null
@@ -1,1 +0,0 @@
-bye
",
        )
        .unwrap();
        let threads = vec![
            thread("new.rs", Side::New, Some(41)),
            // Line 11 is `gone` on the base and `came` on the head.
            thread("new.rs", Side::Old, Some(11)),
            thread("new.rs", Side::New, Some(11)),
            thread("dead.rs", Side::Old, Some(1)),
            thread("new.rs", Side::New, Some(99)),
            thread("new.rs", Side::New, None),
            thread("old.rs", Side::Old, Some(11)),
        ];
        let (files, elsewhere) = layout(diff, threads);

        let summary: Vec<_> = files
            .iter()
            .map(|f| (f.path.as_str(), f.old_path.as_deref(), f.binary))
            .collect();
        assert_eq!(
            summary,
            [
                ("new.rs", Some("old.rs"), false),
                ("img.png", None, true),
                ("dead.rs", None, false),
            ]
        );
        let at = |f: &PreviewFileDto| -> Vec<(u32, u32, String)> {
            f.threads
                .iter()
                .map(|t| (t.hunk, t.row, t.thread.link.thread.clone()))
                .collect()
        };
        assert_eq!(
            at(&files[0]),
            [
                (0, 1, "new.rs:Some(11)".into()),
                (0, 2, "new.rs:Some(11)".into()),
                (1, 1, "new.rs:Some(41)".into()),
            ]
        );
        assert_eq!(at(&files[2]), [(0, 0, "dead.rs:Some(1)".into())]);
        let rest: Vec<_> = elsewhere.iter().map(|t| t.link.thread.as_str()).collect();
        assert_eq!(rest, ["new.rs:Some(99)", "new.rs:None", "old.rs:Some(11)"]);
    }
}
