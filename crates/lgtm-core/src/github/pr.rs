//! A pull request's metadata, and how its head moved since a fetch.

use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct Login {
    pub login: String,
}

/// An author, or "ghost" for deleted accounts, which GitHub returns as null.
pub fn login(l: &Option<Login>) -> String {
    l.as_ref()
        .map_or_else(|| "ghost".into(), |l| l.login.clone())
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadRepo {
    pub name: String,
    pub owner: Login,
}

/// A PR's base and head commits, as GitHub has them now.
pub async fn ends(repo: &super::GhRepo, number: u32) -> anyhow::Result<(String, String)> {
    let mut v = super::gh::rest(&format!(
        "repos/{}/{}/pulls/{number}",
        repo.owner, repo.name
    ))
    .await?;
    let base: String = super::gh::at(&mut v, "/base/sha")?;
    let head: String = super::gh::at(&mut v, "/head/sha")?;
    Ok((base, head))
}
