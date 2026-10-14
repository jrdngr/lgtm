//! The open PRs of bookmarks pushed from the working copy's stack, for picking one to review.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Serialize;
use specta::Type;

use crate::github::heads::{self, HeadPr};
use crate::github::{GhRepo, login, remote};
use crate::jj::{NearBookmark, StackSide};
use crate::review::RepoCtx;

/// At most this many bookmarks are looked up, nearest first.
const MAX_BOOKMARKS: usize = 30;

/// Open PRs as last looked up, by repo and the bookmarks asked about, so following `@`
/// around the stack doesn't ask GitHub each time.
static OPEN: Mutex<Option<HashMap<String, Vec<HeadPr>>>> = Mutex::new(None);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NearbyPrDto {
    pub number: u32,
    pub title: String,
    pub url: String,
    pub author: String,
    /// The pushed bookmark that is the PR's head.
    pub bookmark: String,
    pub draft: bool,
    /// Where the bookmark is from `@`, as a jj revset like `@-`. Empty off `@`'s line.
    pub position: String,
    /// The change the bookmark is on.
    pub change: String,
    /// The bookmark is on `@` or below it, so `@` builds on the PR.
    pub under: bool,
    /// The commit the bookmark was last pushed at.
    pub pushed: String,
}

/// What the search found: the PRs, and the bookmarks looked up to find them, which tell
/// "nothing pushed near `@`" apart from "pushed, but no open PRs".
#[derive(Clone, Debug, Serialize, Type)]
pub struct NearbyPrsDto {
    pub prs: Vec<NearbyPrDto>,
    /// Names of the pushed bookmarks on the stack, nearest first.
    pub bookmarks: Vec<String>,
}

/// Bookmarks on GitHub remotes, with the repo each was pushed to.
fn on_github(near: Vec<NearBookmark>, remotes: &[(String, String)]) -> Vec<(NearBookmark, GhRepo)> {
    near.into_iter()
        .filter_map(|b| {
            let (_, url) = remotes.iter().find(|(n, _)| *n == b.remote)?;
            let repo = remote::parse_url(url).or_else(|| remote::any_repo(url))?;
            Some((b, repo))
        })
        .take(MAX_BOOKMARKS)
        .collect()
}

/// Where a pushed bookmark is now: at its local bookmark when that's on the stack, which
/// follows the commit through squashes and moves, else where it was pushed.
fn placed<'a>(b: &'a NearBookmark, near: &'a [NearBookmark]) -> &'a NearBookmark {
    near.iter()
        .find(|l| l.remote.is_empty() && l.name == b.name)
        .unwrap_or(b)
}

/// The PRs whose head is one of the bookmarks, nearest `@` first: a PR matches when its head
/// branch has the bookmark's name in the repo the bookmark was pushed to.
fn match_prs(
    bookmarks: &[(NearBookmark, GhRepo)],
    prs: &[HeadPr],
    near: &[NearBookmark],
) -> Vec<NearbyPrDto> {
    let mut out: Vec<(u32, bool, NearbyPrDto)> = Vec::new();
    for (b, repo) in bookmarks {
        for pr in prs {
            let Some(head) = &pr.head_repository else {
                continue;
            };
            let same_repo = head.owner.login.eq_ignore_ascii_case(&repo.owner)
                && head.name.eq_ignore_ascii_case(&repo.name);
            if pr.head_ref_name != b.name
                || !same_repo
                || out.iter().any(|(.., o)| o.number == pr.number)
            {
                continue;
            }
            let at = placed(b, near);
            let under = matches!(at.side, StackSide::At | StackSide::Below);
            out.push((
                at.distance,
                !under,
                NearbyPrDto {
                    number: pr.number,
                    title: pr.title.clone(),
                    url: pr.url.clone(),
                    author: login(&pr.author),
                    bookmark: b.name.clone(),
                    draft: pr.is_draft,
                    position: at.position(),
                    change: at.change.clone(),
                    under,
                    pushed: b.commit.clone(),
                },
            ));
        }
    }
    out.sort_by_key(|&(d, off, _)| (d, off));
    out.into_iter().map(|(.., p)| p).collect()
}

impl RepoCtx {
    /// Open PRs of the bookmarks pushed from `workspace`'s stack, nearest `@` first. PRs are
    /// looked up in the repo PR numbers refer to, and may come from forks pushed to. They're
    /// asked of GitHub again only with `refresh`, or when the bookmarks change.
    pub async fn nearby_prs(
        &self,
        workspace: Option<&str>,
        refresh: bool,
    ) -> anyhow::Result<NearbyPrsDto> {
        let near = self.workspace(workspace).await?.jj.bookmarks_near().await?;
        let remotes = crate::github::canonical::remotes(&self.main.jj).await?;
        let bookmarks = on_github(near.clone(), &remotes);
        let mut names: Vec<String> = Vec::new();
        for (b, _) in &bookmarks {
            if !names.contains(&b.name) {
                names.push(b.name.clone());
            }
        }
        if names.is_empty() {
            return Ok(NearbyPrsDto {
                prs: Vec::new(),
                bookmarks: names,
            });
        }
        let (_, _, base) = remote::pick(&remotes, None)?;
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let key = format!("{base}\n{}", names.join("\n"));
        let cached = (!refresh)
            .then(|| OPEN.lock().unwrap().as_ref()?.get(&key).cloned())
            .flatten();
        let prs = match cached {
            Some(prs) => prs,
            None => {
                let prs = heads::open_prs(&base, &refs).await?;
                let mut open = OPEN.lock().unwrap();
                open.get_or_insert_default().insert(key, prs.clone());
                prs
            }
        };
        Ok(NearbyPrsDto {
            prs: match_prs(&bookmarks, &prs, &near),
            bookmarks: names,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::pr::{HeadRepo, Login};

    fn bookmark(name: &str, remote: &str, distance: u32) -> NearBookmark {
        NearBookmark {
            name: name.into(),
            remote: remote.into(),
            distance,
            side: StackSide::Below,
            change: name.into(),
            commit: name.into(),
        }
    }

    fn pr(number: u32, head: &str, owner: &str) -> HeadPr {
        HeadPr {
            number,
            title: format!("PR {number}"),
            url: String::new(),
            is_draft: false,
            author: None,
            head_ref_name: head.into(),
            head_repository: Some(HeadRepo {
                name: "r".into(),
                owner: Login {
                    login: owner.into(),
                },
            }),
        }
    }

    #[test]
    fn matches_heads_in_the_repo_each_bookmark_was_pushed_to() {
        let remotes = vec![
            ("origin".to_string(), "git@github.com:me/r.git".to_string()),
            (
                "upstream".to_string(),
                "https://github.com/them/r".to_string(),
            ),
            ("local".to_string(), "/srv/r.git".to_string()),
        ];
        let near = vec![
            bookmark("fix", "origin", 0),
            bookmark("skipped", "local", 0),
            bookmark("feat", "upstream", 1),
            bookmark("fix", "upstream", 1),
        ];
        let bookmarks = on_github(near.clone(), &remotes);
        assert_eq!(bookmarks.len(), 3);

        // `fix` is both a fork PR from me/r and a PR from them/r; another fork's `feat` and a
        // deleted fork's head don't count.
        let mut gone = pr(5, "feat", "x");
        gone.head_repository = None;
        let prs = [
            pr(4, "feat", "someone"),
            gone,
            pr(3, "feat", "them"),
            pr(2, "fix", "THEM"),
            pr(1, "fix", "me"),
        ];
        let got: Vec<(u32, String)> = match_prs(&bookmarks, &prs, &near)
            .into_iter()
            .map(|p| (p.number, p.bookmark))
            .collect();
        assert_eq!(
            got,
            [(1, "fix".into()), (3, "feat".into()), (2, "fix".into())]
        );
    }

    #[test]
    fn a_squashed_or_moved_bookmark_is_placed_at_its_local_bookmark() {
        // `fix` was pushed two above `@`, then its commit was squashed below `@`: the local
        // bookmark followed it there, and `@` now builds on the PR.
        let remotes = vec![("origin".to_string(), "git@github.com:me/r.git".to_string())];
        let mut pushed = bookmark("fix", "origin", 2);
        pushed.side = StackSide::Above;
        let mut local = bookmark("fix", "", 1);
        local.change = "kq".into();
        let near = vec![local, pushed];
        let bookmarks = on_github(near.clone(), &remotes);
        let got = match_prs(&bookmarks, &[pr(1, "fix", "me")], &near);
        let p = &got[0];
        assert_eq!((p.position.as_str(), p.change.as_str(), p.under), ("@-", "kq", true));
    }
}
