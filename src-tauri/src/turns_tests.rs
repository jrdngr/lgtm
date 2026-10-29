//! Agent turn tests against real jj repos.

use super::*;
use crate::review::View;
use crate::review::tests::{repo, write};

/// Runs a hook's checkpoint for session `s`.
async fn hook(ctx: &RepoCtx, point: Checkpoint, tool: Option<&str>) {
    let prompt = (point == Checkpoint::Start).then_some("Fix it");
    ctx.turn_checkpoint(None, None, "s", tool, point, prompt)
        .await
        .unwrap();
}

fn paths(review: &crate::review::ReviewDto) -> Vec<&str> {
    review.files.iter().map(|f| f.path.as_str()).collect()
}

#[tokio::test]
async fn a_turn_diffs_only_the_files_its_tools_changed() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "mine.rs", "1\n");
    hook(&ctx, Checkpoint::Start, None).await;
    hook(&ctx, Checkpoint::Before, Some("t1")).await;
    write(d, "a.rs", "one\ntwo\n");
    hook(&ctx, Checkpoint::After, Some("t1")).await;
    // The user edits between the agent's tool calls.
    write(d, "b.rs", "x\n");
    hook(&ctx, Checkpoint::Before, Some("t2")).await;
    hook(&ctx, Checkpoint::After, Some("t2")).await;
    hook(&ctx, Checkpoint::End, None).await;

    let turns = ctx.turns(None).await.unwrap();
    assert_eq!(turns.len(), 1);
    let t = &turns[0];
    assert_eq!((t.prompt.as_deref(), t.running), (Some("Fix it"), false));
    assert_eq!((t.added, t.removed), (2, 0));
    let view = View::Turn { turns: vec![t.id] };
    let review = ctx.open_in(None, "@", false, &view).await.unwrap();
    assert_eq!(review.view, view);
    assert_eq!(paths(&review), ["a.rs"]);
}

#[tokio::test]
async fn a_turn_whose_tools_changed_nothing_is_dropped() {
    let (_dir, ctx) = repo();
    hook(&ctx, Checkpoint::Start, None).await;
    hook(&ctx, Checkpoint::Before, Some("t1")).await;
    hook(&ctx, Checkpoint::After, Some("t1")).await;
    hook(&ctx, Checkpoint::End, None).await;
    assert!(ctx.turns(None).await.unwrap().is_empty());
}

#[tokio::test]
async fn a_checkpoint_diffs_everything_since_it_up_to_the_working_copy_now() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "a.rs", "1\n");
    ctx.checkpoint(None).await.unwrap();
    write(d, "a.rs", "2\n");
    write(d, "b.rs", "3\n");
    ctx.workspace(None).await.unwrap().snapshot().await.unwrap();
    let turns = ctx.turns(None).await.unwrap();
    assert_eq!(
        (turns[0].prompt.as_deref(), turns[0].added, turns[0].removed),
        (None, 2, 1)
    );
    let view = View::Turn {
        turns: vec![turns[0].id],
    };
    let review = ctx.open_in(None, "@", false, &view).await.unwrap();
    assert_eq!(paths(&review), ["a.rs", "b.rs"]);
}

#[tokio::test]
async fn undoing_a_turn_restores_its_files_unless_they_changed_since() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "a.rs", "before\n");
    hook(&ctx, Checkpoint::Start, None).await;
    hook(&ctx, Checkpoint::Before, Some("t1")).await;
    write(d, "a.rs", "agent\n");
    hook(&ctx, Checkpoint::After, Some("t1")).await;
    hook(&ctx, Checkpoint::End, None).await;
    let id = ctx.turns(None).await.unwrap()[0].id;

    write(d, "a.rs", "edited after\n");
    let err = ctx.undo_turn(None, id).await.unwrap_err();
    assert!(err.to_string().contains("changed since"), "{err}");

    write(d, "a.rs", "agent\n");
    ctx.undo_turn(None, id).await.unwrap();
    assert_eq!(std::fs::read_to_string(d.join("a.rs")).unwrap(), "before\n");
    assert!(ctx.turns(None).await.unwrap().is_empty());
}

#[tokio::test]
async fn a_view_of_a_removed_turn_falls_back_to_the_whole_review() {
    let (dir, ctx) = repo();
    write(dir.path(), "a.rs", "1\n");
    ctx.checkpoint(None).await.unwrap();
    let id = ctx.turns(None).await.unwrap()[0].id;
    ctx.remove_turn(id).unwrap();
    let review = ctx
        .open_in(None, "@", false, &View::Turn { turns: vec![id] })
        .await
        .unwrap();
    assert_eq!(review.view, View::Combined);
    assert_eq!(paths(&review), ["a.rs"]);
}

#[tokio::test]
async fn a_run_of_turns_diffs_from_the_first_start_to_the_last_end_over_their_files() {
    let (dir, ctx) = repo();
    let d = dir.path();
    for (file, text) in [("a.rs", "1\n"), ("b.rs", "2\n"), ("c.rs", "3\n")] {
        hook(&ctx, Checkpoint::Start, None).await;
        hook(&ctx, Checkpoint::Before, Some(file)).await;
        write(d, file, text);
        hook(&ctx, Checkpoint::After, Some(file)).await;
        hook(&ctx, Checkpoint::End, None).await;
    }
    // Newest first: c, b, a.
    let ids: Vec<u32> = ctx
        .turns(None)
        .await
        .unwrap()
        .iter()
        .map(|t| t.id)
        .collect();
    let view = View::Turn {
        turns: vec![ids[2], ids[1]],
    };
    let review = ctx.open_in(None, "@", false, &view).await.unwrap();
    assert_eq!(paths(&review), ["a.rs", "b.rs"]);
}

/// Saves `text` to `file` the way the editor does.
async fn save(ctx: &RepoCtx, file: &str, text: &str) {
    let ws = ctx.workspace(None).await.unwrap();
    let before = working_commit(&ws).await.unwrap();
    write(&ws.root, file, text);
    ctx.record_save(&ws, file, &before).await.unwrap();
}

#[tokio::test]
async fn saves_in_a_row_make_one_edit_and_an_agent_turn_between_starts_another() {
    let (dir, ctx) = repo();
    // Changed outside the editor, so in no edit.
    write(dir.path(), "other.rs", "x\n");
    save(&ctx, "a.rs", "1\n").await;
    save(&ctx, "b.rs", "2\n").await;
    save(&ctx, "a.rs", "1\n").await;
    hook(&ctx, Checkpoint::Start, None).await;
    hook(&ctx, Checkpoint::Before, Some("t1")).await;
    write(dir.path(), "c.rs", "3\n");
    hook(&ctx, Checkpoint::After, Some("t1")).await;
    hook(&ctx, Checkpoint::End, None).await;
    save(&ctx, "a.rs", "1\n2\n").await;

    let turns = ctx.turns(None).await.unwrap();
    let kinds: Vec<_> = turns.iter().map(|t| (t.kind, t.paths.clone())).collect();
    assert_eq!(
        kinds,
        [
            (TurnKind::Edit, vec!["a.rs".to_string()]),
            (TurnKind::Agent, vec!["c.rs".to_string()]),
            (TurnKind::Edit, vec!["a.rs".to_string(), "b.rs".to_string()]),
        ]
    );
    assert_eq!((turns[2].added, turns[2].removed), (2, 0));
    let view = View::Turn {
        turns: vec![turns[2].id],
    };
    let review = ctx.open_in(None, "@", false, &view).await.unwrap();
    assert_eq!(paths(&review), ["a.rs", "b.rs"]);
}

/// A hook turn that writes `file`.
async fn turn_writing(ctx: &RepoCtx, d: &std::path::Path, file: &str) {
    hook(ctx, Checkpoint::Start, None).await;
    hook(ctx, Checkpoint::Before, Some("t")).await;
    write(d, file, "x\n");
    hook(ctx, Checkpoint::After, Some("t")).await;
    hook(ctx, Checkpoint::End, None).await;
}

#[tokio::test]
async fn squashing_a_change_moves_its_turns_after_those_of_the_change_it_went_into() {
    let (dir, ctx) = repo();
    let d = dir.path();
    turn_writing(&ctx, d, "a.rs").await;
    let base = ctx
        .workspace(None)
        .await
        .unwrap()
        .jj
        .changes("@")
        .await
        .unwrap();
    let base = base[0].change_id.clone();
    let new = JjAction::NewAfter { rev: "@".into() };
    ctx.jj_act(None, &new).await.unwrap();
    turn_writing(&ctx, d, "b.rs").await;

    let squash = JjAction::Squash { revs: "@".into() };
    ctx.jj_act(None, &squash).await.unwrap();
    assert!(ctx.turns(None).await.unwrap().is_empty());
    let edit = JjAction::Edit { rev: base };
    ctx.jj_act(None, &edit).await.unwrap();
    let turns = ctx.turns(None).await.unwrap();
    let files: Vec<&[String]> = turns.iter().map(|t| t.paths.as_slice()).collect();
    assert_eq!(files, [["b.rs".to_string()], ["a.rs".to_string()]]);
    let view = View::Turn {
        turns: vec![turns[0].id],
    };
    let review = ctx.open_in(None, "@", false, &view).await.unwrap();
    assert_eq!(paths(&review), ["b.rs"]);
}

/// The files in `rev`'s diff; none for the root.
async fn changed(ctx: &RepoCtx, rev: &str) -> Vec<String> {
    let jj = &ctx.workspace(None).await.unwrap().jj;
    if jj
        .commit_ids(&format!("{rev} ~ root()"))
        .await
        .unwrap()
        .is_empty()
    {
        return vec![];
    }
    let range = Range {
        from: Some(format!("{rev}-")),
        to: rev.to_string(),
    };
    let diffs = jj.diff(&range).await.unwrap();
    diffs.into_iter().map(|d| d.path).collect()
}

/// Splits `@` around the turns at `picks` (newest first) of turns writing a.rs, b.rs and c.rs
/// in turn, and returns the files in `@--`, `@-` and `@`, and the files of the turns left on `@`.
async fn split_three(picks: &[usize]) -> [Vec<String>; 4] {
    let (dir, ctx) = repo();
    let d = dir.path();
    for file in ["a.rs", "b.rs", "c.rs"] {
        turn_writing(&ctx, d, file).await;
    }
    let turns = ctx.turns(None).await.unwrap();
    let ids: Vec<u32> = picks.iter().map(|i| turns[*i].id).collect();
    ctx.split_at_turns(None, &ids).await.unwrap();
    for file in ["a.rs", "b.rs", "c.rs"] {
        assert_eq!(std::fs::read_to_string(d.join(file)).unwrap(), "x\n");
    }
    let left = ctx.turns(None).await.unwrap();
    let left = left.into_iter().flat_map(|t| t.paths).collect();
    [
        changed(&ctx, "@--").await,
        changed(&ctx, "@-").await,
        changed(&ctx, "@").await,
        left,
    ]
}

#[tokio::test]
async fn splitting_off_the_newest_turns_puts_them_on_top_in_the_working_copy() {
    let [below2, below, at, left] = split_three(&[1, 0]).await;
    let none: Vec<String> = vec![];
    assert_eq!((below2, below), (none, vec!["a.rs".into()]));
    assert_eq!(at, ["b.rs", "c.rs"]);
    assert_eq!(left, ["c.rs", "b.rs"]);
}

#[tokio::test]
async fn splitting_off_the_oldest_turn_puts_it_below_the_working_copy() {
    let [below2, below, at, left] = split_three(&[2]).await;
    let none: Vec<String> = vec![];
    assert_eq!((below2, below), (none, vec!["a.rs".into()]));
    assert_eq!(at, ["b.rs", "c.rs"]);
    assert_eq!(left, ["c.rs", "b.rs"]);
}

#[tokio::test]
async fn splitting_off_a_middle_turn_makes_three_changes_in_order() {
    let [below2, below, at, left] = split_three(&[1]).await;
    let one = |f: &str| vec![f.to_string()];
    assert_eq!((below2, below, at), (one("a.rs"), one("b.rs"), one("c.rs")));
    assert_eq!(left, ["c.rs"]);
}

#[tokio::test]
async fn splitting_off_a_turn_that_edits_an_earlier_one_keeps_both_edits_in_order() {
    let (dir, ctx) = repo();
    let d = dir.path();
    turn_writing(&ctx, d, "a.rs").await;
    hook(&ctx, Checkpoint::Start, None).await;
    hook(&ctx, Checkpoint::Before, Some("t")).await;
    write(d, "a.rs", "x\ny\n");
    hook(&ctx, Checkpoint::After, Some("t")).await;
    hook(&ctx, Checkpoint::End, None).await;
    let id = ctx.turns(None).await.unwrap()[0].id;

    ctx.split_at_turns(None, &[id]).await.unwrap();
    let jj = ctx.workspace(None).await.unwrap().jj.clone();
    assert_eq!(jj.file(Some("@-"), "a.rs").await.unwrap(), b"x\n");
    assert_eq!(changed(&ctx, "@").await, ["a.rs"]);
    assert!(jj.commit_ids("conflicts()").await.unwrap().is_empty());
}

#[tokio::test]
async fn a_folder_moved_in_the_files_sidebar_is_its_own_edit_and_undoing_it_moves_it_back() {
    let (dir, ctx) = repo();
    let d = dir.path();
    std::fs::create_dir_all(d.join("src")).unwrap();
    write(d, "src/a.rs", "1\n");
    write(d, "srcish.rs", "x\n");
    save(&ctx, "notes.md", "n\n").await;
    let ws = ctx.workspace(None).await.unwrap();
    let before = working_commit(&ws).await.unwrap();
    std::fs::create_dir_all(d.join("lib")).unwrap();
    std::fs::rename(d.join("src"), d.join("lib/src")).unwrap();
    // Changed alongside, but not under the moved folder.
    write(d, "srcish.rs", "y\n");
    let roots = ["src".to_string(), "lib/src".to_string()];
    let label = "Move src → lib/src";
    assert!(
        ctx.record_files_change(&ws, &roots, &before, label)
            .await
            .unwrap()
    );
    // A save right after doesn't join a Files sidebar edit.
    save(&ctx, "notes.md", "n2\n").await;

    let turns = ctx.turns(None).await.unwrap();
    let edits: Vec<_> = turns
        .iter()
        .map(|t| (t.prompt.as_deref(), t.paths.clone()))
        .collect();
    assert_eq!(
        edits,
        [
            (None, vec!["notes.md".to_string()]),
            (
                Some(label),
                vec!["lib/src/a.rs".to_string(), "src/a.rs".to_string()]
            ),
            (None, vec!["notes.md".to_string()]),
        ]
    );
    ctx.undo_turn(None, turns[1].id).await.unwrap();
    assert!(d.join("src/a.rs").exists());
    assert!(!d.join("lib/src/a.rs").exists());
    assert_eq!(std::fs::read_to_string(d.join("srcish.rs")).unwrap(), "y\n");
}
