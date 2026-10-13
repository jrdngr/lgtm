//! Writing to a PR: comments go into the viewer's pending review, which is submitted later
//! with a verdict. Conversation replies post right away.

use serde::Deserialize;
use serde_json::json;
use specta::Type;

use super::gh;

/// Where a new review thread goes, on the PR head's diff.
pub struct Spot<'a> {
    pub path: &'a str,
    /// LEFT or RIGHT.
    pub side: &'a str,
    pub line: u32,
    /// The first line and its side, for a range.
    pub start: Option<(&'a str, u32)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Verdict {
    Comment,
    Approve,
    RequestChanges,
}

impl Verdict {
    fn event(self) -> &'static str {
        match self {
            Verdict::Comment => "COMMENT",
            Verdict::Approve => "APPROVE",
            Verdict::RequestChanges => "REQUEST_CHANGES",
        }
    }
}

const PENDING: &str = "query($id:ID!){node(id:$id){...on PullRequest{reviews(last:20,states:PENDING){nodes{id viewerDidAuthor commit{oid}}}}}}";

#[derive(Deserialize)]
struct Oid {
    oid: String,
}

/// The viewer's pending review on the PR, if there is one, and the commit it's on.
async fn pending(pr: &str) -> anyhow::Result<Option<(String, Option<String>)>> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct R {
        id: String,
        viewer_did_author: bool,
        commit: Option<Oid>,
    }
    let mut v = gh::graphql(PENDING, json!({ "id": pr })).await?;
    let reviews: Vec<R> = gh::at(&mut v, "/node/reviews/nodes")?;
    Ok(reviews
        .into_iter()
        .find(|r| r.viewer_did_author)
        .map(|r| (r.id, r.commit.map(|c| c.oid))))
}

const START: &str = "mutation($pr:ID!,$commit:GitObjectID!){addPullRequestReview(input:{pullRequestId:$pr,commitOID:$commit}){pullRequestReview{id}}}";

/// The viewer's pending review, started on `head` when there's none.
async fn review_for(pr: &str, head: &str) -> anyhow::Result<String> {
    if let Some((id, commit)) = pending(pr).await? {
        // GitHub places its threads' lines on the commit it was started on.
        anyhow::ensure!(
            commit.as_deref().is_none_or(|c| c == head),
            "your pending review on GitHub is on an older commit of the PR. Submit it (or \
             discard it on GitHub) before publishing on the refetched head"
        );
        return Ok(id);
    }
    let mut v = gh::graphql(START, json!({ "pr": pr, "commit": head })).await?;
    gh::at(&mut v, "/addPullRequestReview/pullRequestReview/id")
}

const THREAD: &str = "mutation($review:ID!,$path:String!,$body:String!,$line:Int!,$side:DiffSide!,$startLine:Int,$startSide:DiffSide){
addPullRequestReviewThread(input:{pullRequestReviewId:$review,path:$path,body:$body,line:$line,side:$side,startLine:$startLine,startSide:$startSide}){thread{id}}}";

/// Starts a thread in the pending review. Returns the thread's id.
pub async fn add_thread(pr: &str, head: &str, at: &Spot<'_>, body: &str) -> anyhow::Result<String> {
    let review = review_for(pr, head).await?;
    let vars = json!({
        "review": review,
        "path": at.path,
        "body": body,
        "line": at.line,
        "side": at.side,
        "startLine": at.start.map(|s| s.1),
        "startSide": at.start.map(|s| s.0),
    });
    let mut v = gh::graphql(THREAD, vars).await?;
    gh::at(&mut v, "/addPullRequestReviewThread/thread/id")
}

const REPLY: &str = "mutation($review:ID!,$thread:ID!,$body:String!){
addPullRequestReviewThreadReply(input:{pullRequestReviewId:$review,pullRequestReviewThreadId:$thread,body:$body}){comment{id}}}";

/// Replies to a thread in the pending review.
pub async fn reply(pr: &str, head: &str, thread: &str, body: &str) -> anyhow::Result<()> {
    let review = review_for(pr, head).await?;
    gh::graphql(
        REPLY,
        json!({ "review": review, "thread": thread, "body": body }),
    )
    .await?;
    Ok(())
}

const SUBMIT: &str = "mutation($review:ID!,$event:PullRequestReviewEvent!,$body:String){
submitPullRequestReview(input:{pullRequestReviewId:$review,event:$event,body:$body}){pullRequestReview{id}}}";

const REVIEW: &str = "mutation($pr:ID!,$commit:GitObjectID!,$event:PullRequestReviewEvent!,$body:String){
addPullRequestReview(input:{pullRequestId:$pr,commitOID:$commit,event:$event,body:$body}){pullRequestReview{id}}}";

/// Submits the pending review, or with none, a review with just the verdict and summary.
pub async fn submit(pr: &str, head: &str, verdict: Verdict, body: &str) -> anyhow::Result<()> {
    let body = (!body.trim().is_empty()).then_some(body);
    match pending(pr).await?.map(|p| p.0) {
        Some(review) => {
            let vars = json!({ "review": review, "event": verdict.event(), "body": body });
            gh::graphql(SUBMIT, vars).await?;
        }
        None => {
            anyhow::ensure!(
                verdict != Verdict::Comment || body.is_some(),
                "there's nothing to submit: publish a comment or write a summary"
            );
            let vars = json!({ "pr": pr, "commit": head, "event": verdict.event(), "body": body });
            gh::graphql(REVIEW, vars).await?;
        }
    }
    Ok(())
}

const COMMENT: &str = "mutation($pr:ID!,$body:String!){addComment(input:{subjectId:$pr,body:$body}){clientMutationId}}";

/// Posts to the PR's conversation right away.
pub async fn comment(pr: &str, body: &str) -> anyhow::Result<()> {
    gh::graphql(COMMENT, json!({ "pr": pr, "body": body })).await?;
    Ok(())
}

const RESOLVE: &str =
    "mutation($thread:ID!){resolveReviewThread(input:{threadId:$thread}){thread{id}}}";

/// Resolves a review thread right away.
pub async fn resolve(thread: &str) -> anyhow::Result<()> {
    gh::graphql(RESOLVE, json!({ "thread": thread })).await?;
    Ok(())
}

/// Whether the viewer has a pending review on the PR.
pub async fn has_pending(pr: &str) -> anyhow::Result<bool> {
    Ok(pending(pr).await?.is_some())
}
