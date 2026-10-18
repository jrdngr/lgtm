use super::tests::{jj, repo, write};
use super::*;
use crate::store::{Author, MarkColor};

#[tokio::test]
async fn agents_see_marks_and_ranges_left_and_can_comment_and_reply() {
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
    ctx.mark(review.id, &[b], Some(MarkColor::Cyan), &[])
        .unwrap();
    ctx.mark(review.id, &[c], Some(MarkColor::Red), &[])
        .unwrap();
    let summary = ctx.summary(None).unwrap();
    let f = &summary.files[0];
    assert_eq!((f.rows.done, f.rows.left), (1, 2));
    let marks: Vec<_> = f.rows.marks.iter().map(|(k, n)| (k.as_str(), *n)).collect();
    assert_eq!(marks, [("flagged", 1), ("reviewed", 1)]);
    assert_eq!(f.left, ["3-4"], "flagged rows aren't done");

    let (id, state) = ctx.agent_comment(None, "@", "a.rs", 3, 3, "why c?").unwrap();
    assert_eq!(id, review.id);
    let c = &state.comments[0];
    assert!(c.review.ends_with("terminal agent"), "{}", c.review);
    assert_eq!(c.snippet, "@@ -0,0 +3,1 @@\n+fn c() {}");

    let (_, state) = ctx.agent_reply(None, "@", c.id, "because").unwrap();
    assert_eq!(state.comments[0].messages[0].author, Author::Agent);
    let listed = ctx.agent_comments(None).unwrap();
    assert_eq!(listed[0].lines.as_deref(), Some("3"));

    assert!(ctx.agent_comment(None, "@", "nope.rs", 1, 1, "x").is_err());
    let Err(err) = ctx.agent_comment(None, "@", "a.rs", 1, 1, "x") else {
        panic!("a comment on unchanged lines was added");
    };
    assert!(
        err.to_string().contains("nearest change is at line 2"),
        "{err}"
    );
    assert_eq!(ctx.agent_comments(None).unwrap().len(), 1);

    // A write about another revset than the focused review's is refused.
    let Err(err) = ctx.agent_comment(None, "@-", "a.rs", 3, 3, "x") else {
        panic!("a comment about another revset was added");
    };
    assert!(err.to_string().contains("moved to `@`"), "{err}");
    assert!(ctx.agent_reply(None, "@-", c.id, "x").is_err());
    assert_eq!(ctx.agent_comments(None).unwrap()[0].conversation.len(), 1);
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
    let review_of = |fs: Vec<serde_json::Value>| serde_json::json!({ "verdict": "ok", "findings": fs });
    let ids = |run: &crate::store::BotRun, i: usize| {
        let crate::store::report::Output::Review(r) = &run.parts[i].output else {
            panic!("no review");
        };
        r.findings.iter().map(|f| format!("{} {}", f.id, f.title)).collect::<Vec<_>>()
    };

    let (run, created) = ctx
        .publish_findings(None, "@", None, "claude", review_of(vec![finding("low", "nit"), finding("high", "bug")]), None)
        .unwrap();
    assert!(created);
    assert_eq!(run.revset, "@");
    assert_eq!(run.label(), format!("R{}", run.id));
    assert_eq!(ids(&run, 0), ["claude#1 bug", "claude#2 nit"]);

    let label = run.label().to_lowercase();
    ctx.dismiss_findings(run.id, &["claude#1".into(), "claude#2".into()], true)
        .unwrap();
    let (run, created) = ctx
        .publish_findings(None, "@", Some(&label), "claude", review_of(vec![finding("high", "bug"), finding("medium", "new")]), None)
        .unwrap();
    assert!(!created);
    assert_eq!(ids(&run, 0), ["claude#1 bug", "claude#2 new"]);
    assert_eq!(run.dismissed, ["claude#1"], "the bug stays dismissed, the gone nit's mark goes");

    let summary = crate::bots::Summary {
        task: "tidy".into(),
        fixes: vec![("restyle".into(), vec![1])],
        off_task: vec![],
        questions: vec![],
    };
    let (run, _) = ctx
        .publish_findings(None, "@", Some(&label), "codex", review_of(vec![finding("low", "style")]), Some(summary))
        .unwrap();
    assert_eq!(run.personas[1].name, "codex");
    assert!(matches!(
        &run.parts[2].output,
        crate::store::report::Output::Minimal(m) if m.fixes[0].closes == ["codex#1"]
    ));

    let (run, _) = ctx
        .publish_findings(None, "@", Some(&label), "codex", review_of(vec![]), None)
        .unwrap();
    assert_eq!(run.parts.len(), 2, "the old suggestions no longer match the findings");
    assert!(ctx.publish_findings(None, "@", Some("R999"), "x", review_of(vec![]), None).is_err());
    assert!(ctx.publish_findings(None, "@-", Some(&label), "x", review_of(vec![]), None).is_err());
    assert!(ctx.publish_findings(None, "@-", None, "x", review_of(vec![]), None).is_err());
}

#[tokio::test]
async fn reports_rename_to_unique_ids_and_back() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "a.rs", "fn a() {}\n");
    let review = ctx.open(None, "@", false).await.unwrap();
    ctx.focus(None, review.id);
    let empty = || serde_json::json!({ "verdict": "ok", "findings": [] });
    let (a, _) = ctx.publish_findings(None, "@", None, "x", empty(), None).unwrap();
    let (b, _) = ctx.publish_findings(None, "@", None, "x", empty(), None).unwrap();

    assert_eq!(ctx.rename_report(a.id, " auth-pass ").unwrap().label(), "auth-pass");
    assert_eq!(ctx.find_report(None, "AUTH-PASS").unwrap().id, a.id);
    assert!(ctx.rename_report(b.id, "auth-pass").is_err(), "taken");
    assert!(ctx.rename_report(b.id, "R77").is_err(), "looks like a default id");
    assert!(ctx.rename_report(b.id, "two words").is_err());
    assert_eq!(ctx.rename_report(a.id, "").unwrap().label(), format!("R{}", a.id));
}
