//! Placing GitHub threads on a review's rows, and back.

use std::collections::HashMap;

use super::comment_tests::row_text;
use super::tests::{repo, write};
use super::*;
use crate::anchor::Side;
use crate::store::{Author, GhLink, GhPost, Stage, TakeVerdict};

fn thread(id: &str, line: Option<u32>) -> PulledThread {
    PulledThread {
        link: GhLink {
            repo: "o/r".into(),
            pr: 1,
            thread: id.into(),
            url: String::new(),
            resolved: false,
            outdated: line.is_none(),
            posts: vec![],
            line: None,
        },
        path: "a.rs".into(),
        side: Side::New,
        start: None,
        line,
        hunk: "@@ hunk".into(),
        answered: false,
    }
}

/// A workspace checked out for PR o/r#1.
async fn pr_workspace(
    ctx: &mut RepoCtx,
    base: &std::path::Path,
) -> Arc<crate::workspace::Workspace> {
    ctx.workspace_base = Some(base.to_path_buf());
    ctx.create_workspace("pr1").await.unwrap();
    let record = crate::store::PrRecord {
        workspace: "pr1".into(),
        repo: crate::github::GhRepo {
            owner: "o".into(),
            name: "r".into(),
        },
        number: 1,
        id: String::new(),
        title: String::new(),
        url: String::new(),
        author: String::new(),
        base_ref: "main".into(),
        base_remote: "origin".into(),
        head_ref: String::new(),
        head_remote: None,
        added_remote: None,
        head: String::new(),
        stack: false,
        below: None,
    };
    ctx.store.lock().unwrap().save_pr(&record).unwrap();
    ctx.workspace(Some("pr1")).await.unwrap()
}

#[tokio::test]
async fn threads_land_on_their_lines_through_working_copy_edits() {
    let (dir, mut ctx) = repo();
    let base = tempfile::tempdir().unwrap();
    let ws = pr_workspace(&mut ctx, base.path()).await;
    let head = "fn a() {\n    one();\n    two();\n}\n";
    // The working copy dropped a line above the code the PR head has.
    write(&ws.root, "a.rs", "    one();\n    two();\n}\n");
    let review = ctx.open(Some("pr1"), "@", false).await.unwrap();
    let texts = HashMap::from([("a.rs".to_string(), head.to_string())]);
    let threads = vec![thread("T1", Some(3)), thread("T2", None)];
    let state = ctx
        .add_threads(review.id, threads, &texts, "PR #1")
        .unwrap();
    let placed = state.comments.iter().find(|c| c.rows.is_some()).unwrap();
    let span = placed.rows.unwrap();
    assert_eq!(row_text(&review, placed.file, span.end), "    two();");
    let outdated = state.comments.iter().find(|c| c.rows.is_none()).unwrap();
    assert_eq!(outdated.snippet, "@@ hunk");

    // It maps back to the head's line, and pulling again updates rather than adds.
    let spot = ctx.comment_spot(review.id, placed.id, Some(head)).unwrap();
    assert_eq!((spot.side, spot.line, spot.start), (Side::New, 3, None));
    let state = ctx
        .add_threads(review.id, vec![thread("T1", Some(3))], &texts, "PR #1")
        .unwrap();
    assert_eq!(state.comments.len(), 2);

    // Another workspace's review of the same code doesn't show the PR's threads.
    write(dir.path(), "a.rs", "    one();\n    two();\n}\n");
    let other = ctx.open(None, "@", false).await.unwrap();
    assert!(other.state.comments.is_empty());
}

#[tokio::test]
async fn a_thread_stays_on_its_line_when_the_working_copy_edits_it() {
    let (_dir, mut ctx) = repo();
    let base = tempfile::tempdir().unwrap();
    let ws = pr_workspace(&mut ctx, base.path()).await;
    let head = "fn old_name() {\n    one();\n}\n\nfn other() {}\n";
    write(&ws.root, "a.rs", head);
    let review = ctx.open(Some("pr1"), "@", false).await.unwrap();
    let texts = HashMap::from([("a.rs".to_string(), head.to_string())]);
    ctx.add_threads(review.id, vec![thread("T1", Some(1))], &texts, "PR #1")
        .unwrap();

    // Renaming the function, with a new function added just above it, edits the thread's line.
    write(
        &ws.root,
        "a.rs",
        "fn helper() {}\n\nfn new_name() {\n    one();\n}\n\nfn other() {}\n",
    );
    let review = ctx.open(Some("pr1"), "@", false).await.unwrap();
    let c = &review.state.comments[0];
    let span = c.rows.expect("the thread keeps its rows");
    assert_eq!(row_text(&review, c.file, span.end), "fn new_name() {");
}

#[tokio::test]
async fn a_thread_on_a_line_the_working_copy_edited_lands_on_its_replacement() {
    let (_dir, mut ctx) = repo();
    let base = tempfile::tempdir().unwrap();
    let ws = pr_workspace(&mut ctx, base.path()).await;
    let head = "fn a() {\n    one();\n    two();\n}\n";
    write(&ws.root, "a.rs", "fn a() {\n    one();\n    three();\n}\n");
    let review = ctx.open(Some("pr1"), "@", false).await.unwrap();
    let texts = HashMap::from([("a.rs".to_string(), head.to_string())]);
    let state = ctx
        .add_threads(review.id, vec![thread("T1", Some(3))], &texts, "PR #1")
        .unwrap();
    let c = &state.comments[0];
    let span = c.rows.expect("the thread is placed");
    assert_eq!(row_text(&review, c.file, span.end), "    three();");
    // A terminal agent finds it among the threads, not the notes.
    assert!(
        ctx.review(review.id)
            .unwrap()
            .lock()
            .unwrap()
            .agent_comments()
            .is_empty()
    );
}

#[tokio::test]
async fn a_thread_whose_line_is_deleted_stays_near_it() {
    let (_dir, mut ctx) = repo();
    let base = tempfile::tempdir().unwrap();
    let ws = pr_workspace(&mut ctx, base.path()).await;
    let head = "fn a() {\n    one();\n    two();\n    four();\n}\n";
    write(&ws.root, "a.rs", head);
    let review = ctx.open(Some("pr1"), "@", false).await.unwrap();
    let texts = HashMap::from([("a.rs".to_string(), head.to_string())]);
    ctx.add_threads(review.id, vec![thread("T1", Some(3))], &texts, "PR #1")
        .unwrap();

    write(&ws.root, "a.rs", "fn a() {\n    one();\n    four();\n}\n");
    let review = ctx.open(Some("pr1"), "@", false).await.unwrap();
    let c = &review.state.comments[0];
    let span = c.rows.expect("the thread keeps a place");
    assert_eq!(row_text(&review, c.file, span.end), "    four();");
}

#[tokio::test]
async fn quotes_answer_parts_of_a_pr_comment_in_its_place() {
    let (_dir, mut ctx) = repo();
    let base = tempfile::tempdir().unwrap();
    let ws = pr_workspace(&mut ctx, base.path()).await;
    write(&ws.root, "a.rs", "fn a() {}\n");
    let review = ctx.open(Some("pr1"), "@", false).await.unwrap();
    let mut on_pr = thread("C1", None);
    on_pr.path = String::new();
    on_pr.link.posts = vec![GhPost {
        author: "bot".into(),
        body: "Why a 500? And why count it?".into(),
        created_at: String::new(),
        pending: false,
    }];
    let state = ctx
        .add_threads(review.id, vec![on_pr], &HashMap::new(), "PR #1")
        .unwrap();
    let parent = state.threads[0].id;

    let state = ctx.add_quote(review.id, parent, " Why a 500? ").unwrap();
    let quote = state.threads.iter().find(|t| t.id != parent).unwrap();
    let q = quote.state.quote.as_ref().unwrap();
    assert_eq!(
        (q.of, q.author.as_str(), q.text.as_str()),
        (parent, "bot", "Why a 500?")
    );

    // A reply is worked out like one on a GitHub thread, not sent like a new draft.
    let state = ctx
        .edit_comment(review.id, quote.id, "It isn't now.")
        .unwrap();
    let edited = state.threads.iter().find(|t| t.id == quote.id).unwrap();
    assert_eq!(edited.state.stage, Stage::Reply);
}

#[tokio::test]
async fn a_reply_posted_from_elsewhere_finishes_its_thread() {
    let (_dir, mut ctx) = repo();
    let base = tempfile::tempdir().unwrap();
    let ws = pr_workspace(&mut ctx, base.path()).await;
    write(&ws.root, "a.rs", "fn a() {}\n");
    let review = ctx.open(Some("pr1"), "@", false).await.unwrap();
    let texts = HashMap::new();
    let post = |author: &str, body: &str| GhPost {
        author: author.into(),
        body: body.into(),
        created_at: String::new(),
        pending: false,
    };
    let mut asked = thread("T1", None);
    asked.link.posts = vec![post("rev", "why?")];
    let state = ctx
        .add_threads(review.id, vec![asked], &texts, "PR #1")
        .unwrap();
    let id = state.comments[0].id;

    // A terminal agent's take sorts the thread like triage does.
    ctx.focus(Some("pr1"), review.id);
    let take = |verdict| crate::store::Take {
        verdict,
        why: "facts".into(),
        question: None,
    };
    let stage = |state: &StateDto| state.comments[0].thread.as_ref().unwrap().stage;
    let (_, state) = ctx
        .agent_set_take(Some("pr1"), 1, id, take(TakeVerdict::Agree), None, None)
        .unwrap();
    assert_eq!(stage(&state), Stage::ToFix);
    let reply = Some("Because.".to_string());
    let (_, state) = ctx
        .agent_set_take(
            Some("pr1"),
            1,
            id,
            take(TakeVerdict::Answer),
            reply,
            Some(" Why 2 ".into()),
        )
        .unwrap();
    assert_eq!(stage(&state), Stage::Reply);
    assert_eq!(state.comments[0].thread.as_ref().unwrap().summary, "Why 2");
    // Only the human marks a reply ready.
    assert!(
        ctx.agent_set_stage(Some("pr1"), 1, id, Stage::Ready)
            .is_err()
    );
    // Thread writes name the thread's PR, and a thread of another PR is refused.
    let Err(err) = ctx.agent_set_stage(Some("pr1"), 2, id, Stage::Done) else {
        panic!("a write about another PR went through");
    };
    assert!(err.to_string().contains("moved to PR #1"), "{err}");
    assert!(ctx.agent_suggest_reply(Some("pr1"), 2, id, "x").is_err());
    let (_, t) = ctx
        .with_current(Some("pr1"), |r| r.threads[0].clone())
        .unwrap();
    assert_eq!(
        (t.state.stage, t.suggestion),
        (Stage::Reply, None),
        "nothing changed"
    );

    let mut answered = thread("T1", None);
    answered.link.posts = vec![post("rev", "why?"), post("me", "Because.")];
    answered.answered = true;
    let state = ctx
        .add_threads(review.id, vec![answered], &texts, "PR #1")
        .unwrap();
    let c = &state.comments[0];
    assert_eq!(c.thread.as_ref().unwrap().stage, Stage::Done);
    assert_eq!(c.body, "", "the sent reply isn't left to send again");
}

#[tokio::test]
async fn a_terminal_agent_suggests_a_reply_that_becomes_the_thread_reply() {
    let (_dir, mut ctx) = repo();
    let base = tempfile::tempdir().unwrap();
    let ws = pr_workspace(&mut ctx, base.path()).await;
    write(&ws.root, "a.rs", "fn a() {}\n");
    let review = ctx.open(Some("pr1"), "@", false).await.unwrap();
    let state = ctx
        .add_threads(
            review.id,
            vec![thread("T1", None)],
            &HashMap::new(),
            "PR #1",
        )
        .unwrap();
    let id = state.comments[0].id;
    ctx.focus(Some("pr1"), review.id);

    ctx.add_chat_message(review.id, id, Author::User, "  ")
        .unwrap();
    let state = ctx
        .add_chat_message(review.id, id, Author::User, "Is that true?")
        .unwrap();
    assert_eq!(
        state.comments[0].messages.len(),
        1,
        "blank text adds nothing"
    );

    let (_, state) = ctx
        .agent_suggest_reply(Some("pr1"), 1, id, "Good catch.\n")
        .unwrap();
    assert_eq!(state.comments[0].suggestion.as_deref(), Some("Good catch."));

    let state = ctx.take_suggestion(review.id, id).unwrap();
    let c = &state.comments[0];
    assert_eq!(c.body, "Good catch.");
    assert_eq!(c.suggestion, None);
    assert_eq!(c.thread.as_ref().unwrap().stage, Stage::Reply);
    assert_eq!(c.messages.len(), 1, "the chat stays");
    assert!(ctx.take_suggestion(review.id, id).is_err());

    ctx.agent_suggest_reply(Some("pr1"), 1, id, "Done.")
        .unwrap();
    let state = ctx.dismiss_suggestion(review.id, id).unwrap();
    assert_eq!(state.comments[0].suggestion, None);
    assert_eq!(state.comments[0].body, "Good catch.");
}
