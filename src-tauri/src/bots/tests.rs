use serde_json::json;

use super::output::{Challenge, Finding, Review, Severity};
use super::pipeline::{drop_unfinished, extend, next_step, wanted};
use super::*;
use crate::store::Store;

fn persona(name: &str) -> Persona {
    Persona {
        name: name.into(),
        description: "d".into(),
        off_lens: true,
        profile: "- p".into(),
    }
}

fn run(names: &[&str], options: BotOptions) -> BotRun {
    BotRun {
        id: 0,
        workspace: None,
        revset: "@".into(),
        from: Some("a".into()),
        to: "b".into(),
        personas: names.iter().map(|n| persona(n)).collect(),
        options: options.normalized(names.len()),
        parts: vec![],
        done: vec![],
        created_at: 1.0,
        fixes: vec![],
        active: false,
    }
}

fn part(step: Step, who: Option<&str>, status: Status, output: Option<Output>) -> Part {
    Part {
        output,
        status,
        ..Part::running(step, who.map(str::to_string))
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
        off_lens: false,
        sources: vec![],
        raised_by: vec![],
        contested: None,
    }
}

fn review(findings: Vec<Finding>) -> Option<Output> {
    Some(Output::Review(Review {
        verdict: "v".into(),
        findings,
    }))
}

fn names(parts: Vec<Option<String>>) -> Vec<String> {
    parts.into_iter().flatten().collect()
}

const CROSS: BotOptions = BotOptions {
    synthesize: false,
    cross_review: true,
    adversarial: false,
    minimal: false,
};

#[test]
fn cross_review_challenges_what_is_worth_it_and_rebuts_only_challenged_personas() {
    let mut r = run(&["sec", "simple"], CROSS);
    assert!(
        r.options.synthesize,
        "cross-review needs a synthesis to rule"
    );
    assert_eq!(next_step(&r), Some(Step::Reports));
    assert_eq!(names(wanted(Step::Reports, &r)), ["sec", "simple"]);

    let high = review(vec![finding("sec#1", Severity::High)]);
    // Only low findings, which the challenge pass doesn't look at.
    let low = review(vec![finding("simple#1", Severity::Low)]);
    r.parts = vec![
        part(Step::Reports, Some("sec"), Status::Done, high),
        part(Step::Reports, Some("simple"), Status::Done, low),
    ];
    r.done.push(Step::Reports);
    assert_eq!(next_step(&r), Some(Step::Challenges));
    assert_eq!(names(wanted(Step::Challenges, &r)), ["simple"]);

    let challenge = Challenge {
        finding: "sec#1".into(),
        argument: "no".into(),
    };
    r.parts.push(part(
        Step::Challenges,
        Some("simple"),
        Status::Done,
        Some(Output::Challenges {
            challenges: vec![challenge],
        }),
    ));
    r.done.push(Step::Challenges);
    assert_eq!(names(wanted(Step::Rebuttals, &r)), ["sec"]);

    r.done.extend([Step::Rebuttals, Step::Synthesis]);
    assert_eq!(next_step(&r), None);
}

#[test]
fn a_lone_persona_gets_no_cross_review() {
    let r = run(&["general"], CROSS);
    assert!(!r.options.cross_review && !r.options.synthesize);
}

#[test]
fn a_retry_reruns_only_what_did_not_finish() {
    let mut r = run(&["sec", "general"], BotOptions::default());
    r.parts = vec![
        part(Step::Reports, Some("sec"), Status::Done, review(vec![])),
        part(Step::Reports, Some("general"), Status::Failed, None),
    ];
    drop_unfinished(&mut r);
    assert_eq!(r.parts.len(), 1);
    assert_eq!(names(wanted(Step::Reports, &r)), ["general"]);
}

#[test]
fn adding_cross_review_redoes_synthesis_and_minimal() {
    let mut r = run(
        &["sec", "general"],
        BotOptions {
            synthesize: true,
            minimal: true,
            ..BotOptions::default()
        },
    );
    r.done = vec![Step::Reports, Step::Synthesis, Step::Minimal];
    r.parts = vec![
        part(Step::Reports, Some("sec"), Status::Done, review(vec![])),
        part(Step::Synthesis, None, Status::Done, None),
        part(Step::Minimal, None, Status::Done, None),
    ];
    let minimal = BotOptions {
        minimal: true,
        ..BotOptions::default()
    };
    extend(&mut r, minimal);
    assert_eq!(next_step(&r), None, "nothing new to do");

    extend(&mut r, CROSS);
    assert_eq!(r.done, [Step::Reports]);
    assert_eq!(r.parts.len(), 1);
    assert_eq!(next_step(&r), Some(Step::Challenges));
}

#[test]
fn adding_synthesis_redoes_a_lone_personas_minimal_pick() {
    let minimal = BotOptions {
        minimal: true,
        ..BotOptions::default()
    };
    let mut r = run(&["general"], minimal);
    r.done = vec![Step::Reports, Step::Minimal];
    r.parts = vec![
        part(Step::Reports, Some("general"), Status::Done, review(vec![])),
        part(Step::Minimal, None, Status::Done, None),
    ];
    let synthesize = BotOptions {
        synthesize: true,
        ..BotOptions::default()
    };
    extend(&mut r, synthesize);
    assert_eq!(r.done, [Step::Reports]);
    assert_eq!(r.parts.len(), 1);
    assert_eq!(next_step(&r), Some(Step::Synthesis));
}

#[test]
fn a_review_is_sorted_by_severity_and_numbered_by_persona() {
    let value = json!({
        "verdict": "fine",
        "findings": [
            { "severity": "low", "title": "a", "locations": [{ "path": "x.rs", "start": 1, "end": 2 }],
              "facts": [], "explanation": "", "fix": "f", "offLens": false },
            { "severity": "critical", "title": "b", "locations": [], "facts": ["x"],
              "explanation": "", "fix": null, "offLens": true },
        ],
    });
    let Output::Review(r) = Output::parse(Step::Reports, Some("sec"), value).unwrap() else {
        panic!("not a review");
    };
    let got: Vec<_> = r
        .findings
        .iter()
        .map(|f| (f.id.as_str(), f.title.as_str()))
        .collect();
    assert_eq!(got, [("sec#1", "b"), ("sec#2", "a")]);
    assert!(Output::parse(Step::Reports, Some("sec"), json!({ "verdict": 1 })).is_err());
}

#[test]
fn runs_round_trip_through_the_store_and_unreadable_ones_are_skipped() {
    let store = Store::in_memory();
    let mut r = run(&["sec"], BotOptions::default());
    let info = review(vec![finding("sec#1", Severity::Info)]);
    r.parts.push(Part {
        session: Some("s".into()),
        ..part(Step::Reports, Some("sec"), Status::Done, info)
    });
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
fn live_activity_fills_running_parts() {
    let live = Live::default();
    let mut r = run(&["sec"], BotOptions::default());
    r.id = 7;
    r.parts
        .push(Part::running(Step::Reports, Some("sec".into())));
    assert!(live.begin(7) && !live.begin(7));
    live.set_activity(7, 0, "Read a.rs".into());
    let filled = live.fill(r.clone());
    assert!(filled.active);
    assert_eq!(filled.parts[0].activity.as_deref(), Some("Read a.rs"));
    live.end(7);
    let ended = live.fill(r);
    assert!(!ended.active);
    assert_eq!(ended.parts[0].status, Status::Stopped);
}

#[test]
fn live_fixes_fill_text_and_show_stopped_once_gone() {
    let live = Live::default();
    let mut r = run(&["sec"], BotOptions::default());
    r.id = 7;
    r.fixes.push(Fix {
        findings: vec!["#sec#1".into()],
        status: Status::Running,
        text: String::new(),
        error: None,
        created_at: 0.0,
    });
    live.begin_fix(7, 0);
    live.append_fix(7, 0, "a");
    assert_eq!(live.append_fix(7, 0, "b"), "ab");
    assert!(live.has_fix(7) && !live.has_fix(8));
    assert_eq!(live.fill(r.clone()).fixes[0].text, "ab");
    live.end_fix(7, 0);
    assert!(!live.has_fix(7));
    assert_eq!(live.fill(r).fixes[0].status, Status::Stopped);
}

/// Runs the real `claude` for every step, so it's skipped unless asked for with `--ignored`.
#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn personas_review_cross_review_and_synthesize_a_real_change() {
    use std::sync::Arc;
    use std::time::Duration;

    use crate::agent::Runs;
    use crate::review::tests::{jj, repo, write};

    let (dir, ctx) = repo();
    let defaults = ["security-bot".to_string(), "simplicity-bot".to_string()];
    super::install::install(dir.path(), &defaults, personas::Scope::Repo).unwrap();
    write(
        dir.path(),
        "login.py",
        "def login(user, pw):\n    print(f'login {user} {pw}')\n    return check(user, pw)\n",
    );
    jj(dir.path(), &["describe", "-m", "Log logins"]);
    let ctx = Arc::new(ctx);
    let options = BotOptions {
        cross_review: true,
        minimal: true,
        ..BotOptions::default()
    };
    let notify: Notify = Arc::new(|_| {});
    let run = ctx
        .clone()
        .start_bots(None, "@", &defaults, options, Runs::default(), notify)
        .await
        .unwrap();
    while ctx.bots.is_active(run.id) {
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    let run = ctx.bot_run(run.id).unwrap();
    println!("{}", serde_json::to_string_pretty(&run.parts).unwrap());
    assert!(run.done.contains(&Step::Minimal), "{:?}", run.done);
    let synthesis = run.parts.iter().find(|p| p.step == Step::Synthesis);
    let Some(Output::Synthesis(s)) = synthesis.and_then(|p| p.output.as_ref()) else {
        panic!("no synthesis");
    };
    assert!(s.findings.iter().any(|f| f.severity == Severity::Critical));
}

/// Runs the real `claude`, so it's skipped unless asked for with `--ignored`.
#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn a_headless_fix_edits_the_working_copy() {
    use std::sync::Arc;
    use std::time::Duration;

    use crate::agent::Runs;
    use crate::review::tests::{repo, write};

    let (dir, ctx) = repo();
    write(
        dir.path(),
        "login.py",
        "def login(user, pw):\n    print(f'login {user} {pw}')\n    return True\n",
    );
    let ctx = Arc::new(ctx);
    let mut r = run(&["sec"], BotOptions::default());
    r.id = ctx.store.lock().unwrap().add_bot_run(&r).unwrap();
    let text = "**🔴 CRITICAL — login() prints the plaintext password** (sec)\n\n\
                Where: `login.py:2`\n\n**Fix:** Log only the username.";
    let notify: Notify = Arc::new(|_| {});
    ctx.clone()
        .fix_findings(
            r.id,
            vec!["sec#1".into()],
            text.into(),
            Runs::default(),
            notify,
        )
        .await
        .unwrap();
    let fix = loop {
        let run = ctx.bot_run(r.id).unwrap();
        if run.fixes[0].status != Status::Running {
            break run.fixes[0].clone();
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    };
    println!("{fix:#?}");
    assert_eq!(fix.status, Status::Done);
    let code = std::fs::read_to_string(dir.path().join("login.py")).unwrap();
    assert!(!code.contains("{pw}"), "{code}");
}
