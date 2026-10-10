//! M2 review workflow tests: file checks, amber rows, views, refresh.

use super::tests::{jj, marked, repo, row_of, write};
use super::*;
use crate::store::MarkState;
use views::Since;

fn rows_text(review: &ReviewDto, rows: &[RowRef]) -> Vec<String> {
    rows.iter()
        .map(|r| {
            let f = &review.files[r.file as usize];
            f.hunks
                .iter()
                .flat_map(|h| &h.rows)
                .nth(r.row as usize)
                .unwrap()
                .text
                .clone()
        })
        .collect()
}

#[tokio::test]
async fn checked_file_unchecks_when_new_rows_arrive_and_they_turn_amber() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "a.py", "def f():\n    return 1\n");
    let review = ctx.open("@", false).await.unwrap();
    let state = ctx.set_checked(review.id, &[0], true).unwrap();
    assert_eq!(state.checks, [CheckState::Checked]);

    // Same code in a new commit on top of a new parent: still checked.
    jj(d, &["describe", "-m", "a"]);
    let review = ctx.open("@", false).await.unwrap();
    assert_eq!(review.state.checks, [CheckState::Checked]);

    write(d, "a.py", "def f():\n    log()\n    return 1\n");
    let review = ctx.open("@", false).await.unwrap();
    assert_eq!(review.state.checks, [CheckState::ChangedSinceReviewed]);
    assert_eq!(rows_text(&review, &review.state.amber), ["    log()"]);

    // Checking again covers the new row; undo brings back the stale check.
    let state = ctx.set_checked(review.id, &[0], true).unwrap();
    assert_eq!(
        (state.checks[0], state.amber.len()),
        (CheckState::Checked, 0)
    );
    let state = ctx.undo(review.id).unwrap();
    assert_eq!(state.checks, [CheckState::ChangedSinceReviewed]);
    let state = ctx.set_checked(review.id, &[0], false).unwrap();
    assert_eq!(
        (state.checks[0], state.amber.len()),
        (CheckState::Unchecked, 0)
    );
}

#[tokio::test]
async fn whole_file_reviewed_marks_lines_and_checks_in_one_step() {
    let (dir, ctx) = repo();
    write(dir.path(), "a.go", "package a\n");
    let review = ctx.open("@", false).await.unwrap();
    let row = row_of(&review, 0, "package a", 0);
    let state = ctx
        .mark(review.id, &[row], Some(MarkState::Reviewed), &[0])
        .unwrap();
    assert_eq!(
        (state.marks.len(), state.checks[0]),
        (1, CheckState::Checked)
    );
    let state = ctx.undo(review.id).unwrap();
    assert_eq!(
        (state.marks.len(), state.checks[0]),
        (0, CheckState::Unchecked)
    );
}

#[tokio::test]
async fn rows_replacing_a_lost_mark_turn_amber_in_the_next_session() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "a.rs", "fn a() {\n    one();\n}\n");
    let review = ctx.open("@", false).await.unwrap();
    let row = row_of(&review, 0, "    one();", 0);
    ctx.mark(review.id, &[row], Some(MarkState::Reviewed), &[])
        .unwrap();

    write(d, "a.rs", "fn a() {\n    two();\n}\n");
    let review = ctx.open("@", false).await.unwrap();
    assert!(review.state.marks.is_empty());
    assert_eq!(rows_text(&review, &review.state.amber), ["    two();"]);

    // Reviewing the amber row clears it.
    let row = row_of(&review, 0, "    two();", 0);
    let state = ctx
        .mark(review.id, &[row], Some(MarkState::Reviewed), &[])
        .unwrap();
    assert!(state.amber.is_empty());
}

#[tokio::test]
async fn skip_globs_from_config_flag_files() {
    let (dir, ctx) = repo();
    let d = dir.path();
    std::fs::create_dir_all(d.join(".lgtm")).unwrap();
    write(d, ".lgtm/config.toml", "skip = [\"*.lock\"]\n");
    write(d, "Cargo.lock", "x\n");
    write(d, "main.rs", "fn main() {}\n");
    let review = ctx.open("@", false).await.unwrap();
    let skipped: Vec<_> = review
        .files
        .iter()
        .map(|f| (f.path.as_str(), f.skipped))
        .collect();
    assert!(skipped.contains(&("Cargo.lock", true)));
    assert!(skipped.contains(&("main.rs", false)));
}

#[tokio::test]
async fn per_change_and_since_last_session_views() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "a.txt", "one\n");
    jj(d, &["describe", "-m", "first"]);
    jj(d, &["new", "-m", "second"]);
    write(d, "b.txt", "two\n");
    let review = ctx.open("root()..@", false).await.unwrap();
    assert_eq!(review.changes.len(), 2);
    assert!(review.changes.iter().all(|c| c.since == Since::Unknown));
    let first = review.changes[0].change_id.clone();
    let second = review.changes[1].change_id.clone();

    let one = ctx
        .refresh(
            review.id,
            Some(View::Change {
                change: first.clone(),
            }),
        )
        .await
        .unwrap()
        .review;
    let paths: Vec<_> = one.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, ["a.txt"]);

    // Next session: the second change was edited, the first wasn't.
    write(d, "b.txt", "two\nthree\n");
    let review = ctx.open("root()..@", false).await.unwrap();
    let since: Vec<_> = review.changes.iter().map(|c| c.since).collect();
    assert_eq!(since, [Since::Same, Since::Changed]);
    assert!(review.previous_session.is_some());

    let view = View::SinceLast { change: second };
    let inter = ctx.refresh(review.id, Some(view)).await.unwrap().review;
    assert!(inter.interdiff);
    let added: Vec<_> = inter.files[0]
        .hunks
        .iter()
        .flat_map(|h| &h.rows)
        .filter(|r| r.kind == crate::diff::RowKind::Added)
        .map(|r| r.text.as_str())
        .collect();
    assert_eq!(added, ["three"]);
    let same = View::SinceLast { change: first };
    let same = ctx.refresh(review.id, Some(same)).await.unwrap().review;
    assert!(same.files.is_empty());

    // Loading the same revset again continues the session: still compared with the first.
    let again = ctx.open("root()..@", true).await.unwrap();
    let since: Vec<_> = again.changes.iter().map(|c| c.since).collect();
    assert_eq!(since, [Since::Same, Since::Changed]);

    // Abandoning a change lists it as removed next session.
    jj(d, &["abandon", "@-"]);
    let review = ctx.open("root()..@", false).await.unwrap();
    assert_eq!(review.removed_changes.len(), 1);
    assert_eq!(review.removed_changes[0].description, "first");
}

#[tokio::test]
async fn refresh_maps_rows_to_where_the_code_moved() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "a.rs", "fn a() {}\nfn b() {}\n");
    let review = ctx.open("@", false).await.unwrap();
    let b = row_of(&review, 0, "fn b() {}", 0);
    let state = ctx
        .mark(review.id, &[b], Some(MarkState::Flagged), &[])
        .unwrap();
    assert_eq!(marked(&review, &state.marks).len(), 1);

    write(d, "a.rs", "fn z() {}\nfn a() {}\nfn b() {}\n");
    ctx.snapshot().await.unwrap();
    let refreshed = ctx.refresh(review.id, None).await.unwrap();
    let new_b = row_of(&refreshed.review, 0, "fn b() {}", 0);
    let moved = refreshed.moves.iter().find(|m| m.from == b).unwrap();
    assert_eq!(moved.to, new_b);
    assert_eq!(
        marked(&refreshed.review, &refreshed.review.state.marks),
        [("fn b() {}".to_string(), MarkState::Flagged)]
    );
    // Undo history survives the refresh.
    assert!(ctx.undo(review.id).unwrap().marks.is_empty());
}

#[tokio::test]
async fn unchanged_files_reused_on_refresh_still_take_marks() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "a.rs", "fn a() {}\n");
    write(d, "b.rs", "fn b() {}\n");
    let review = ctx.open("@", false).await.unwrap();

    // Editing a.rs makes a new commit; b.rs is reused from the cache.
    write(d, "a.rs", "fn a2() {}\n");
    ctx.snapshot().await.unwrap();
    let review = ctx.refresh(review.id, None).await.unwrap().review;
    assert_eq!(
        rows_text(&review, &[row_of(&review, 0, "fn a2() {}", 0)]),
        ["fn a2() {}"]
    );
    let b = row_of(&review, 1, "fn b() {}", 0);
    ctx.mark(review.id, &[b], Some(MarkState::Reviewed), &[])
        .unwrap();

    let reopened = ctx.open("@", false).await.unwrap();
    assert_eq!(
        marked(&reopened, &reopened.state.marks),
        [("fn b() {}".to_string(), MarkState::Reviewed)]
    );
}
