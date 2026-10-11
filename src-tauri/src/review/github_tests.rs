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
        },
        path: "a.rs".into(),
        side: Side::New,
        start: None,
        line,
        hunk: "@@ hunk".into(),
    }
}

#[tokio::test]
async fn threads_land_on_their_lines_through_working_copy_edits() {
    let (dir, ctx) = repo();
    let head = "fn a() {\n    one();\n    two();\n}\n";
    // The working copy dropped a line above the code the PR head has.
    write(dir.path(), "a.rs", "    one();\n    two();\n}\n");
    let review = ctx.open(None, "@", false).await.unwrap();
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
}
