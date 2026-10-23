//! Agent turn tests against real jj repos.

use super::*;
use crate::review::View;
use crate::review::tests::{repo, write};

/// Runs a hook's checkpoint for session `s`.
async fn hook(ctx: &RepoCtx, point: Checkpoint, tool: Option<&str>) {
    let prompt = (point == Checkpoint::Start).then_some("Fix it");
    ctx.turn_checkpoint(None, "s", tool, point, prompt)
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
