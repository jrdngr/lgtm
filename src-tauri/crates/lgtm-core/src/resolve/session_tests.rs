use super::*;
use crate::resolve::PartView;

const TWO: &str = "top
<<<<<<< conflict 1 of 2
%%%%%%% diff from: base0000 b0000000 \"base\"
\\\\\\\\\\\\\\        to: left0000 l0000000 \"left\"
-b
+l
+++++++ right000 r0000000 \"right\"
r
>>>>>>> conflict 1 of 2 ends
mid
<<<<<<< conflict 2 of 2
%%%%%%% diff from: base0000 b0000000 \"base\"
\\\\\\\\\\\\\\        to: left0000 l0000000 \"left\"
-c
+L
+++++++ right000 r0000000 \"right\"
R
>>>>>>> conflict 2 of 2 ends
end
";

fn second_conflict_raw(s: &Session) -> String {
    s.conflicts()[1].raw.clone()
}

#[test]
fn picks_go_in_press_order_and_a_second_press_takes_one_out() {
    let mut s = Session::new("f", TWO);
    s.toggle(0, 2);
    s.toggle(0, 1);
    assert!(s.conflict_text(0) == "r\nl\n");
    s.toggle(0, 2);
    assert_eq!(s.conflict_text(0), "l\n");
    s.toggle(0, 0);
    assert_eq!(s.conflict_text(0), "l\nb\n");
}

#[test]
fn saving_writes_resolved_conflicts_and_leaves_open_ones_as_markers() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = Session::new("f", TWO);
    s.toggle(0, 1);
    assert_eq!(s.count(), (1, 2));
    assert!(!s.done());
    s.save(dir.path()).unwrap();
    let want = format!("top\nl\nmid\n{}end\n", second_conflict_raw(&s));
    assert_eq!(std::fs::read_to_string(dir.path().join("f")).unwrap(), want);
    // Read again, only the open conflict is left.
    let again = Session::new("f", &want);
    assert_eq!(again.count(), (0, 1));
}

#[test]
fn one_history_undoes_picks_moves_and_edits_in_turn() {
    let mut s = Session::new("f", TWO);
    s.toggle(0, 1);
    s.toggle(0, 2);
    s.place(0, 1, 0);
    s.edit(0, 0, "R!\n");
    assert_eq!(s.conflict_text(0), "R!\nl\n");
    s.undo();
    assert_eq!(s.conflict_text(0), "r\nl\n");
    s.undo();
    assert_eq!(s.conflict_text(0), "l\nr\n");
    s.redo();
    s.redo();
    assert_eq!(s.conflict_text(0), "R!\nl\n");
    assert!(!s.can_redo());
}

#[test]
fn moving_a_pick_past_either_end_does_nothing_and_adds_no_undo_step() {
    let mut s = Session::new("f", TWO);
    s.toggle(0, 1);
    s.toggle(0, 2);
    assert!(!s.place(0, 1, 2));
    s.undo();
    assert_eq!(s.conflict_text(0), "l\n");
}

#[test]
fn an_edit_flips_to_its_original_and_back_and_matching_text_clears_it() {
    let mut s = Session::new("f", TWO);
    s.toggle(1, 2);
    s.edit(1, 0, "RR\n");
    s.flip_original(1, 0);
    assert_eq!(s.conflict_text(1), "R\n");
    assert!(s.picks(1)[0].edited.is_some());
    s.flip_original(1, 0);
    assert_eq!(s.conflict_text(1), "RR\n");
    s.edit(1, 0, "R\n");
    assert_eq!(s.picks(1)[0].edited, None);
}

const DELETED: &str = "<<<<<<< conflict 1 of 1
+++++++ xxxxxxxx x0000000 \"X\"
%%%%%%% diff from: base0000 b0000000 \"base\"
\\\\\\\\\\\\\\        to: yyyyyyyy y0000000 \"Y\"
 keep
-base
+mod
>>>>>>> conflict 1 of 1 ends
";

fn deleted() -> Session {
    let mut s = Session::new("g", DELETED);
    let by = s.conflicts()[0].sides[0].label.clone();
    s.set_fate(Fate::Deleted { by }, vec![1]);
    s
}

#[test]
fn a_deleted_file_needs_a_choice_and_its_deleting_side_cant_be_picked() {
    let mut s = deleted();
    assert!(!s.toggle(0, 1));
    s.toggle(0, 2);
    assert!(!s.done());
    assert!(!s.choose(Choice::Carry));
    s.choose(Choice::Keep);
    assert!(s.done());
}

#[test]
fn delete_removes_the_file_on_save() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("g"), DELETED).unwrap();
    let mut s = deleted();
    s.choose(Choice::Delete);
    assert!(s.done() && s.count() == (1, 1));
    assert!(s.save(dir.path()).unwrap().removed);
    assert!(!dir.path().join("g").exists());
}

#[test]
fn carrying_edits_merges_them_into_the_file_it_moved_to() {
    let dir = tempfile::tempdir().unwrap();
    let text = "<<<<<<< conflict 1 of 1
+++++++ mmmmmmmm m0000000 \"mv\"
%%%%%%% diff from: base0000 b0000000 \"base\"
\\\\\\\\\\\\\\        to: eeeeeeee e0000000 \"edit\"
 one
-two
+two edited
 three
>>>>>>> conflict 1 of 1 ends
";
    std::fs::write(dir.path().join("f"), text).unwrap();
    std::fs::write(dir.path().join("h"), "one\ntwo\nthree\nfour\n").unwrap();
    let mut s = Session::new("f", text);
    let by = s.conflicts()[0].sides[0].label.clone();
    s.set_fate(Fate::Moved { to: "h".into(), by }, vec![1]);
    s.choose(Choice::Carry);
    let saved = s.save(dir.path()).unwrap();
    assert_eq!(saved.clash, None);
    assert!(!dir.path().join("f").exists());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("h")).unwrap(),
        "one\ntwo edited\nthree\nfour\n"
    );
}

#[test]
fn the_view_numbers_lines_as_saved_and_lists_unpicked_sides_with_the_base_last() {
    let mut s = Session::new("f", TWO);
    s.toggle(0, 2);
    let v = s.view();
    let PartView::Conflict(c) = &v.parts[1] else {
        panic!("not a conflict");
    };
    assert_eq!(c.line, 2);
    assert_eq!(c.picks.iter().map(|p| p.key).collect::<Vec<_>>(), [2]);
    assert_eq!(c.rest.iter().map(|p| p.key).collect::<Vec<_>>(), [1, 0]);
    let PartView::Text { line, .. } = &v.parts[2] else {
        panic!("not text");
    };
    assert_eq!(*line, 3);
    assert_eq!((v.resolved, v.total, v.done), (1, 2, false));
}
