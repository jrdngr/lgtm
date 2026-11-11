//! M2 review workflow tests: file checks, amber rows, views, refresh.

use super::tests::{jj, marked, repo, row_of, write};
use super::*;
use crate::store::MarkColor;
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
    let review = ctx.open(None, "@", false).await.unwrap();
    let state = ctx.set_checked(review.id, &[0], true).unwrap();
    assert_eq!(state.checks, [CheckState::Checked]);

    // Same code in a new commit on top of a new parent: still checked.
    jj(d, &["describe", "-m", "a"]);
    let review = ctx.open(None, "@", false).await.unwrap();
    assert_eq!(review.state.checks, [CheckState::Checked]);

    write(d, "a.py", "def f():\n    log()\n    return 1\n");
    let review = ctx.open(None, "@", false).await.unwrap();
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
async fn checked_file_unchecks_when_binary_contents_or_mode_change() {
    use std::os::unix::fs::PermissionsExt;
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "a.sh", "echo a\n");
    std::fs::write(d.join("img.bin"), b"\0one").unwrap();
    jj(d, &["commit", "-m", "base"]);
    write(d, "a.sh", "echo a\necho b\n");
    std::fs::write(d.join("img.bin"), b"\0two").unwrap();
    let review = ctx.open(None, "@", false).await.unwrap();
    let paths: Vec<_> = review.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, ["a.sh", "img.bin"]);
    let state = ctx.set_checked(review.id, &[0, 1], true).unwrap();
    assert_eq!(state.checks, [CheckState::Checked; 2]);

    std::fs::set_permissions(d.join("a.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(d.join("img.bin"), b"\0three").unwrap();
    let review = ctx.open(None, "@", false).await.unwrap();
    assert_eq!(review.state.checks, [CheckState::ChangedSinceReviewed; 2]);
}

#[tokio::test]
async fn whole_file_reviewed_marks_lines_and_checks_in_one_step() {
    let (dir, ctx) = repo();
    write(dir.path(), "a.go", "package a\n");
    let review = ctx.open(None, "@", false).await.unwrap();
    let row = row_of(&review, 0, "package a", 0);
    let state = ctx
        .mark(review.id, &[row], Some(MarkColor::Green), &[0])
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
async fn clearing_a_file_drops_its_marks_and_check_but_not_other_files() {
    let (dir, ctx) = repo();
    write(dir.path(), "a.go", "package a\n\nvar x = 1\n");
    write(dir.path(), "b.go", "package b\n");
    let review = ctx.open(None, "@", false).await.unwrap();
    let a = row_of(&review, 0, "package a", 0);
    let x = row_of(&review, 0, "var x = 1", 0);
    let b = row_of(&review, 1, "package b", 0);
    ctx.mark(review.id, &[a], Some(MarkColor::Green), &[0])
        .unwrap();
    ctx.mark(review.id, &[x, b], Some(MarkColor::Red), &[])
        .unwrap();

    let state = ctx.clear_file(review.id, 0).unwrap();
    assert_eq!(
        (marked(&review, &state.marks), state.checks[0]),
        (
            vec![("package b".to_string(), MarkColor::Red)],
            CheckState::Unchecked
        )
    );
    let state = ctx.undo(review.id).unwrap();
    assert_eq!(
        (state.marks.len(), state.checks[0]),
        (3, CheckState::Checked)
    );
}

#[tokio::test]
async fn rows_replacing_a_lost_mark_turn_amber_in_the_next_session() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "a.rs", "fn a() {\n    one();\n}\n");
    let review = ctx.open(None, "@", false).await.unwrap();
    let row = row_of(&review, 0, "    one();", 0);
    ctx.mark(review.id, &[row], Some(MarkColor::Green), &[])
        .unwrap();

    write(d, "a.rs", "fn a() {\n    two();\n}\n");
    let review = ctx.open(None, "@", false).await.unwrap();
    assert!(review.state.marks.is_empty());
    assert_eq!(rows_text(&review, &review.state.amber), ["    two();"]);

    // Reviewing the amber row clears it.
    let row = row_of(&review, 0, "    two();", 0);
    let state = ctx
        .mark(review.id, &[row], Some(MarkColor::Green), &[])
        .unwrap();
    assert!(state.amber.is_empty());
}

fn skipped(review: &ReviewDto) -> Vec<(&str, bool)> {
    let files = review.files.iter();
    files.map(|f| (f.path.as_str(), f.skipped)).collect()
}

#[tokio::test]
async fn skip_globs_from_the_base_config_flag_files() {
    let (dir, ctx) = repo();
    let d = dir.path();
    std::fs::create_dir_all(d.join(".lgtm")).unwrap();
    write(d, ".lgtm/config.toml", "skip = [\"*.lock\"]\n");
    jj(d, &["new"]);
    write(d, "Cargo.lock", "x\n");
    write(d, "main.rs", "fn main() {}\n");
    let review = ctx.open(None, "@", false).await.unwrap();
    let skipped = skipped(&review);
    assert!(skipped.contains(&("Cargo.lock", true)));
    assert!(skipped.contains(&("main.rs", false)));
}

#[tokio::test]
async fn the_reviewed_changes_cannot_set_skip_globs() {
    let (dir, ctx) = repo();
    let d = dir.path();
    std::fs::create_dir_all(d.join(".lgtm")).unwrap();
    write(d, ".lgtm/config.toml", "skip = [\"*.rs\"]\n");
    write(d, "main.rs", "fn main() {}\n");
    let review = ctx.open(None, "@", false).await.unwrap();
    assert!(skipped(&review).contains(&("main.rs", false)));
}

#[tokio::test]
async fn per_change_and_since_last_session_views() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "a.txt", "one\n");
    jj(d, &["describe", "-m", "first"]);
    jj(d, &["new", "-m", "second"]);
    write(d, "b.txt", "two\n");
    let review = ctx.open(None, "root()..@", false).await.unwrap();
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
        .loaded()
        .0;
    let paths: Vec<_> = one.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, ["a.txt"]);

    // Next session: the second change was edited, the first wasn't.
    write(d, "b.txt", "two\nthree\n");
    let review = ctx.open(None, "root()..@", false).await.unwrap();
    let since: Vec<_> = review.changes.iter().map(|c| c.since).collect();
    assert_eq!(since, [Since::Same, Since::Changed]);
    assert!(review.previous_session.is_some());

    let view = View::SinceLast { change: second };
    let inter = ctx.refresh(review.id, Some(view)).await.unwrap().loaded().0;
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
    let same = ctx.refresh(review.id, Some(same)).await.unwrap().loaded().0;
    assert!(same.files.is_empty());

    // Loading the same revset again continues the session: still compared with the first.
    let again = ctx.open(None, "root()..@", true).await.unwrap();
    let since: Vec<_> = again.changes.iter().map(|c| c.since).collect();
    assert_eq!(since, [Since::Same, Since::Changed]);

    // Abandoning a change lists it as removed next session.
    jj(d, &["abandon", "@-"]);
    let review = ctx.open(None, "root()..@", false).await.unwrap();
    assert_eq!(review.removed_changes.len(), 1);
    assert_eq!(review.removed_changes[0].description, "first");
}

#[tokio::test]
async fn view_of_a_change_that_left_the_revset_falls_back_to_combined() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "a.txt", "one\n");
    let review = ctx.open(None, "@", false).await.unwrap();
    let change = review.changes[0].change_id.clone();
    let view = View::Change { change };
    ctx.refresh(review.id, Some(view)).await.unwrap();

    jj(d, &["new"]);
    write(d, "b.txt", "two\n");
    ctx.main.snapshot().await.unwrap();
    let refreshed = ctx.refresh(review.id, None).await.unwrap().loaded().0;
    assert_eq!(refreshed.view, View::Combined);
    let paths: Vec<_> = refreshed.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, ["b.txt"]);
    assert!(ctx.shows_whole(review.id, "@").unwrap());
}

#[tokio::test]
async fn refresh_maps_rows_to_where_the_code_moved() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "a.rs", "fn a() {}\nfn b() {}\n");
    let review = ctx.open(None, "@", false).await.unwrap();
    let b = row_of(&review, 0, "fn b() {}", 0);
    let state = ctx
        .mark(review.id, &[b], Some(MarkColor::Red), &[])
        .unwrap();
    assert_eq!(marked(&review, &state.marks).len(), 1);

    write(d, "a.rs", "fn z() {}\nfn a() {}\nfn b() {}\n");
    ctx.main.snapshot().await.unwrap();
    let (refreshed, moves) = ctx.refresh(review.id, None).await.unwrap().loaded();
    let new_b = row_of(&refreshed, 0, "fn b() {}", 0);
    let moved = moves.iter().find(|m| m.from == b).unwrap();
    assert_eq!(moved.to, new_b);
    assert_eq!(
        marked(&refreshed, &refreshed.state.marks),
        [("fn b() {}".to_string(), MarkColor::Red)]
    );
    // Undo history survives the refresh.
    assert!(ctx.undo(review.id).unwrap().marks.is_empty());
}

#[tokio::test]
async fn refresh_keeps_rows_of_files_loaded_without_contents() {
    let (dir, ctx) = repo();
    let d = dir.path();
    let big: String = (0..400_000).map(|i| format!("line {i}\n")).collect();
    assert!(big.len() > 4 << 20);
    jj(
        d,
        &[
            "config",
            "set",
            "--repo",
            "snapshot.max-new-file-size",
            "8MiB",
        ],
    );
    write(d, "a.txt", &big);
    jj(d, &["commit", "-m", "base"]);
    write(d, "a.txt", &big.replacen("line 7\n", "line seven\n", 1));
    write(d, "b.rs", "fn b() {}\n");
    let review = ctx.open(None, "@", false).await.unwrap();
    let a = review.files.iter().position(|f| f.path == "a.txt").unwrap();

    write(d, "b.rs", "fn b2() {}\n");
    ctx.main.snapshot().await.unwrap();
    let (refreshed, moves) = ctx.refresh(review.id, None).await.unwrap().loaded();
    let moved: Vec<_> = moves.iter().filter(|m| m.from.file == a as u32).collect();
    assert_eq!(moved.len(), refreshed.files[a].hunks[0].rows.len());
    assert!(moved.iter().all(|m| m.from == m.to));
}

#[tokio::test]
async fn unchanged_files_reused_on_refresh_still_take_marks() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "a.rs", "fn a() {}\n");
    write(d, "b.rs", "fn b() {}\n");
    let review = ctx.open(None, "@", false).await.unwrap();

    // Editing a.rs makes a new commit; b.rs is reused from the cache.
    write(d, "a.rs", "fn a2() {}\n");
    ctx.main.snapshot().await.unwrap();
    let review = ctx.refresh(review.id, None).await.unwrap().loaded().0;
    assert_eq!(
        rows_text(&review, &[row_of(&review, 0, "fn a2() {}", 0)]),
        ["fn a2() {}"]
    );
    let b = row_of(&review, 1, "fn b() {}", 0);
    ctx.mark(review.id, &[b], Some(MarkColor::Green), &[])
        .unwrap();

    let reopened = ctx.open(None, "@", false).await.unwrap();
    assert_eq!(
        marked(&reopened, &reopened.state.marks),
        [("fn b() {}".to_string(), MarkColor::Green)]
    );
}

#[tokio::test]
async fn refresh_without_new_commits_keeps_the_files_but_not_the_change_details() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "a.rs", "fn a() {}\n");
    let review = ctx.open(None, "@", false).await.unwrap();

    jj(d, &["bookmark", "set", "feature", "-r", "@"]);
    let RefreshDto::Same { changes, .. } = ctx.refresh(review.id, None).await.unwrap() else {
        panic!("reloaded a review whose commits didn't change");
    };
    assert_eq!(changes[0].commit_id, review.changes[0].commit_id);
    assert!(changes[0].bookmarks[0].starts_with("feature"));

    // A new session compares against other commits, so the files load again.
    ctx.seal_session(review.id).unwrap();
    ctx.refresh(review.id, None).await.unwrap().loaded();
    let view = View::Change {
        change: review.changes[0].change_id.clone(),
    };
    ctx.refresh(review.id, Some(view)).await.unwrap().loaded();
}
