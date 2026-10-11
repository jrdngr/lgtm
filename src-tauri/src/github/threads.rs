//! A PR's discussion: review threads on code, conversation comments, and reviews.

use serde::Deserialize;
use serde_json::{Value, json};

use super::gh;
use super::pr::Login;
use super::remote::GhRepo;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadComment {
    pub author: Option<Login>,
    pub body: String,
    pub created_at: String,
    pub url: String,
    /// PENDING while it's in an unsubmitted review.
    pub state: String,
    /// The diff the comment was made on, ending at its line.
    pub diff_hunk: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Thread {
    pub id: String,
    pub is_resolved: bool,
    pub is_outdated: bool,
    pub path: String,
    /// The last line, on the PR head's diff. Null once outdated.
    pub line: Option<u32>,
    pub start_line: Option<u32>,
    /// LEFT (the base) or RIGHT (the head).
    pub diff_side: String,
    pub start_diff_side: Option<String>,
    pub comments: Nodes<ThreadComment>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Nodes<T> {
    pub nodes: Vec<T>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub author: Option<Login>,
    pub body: String,
    pub created_at: String,
    pub url: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Review {
    pub author: Option<Login>,
    pub body: String,
    /// PENDING, COMMENTED, APPROVED, CHANGES_REQUESTED or DISMISSED.
    pub state: String,
    pub submitted_at: Option<String>,
    pub url: String,
    pub viewer_did_author: bool,
}

pub struct Discussion {
    /// The PR's head on GitHub, which threads' lines are numbered on.
    pub head: String,
    pub threads: Vec<Thread>,
    pub comments: Vec<Comment>,
    pub reviews: Vec<Review>,
}

const QUERY: &str = "query($owner:String!,$name:String!,$number:Int!,$after:String){
repository(owner:$owner,name:$name){pullRequest(number:$number){
  headRefOid
  reviewThreads(first:50,after:$after){pageInfo{hasNextPage endCursor} nodes{
    id isResolved isOutdated path line startLine diffSide startDiffSide
    comments(first:100){nodes{author{login} body createdAt url state diffHunk}}
  }}
  comments(last:100){nodes{author{login} body createdAt url}}
  reviews(last:100){nodes{author{login} body state submittedAt url viewerDidAuthor}}
}}}";

pub async fn discussion(repo: &GhRepo, number: u32) -> anyhow::Result<Discussion> {
    let mut threads = Vec::new();
    let mut after = Value::Null;
    loop {
        let vars =
            json!({ "owner": repo.owner, "name": repo.name, "number": number, "after": after });
        let mut v = gh::graphql(QUERY, vars).await?;
        let mut pr: Value = gh::at(&mut v, "/repository/pullRequest")?;
        let more = pr.pointer("/reviewThreads/pageInfo/hasNextPage") == Some(&json!(true));
        after = pr["reviewThreads"]["pageInfo"]["endCursor"].take();
        let page: Nodes<Thread> = gh::at(&mut pr, "/reviewThreads")?;
        threads.extend(page.nodes);
        if !more {
            let comments: Nodes<Comment> = gh::at(&mut pr, "/comments")?;
            let reviews: Nodes<Review> = gh::at(&mut pr, "/reviews")?;
            return Ok(Discussion {
                head: gh::at(&mut pr, "/headRefOid")?,
                threads,
                comments: comments.nodes,
                reviews: reviews.nodes,
            });
        }
    }
}
