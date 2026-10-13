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
    ctx.mark(review.id, &[b], Some(MarkColor::Violet), &[])
        .unwrap();
    ctx.mark(review.id, &[c], Some(MarkColor::Red), &[])
        .unwrap();
    let summary = ctx.summary(None).unwrap();
    let f = &summary.files[0];
    assert_eq!((f.rows.done, f.rows.left), (1, 2));
    let marks: Vec<_> = f.rows.marks.iter().map(|(k, n)| (k.as_str(), *n)).collect();
    assert_eq!(marks, [("flagged", 1), ("reviewed", 1)]);
    assert_eq!(f.left, ["3-4"], "flagged rows aren't done");

    let (id, state) = ctx.agent_comment(None, "a.rs", 3, 3, "why c?").unwrap();
    assert_eq!(id, review.id);
    let c = &state.comments[0];
    assert!(c.review.ends_with("terminal agent"), "{}", c.review);
    assert_eq!(c.snippet, "+fn c() {}");

    let (_, state) = ctx.agent_reply(None, c.id, "because").unwrap();
    assert_eq!(state.comments[0].messages[0].author, Author::Agent);
    let listed = ctx.agent_comments(None).unwrap();
    assert_eq!(listed[0].lines.as_deref(), Some("3"));

    assert!(ctx.agent_comment(None, "nope.rs", 1, 1, "x").is_err());
}

#[tokio::test]
async fn submitted_findings_become_a_finished_run() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "a.rs", "fn a() {}\n");
    let review = ctx.open(None, "@", false).await.unwrap();
    ctx.focus(None, review.id);
    let finding = |severity: &str, title: &str| {
        serde_json::json!({ "severity": severity, "title": title, "locations": [{ "path": "a.rs", "start": 1, "end": 1 }],
            "facts": [], "explanation": "e", "fix": null })
    };
    let review_json = serde_json::json!({ "verdict": "ok", "findings": [finding("low", "nit"), finding("high", "bug")] });
    let run = ctx.submit_findings(None, "claude", review_json).unwrap();
    assert_eq!(run.revset, "@");
    let Some(crate::bots::Output::Review(r)) = &run.parts[0].output else {
        panic!("no review");
    };
    let ids: Vec<(&str, &str)> = r
        .findings
        .iter()
        .map(|f| (f.id.as_str(), f.title.as_str()))
        .collect();
    assert_eq!(ids, [("claude#1", "bug"), ("claude#2", "nit")]);
    assert_eq!(ctx.bot_runs(None).unwrap()[0].id, run.id);
}
