//! Agent canvas ops over hand-built views.

use super::*;

fn group(v: &mut ViewCanvas, id: &str, members: &[&str]) {
    v.set_group(CanvasGroup {
        id: id.into(),
        label: id.into(),
        color: None,
        members: members.iter().map(|m| m.to_string()).collect(),
        by: CanvasAuthor::User,
    })
    .unwrap();
}

fn members(v: &ViewCanvas, id: &str) -> Vec<String> {
    v.groups
        .iter()
        .find(|g| g.id == id)
        .unwrap()
        .members
        .clone()
}

fn ops(json: serde_json::Value) -> Vec<CanvasOp> {
    serde_json::from_value(json).unwrap()
}

#[test]
fn grouping_takes_members_from_their_old_group_and_drops_emptied_groups() {
    let mut v = ViewCanvas::default();
    group(&mut v, "core", &["c:a", "c:b"]);
    group(&mut v, "store", &["c:db"]);
    group(&mut v, "all", &["core", "c:db"]);
    assert_eq!(members(&v, "all"), ["core", "c:db"]);
    assert!(
        !v.groups.iter().any(|g| g.id == "store"),
        "store lost its only member"
    );
    assert!(
        v.set_group(CanvasGroup {
            id: "core".into(),
            label: "core".into(),
            color: None,
            members: vec!["all".into()],
            by: CanvasAuthor::User,
        })
        .is_err(),
        "a group can't hold its own parent"
    );
}

#[test]
fn ungrouping_hands_members_to_the_parent_in_place() {
    let mut v = ViewCanvas::default();
    group(&mut v, "inner", &["c:a", "c:b"]);
    group(&mut v, "outer", &["c:x", "inner", "c:y"]);
    assert!(v.ungroup("inner"));
    assert_eq!(members(&v, "outer"), ["c:x", "c:a", "c:b", "c:y"]);
}

#[test]
fn an_agent_pass_records_what_it_touched_and_rejects_bad_ids() {
    let mut c = Canvas::default();
    let pass = ops(serde_json::json!([
        {"op": "move", "part": "c:api", "x": 10, "y": 20},
        {"op": "group", "label": "Core", "members": ["c:api", "c:web"]},
        {"op": "color", "target": "c:api", "color": "orange"},
        {"op": "note", "text": "Phase 1", "x": 0, "y": 0},
        {"op": "arrow", "from": {"target": "n1"}, "to": {"target": "g1"}},
        {"op": "style_edge", "from": "c:web", "to": "c:api", "color": "blue"},
    ]));
    c.apply("containers", pass).unwrap();
    let v = &c.views["containers"];
    assert_eq!(v.positions["c:api"], Pt { x: 10, y: 20 });
    assert_eq!(c.colors["c:api"], Swatch::Orange);
    assert_eq!(v.groups[0].by, CanvasAuthor::Agent);
    assert_eq!(v.arrows[0].to.target.as_deref(), Some("g1"));
    assert!(v.arrows[0].head);
    let pass = c.agent.as_ref().unwrap();
    assert_eq!(pass.touched, ["a1", "c:api", "c:web>c:api", "g1", "n1"]);

    let bad =
        ops(serde_json::json!([{"op": "note", "id": "has space", "text": "", "x": 0, "y": 0}]));
    assert!(c.apply("context", bad).is_err());
    let missing = ops(serde_json::json!([{"op": "color", "target": "nope", "color": null}]));
    assert!(c.apply("context", missing).is_err());
}

#[test]
fn a_shape_op_on_an_existing_shape_changes_only_the_fields_given() {
    let mut c = Canvas::default();
    c.apply(
        "main",
        ops(serde_json::json!([
            {"op": "shape", "id": "db", "kind": "ellipse", "x": 5, "y": 6, "text": "Postgres", "color": "green"},
            {"op": "shape", "x": 300, "y": 0},
        ])),
    )
    .unwrap();
    c.apply(
        "main",
        ops(serde_json::json!([
            {"op": "shape", "id": "s:db", "text": "Primary DB", "w": 200},
            {"op": "move", "target": "s:1", "x": 40, "y": 50},
        ])),
    )
    .unwrap();
    let v = &c.views["main"];
    let db = &v.shapes[0];
    assert_eq!(db.id, "s:db", "a bare id gets the shape prefix");
    assert_eq!(
        (db.kind, db.at, db.w, db.h, db.text.as_str(), db.color),
        (
            ShapeKind::Ellipse,
            Pt { x: 5, y: 6 },
            200,
            64,
            "Primary DB",
            Some(Swatch::Green)
        )
    );
    assert_eq!(v.shapes[1].at, Pt { x: 40, y: 50 });
}

#[test]
fn agent_tags_sit_apart_from_the_users_and_move_rather_than_double_up() {
    let mut c = Canvas::default();
    c.apply(
        "main",
        ops(serde_json::json!([
            {"op": "shape", "id": "a"},
            {"op": "shape", "id": "b"},
            {"op": "tag", "digit": "1", "target": "s:a"},
        ])),
    )
    .unwrap();
    c.views
        .get_mut("main")
        .unwrap()
        .tags
        .insert("1".into(), "s:b".into());
    c.apply(
        "main",
        ops(serde_json::json!([
            {"op": "tag", "digit": "2", "target": "s:a"},
            {"op": "tag", "digit": "3", "target": "s:b"},
            {"op": "tag", "digit": "3"},
        ])),
    )
    .unwrap();
    let v = &c.views["main"];
    assert_eq!(
        v.agent_tags.iter().collect::<Vec<_>>(),
        [(&"2".to_string(), &"s:a".to_string())]
    );
    assert_eq!(v.tags["1"], "s:b");

    for bad in [
        serde_json::json!([{"op": "tag", "digit": "12", "target": "s:a"}]),
        serde_json::json!([{"op": "tag", "digit": "4", "target": "nope"}]),
    ] {
        assert!(c.apply("main", ops(bad)).is_err());
    }
    c.apply(
        "main",
        ops(serde_json::json!([{"op": "remove", "id": "s:a"}])),
    )
    .unwrap();
    assert!(c.views["main"].agent_tags.is_empty());
}

#[test]
fn removing_a_shape_drops_it_from_its_group_and_drops_its_tag() {
    let mut c = Canvas::default();
    c.apply(
        "main",
        ops(serde_json::json!([
            {"op": "shape", "id": "a"},
            {"op": "shape", "id": "b"},
            {"op": "group", "id": "pair", "label": "Pair", "members": ["s:a", "s:b"]},
        ])),
    )
    .unwrap();
    let v = c.views.get_mut("main").unwrap();
    v.tags.insert("3".into(), "s:a".into());
    v.tags.insert("4".into(), "s:b".into());
    c.apply(
        "main",
        ops(serde_json::json!([{"op": "remove", "id": "s:a"}])),
    )
    .unwrap();
    let v = &c.views["main"];
    assert_eq!(members(v, "pair"), ["s:b"]);
    assert_eq!(v.tags.keys().collect::<Vec<_>>(), ["4"]);
}
