//! Fixes an agent links to PR threads, against a real jj repo.

use std::collections::HashMap;

use super::tests::{repo, write};
use super::*;
use crate::anchor::Side;
use crate::store::{GhLink, Stage};
use crate::turns::Checkpoint;

fn thread(id: &str, line: u32) -> PulledThread {
    PulledThread {
        link: GhLink {
            repo: "o/r".into(),
            pr: 1,
            thread: id.into(),
            url: String::new(),
            resolved: false,
            outdated: false,
            posts: vec![],
            line: None,
        },
        path: "a.rs".into(),
        side: Side::New,
        start: None,
        line: Some(line),
        hunk: "@@ hunk".into(),
        answered: false,
    }
}

fn pr_record() -> crate::store::PrRecord {
    crate::store::PrRecord {
        workspace: String::new(),
        repo: crate::github::GhRepo {
            owner: "o".into(),
            name: "r".into(),
        },
        number: 1,
        pinned: false,
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
    }
}

fn paths(review: &ReviewDto) -> Vec<&str> {
    review.files.iter().map(|f| f.path.as_str()).collect()
}

#[tokio::test]
async fn each_fix_in_a_turn_links_only_what_changed_since_the_last_one() {
    let (dir, ctx) = repo();
    let d = dir.path();
    let head = "fn a() {\n    one();\n    two();\n}\n";
    write(d, "a.rs", head);
    ctx.store.lock().unwrap().save_pr(&pr_record()).unwrap();
    let review = ctx.open(None, "@", false).await.unwrap();
    let texts = HashMap::from([("a.rs".to_string(), head.to_string())]);
    let state = ctx
        .add_threads(
            review.id,
            vec![thread("T1", 2), thread("T2", 3)],
            &texts,
            "PR #1",
        )
        .unwrap();
    let id = |gh: &str| {
        let t = state
            .threads
            .iter()
            .find(|t| t.github.as_ref().unwrap().thread == gh);
        t.unwrap().id
    };
    let (t1, t2) = (id("T1"), id("T2"));
    ctx.focus(None, review.id);

    let hook = async |point, tool| {
        let prompt = (point == Checkpoint::Start).then_some("Fix the review");
        ctx.turn_checkpoint(None, "s", tool, point, prompt)
            .await
            .unwrap();
    };
    hook(Checkpoint::Start, None).await;
    hook(Checkpoint::Before, Some("e1")).await;
    write(d, "a.rs", "fn a() {\n    uno();\n    two();\n}\n");
    hook(Checkpoint::After, Some("e1")).await;
    let (_, state, first) = ctx.agent_fixed(None, Some("s"), 1, &[t1]).await.unwrap();
    // The user edits a file between the agent's fixes.
    write(d, "mine.rs", "x\n");
    hook(Checkpoint::Before, Some("e2")).await;
    write(d, "b.rs", "b\n");
    hook(Checkpoint::After, Some("e2")).await;
    let (_, state2, second) = ctx.agent_fixed(None, Some("s"), 1, &[t2]).await.unwrap();

    let of = |s: &StateDto, t: u32| s.threads.iter().find(|x| x.id == t).cloned().unwrap();
    assert_eq!(of(&state, t1).state.stage, Stage::Review);
    assert_eq!(of(&state2, t2).state.stage, Stage::Review);
    assert_eq!(of(&state2, t1).fixes.len(), 1);
    assert_eq!((first.added, first.removed), (1, 1));

    let view = View::Fix { fix: first.id };
    let shown = ctx.open_in(None, "@", false, &view).await.unwrap();
    assert_eq!((shown.view.clone(), paths(&shown)), (view, vec!["a.rs"]));
    let view = View::Fix { fix: second.id };
    let shown = ctx.open_in(None, "@", false, &view).await.unwrap();
    assert_eq!(paths(&shown), ["b.rs"]);

    let Err(again) = ctx.agent_fixed(None, Some("s"), 1, &[t1]).await else {
        panic!("a fix with no changes was linked");
    };
    assert!(again.to_string().contains("no files"));
}

#[tokio::test]
async fn a_fix_needs_a_running_agent_turn() {
    let (_dir, ctx) = repo();
    let Err(err) = ctx.agent_fixed(None, None, 1, &[1]).await else {
        panic!("a fix with no turn was linked");
    };
    assert!(err.to_string().contains("no running agent turn"));
}
