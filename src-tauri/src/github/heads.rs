//! Open PRs by head branch name.

use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::gh;
use super::pr::{HeadRepo, Login};
use super::remote::GhRepo;

/// An open PR whose head is one of the branches asked about.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadPr {
    pub number: u32,
    pub title: String,
    pub url: String,
    pub is_draft: bool,
    pub author: Option<Login>,
    pub head_ref_name: String,
    /// Gone when the fork was deleted.
    pub head_repository: Option<HeadRepo>,
}

const FIELDS: &str =
    "nodes{number title url isDraft author{login} headRefName headRepository{name owner{login}}}";

/// Open PRs in `repo` from any of `branches`, which may come from forks. One query, with a
/// field per branch.
pub async fn open_prs(repo: &GhRepo, branches: &[&str]) -> anyhow::Result<Vec<HeadPr>> {
    if branches.is_empty() {
        return Ok(Vec::new());
    }
    let params: String = (0..branches.len())
        .map(|i| format!(",$h{i}:String!"))
        .collect();
    let fields: String = (0..branches.len())
        .map(|i| format!("h{i}:pullRequests(headRefName:$h{i},states:OPEN,first:20){{{FIELDS}}} "))
        .collect();
    let query = format!(
        "query($owner:String!,$name:String!{params}){{repository(owner:$owner,name:$name){{{fields}}}}}"
    );
    let mut vars = Map::new();
    vars.insert("owner".into(), json!(repo.owner));
    vars.insert("name".into(), json!(repo.name));
    for (i, b) in branches.iter().enumerate() {
        vars.insert(format!("h{i}"), json!(b));
    }
    let mut v = gh::graphql(&query, Value::Object(vars)).await?;
    let mut prs = Vec::new();
    for i in 0..branches.len() {
        prs.extend(gh::at::<Vec<HeadPr>>(
            &mut v,
            &format!("/repository/h{i}/nodes"),
        )?);
    }
    Ok(prs)
}
