use super::queue::{Choice, NewQuestion, QState, Update};
use super::*;

fn question(subject: &str) -> NewQuestion {
    NewQuestion {
        sub: false,
        subject: subject.into(),
        context: vec![],
        options: ["a", "b"]
            .map(|l| Choice {
                label: l.into(),
                ..Choice::default()
            })
            .to_vec(),
        why: "w".into(),
        sources: vec![],
        depends: vec![],
        auto: None,
    }
}

#[tokio::test]
async fn a_decision_drops_blank_notes_and_undo_takes_it_back() {
    let (_dir, ctx) = crate::review::tests::repo();
    let ws = ctx.workspace(None).await.unwrap();
    let new = AskIn::New {
        prompt: "p".into(),
        attach: vec![],
    };
    let (id, _) = ctx.ask_qa(&ws, new, None, vec![question("x")]).unwrap();
    let decide = QuestionAct::Decide {
        id: "Q1".into(),
        answer: edit::AnswerInput {
            option: Some(1),
            note: Some(" ".into()),
            other: Some("".into()),
        },
    };
    let s = ctx.act_on_question(&ws, &id, decide).unwrap();
    let answer = s.queue.get("Q1").unwrap().answer.clone().unwrap();
    assert_eq!(
        (answer.option, answer.note, answer.other),
        (Some(1), None, None)
    );
    let s = ctx.act_on_question(&ws, &id, QuestionAct::Undo).unwrap();
    assert_eq!(s.count(QState::Open), 1);
}

#[tokio::test]
async fn the_asking_terminal_runs_the_session() {
    let (_dir, ctx) = crate::review::tests::repo();
    let ws = ctx.workspace(None).await.unwrap();
    let new = AskIn::New {
        prompt: "p".into(),
        attach: vec![],
    };
    let (id, ids) = ctx
        .ask_qa(
            &ws,
            new,
            Some("t1".into()),
            vec![question("x"), question("y")],
        )
        .unwrap();
    assert_eq!(ids, ["Q1", "Q2"]);
    let (_, more) = ctx
        .ask_qa(&ws, AskIn::Session(id.clone()), None, vec![question("z")])
        .unwrap();
    assert_eq!(more, ["Q3"]);
    let s = ctx.qa_session(&ws, &id).unwrap();
    assert!(s.from_terminal);
    assert_eq!(s.terminal.as_deref(), Some("t1"));
    let edit = SessionEdit::Terminal { id: "t2".into() };
    let s = ctx.edit_qa_session(&ws, &id, edit).unwrap();
    assert_eq!(s.terminal.as_deref(), Some("t2"));
}

#[tokio::test]
async fn questions_for_a_sent_session_start_a_new_one() {
    let (_dir, ctx) = crate::review::tests::repo();
    let ws = ctx.workspace(None).await.unwrap();
    let old = ctx.create_qa(&ws, "p", vec!["a.md".into()]).unwrap().id;
    let sent = SessionEdit::Sent {
        terminal: "t".into(),
    };
    ctx.edit_qa_session(&ws, &old, sent).unwrap();
    let (id, ids) = ctx
        .ask_qa(&ws, AskIn::Session(old.clone()), None, vec![question("x")])
        .unwrap();
    assert_ne!(id, old);
    assert_eq!(ids, ["Q1"]);
    let s = ctx.qa_session(&ws, &id).unwrap();
    assert_eq!(
        (s.prompt.as_str(), s.attach.as_slice()),
        ("p", ["a.md".to_string()].as_slice())
    );
    assert!(s.from_terminal && s.sent.is_none());
    assert!(ctx.qa_session(&ws, &old).unwrap().queue.get("Q1").is_none());
}

#[tokio::test]
async fn an_update_needs_the_question_it_follows() {
    let (_dir, ctx) = crate::review::tests::repo();
    let ws = ctx.workspace(None).await.unwrap();
    let id = ctx.create_qa(&ws, "p", vec![]).unwrap().id;
    let s = ctx.qa_session(&ws, &id).unwrap();
    assert!(!s.from_terminal);
    assert!(ctx.update_qa(&ws, &id, "Q9", Update::default()).is_err());
}
