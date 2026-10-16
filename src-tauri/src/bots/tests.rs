use serde_json::json;

use super::output::{Finding, Review, Severity};
use super::submitted::Summary;
use super::*;
use crate::store::Store;

fn run(parts: Vec<Part>) -> BotRun {
    BotRun {
        id: 0,
        name: None,
        workspace: None,
        revset: "@".into(),
        from: Some("a".into()),
        to: "b".into(),
        personas: vec![Persona {
            name: "sec".into(),
            description: "d".into(),
        }],
        parts,
        created_at: 1.0,
        dismissed: vec![],
    }
}

fn finding(id: &str, severity: Severity) -> Finding {
    Finding {
        id: id.into(),
        severity,
        title: "t".into(),
        locations: vec![],
        facts: vec![],
        explanation: "e".into(),
        fix: None,
    }
}

#[test]
fn published_fixes_name_findings_by_the_ids_they_get_once_sorted() {
    let value = json!({
        "verdict": "v",
        "findings": [
            { "severity": "low", "title": "a", "locations": [], "facts": [], "explanation": "", "fix": null },
            { "severity": "critical", "title": "b", "locations": [], "facts": [], "explanation": "", "fix": null },
        ],
    });
    let summary = Summary {
        task: "t".into(),
        fixes: vec![("fix b".into(), vec![2]), ("fix a".into(), vec![1])],
        off_task: vec![],
        questions: vec![("q".into(), vec![2])],
    };
    let m = summary.to_filter("cq", &value).unwrap();
    let closes: Vec<_> = m.fixes.iter().map(|f| f.closes.clone()).collect();
    assert_eq!(closes, [vec!["cq#1".to_string()], vec!["cq#2".to_string()]]);
    assert_eq!(m.questions[0].findings, ["cq#1"]);

    let bad = Summary {
        off_task: vec![3],
        ..summary
    };
    assert!(bad.to_filter("cq", &value).is_err());
}

#[test]
fn a_review_is_sorted_by_severity_and_numbered_by_reviewer() {
    let value = json!({
        "verdict": "fine",
        "findings": [
            { "severity": "low", "title": "a", "locations": [{ "path": "x.rs", "start": 1, "end": 2 }],
              "facts": [], "explanation": "", "fix": "f" },
            { "severity": "critical", "title": "b", "locations": [], "facts": ["x"],
              "explanation": "", "fix": null },
        ],
    });
    let r = Review::parse("sec", value).unwrap();
    let got: Vec<_> = r
        .findings
        .iter()
        .map(|f| (f.id.as_str(), f.title.as_str()))
        .collect();
    assert_eq!(got, [("sec#1", "b"), ("sec#2", "a")]);
    assert!(Review::parse("sec", json!({ "verdict": 1 })).is_err());
}

#[test]
fn reports_round_trip_through_the_store_and_unreadable_ones_are_skipped() {
    let store = Store::in_memory();
    let mut r = run(vec![Part {
        persona: Some("sec".into()),
        output: Output::Review(Review {
            verdict: "v".into(),
            findings: vec![finding("sec#1", Severity::Info)],
        }),
    }]);
    r.id = store.add_bot_run(&r).unwrap();
    assert_eq!(store.bot_run(r.id).unwrap(), Some(r.clone()));
    store
        .add_bot_run_raw_for_test("", "{\"old\": true}")
        .unwrap();
    assert_eq!(store.bot_runs("").unwrap(), [r.clone()]);
    assert!(store.bot_runs("other").unwrap().is_empty());
    store.delete_bot_run(r.id).unwrap();
    assert_eq!(store.bot_run(r.id).unwrap(), None);
}

#[test]
fn an_old_headless_run_keeps_its_reviewers_findings() {
    let store = Store::in_memory();
    let review = json!({ "kind": "review", "verdict": "v", "findings": [] });
    let minimal = json!({ "kind": "minimal", "task": "t", "offTask": [] });
    let old = |parts: serde_json::Value| {
        json!({
            "id": 0, "workspace": null, "revset": "@", "from": null, "to": "b",
            "personas": [{ "name": "sec", "description": "d", "offLens": true, "profile": "p" }],
            "options": { "crossReview": true, "suggest": true }, "done": [], "fixes": [],
            "createdAt": 1.0, "parts": parts,
        })
        .to_string()
    };
    let part = |step: &str, status: &str, output: &serde_json::Value| {
        json!({ "step": step, "persona": "sec", "status": status, "output": output })
    };
    store
        .add_bot_run_raw_for_test(
            "a",
            &old(json!([
                part("reports", "done", &review),
                part("reports", "failed", &serde_json::Value::Null),
                json!({ "step": "minimal", "persona": null, "status": "done", "output": minimal }),
            ])),
        )
        .unwrap();
    store
        .add_bot_run_raw_for_test(
            "b",
            &old(json!([
                part("reports", "done", &review),
                part("challenges", "done", &json!({ "kind": "challenges", "challenges": [] })),
                part("synthesis", "done", &json!({ "kind": "synthesis" })),
                part("minimal", "done", &minimal),
            ])),
        )
        .unwrap();

    let kinds = |ws: &str| -> Vec<&'static str> {
        store.bot_runs(ws).unwrap()[0]
            .parts
            .iter()
            .map(|p| match p.output {
                Output::Review(_) => "review",
                Output::Minimal(_) => "minimal",
            })
            .collect()
    };
    assert_eq!(kinds("a"), ["review", "minimal"]);
    assert_eq!(kinds("b"), ["review"], "its fixes named the synthesis's findings");
}
