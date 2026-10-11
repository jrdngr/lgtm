//! GitHub repos behind git remotes, and what a user types to name a PR.

use anyhow::{Context, bail};

/// A GitHub repository.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GhRepo {
    pub owner: String,
    pub name: String,
}

impl std::fmt::Display for GhRepo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.owner, self.name)
    }
}

fn split_repo(path: &str) -> Option<GhRepo> {
    let path = path.trim_end_matches('/').trim_end_matches(".git");
    let mut parts = path.split('/');
    let (owner, name) = (parts.next()?, parts.next()?);
    (!owner.is_empty() && !name.is_empty()).then(|| GhRepo {
        owner: owner.to_string(),
        name: name.to_string(),
    })
}

/// The GitHub repo a remote URL points at: `git@github.com:o/n.git`,
/// `https://github.com/o/n`, or `ssh://git@github.com/o/n`.
pub fn parse_url(url: &str) -> Option<GhRepo> {
    let rest = url
        .strip_prefix("git@github.com:")
        .or_else(|| url.strip_prefix("ssh://git@github.com/"))
        .or_else(|| url.strip_prefix("https://github.com/"))
        .or_else(|| url.strip_prefix("http://github.com/"))?;
    split_repo(rest)
}

/// The URL of `repo`, over SSH when `like` (the base repo's URL) is.
pub fn url_like(like: &str, repo: &GhRepo) -> String {
    if like.starts_with("git@") || like.starts_with("ssh://") {
        format!("git@github.com:{repo}.git")
    } else {
        format!("https://github.com/{repo}.git")
    }
}

/// The remote a fork's branches are fetched from.
pub fn fork_remote(owner: &str) -> String {
    format!("gh-{owner}")
}

/// The remote for `want`, or with none, the repo PR numbers refer to: `upstream`, then
/// `origin`, then the first GitHub remote. Returns the remote's name, URL and repo.
pub fn pick(
    remotes: &[(String, String)],
    want: Option<&GhRepo>,
) -> anyhow::Result<(String, String, GhRepo)> {
    let github: Vec<(&String, &String, GhRepo)> = remotes
        .iter()
        .filter_map(|(n, u)| Some((n, u, parse_url(u)?)))
        .collect();
    let found = match want {
        Some(w) => github.iter().find(|(_, _, r)| {
            r.owner.eq_ignore_ascii_case(&w.owner) && r.name.eq_ignore_ascii_case(&w.name)
        }),
        None => ["upstream", "origin"]
            .iter()
            .find_map(|n| github.iter().find(|(name, _, _)| name.as_str() == *n))
            // `gh-*` remotes are forks lgtm added, not the repo's own.
            .or(github.iter().find(|(name, _, _)| !name.starts_with("gh-"))),
    };
    match (found, want) {
        (Some((n, u, r)), _) => Ok((n.to_string(), u.to_string(), r.clone())),
        (None, Some(w)) => bail!("no git remote points at {w}"),
        (None, None) => bail!("this repo has no GitHub remote"),
    }
}

/// A PR as typed: `123`, `#123`, or a URL like `https://github.com/o/n/pull/123`.
pub fn parse_pr(input: &str) -> anyhow::Result<(Option<GhRepo>, u32)> {
    let input = input.trim();
    let number = |s: &str| {
        s.trim_start_matches('#')
            .parse::<u32>()
            .ok()
            .filter(|n| *n > 0)
    };
    if let Some(n) = number(input) {
        return Ok((None, n));
    }
    // Links often carry `#discussion_r1` or `?w=1`.
    let input = input.split(['#', '?']).next().unwrap_or_default();
    let path = ["https://", "http://", ""]
        .iter()
        .filter_map(|scheme| input.strip_prefix(scheme))
        .find_map(|rest| {
            rest.strip_prefix("github.com/")
                .or_else(|| rest.strip_prefix("www.github.com/"))
        })
        .context("type a PR number or its github.com URL")?;
    let mut parts = path.split('/');
    let (owner, name, pull, n) = (parts.next(), parts.next(), parts.next(), parts.next());
    match (owner, name, pull, n.and_then(number)) {
        (Some(o), Some(r), Some("pull"), Some(n)) => Ok((
            Some(GhRepo {
                owner: o.to_string(),
                name: r.to_string(),
            }),
            n,
        )),
        _ => bail!("type a PR number or its github.com URL"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(o: &str, n: &str) -> GhRepo {
        GhRepo {
            owner: o.into(),
            name: n.into(),
        }
    }

    #[test]
    fn remote_urls_and_pr_refs_parse() {
        assert_eq!(parse_url("git@github.com:a/b.git"), Some(repo("a", "b")));
        assert_eq!(parse_url("https://github.com/a/b"), Some(repo("a", "b")));
        assert_eq!(
            parse_url("ssh://git@github.com/a/b.git"),
            Some(repo("a", "b"))
        );
        assert_eq!(parse_url("https://gitlab.com/a/b"), None);
        assert_eq!(parse_pr("#12").unwrap(), (None, 12));
        assert_eq!(
            parse_pr("https://github.com/a/b/pull/7/files").unwrap(),
            (Some(repo("a", "b")), 7)
        );
        assert!(parse_pr("https://github.com/a/b/issues/7").is_err());
        let full = parse_pr("http://www.github.com/a/b/pull/7#issuecomment-1").unwrap();
        assert_eq!(full, (Some(repo("a", "b")), 7));
        assert_eq!(
            url_like("git@github.com:x/y", &repo("f", "y")),
            "git@github.com:f/y.git"
        );
    }

    #[test]
    fn picks_upstream_then_origin_or_the_named_repo() {
        let remotes = vec![
            ("origin".to_string(), "git@github.com:me/r.git".to_string()),
            (
                "upstream".to_string(),
                "https://github.com/them/r".to_string(),
            ),
        ];
        assert_eq!(pick(&remotes, None).unwrap().0, "upstream");
        assert_eq!(pick(&remotes, Some(&repo("ME", "r"))).unwrap().0, "origin");
        assert!(pick(&remotes, Some(&repo("x", "r"))).is_err());
        assert!(pick(&[], None).is_err());
    }
}
