//! Comments for the agent in the app: writing them on a range, the agent's view of them, and
//! undo.

use super::tests::{jj, repo, row_of, write};

#[tokio::test]
async fn a_comment_on_a_range_lists_for_agents_and_resolving_and_deleting_undo() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "a.py", "def f():\n    a = 1\n    return a\n");
    jj(d, &["describe", "-m", "f"]);
    let review = ctx.open(None, "@", false).await.unwrap();
    ctx.focus(None, review.id);
    let start = row_of(&review, 0, "    a = 1", 0).row;
    let end = row_of(&review, 0, "    return a", 0).row;

    let state = ctx
        .add_agent_comment(review.id, 0, start, end, "inline a")
        .unwrap();
    let c = &state.agent_comments[0];
    assert_eq!(
        (c.number, c.rows.map(|r| (r.start, r.end))),
        (1, Some((start, end)))
    );

    let listed = ctx.agent_comments_listing(None).unwrap();
    assert_eq!(listed[0]["number"], "C1");
    assert_eq!(listed[0]["line"], 3);
    assert_eq!(listed[0]["startLine"], 2);
    assert_eq!(listed[0]["code"], "+    a = 1\n+    return a");

    assert!(
        ctx.set_agent_comment_done(review.id, 1, true)
            .unwrap()
            .agent_comments[0]
            .done
    );
    assert_eq!(
        ctx.agent_comments_listing(None).unwrap()[0]["resolved"],
        true
    );
    assert!(
        ctx.delete_agent_comment(review.id, 1)
            .unwrap()
            .agent_comments
            .is_empty()
    );

    // Undo brings it back resolved, then open.
    assert!(ctx.undo(review.id).unwrap().agent_comments[0].done);
    assert!(!ctx.undo(review.id).unwrap().agent_comments[0].done);
    assert!(ctx.undo(review.id).unwrap().agent_comments.is_empty());
    assert_eq!(
        ctx.redo(review.id).unwrap().agent_comments[0].body,
        "inline a"
    );
}

#[tokio::test]
async fn a_comment_whose_code_goes_stays_until_resolved_then_moves_to_the_nearest_line() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "a.py", "x = 1\ny = 2\nz = 3\n");
    jj(d, &["describe", "-m", "xyz"]);
    let review = ctx.open(None, "@", false).await.unwrap();
    let y = row_of(&review, 0, "y = 2", 0).row;
    ctx.add_agent_comment(review.id, 0, y, y, "drop y").unwrap();

    write(d, "a.py", "x = 1\nz = 3\n");
    let review = ctx.open(None, "@", false).await.unwrap();
    assert_eq!(review.state.agent_comments[0].rows, None);
    let state = ctx.set_agent_comment_done(review.id, 1, true).unwrap();
    assert!(state.agent_comments[0].rows.is_some());
}
