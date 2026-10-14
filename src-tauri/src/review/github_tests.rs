//! Placing GitHub threads on a review's rows, and back.

use std::collections::HashMap;

use super::comment_tests::row_text;
use super::tests::{repo, write};
use super::*;
use crate::anchor::Side;
use crate::store::GhLink;

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
    let record = crate::pr::PrRecord {
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
