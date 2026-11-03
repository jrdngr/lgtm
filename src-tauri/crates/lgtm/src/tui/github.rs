//! Pulling the review's PR threads from GitHub: the PR given on the command line, or else the
//! open PR from one of the review's bookmarks.

use std::collections::HashMap;

use lgtm_core::github::pulled::{PulledThread, pulled};
use lgtm_core::github::{canonical, heads, remote, threads};
use lgtm_core::jj::Jj;

use lgtm_core::anchor::Side;

/// A PR's threads on code, ready to store.
pub struct Pulled {
    /// `owner/name`.
    pub repo: String,
    pub number: u32,
    pub threads: Vec<PulledThread>,
    /// The threads' files at the PR head, by path.
    pub head_texts: HashMap<String, String>,
}

/// Finds the PR, `pr` as typed or by `bookmarks` (newest first), and pulls its threads.
pub async fn pull(jj: &Jj, pr: Option<&str>, bookmarks: &[String]) -> anyhow::Result<Pulled> {
    let remotes = canonical::remotes(jj).await?;
    let (repo, number) = match pr {
        Some(typed) => {
            let (repo, number) = remote::parse_pr(typed)?;
            let repo = match repo {
                Some(r) => r,
                None => remote::pick(&remotes, None)?.2,
            };
            (repo, number)
        }
        None => {
            let base = remote::pick(&remotes, None)?.2;
            let names: Vec<&str> = bookmarks.iter().map(String::as_str).collect();
            let open = heads::open_prs(&base, &names).await?;
            let found = bookmarks
                .iter()
                .find_map(|b| open.iter().find(|p| p.head_ref_name == *b))
                .ok_or_else(|| anyhow::anyhow!("no open PR has this review's bookmarks"))?;
            (base, found.number)
        }
    };
    let discussion = threads::discussion(&repo, number).await?;
    let name = repo.to_string();
    // GitHub numbers lines on the PR head. Without it here, its new-side lines can't be placed.
    let fetched = jj
        .commit_ids(&discussion.head)
        .await
        .is_ok_and(|c| !c.is_empty());
    let threads: Vec<PulledThread> = discussion
        .threads
        .into_iter()
        .map(|t| pulled(t, &name, number, &discussion.viewer))
        .map(|mut t| {
            if !fetched && t.side == Side::New {
                t.line = None;
            }
            t
        })
        .collect();
    let mut paths: Vec<String> = threads
        .iter()
        .filter(|t| t.line.is_some() && t.side == Side::New)
        .map(|t| t.path.clone())
        .collect();
    paths.sort();
    paths.dedup();
    let head_texts = match fetched {
        true => jj
            .files_at(&discussion.head, &paths)
            .await
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(path, bytes)| Some((path, String::from_utf8(bytes).ok()?)))
            .collect(),
        false => HashMap::new(),
    };
    Ok(Pulled {
        repo: name,
        number,
        threads,
        head_texts,
    })
}
