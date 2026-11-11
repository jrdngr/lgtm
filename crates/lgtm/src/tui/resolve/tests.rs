use super::*;
use crate::edit::spec::keys;

const TEXT: &str = "top
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

fn press(r: &mut Resolver, spec: &str) {
    for k in keys(spec) {
        r.key(k);
    }
}

#[test]
fn numbers_pick_the_conflict_under_the_cursor_and_j_k_move_the_side_under_it() {
    let mut r = Resolver::new("f", TEXT);
    press(&mut r, "21");
    assert_eq!(r.session.conflict_text(0), "r\nl\n");
    // The cursor is on the conflict's head; down to the first kept side, then move it down.
    press(&mut r, "jJ");
    assert_eq!(r.session.conflict_text(0), "l\nr\n");
    // At the bottom already, J does nothing.
    press(&mut r, "J");
    assert_eq!(r.session.conflict_text(0), "l\nr\n");
    press(&mut r, "K");
    assert_eq!(r.session.conflict_text(0), "r\nl\n");
}

#[test]
fn n_and_shift_n_go_round_every_conflict_resolved_or_not() {
    let mut r = Resolver::new("f", TEXT);
    press(&mut r, "1");
    let head = |r: &Resolver| r.rows[r.cursor].clone();
    press(&mut r, "n");
    assert_eq!(head(&r), Row::Head { conflict: 1 });
    press(&mut r, "n");
    assert_eq!(head(&r), Row::Head { conflict: 0 });
    press(&mut r, "N");
    assert_eq!(head(&r), Row::Head { conflict: 1 });
}

#[test]
fn i_edits_a_kept_side_and_esc_in_normal_mode_keeps_it_as_one_undo_step() {
    let mut r = Resolver::new("f", TEXT);
    press(&mut r, "2ji");
    assert!(r.typing());
    press(&mut r, "x<esc>");
    assert_eq!(r.mode(), Some(Mode::Normal));
    press(&mut r, "<esc>");
    assert_eq!(r.mode(), None);
    assert_eq!(r.session.conflict_text(0), "xr\n");
    press(&mut r, "u");
    assert_eq!(r.session.conflict_text(0), "r\n");
}

#[test]
fn saving_in_the_editor_writes_the_picks_and_leaves_nothing_unsaved() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f"), TEXT).unwrap();
    let mut e = crate::tui::edit::Editor::new(dir.path());
    e.open("f", 1).unwrap();
    for k in keys("1n2") {
        e.key(k);
    }
    assert_eq!(e.unsaved(), ["f"]);
    for k in keys(":w<ret>") {
        e.key(k);
    }
    assert_eq!(
        std::fs::read_to_string(dir.path().join("f")).unwrap(),
        "top\nl\nmid\nR\nend\n"
    );
    assert!(e.unsaved().is_empty());
}
