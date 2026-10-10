//! Comment tests against real jj repos.

use super::tests::{repo, row_of, write};
use super::*;

fn row_text(review: &ReviewDto, file: u32, row: u32) -> String {
    let rows = review.files[file as usize]
        .hunks
        .iter()
        .flat_map(|h| &h.rows);
    rows.clone().nth(row as usize).unwrap().text.clone()
}

#[tokio::test]
async fn comments_follow_their_code_and_outlive_it() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "a.rs", "fn a() {\n    one();\n    two();\n}\n");
    let review = ctx.open(None, "@", false).await.unwrap();
    let rows = [
        row_of(&review, 0, "    one();", 0),
        row_of(&review, 0, "    two();", 0),
    ];
    let state = ctx
        .add_comment(review.id, &rows, "why two?", "+one\n+two")
        .unwrap();
    assert_eq!(state.comments.len(), 1);

    // Code added above moves the comment with its rows.
    write(
        d,
        "a.rs",
        "fn z() {}\n\nfn a() {\n    one();\n    two();\n}\n",
    );
    let review = ctx.open(None, "@", false).await.unwrap();
    let c = &review.state.comments[0];
    let span = c.rows.unwrap();
    assert_eq!(row_text(&review, c.file, span.start), "    one();");
    assert_eq!(row_text(&review, c.file, span.end), "    two();");

    let id = c.id;
    let state = ctx.edit_comment(review.id, id, "why both?").unwrap();
    assert_eq!(state.comments[0].body, "why both?");

    // With its code gone, the comment stays on its file without rows.
    write(d, "a.rs", "fn z() {}\n");
    let review = ctx.open(None, "@", false).await.unwrap();
    let c = &review.state.comments[0];
    assert_eq!((c.rows, c.snippet.as_str()), (None, "+one\n+two"));

    let state = ctx.delete_comment(review.id, id).unwrap();
    assert!(state.comments.is_empty());
}

#[tokio::test]
async fn reviews_know_whether_they_include_the_working_copy() {
    let (dir, ctx) = repo();
    write(dir.path(), "a.rs", "fn a() {}\n");
    super::tests::jj(dir.path(), &["new"]);
    assert!(
        ctx.open(None, "@-::@", false)
            .await
            .unwrap()
            .has_working_copy
    );
    assert!(!ctx.open(None, "@-", false).await.unwrap().has_working_copy);
}
