use super::*;

fn new(subject: &str, sub: bool) -> NewQuestion {
    NewQuestion {
        sub,
        subject: subject.into(),
        context: vec!["c".into()],
        options: vec![
            Choice {
                label: "yes".into(),
                means: "do it".into(),
                pros: vec!["simple".into()],
                risks: vec!["breaks old files".into()],
                ..Choice::default()
            },
            Choice {
                label: "no".into(),
                ..Choice::default()
            },
        ],
        why: "because".into(),
        sources: vec![],
        depends: vec![],
        auto: None,
    }
}

fn after(subject: &str, depends: &[&str]) -> NewQuestion {
    NewQuestion {
        depends: depends.iter().map(|d| d.to_string()).collect(),
        ..new(subject, false)
    }
}

fn rw(id: &str, subject: &str) -> Rewrite {
    Rewrite {
        id: id.into(),
        what: "new option".into(),
        subject: subject.into(),
        context: vec![],
        options: vec![],
        why: String::new(),
        auto: None,
    }
}

fn pick(option: u32, at: f64) -> Answer {
    Answer {
        option: Some(option),
        note: None,
        other: None,
        at,
        auto: None,
    }
}

fn ids(q: &Queue) -> Vec<&str> {
    q.questions.iter().map(|q| q.id.as_str()).collect()
}

#[test]
fn subquestions_number_under_their_parent_and_rounds_continue() {
    let mut q = Queue::default();
    q.add_round(vec![
        new("a", false),
        new("b", false),
        new("b1", true),
        new("b2", true),
    ]);
    assert_eq!(ids(&q), ["Q1", "Q2", "Q2a", "Q2b"]);
    q.add_round(vec![new("c", true), new("d", false)]);
    // A subquestion with no parent in its batch starts a new number.
    assert_eq!(ids(&q)[4..], ["Q3", "Q4"]);
    assert_eq!(q.get("Q4").unwrap().round, 2);
}

#[test]
fn undo_reopens_the_latest_decision() {
    let mut q = Queue::default();
    q.add_round(vec![new("a", false), new("b", false)]);
    q.decide("Q2", pick(1, 5.0)).unwrap();
    q.decide("Q1", pick(0, 9.0)).unwrap();
    assert!(q.decide("Q1", pick(2, 10.0)).is_err());
    assert_eq!(q.undo().as_deref(), Some("Q1"));
    assert_eq!(q.get("Q1").unwrap().state, QState::Open);
    assert_eq!(q.undo().as_deref(), Some("Q2"));
    assert_eq!(q.undo(), None);
}

#[test]
fn dependents_are_what_depends_on_an_answer_and_its_subquestions() {
    let mut q = Queue::default();
    q.add_round(vec![
        new("a", false),
        after("b", &["Q1"]),
        new("b1", true),
        after("c", &["Q1"]),
        after("d", &["Q1"]),
        new("e", false),
    ]);
    q.questions[3].state = QState::Dropped;
    q.decide("Q4", pick(0, 1.0)).unwrap();
    let ids = |id| {
        q.dependents(id)
            .iter()
            .map(|d| d.id.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(ids("Q1"), ["Q2", "Q4"]);
    assert_eq!(ids("Q2"), ["Q2a"]);
    assert!(ids("Q5").is_empty());
}

#[test]
fn an_update_reopens_only_decided_questions_that_depend_on_the_answer() {
    let mut q = Queue::default();
    q.add_round(vec![new("a", false), new("b", false), after("c", &["Q2"])]);
    q.decide("Q1", pick(0, 1.0)).unwrap();
    q.decide("Q3", pick(1, 2.0)).unwrap();
    q.decide("Q2", pick(1, 3.0)).unwrap();
    q.apply(
        "Q2",
        Update {
            rewrite: vec![rw("Q1", "a2"), rw("Q3", "c2")],
            ..Update::default()
        },
    );
    let a = q.get("Q1").unwrap();
    assert_eq!(
        (a.state, &a.answer, a.text.subject.as_str()),
        (QState::Decided, &Some(pick(0, 1.0)), "a")
    );
    let c = q.get("Q3").unwrap();
    assert_eq!((c.state, c.answer.is_none()), (QState::Open, true));
    let change = c.change.as_ref().unwrap();
    assert_eq!(
        (change.after.as_str(), change.previous.subject.as_str()),
        ("Q2", "c")
    );
    assert_eq!(change.answer, Some(pick(1, 2.0)));

    // Rewriting it again while it's open keeps the text the user last saw as the previous.
    q.apply(
        "Q2",
        Update {
            rewrite: vec![rw("Q3", "c3")],
            ..Update::default()
        },
    );
    let c = q.get("Q3").unwrap();
    assert_eq!(c.text.subject, "c3");
    assert_eq!(c.change.as_ref().unwrap().previous.subject, "c");
    assert_eq!(c.change.as_ref().unwrap().answer, Some(pick(1, 2.0)));

    // Undoing Q2 puts Q3 back as it was decided.
    assert_eq!(q.undo().as_deref(), Some("Q2"));
    let c = q.get("Q3").unwrap();
    assert_eq!(
        (
            c.state,
            &c.answer,
            c.text.subject.as_str(),
            c.change.is_none()
        ),
        (QState::Decided, &Some(pick(1, 2.0)), "c", true)
    );
}

#[test]
fn an_update_drops_rewrites_and_adds_without_touching_decisions() {
    let mut q = Queue::default();
    q.add_round(vec![new("a", false), new("b", false), new("c", false)]);
    q.decide("Q1", pick(0, 1.0)).unwrap();
    q.apply(
        "Q1",
        Update {
            drop: vec![
                Drop {
                    id: "Q2".into(),
                    why: "settled by Q1".into(),
                },
                Drop {
                    id: "Q1".into(),
                    why: "self".into(),
                },
            ],
            rewrite: vec![rw("Q3", "c2"), rw("Q9", "nope")],
            add: vec![after("d", &["Q1"])],
        },
    );
    assert_eq!(q.get("Q1").unwrap().state, QState::Decided);
    assert_eq!(
        q.get("Q2").unwrap().dropped.as_deref(),
        Some("settled by Q1")
    );
    // An open question changes without a change marker.
    let c = q.get("Q3").unwrap();
    assert_eq!((c.text.subject.as_str(), c.change.is_none()), ("c2", true));
    assert_eq!(q.get("Q4").unwrap().depends, ["Q1"]);
    assert_eq!(ids(&q), ["Q1", "Q2", "Q3", "Q4"]);
    assert_eq!(q.open_count(), 2);

    // A second update after the same answer adds to the first, and undoing Q1 puts back
    // everything both did, dropping the question they added.
    q.apply(
        "Q1",
        Update {
            rewrite: vec![rw("Q3", "c3")],
            ..Update::default()
        },
    );
    assert_eq!(q.undo().as_deref(), Some("Q1"));
    assert_eq!(ids(&q), ["Q1", "Q2", "Q3"]);
    assert_eq!(q.get("Q2").unwrap().state, QState::Open);
    assert_eq!(q.get("Q3").unwrap().text.subject, "c");
    assert!(q.effects.is_empty());
}

#[test]
fn rewriting_a_decided_question_asks_it_again() {
    let mut q = Queue::default();
    q.add_round(vec![new("a", false), new("b", false)]);
    q.decide("Q2", pick(1, 1.0)).unwrap();
    q.reask(rw("Q2", "b?")).unwrap();
    let b = q.get("Q2").unwrap();
    assert_eq!((b.state, b.answer.is_none()), (QState::Open, true));
    assert_eq!(b.change.as_ref().unwrap().after, "discussion");
}

#[test]
fn a_defer_cascades_through_open_dependents_and_undoes_in_one_step() {
    let mut q = Queue::default();
    q.add_round(vec![
        new("a", false),
        after("b", &["Q1"]),
        after("c", &["Q2"]),
        new("c1", true),
        after("d", &["Q1"]),
        after("e", &["Q4"]),
        new("f", false),
    ]);
    q.decide("Q4", pick(0, 0.5)).unwrap();
    q.decide("Q6", pick(0, 1.0)).unwrap();
    // A decided dependent stops the cascade: Q5 hangs off Q4.
    assert_eq!(q.defer_preview("Q1"), ["Q2", "Q3", "Q3a"]);
    assert_eq!(q.defer("Q1", 2.0).unwrap(), ["Q1", "Q2", "Q3", "Q3a"]);
    assert_eq!(q.get("Q4").unwrap().state, QState::Decided);
    assert_eq!(q.open_count(), 1);
    assert!(q.defer("Q1", 3.0).is_err());

    assert_eq!(q.undo().as_deref(), Some("Q1"));
    assert_eq!(q.open_count(), 5);
    assert_eq!(q.undo().as_deref(), Some("Q6"));
}

fn auto(subject: &str, why: &str) -> NewQuestion {
    NewQuestion {
        auto: Some(why.into()),
        ..new(subject, false)
    }
}

#[test]
fn auto_answers_come_decided_and_undo_leaves_them() {
    let mut q = Queue::default();
    q.add_round(vec![
        auto("a", "settings are TOML"),
        after("b", &["Q1"]),
        auto("c", " "),
    ]);
    let a = q.get("Q1").unwrap();
    assert_eq!(a.state, QState::Decided);
    let answer = a.answer.as_ref().unwrap();
    assert_eq!(
        (answer.option, answer.auto.as_deref()),
        (Some(0), Some("settings are TOML"))
    );
    // A blank reason asks the question.
    assert_eq!(q.get("Q3").unwrap().state, QState::Open);
    assert_eq!(q.dependents("Q1")[0].id, "Q2");
    assert_eq!(q.undo(), None);
}

#[test]
fn reopening_an_auto_answer_marks_it_and_undo_puts_it_back() {
    let mut q = Queue::default();
    q.add_round(vec![auto("a", "obvious"), new("b", false)]);
    q.decide("Q2", pick(0, 1.0)).unwrap();
    q.change_answer("Q1", 2.0).unwrap();
    let a = q.get("Q1").unwrap();
    let change = a.change.as_ref().unwrap();
    assert_eq!((a.state, change.after.as_str()), (QState::Open, "auto"));
    assert!(change.answer.as_ref().unwrap().is_auto());

    // The user's own answer undoes first, then the reopen, then the earlier decision.
    q.decide("Q1", pick(1, 3.0)).unwrap();
    assert_eq!(q.undo().as_deref(), Some("Q1"));
    assert_eq!(q.get("Q1").unwrap().state, QState::Open);
    assert_eq!(q.undo().as_deref(), Some("Q1"));
    let a = q.get("Q1").unwrap();
    assert_eq!(
        (
            a.state,
            a.answer.as_ref().unwrap().is_auto(),
            a.change.is_none()
        ),
        (QState::Decided, true, true)
    );
    assert_eq!(q.undo().as_deref(), Some("Q2"));
    assert_eq!(q.undo(), None);
}

/// Q1 to answer, and Q2, auto-answered, depending on it.
fn auto_after_answer() -> Queue {
    let mut q = Queue::default();
    q.add_round(vec![
        new("a", false),
        NewQuestion {
            depends: vec!["Q1".into()],
            ..auto("b", "obvious")
        },
    ]);
    q.decide("Q1", pick(0, 1.0)).unwrap();
    q
}

fn auto_why(q: &Queue, id: &str) -> Option<String> {
    let q = q.get(id).unwrap();
    q.answer.as_ref().and_then(|a| a.auto.clone())
}

#[test]
fn a_rewrite_that_keeps_auto_keeps_the_auto_answer() {
    let mut q = auto_after_answer();
    q.apply(
        "Q1",
        Update {
            rewrite: vec![Rewrite {
                auto: Some(" still obvious ".into()),
                ..rw("Q2", "b2")
            }],
            ..Update::default()
        },
    );
    let b = q.get("Q2").unwrap();
    assert_eq!((b.state, b.text.subject.as_str()), (QState::Decided, "b2"));
    assert_eq!(auto_why(&q, "Q2").as_deref(), Some("still obvious"));
    assert!(b.change.is_none());

    // Undoing Q1 puts Q2 back as it was.
    assert_eq!(q.undo().as_deref(), Some("Q1"));
    assert_eq!(q.get("Q2").unwrap().text.subject, "b");
    assert_eq!(auto_why(&q, "Q2").as_deref(), Some("obvious"));
}

#[test]
fn a_rewrite_without_auto_opens_the_question_plainly() {
    let mut q = auto_after_answer();
    q.apply(
        "Q1",
        Update {
            rewrite: vec![Rewrite {
                auto: Some(" ".into()),
                ..rw("Q2", "b2")
            }],
            ..Update::default()
        },
    );
    let b = q.get("Q2").unwrap();
    assert_eq!(
        (b.state, &b.answer, &b.change),
        (QState::Open, &None, &None)
    );

    // The user's answer to Q2 undoes first, then Q1's, which brings the auto answer back.
    let mine = Answer {
        option: None,
        other: Some("mine".into()),
        ..pick(0, 2.0)
    };
    q.decide("Q2", mine).unwrap();
    assert_eq!(q.undo().as_deref(), Some("Q2"));
    assert_eq!(q.get("Q2").unwrap().state, QState::Open);
    assert_eq!(q.undo().as_deref(), Some("Q1"));
    let b = q.get("Q2").unwrap();
    assert_eq!((b.state, b.text.subject.as_str()), (QState::Decided, "b"));
    assert_eq!(auto_why(&q, "Q2").as_deref(), Some("obvious"));
    assert_eq!(q.undo(), None);
}
