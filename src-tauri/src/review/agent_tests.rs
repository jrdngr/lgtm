use super::tests::{jj, repo, write};
use super::*;
use crate::store::MarkColor;

#[tokio::test]
async fn agents_see_marks_and_ranges_left_and_writes_wait_for_the_review() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "a.rs", "fn a() {}\n");
    jj(d, &["commit", "-m", "base"]);
    write(d, "a.rs", "fn a() {}\nfn b() {}\nfn c() {}\nfn e() {}\n");
    let review = ctx.open(None, "@", false).await.unwrap();
    assert!(ctx.summary(None).is_err(), "nothing is focused yet");
    ctx.focus(None, review.id);

    let b = tests::row_of(&review, 0, "fn b() {}", 0);
    let c = tests::row_of(&review, 0, "fn c() {}", 0);
    ctx.mark(review.id, &[b], Some(MarkColor::Green), &[])
        .unwrap();
    ctx.mark(review.id, &[c], Some(MarkColor::Red), &[])
        .unwrap();
    let summary = ctx.summary_with(None, true).unwrap();
    let f = &summary.files[0];
    assert_eq!((f.rows.done, f.rows.left), (1, 2));
    let marks: Vec<_> = f.rows.marks.iter().map(|(k, n)| (k.as_str(), *n)).collect();
    assert_eq!(marks, [("flagged", 1), ("reviewed", 1)]);
    assert_eq!(f.left, ["3-4"], "flagged rows aren't done");
    let marked: Vec<_> = f
        .marked
        .iter()
        .map(|(k, r)| (k.as_str(), r.clone()))
        .collect();
    assert_eq!(
        marked,
        [
            ("flagged", vec!["3".to_string()]),
            ("reviewed", vec!["2".to_string()])
        ]
    );

    // Without marks in use, marked rows are left like any other.
    let f = &ctx.summary_with(None, false).unwrap().files[0];
    assert_eq!(
        (f.rows.done, f.rows.left, f.left.clone()),
        (0, 3, vec!["2-4".to_string()])
    );
    assert!(f.rows.marks.is_empty() && f.marked.is_empty());

    let empty = || serde_json::json!({ "verdict": "ok", "findings": [] });
    // A write about another revset than the focused review's is refused.
    let Err(err) = ctx
        .publish_findings(None, "@-", None, "x", empty(), None)
        .await
    else {
        panic!("findings about another revset were published");
    };
    assert!(err.to_string().contains("moved to `@`"), "{err}");
    let (report, _) = ctx
        .publish_findings(None, "@", None, "x", empty(), None)
        .await
        .unwrap();

    // Once the revset moves to other commits, even before the file watcher snapshots the edit,
    // writes wait until the review loads them.
    write(
        d,
        "a.rs",
        "fn a() {}\nfn b() {}\nfn c() {}\nfn e() {}\nfn f() {}\n",
    );
    let Err(err) = ctx
        .publish_findings(None, "@", None, "x", empty(), None)
        .await
    else {
        panic!("findings went into an out-of-date review");
    };
    assert!(err.to_string().contains("is reloading"), "{err}");
    let label = report.label();
    let Err(err) = ctx
        .publish_findings(None, "@", Some(&label), "x", empty(), None)
        .await
    else {
        panic!("findings went into a report of other commits");
    };
    assert!(err.to_string().contains("other commits"), "{err}");
    ctx.refresh(review.id, None).await.unwrap();
    ctx.publish_findings(None, "@", None, "x", empty(), None)
        .await
        .unwrap();
}

#[tokio::test]
async fn published_findings_start_a_report_then_replace_their_section() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "a.rs", "fn a() {}\n");
    let review = ctx.open(None, "@", false).await.unwrap();
    ctx.focus(None, review.id);
    let finding = |severity: &str, title: &str| {
        serde_json::json!({ "severity": severity, "title": title, "locations": [{ "path": "a.rs", "start": 1, "end": 1 }],
            "facts": [], "explanation": "e", "fix": null })
    };
    let review_of =
        |fs: Vec<serde_json::Value>| serde_json::json!({ "verdict": "ok", "findings": fs });
    let ids = |run: &crate::store::ReportRun, i: usize| {
        let crate::store::report::Output::Review(r) = &run.parts[i].output else {
            panic!("no review");
        };
        r.findings
            .iter()
            .map(|f| format!("{} {}", f.id, f.title))
            .collect::<Vec<_>>()
    };

    let (run, created) = ctx
        .publish_findings(
            None,
            "@",
            None,
            "claude",
            review_of(vec![finding("low", "nit"), finding("high", "bug")]),
            None,
        )
        .await
        .unwrap();
    assert!(created);
    assert_eq!(run.revset, "@");
    assert_eq!(run.label(), format!("R{}", run.id));
    assert_eq!(ids(&run, 0), ["claude#1 bug", "claude#2 nit"]);

    let label = run.label().to_lowercase();
    ctx.dismiss_findings(run.id, &["claude#1".into(), "claude#2".into()], true)
        .unwrap();
    let (run, created) = ctx
        .publish_findings(
            None,
            "@",
            Some(&label),
            "claude",
            review_of(vec![finding("high", "bug"), finding("medium", "new")]),
            None,
        )
        .await
        .unwrap();
    assert!(!created);
    assert_eq!(ids(&run, 0), ["claude#1 bug", "claude#2 new"]);
    assert_eq!(
        run.dismissed,
        ["claude#1"],
        "the bug stays dismissed, the gone nit's mark goes"
    );

    let summary = crate::reports::Summary {
        task: "tidy".into(),
        fixes: vec![("restyle".into(), vec![1])],
        off_task: vec![],
        questions: vec![],
    };
    let (run, _) = ctx
        .publish_findings(
            None,
            "@",
            Some(&label),
            "codex",
            review_of(vec![finding("low", "style")]),
            Some(summary),
        )
        .await
        .unwrap();
    assert_eq!(run.personas[1].name, "codex");
    assert!(matches!(
        &run.parts[2].output,
        crate::store::report::Output::Minimal(m) if m.fixes[0].closes == ["codex#1"]
    ));

    let (run, _) = ctx
        .publish_findings(None, "@", Some(&label), "codex", review_of(vec![]), None)
        .await
        .unwrap();
    assert_eq!(
        run.parts.len(),
        2,
        "the old suggestions no longer match the findings"
    );

    // A report of the whole range takes findings only while its base is the revset's too.
    assert!(run.whole);
    let mut moved = run.clone();
    moved.from = Some("view-base".into());
    ctx.store.lock().unwrap().save_report_run(&moved).unwrap();
    let Err(err) = ctx
        .publish_findings(None, "@", Some(&label), "codex", review_of(vec![]), None)
        .await
    else {
        panic!("findings went into a report of another base");
    };
    assert!(err.to_string().contains("publish a new report"), "{err}");
    assert_eq!(ctx.report_run(run.id).unwrap(), moved);

    // One stored with an open view's base, before reports took the whole range, still takes
    // findings while its head is the revset's.
    let legacy = crate::store::ReportRun {
        whole: false,
        ..moved
    };
    ctx.store.lock().unwrap().save_report_run(&legacy).unwrap();
    ctx.publish_findings(None, "@", Some(&label), "codex", review_of(vec![]), None)
        .await
        .unwrap();
    assert!(
        ctx.publish_findings(None, "@", Some("R999"), "x", review_of(vec![]), None)
            .await
            .is_err()
    );
    assert!(
        ctx.publish_findings(None, "@-", Some(&label), "x", review_of(vec![]), None)
            .await
            .is_err()
    );
    assert!(
        ctx.publish_findings(None, "@-", None, "x", review_of(vec![]), None)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn reports_rename_to_unique_ids_and_back() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "a.rs", "fn a() {}\n");
    let review = ctx.open(None, "@", false).await.unwrap();
    ctx.focus(None, review.id);
    let empty = || serde_json::json!({ "verdict": "ok", "findings": [] });
    let (a, _) = ctx
        .publish_findings(None, "@", None, "x", empty(), None)
        .await
        .unwrap();
    let (b, _) = ctx
        .publish_findings(None, "@", None, "x", empty(), None)
        .await
        .unwrap();

    assert_eq!(
        ctx.rename_report(a.id, " auth-pass ").unwrap().label(),
        "auth-pass"
    );
    assert_eq!(ctx.find_report(None, "AUTH-PASS").unwrap().id, a.id);
    assert!(ctx.rename_report(b.id, "auth-pass").is_err(), "taken");
    assert!(
        ctx.rename_report(b.id, "R77").is_err(),
        "looks like a default id"
    );
    assert!(ctx.rename_report(b.id, "two words").is_err());
    assert_eq!(
        ctx.rename_report(a.id, "").unwrap().label(),
        format!("R{}", a.id)
    );
}

#[tokio::test]
async fn agent_checks_never_replace_or_remove_the_users_and_theirs_replace_the_agents() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "a.rs", "fn a() {}\n");
    write(d, "b.rs", "fn b() {}\n");
    let review = ctx.open(None, "@", false).await.unwrap();
    ctx.focus(None, review.id);
    let check = |paths: &[&str], on: bool| {
        let paths: Vec<String> = paths.iter().map(|p| p.to_string()).collect();
        let ctx = &ctx;
        async move {
            ctx.agent_check_files(None, "@", &paths, on)
                .await
                .unwrap()
                .1
        }
    };

    ctx.set_checked(review.id, &[0], true).unwrap();
    let state = check(&["a.rs", "b.rs"], true).await;
    assert_eq!(state.checks, [CheckState::Checked; 2]);
    assert_eq!(state.agent_checks, [1], "a.rs stays the user's");
    let state = check(&["a.rs"], false).await;
    assert_eq!(
        state.checks[0],
        CheckState::Checked,
        "the agent can't uncheck the user's"
    );

    let state = ctx.set_checked(review.id, &[1], true).unwrap();
    assert!(
        state.agent_checks.is_empty(),
        "the user's check replaced the agent's"
    );
    let summary = ctx.summary_with(None, false).unwrap();
    assert!(
        summary
            .files
            .iter()
            .all(|f| f.checked && !f.checked_by_agent)
    );
}

#[tokio::test]
async fn agent_marks_leave_the_users_and_clearing_clears_only_the_agents() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "a.rs", "fn a() {}\nfn b() {}\nfn c() {}\n");
    let review = ctx.open(None, "@", false).await.unwrap();
    ctx.focus(None, review.id);
    let b = tests::row_of(&review, 0, "fn b() {}", 0);
    ctx.mark(review.id, &[b], Some(MarkColor::Red), &[])
        .unwrap();
    let mark = |lines: &[&str], color| {
        let lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
        let ctx = &ctx;
        async move {
            ctx.agent_mark_lines(None, "@", "a.rs", &lines, &[], color)
                .await
        }
    };

    let state = mark(&["1-3"], Some(MarkColor::AgentReviewed))
        .await
        .unwrap()
        .1;
    let colors: Vec<MarkColor> = state.marks.iter().map(|m| m.color).collect();
    use MarkColor::{AgentReviewed, Red};
    assert_eq!(colors, [AgentReviewed, Red, AgentReviewed]);
    let summary = ctx.summary_with(None, true).unwrap();
    assert_eq!(
        summary.files[0].left,
        ["2"],
        "agent reviewed rows are done, flagged ones not"
    );
    let marked: Vec<_> = summary.files[0].marked.keys().cloned().collect();
    assert_eq!(marked, ["agent reviewed", "flagged"]);

    let state = mark(&["1-3"], None).await.unwrap().1;
    let colors: Vec<MarkColor> = state.marks.iter().map(|m| m.color).collect();
    assert_eq!(colors, [Red]);
    let Err(err) = mark(&["9"], Some(MarkColor::AgentFlagged)).await else {
        panic!("marked lines with no changed rows");
    };
    assert!(err.to_string().contains("no changed rows"), "{err}");
    let Err(err) = ctx.agent_mark(review.id, &[b], Some(Red)) else {
        panic!("an agent marked with the user's color");
    };
    assert!(err.to_string().contains("own colors"), "{err}");
}
