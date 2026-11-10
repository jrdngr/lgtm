use super::*;

fn tabs(terms: &[TermId]) -> Vec<Tab> {
    terms.iter().map(|t| Tab::Term(*t)).collect()
}

/// The spaces in the shown layout's slots, in reading order, `None` for empty ones.
fn shown(m: &Model) -> Vec<Option<SpaceId>> {
    m.layout().slots.leaves().iter().map(|s| s.space).collect()
}

fn space_of(m: &Model, t: TermId) -> SpaceId {
    m.space_of(t.into()).unwrap().id
}

#[test]
fn a_split_makes_an_empty_focused_slot_that_new_tabs_fill() {
    let (mut m, a) = Model::new();
    let s1 = space_of(&m, a);
    m.split_slot(Axis::Row);
    assert_eq!(shown(&m), [Some(s1), None]);
    assert_eq!(m.tab(), None);
    let b = m.new_tab();
    let s2 = space_of(&m, b);
    assert_eq!(shown(&m), [Some(s1), Some(s2)]);
    assert_eq!(m.term(), Some(b));
    assert!(m.is_valid());
}

#[test]
fn putting_a_shown_space_in_another_slot_empties_its_old_one() {
    let (mut m, a) = Model::new();
    let s1 = space_of(&m, a);
    m.split_slot(Axis::Row);
    let right = m.layout().focus;
    m.assign(right, s1);
    assert_eq!(shown(&m), [None, Some(s1)]);
    assert_eq!(m.term(), Some(a));
}

#[test]
fn focusing_a_hidden_terminal_swaps_its_space_into_the_focused_slot() {
    let (mut m, a) = Model::new();
    let s1 = space_of(&m, a);
    m.split_slot(Axis::Row);
    let b = m.add_space();
    let c = m.add_space_behind();
    let (s2, s3) = (space_of(&m, b), space_of(&m, c));
    m.focus_term(a);
    assert_eq!(shown(&m), [Some(s1), Some(s2)]);
    m.focus_term(c);
    assert_eq!(shown(&m), [Some(s3), Some(s2)]);
    assert_eq!(m.term(), Some(c));
    assert_eq!(m.terms(), [a, b, c]);
}

#[test]
fn each_layout_keeps_its_own_spaces() {
    let (mut m, a) = Model::new();
    let s1 = space_of(&m, a);
    m.new_layout();
    assert_eq!(shown(&m), [None]);
    let b = m.add_space();
    m.cycle_layout(1);
    assert_eq!(shown(&m), [Some(s1)]);
    m.cycle_layout(1);
    assert_eq!(shown(&m), [Some(space_of(&m, b))]);
    let first = m.layouts[0].id;
    m.delete_layout(first);
    m.delete_layout(m.layouts[0].id);
    assert_eq!(m.layouts.len(), 1);
    assert_eq!(m.spaces.len(), 2);
}

#[test]
fn closing_a_space_empties_its_slots_and_the_last_one_stays() {
    let (mut m, a) = Model::new();
    let below = m.split_split(Axis::Column);
    m.split_slot(Axis::Row);
    let b = m.add_space();
    m.new_layout();
    m.focus_term(b);
    assert!(m.remove_tab(below.into()));
    assert_eq!(m.space_of(a.into()).unwrap().splits.leaves().len(), 1);
    assert!(m.remove_tab(b.into()));
    assert_eq!(shown(&m), [None]);
    assert_eq!(m.layouts[0].spaces().len(), 1);
    assert!(!m.remove_tab(a.into()));
    assert!(m.is_valid());
}

#[test]
fn only_an_empty_slot_that_isnt_alone_closes() {
    let (mut m, _) = Model::new();
    assert!(!m.close_slot());
    m.split_slot(Axis::Row);
    m.split_slot(Axis::Column);
    assert!(m.close_slot());
    assert_eq!(shown(&m).len(), 2);
    assert!(m.close_slot());
    assert_eq!(shown(&m).len(), 1);
    assert!(m.term().is_some());
}

#[test]
fn new_spaces_take_the_first_free_color() {
    let (mut m, a) = Model::new();
    let b = m.add_space_behind();
    let c = m.add_space_behind();
    m.set_color(space_of(&m, b), 5);
    let d = m.add_space_behind();
    let color = |m: &Model, t| m.space_of(Tab::Term(t)).unwrap().color;
    assert_eq!([a, b, c, d].map(|t| color(&m, t)), [0, 5, 2, 1]);
}

#[test]
fn moving_a_space_in_the_list_puts_it_before_the_target() {
    let (mut m, a) = Model::new();
    let b = m.add_space_behind();
    let c = m.add_space_behind();
    m.move_space(space_of(&m, c), 0);
    assert_eq!(m.terms(), [c, a, b]);
    m.move_space(space_of(&m, c), 3);
    assert_eq!(m.terms(), [a, b, c]);
}

#[test]
fn dropping_a_shown_space_into_another_slot_swaps_them() {
    let (mut m, a) = Model::new();
    let s1 = space_of(&m, a);
    m.split_slot(Axis::Row);
    let b = m.add_space();
    let s2 = space_of(&m, b);
    let left = m.layout().slot_of(s1).unwrap();
    m.drop_in_window(s2, WindowDrop::Slot(left, None));
    assert_eq!(shown(&m), [Some(s2), Some(s1)]);
    assert_eq!(m.term(), Some(b));
}

#[test]
fn dropping_beside_a_slot_closes_the_one_left_behind() {
    let (mut m, a) = Model::new();
    let s1 = space_of(&m, a);
    m.split_slot(Axis::Row);
    let b = m.add_space();
    let s2 = space_of(&m, b);
    let left = m.layout().slot_of(s1).unwrap();
    m.drop_in_window(s2, WindowDrop::Slot(left, Some(Side::Top)));
    assert_eq!(shown(&m), [Some(s2), Some(s1)]);
    assert!(matches!(
        m.layout().slots,
        Tree::Branch {
            axis: Axis::Column,
            ..
        }
    ));
    m.drop_in_window(s2, WindowDrop::Edge(Side::Right));
    assert_eq!(shown(&m), [Some(s1), Some(s2)]);
    let c = m.add_space_behind();
    m.drop_in_window(space_of(&m, c), WindowDrop::Edge(Side::Left));
    assert_eq!(shown(&m).len(), 3);
    assert_eq!(m.term(), Some(c));
    assert!(m.is_valid());
}

#[test]
fn dropping_the_only_shown_space_on_itself_or_an_edge_does_nothing() {
    let (mut m, a) = Model::new();
    let s1 = space_of(&m, a);
    let slot = m.layout().focus;
    m.drop_in_window(s1, WindowDrop::Slot(slot, None));
    m.drop_in_window(s1, WindowDrop::Edge(Side::Left));
    m.drop_in_window(s1, WindowDrop::Slot(slot, Some(Side::Left)));
    assert_eq!(shown(&m), [Some(s1)]);
}

#[test]
fn closing_a_tab_keeps_the_active_tab_in_range() {
    let (mut m, a) = Model::new();
    let b = m.new_tab();
    let c = m.new_tab();
    assert_eq!(m.split().unwrap().tabs, tabs(&[a, b, c]));
    assert!(m.remove_tab(c.into()));
    assert_eq!(m.term(), Some(b));
    m.select_tab(0);
    assert!(m.remove_tab(b.into()));
    assert_eq!(m.term(), Some(a));
}

#[test]
fn models_survive_a_save_and_load() {
    let (mut m, _) = Model::new();
    m.split_split(Axis::Row);
    m.split_slot(Axis::Column);
    m.new_layout();
    let json = serde_json::to_string(&m).unwrap();
    let back: Model = serde_json::from_str(&json).unwrap();
    assert!(back.is_valid());
    assert_eq!(back.terms(), m.terms());
    assert_eq!(back.active, m.active);
    let broken = json.replace("\"active\":1", "\"active\":7");
    let broken: Model = serde_json::from_str(&broken).unwrap();
    assert!(!broken.is_valid());
}

#[test]
fn degenerate_trees_duplicate_ids_and_twice_shown_spaces_are_invalid() {
    let (mut m, a) = Model::new();
    m.split_split(Axis::Row);
    assert!(m.is_valid());

    let mut lone = m.clone();
    let space = &mut lone.spaces[0];
    let Tree::Branch { children, .. } = &mut space.splits else {
        panic!("split space");
    };
    children.pop();
    space.focus = children[0].tree.first().id;
    assert!(!lone.is_valid());

    let mut twin = m.clone();
    let Tree::Branch { children, .. } = &mut twin.spaces[0].splits else {
        panic!("split space");
    };
    let Tree::Leaf(first) = &children[0].tree else {
        panic!("leaf");
    };
    let id = first.id;
    let Tree::Leaf(second) = &mut children[1].tree else {
        panic!("leaf");
    };
    second.id = id;
    assert!(!twin.is_valid());

    let mut twice = m.clone();
    twice.split_slot(Axis::Row);
    let s1 = space_of(&twice, a);
    let empty = twice.layout().focus;
    twice.layouts[0].slots.find_mut(empty).unwrap().space = Some(s1);
    assert!(!twice.is_valid());
}

#[test]
fn a_tab_moves_into_a_split_of_its_own() {
    let (mut m, a) = Model::new();
    let b = m.new_tab();
    m.move_tab_to_new_split();
    assert_eq!(m.term(), Some(b));
    assert_eq!(m.split().unwrap().tabs, tabs(&[b]));
    let splits = m.space().unwrap().splits.leaves();
    assert_eq!(splits.len(), 2);
    assert_eq!(splits[0].tabs, tabs(&[a]));
    m.move_tab_to_new_split();
    assert_eq!(m.space().unwrap().splits.leaves().len(), 2);
}

#[test]
fn new_tabs_open_after_the_active_one() {
    let (mut m, a) = Model::new();
    let b = m.new_tab();
    m.select_tab(0);
    let c = m.new_tab();
    assert_eq!(m.split().unwrap().tabs, tabs(&[a, c, b]));
    m.cycle_tab(-2);
    assert_eq!(m.term(), Some(b));
}

#[test]
fn moving_focus_crosses_slots_and_reaches_empty_ones() {
    let (mut m, a) = Model::new();
    let b = m.split_slot_with_space(Axis::Row);
    m.split_slot(Axis::Column);
    let empty = m.layout().focus;
    m.focus_dir(Dir::Left);
    assert_eq!(m.term(), Some(a));
    m.cycle_split(1);
    assert_eq!(m.term(), Some(b));
    m.focus_dir(Dir::Down);
    assert_eq!((m.tab(), m.layout().focus), (None, empty));
}

#[test]
fn zoom_ends_when_focus_leaves_the_split_or_it_splits() {
    let (mut m, a) = Model::new();
    let b = m.split_split(Axis::Row);
    m.toggle_zoom();
    m.focus_dir(Dir::Left);
    assert_eq!(m.term(), Some(b));
    m.focus_term(a);
    assert_eq!(m.layout().zoom, None);
    m.toggle_zoom();
    m.split_split(Axis::Row);
    assert_eq!(m.layout().zoom, None);
}

#[test]
fn opening_at_a_terminal_leaves_focus_and_zoom_alone() {
    let (mut m, a) = Model::new();
    let b = m.split_split(Axis::Row);
    m.toggle_zoom();
    let zoom = m.layout().zoom;
    let tab = m.new_tab_at(a.into()).unwrap();
    let split = m.split_split_at(a.into(), Axis::Column).unwrap();
    let space = m.split_slot_at(a.into(), Axis::Row).unwrap();
    assert_eq!((m.term(), m.layout().zoom), (Some(b), zoom));
    assert_eq!(
        m.locate(tab.into()).unwrap().split,
        m.locate(a.into()).unwrap().split
    );
    assert_eq!(m.split().unwrap().tabs, tabs(&[b]));
    assert!(m.locate(split.into()).is_some());
    assert_eq!(shown(&m).len(), 2);
    assert!(m.layout().slot_of(space_of(&m, space)).is_some());
    assert!(m.is_valid());
}

#[test]
fn layout_names_stay_put_when_another_is_deleted() {
    let (mut m, _) = Model::new();
    m.new_layout();
    m.new_layout();
    let names =
        |m: &Model| -> Vec<String> { m.layouts.iter().filter_map(|l| l.name.clone()).collect() };
    assert_eq!(names(&m), ["Layout 1", "Layout 2", "Layout 3"]);
    m.delete_layout(m.layouts[0].id);
    assert_eq!(names(&m), ["Layout 2", "Layout 3"]);
    m.new_layout();
    assert_eq!(names(&m), ["Layout 2", "Layout 3", "Layout 1"]);
}
