//! A pull request's metadata, and how its head moved since a fetch.

use serde::Deserialize;
use serde_json::json;

use super::gh;
use super::remote::GhRepo;

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

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrInfo {
    /// The node id mutations take.
    pub id: String,
    pub number: u32,
    pub title: String,
    pub url: String,
    /// OPEN, CLOSED or MERGED.
    pub state: String,
    pub author: Option<Login>,
    pub base_ref_name: String,
    pub head_ref_name: String,
    pub head_ref_oid: String,
    pub is_cross_repository: bool,
    /// Gone when the fork was deleted.
    pub head_repository: Option<HeadRepo>,
}

const QUERY: &str = "query($owner:String!,$name:String!,$number:Int!){repository(owner:$owner,name:$name){pullRequest(number:$number){
  id number title url state author{login}
  baseRefName headRefName headRefOid isCrossRepository headRepository{name owner{login}}
}}}";

pub async fn info(repo: &GhRepo, number: u32) -> anyhow::Result<PrInfo> {
    let mut v = gh::graphql(
        QUERY,
        json!({ "owner": repo.owner, "name": repo.name, "number": number }),
    )
    .await?;
    gh::at(&mut v, "/repository/pullRequest")
}

/// How far `new` is from `old`: commits ahead, and whether `old` is still under it.
pub async fn compare(repo: &GhRepo, old: &str, new: &str) -> anyhow::Result<(u32, bool)> {
    let v = gh::rest(&format!("repos/{repo}/compare/{old}...{new}")).await?;
    let ahead = v["ahead_by"].as_u64().unwrap_or(0) as u32;
    let kept = matches!(v["status"].as_str(), Some("ahead" | "identical"));
    Ok((ahead, kept))
}
