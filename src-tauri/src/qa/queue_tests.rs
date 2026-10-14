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
    }
}

fn pick(option: u32, at: f64) -> Answer {
    Answer {
        option: Some(option),
        note: None,
        other: None,
        at,
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

#[test]
fn the_text_follows_the_qqueue_template() {
    let mut q = Queue::default();
    q.add_round(vec![new("Where files live", false)]);
    q.decide(
        "Q1",
        Answer {
            note: Some("keep it simple".into()),
            ..pick(1, 1.0)
        },
    )
    .unwrap();
    let first = q.get("Q1").unwrap();
    assert_eq!(answer_text(first), "option 2: no (note: keep it simple)");
    assert_eq!(
        question_text(first),
        "Q1 — Where files live\n\nContext:\n\n- c\n\nOptions:\n\n1. yes (recommended)\n   do it\n\
         \x20  - Pro: simple\n   - Risk: breaks old files\n2. no\n\nRecommendation: option 1. because"
    );
}
