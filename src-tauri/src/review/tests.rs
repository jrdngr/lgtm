//! Review tests against real jj repos.

use super::*;
use crate::diff::RowKind;

pub use crate::jj::test_support::{jj, write};

pub fn repo() -> (tempfile::TempDir, RepoCtx) {
    let (dir, j) = crate::jj::test_support::repo();
    let ctx = RepoCtx::new(dir.path().into(), j, Store::in_memory());
    (dir, ctx)
}

/// (file, text) of each marked row with its state.
pub fn marked(review: &ReviewDto, marks: &[MarkDto]) -> Vec<(String, MarkColor)> {
    marks
        .iter()
        .map(|m| {
            let f = &review.files[m.file as usize];
            let row = f
                .hunks
                .iter()
                .flat_map(|h| &h.rows)
                .nth(m.row as usize)
                .unwrap();
            (row.text.clone(), m.color)
        })
        .collect()
}

pub fn row_of(review: &ReviewDto, file: usize, text: &str, nth: usize) -> RowRef {
    let rows = review.files[file].hunks.iter().flat_map(|h| &h.rows);
    let idx = rows
        .enumerate()
        .filter(|(_, r)| r.text == text && r.kind != RowKind::Context)
        .nth(nth)
        .unwrap_or_else(|| panic!("no changed row {text:?}"))
        .0;
    RowRef {
        file: file as u32,
        row: idx as u32,
    }
}

#[tokio::test]
async fn marks_follow_code_through_edits_and_rewrites() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(
        d,
        "a.rs",
        "fn a() {\n    x();\n}\n\nfn b() {\n    x();\n}\n",
    );
    jj(d, &["describe", "-m", "add a"]);

    let review = ctx.open(None, "@", false).await.unwrap();
    let in_b = row_of(&review, 0, "    x();", 1);
    let marks = ctx
        .mark(review.id, &[in_b], Some(MarkColor::Red), &[])
        .unwrap()
        .marks;
    assert_eq!(
        marked(&review, &marks),
        [("    x();".into(), MarkColor::Red)]
    );

    // Insert a function above both and reformat: the mark stays on the copy in `b`.
    write(
        d,
        "a.rs",
        "fn z() {}\n\nfn a() {\n    x();\n}\n\nfn b() {\n      x();\n}\n",
    );
    let review = ctx.open(None, "@", false).await.unwrap();
    let b_row = row_of(&review, 0, "      x();", 0);
    assert_eq!(review.state.marks.len(), 1);
    assert_eq!(
        (review.state.marks[0].file, review.state.marks[0].row),
        (b_row.file, b_row.row)
    );

    // A new commit on top doesn't change the reviewed stack's code.
    jj(d, &["new", "-m", "next"]);
    let review = ctx.open(None, "@-", false).await.unwrap();
    assert_eq!(review.state.marks.len(), 1);
}

#[tokio::test]
async fn undo_and_redo_step_through_mark_changes() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "a.py", "def f():\n    return 1\n");
    let review = ctx.open(None, "@", false).await.unwrap();
    let row = row_of(&review, 0, "    return 1", 0);
    ctx.mark(review.id, &[row], Some(MarkColor::Cyan), &[])
        .unwrap();
    let marks = ctx
        .mark(review.id, &[row], Some(MarkColor::Gray), &[])
        .unwrap()
        .marks;
    assert_eq!(marks[0].color, MarkColor::Gray);

    assert_eq!(ctx.undo(review.id).unwrap().marks[0].color, MarkColor::Cyan);
    assert!(ctx.undo(review.id).unwrap().marks.is_empty());
    assert_eq!(ctx.redo(review.id).unwrap().marks[0].color, MarkColor::Cyan);
}

#[tokio::test]
async fn marks_survive_a_rename_added_to_the_reviewed_change() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "old.py", "def f():\n    return 1\n");
    jj(d, &["new", "-m", "change"]);
    write(d, "old.py", "def f():\n    log()\n    return 1\n");
    let review = ctx.open(None, "@", false).await.unwrap();
    let row = row_of(&review, 0, "    log()", 0);
    ctx.mark(review.id, &[row], Some(MarkColor::Cyan), &[])
        .unwrap();

    std::fs::rename(d.join("old.py"), d.join("new.py")).unwrap();
    let review = ctx.open(None, "@", false).await.unwrap();
    assert_eq!(review.files[0].old_path.as_deref(), Some("old.py"));
    assert_eq!(
        marked(&review, &review.state.marks),
        [("    log()".into(), MarkColor::Cyan)]
    );
}

#[tokio::test]
async fn marks_follow_a_heavily_edited_rename_reported_as_delete_and_add() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "old.rs", "fn a() {}\nfn b() {}\nfn c() {}\n");
    jj(d, &["new", "-m", "change"]);
    write(
        d,
        "old.rs",
        "fn a() {}\nfn b() {}\nfn c() {}\nlet marked = 1;\n",
    );
    let review = ctx.open(None, "@", false).await.unwrap();
    let file = review
        .files
        .iter()
        .position(|f| f.path == "old.rs")
        .unwrap();
    let row = row_of(&review, file, "let marked = 1;", 0);
    ctx.mark(review.id, &[row], Some(MarkColor::Red), &[])
        .unwrap();

    std::fs::remove_file(d.join("old.rs")).unwrap();
    write(
        d,
        "new.rs",
        "fn x() {}\nfn y() {}\nfn z() {}\nfn w() {}\nlet marked = 1;\n",
    );
    let review = ctx.open(None, "@", false).await.unwrap();
    assert!(review.files.iter().all(|f| f.old_path.is_none()));
    assert_eq!(
        marked(&review, &review.state.marks),
        [("let marked = 1;".into(), MarkColor::Red)]
    );
}

#[tokio::test]
async fn a_mark_on_a_split_file_matches_in_only_one_of_its_parts() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "old.rs", "fn a() {}\n");
    jj(d, &["new", "-m", "change"]);
    write(d, "old.rs", "fn a() {}\nuse x;\n");
    let review = ctx.open(None, "@", false).await.unwrap();
    let row = row_of(&review, 0, "use x;", 0);
    ctx.mark(review.id, &[row], Some(MarkColor::Cyan), &[])
        .unwrap();

    std::fs::remove_file(d.join("old.rs")).unwrap();
    write(d, "b.rs", "use x;\nfn b() {}\n");
    write(d, "c.rs", "use x;\nfn c() {}\n");
    let review = ctx.open(None, "@", false).await.unwrap();
    assert_eq!(
        marked(&review, &review.state.marks),
        [("use x;".into(), MarkColor::Cyan)]
    );
}

#[tokio::test]
async fn reapplying_same_state_is_a_no_op_and_reset_removes_review_marks() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "a.go", "package a\n\nfunc F() {}\n");
    let review = ctx.open(None, "@", false).await.unwrap();
    let row = row_of(&review, 0, "func F() {}", 0);
    ctx.mark(review.id, &[row], Some(MarkColor::Cyan), &[])
        .unwrap();
    ctx.mark(review.id, &[row], Some(MarkColor::Cyan), &[])
        .unwrap();
    // One undo step undoes the only real change.
    assert!(ctx.undo(review.id).unwrap().marks.is_empty());
    ctx.redo(review.id).unwrap();
    assert!(ctx.reset(review.id).unwrap().marks.is_empty());
}

#[tokio::test]
async fn review_with_two_heads_is_rejected_without_a_session() {
    let (dir, ctx) = repo();
    let d = dir.path();
    jj(d, &["new", "root()", "-m", "a"]);
    jj(d, &["new", "root()", "-m", "b"]);
    ctx.open(None, "@", false).await.unwrap();
    let err = ctx.open(None, "root()+", false).await.err().unwrap();
    assert!(format!("{err:#}").contains("2 heads"), "{err:#}");
    let store = ctx.store.lock().unwrap();
    assert_eq!(
        store.last_revset(ctx.main.key()).unwrap().as_deref(),
        Some("@")
    );
    let label = ctx.main.label("root()+");
    assert_eq!(store.last_session(&label).unwrap(), None);
}
