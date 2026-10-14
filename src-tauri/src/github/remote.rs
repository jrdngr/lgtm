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

/// A remote URL's host and path: `[user@]host:path` (scp style) or
/// `scheme://[user@]host[:port]/path`.
fn host_path(url: &str) -> Option<(&str, &str)> {
    let (rest, scp) = match url.split_once("://") {
        Some((_, rest)) => (rest, false),
        None => (url, true),
    };
    let rest = rest.rsplit_once('@').map_or(rest, |(_, r)| r);
    let (host, path) = if scp {
        rest.split_once(':')?
    } else {
        let (host, path) = rest.split_once('/')?;
        (host.split(':').next()?, path)
    };
    Some((host, path))
}

/// The GitHub repo a remote URL points at, over HTTPS or SSH, including SSH host aliases
/// like `github.com-work` from `~/.ssh/config`: any host with "github" in it.
pub fn parse_url(url: &str) -> Option<GhRepo> {
    let (host, _) = host_path(url.trim())?;
    if !host.to_ascii_lowercase().contains("github") {
        return None;
    }
    any_repo(url)
}

/// The `owner/name` a remote URL ends in, on any host.
pub fn any_repo(url: &str) -> Option<GhRepo> {
    split_repo(host_path(url.trim())?.1.trim_start_matches('/'))
}

/// The URL of `repo` on the same host, user and scheme as `like` (the base repo's URL), so
/// SSH aliases and credentials carry over.
pub fn url_like(like: &str, repo: &GhRepo) -> String {
    match parse_url(like) {
        Some(base) => {
            let path = format!("{base}");
            match like.rfind(&path) {
                Some(at) => format!("{}{repo}.git", &like[..at]),
                None => format!("https://github.com/{repo}.git"),
            }
        }
        None => format!("https://github.com/{repo}.git"),
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
    let mut github: Vec<(&String, &String, GhRepo)> = remotes
        .iter()
        .filter_map(|(n, u)| Some((n, u, parse_url(u)?)))
        .collect();
    // With no host that says GitHub, the remotes may use SSH aliases: take them all.
    if github.is_empty() {
        github = remotes
            .iter()
            .filter_map(|(n, u)| Some((n, u, any_repo(u)?)))
            .collect();
    }
    let same = |r: &GhRepo, w: &GhRepo| {
        r.owner.eq_ignore_ascii_case(&w.owner) && r.name.eq_ignore_ascii_case(&w.name)
    };
    // A named repo matches on its path too, for SSH aliases without "github" in them.
    let named = want.and_then(|w| {
        remotes
            .iter()
            .find(|(_, u)| any_repo(u).is_some_and(|r| same(&r, w)))
            .map(|(n, u)| (n, u, w.clone()))
    });
    let found = match want {
        Some(w) => github
            .iter()
            .find(|(_, _, r)| same(r, w))
            .cloned()
            .or(named),
        None => ["upstream", "origin"]
            .iter()
            .find_map(|n| github.iter().find(|(name, _, _)| name.as_str() == *n))
            // `gh-*` remotes are forks lgtm added, not the repo's own.
            .or(github.iter().find(|(name, _, _)| !name.starts_with("gh-")))
            .cloned(),
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
        assert_eq!(
            parse_url("org-1@github.com-work:a/b.git"),
            Some(repo("a", "b"))
        );
        assert_eq!(
            parse_url("ssh://git@ssh.github.com:443/a/b.git"),
            Some(repo("a", "b"))
        );
        assert_eq!(
            parse_url("https://me:tok@github.com/a/b.git"),
            Some(repo("a", "b"))
        );
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
        assert_eq!(
            url_like("git@github-work:x/y.git", &repo("f", "y")),
            "git@github-work:f/y.git"
        );
        assert_eq!(
            url_like("https://github.com/x/y", &repo("f", "y")),
            "https://github.com/f/y.git"
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
        let aliased = vec![("origin".to_string(), "work:me/r.git".to_string())];
        assert_eq!(pick(&aliased, Some(&repo("me", "r"))).unwrap().0, "origin");
        assert!(pick(&[], None).is_err());
        assert_eq!(pick(&aliased, None).unwrap().2, repo("me", "r"));
    }
}
