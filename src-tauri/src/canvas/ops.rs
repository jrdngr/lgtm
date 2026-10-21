//! An agent's changes to a canvas, applied as one pass the user can undo.

use anyhow::{Context, bail};
use rmcp::schemars::{self, JsonSchema};
use serde::Deserialize;

use super::model::*;

/// An agent's change to a view's canvas.
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum CanvasOp {
    /// Puts a part's, shape's or note's top left at (x, y). Design parts are about 210 by 150
    /// (containers) or 140 by 48 (components); leave 40 or more between things.
    Move {
        #[serde(alias = "part")]
        target: String,
        x: i32,
        y: i32,
    },
    /// Colors a part (in every view of its design), shape, group, note or arrow; null clears it.
    Color {
        target: String,
        color: Option<Swatch>,
    },
    /// Adds a shape, or with an existing id changes the fields given.
    Shape {
        /// `s:` and letters, digits, `-` and `_`. Made up when left out.
        id: Option<String>,
        kind: Option<ShapeKind>,
        x: Option<i32>,
        y: Option<i32>,
        /// 160 by 64 when new and left out.
        w: Option<i32>,
        h: Option<i32>,
        /// Drawn centered inside it.
        text: Option<String>,
        color: Option<Swatch>,
        /// Tinted inside (the default), or an outline only.
        fill: Option<bool>,
        pattern: Option<LinePattern>,
    },
    /// Makes a group, or with an existing id replaces its label, color and members. Members
    /// are part, shape and other groups' ids; each leaves any group it was in.
    Group {
        /// Letters, digits, `-` and `_`. Made up when left out.
        id: Option<String>,
        label: String,
        color: Option<Swatch>,
        members: Vec<String>,
    },
    /// Removes a group, keeping its members, which join its parent group if it had one.
    Ungroup { id: String },
    /// Styles the edge between two design parts, which then shows at rest with its label.
    StyleEdge {
        from: String,
        to: String,
        #[serde(default)]
        pattern: LinePattern,
        color: Option<Swatch>,
        #[serde(default)]
        weight: LineWeight,
        #[serde(default)]
        route: Route,
    },
    /// Drops an edge's style.
    ClearEdge { from: String, to: String },
    /// Adds text, or with an existing id changes it. Sticky text sits on a note card.
    Note {
        id: Option<String>,
        text: String,
        x: i32,
        y: i32,
        #[serde(default)]
        size: NoteSize,
        color: Option<Swatch>,
        #[serde(default)]
        sticky: bool,
    },
    /// Adds an arrow, or with an existing id changes it. An end with a target sticks to that
    /// part, shape, group or note. Without a head it's a plain line.
    Arrow {
        id: Option<String>,
        from: ArrowEnd,
        to: ArrowEnd,
        color: Option<Swatch>,
        #[serde(default)]
        pattern: LinePattern,
        head: Option<bool>,
    },
    /// Removes a shape, note, arrow or group (keeping its members).
    Remove { id: String },
    /// Drops every design part's position, so the view lays itself out again.
    ResetLayout,
}

fn check_id(id: &str) -> anyhow::Result<()> {
    let chars = id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if id.is_empty() || !chars || is_part(id) {
        bail!("`{id}` isn't an id: use letters, digits, `-` and `_`");
    }
    Ok(())
}

/// `id` as a shape id, which starts with `s:`.
fn shape_id(id: &str) -> anyhow::Result<String> {
    let bare = id.strip_prefix("s:").unwrap_or(id);
    check_id(bare)?;
    Ok(format!("s:{bare}"))
}

impl ViewCanvas {
    fn taken(&self, id: &str) -> bool {
        self.groups.iter().any(|g| g.id == id)
            || self.notes.iter().any(|n| n.id == id)
            || self.arrows.iter().any(|a| a.id == id)
            || self.shapes.iter().any(|s| s.id == id)
    }

    fn fresh_id(&self, prefix: &str) -> String {
        (1..)
            .map(|n| format!("{prefix}{n}"))
            .find(|id| !self.taken(id))
            .unwrap()
    }

    fn parent_of(&self, member: &str) -> Option<usize> {
        self.groups
            .iter()
            .position(|g| g.members.iter().any(|m| m == member))
    }

    /// Whether group `id` is `other` or holds it, at any depth.
    fn holds(&self, id: &str, other: &str) -> bool {
        id == other
            || self.groups.iter().find(|g| g.id == id).is_some_and(|g| {
                g.members
                    .iter()
                    .any(|m| !is_part(m) && self.holds(m, other))
            })
    }

    /// Removes group `id`; its members join its parent, if it had one.
    pub fn ungroup(&mut self, id: &str) -> bool {
        let Some(at) = self.groups.iter().position(|g| g.id == id) else {
            return false;
        };
        let g = self.groups.remove(at);
        if let Some(p) = self.parent_of(id) {
            let members = &mut self.groups[p].members;
            let i = members.iter().position(|m| m == id).unwrap();
            members.splice(i..=i, g.members);
        }
        true
    }

    /// Makes or replaces group `id` around `members`, taking each from the group it was in.
    pub fn set_group(&mut self, mut g: CanvasGroup) -> anyhow::Result<()> {
        for m in &g.members {
            if !is_part(m) && self.holds(m, &g.id) {
                bail!("group `{}` can't hold `{m}`, which holds it", g.id);
            }
            if !is_part(m) && !self.groups.iter().any(|x| &x.id == m) {
                bail!("no group `{m}`");
            }
        }
        g.members.dedup();
        for x in &mut self.groups {
            if x.id != g.id {
                x.members.retain(|m| !g.members.contains(m));
            }
        }
        match self.groups.iter_mut().find(|x| x.id == g.id) {
            Some(x) => *x = g,
            None => self.groups.push(g),
        }
        self.groups.retain(|x| !x.members.is_empty());
        Ok(())
    }

    /// Removes a shape, note, arrow or group, and any tag on it.
    fn remove(&mut self, id: &str) -> bool {
        let before = self.shapes.len() + self.notes.len() + self.arrows.len();
        self.shapes.retain(|s| s.id != id);
        self.notes.retain(|n| n.id != id);
        self.arrows.retain(|a| a.id != id);
        let gone = before != self.shapes.len() + self.notes.len() + self.arrows.len();
        if gone {
            for g in &mut self.groups {
                g.members.retain(|m| m != id);
            }
            self.groups.retain(|g| !g.members.is_empty());
        }
        let gone = gone || self.ungroup(id);
        if gone {
            self.tags.retain(|_, on| on != id);
        }
        gone
    }

    fn set_shape(&mut self, op: ShapeOp, by: CanvasAuthor) -> anyhow::Result<String> {
        let id = match op.id {
            Some(id) => shape_id(&id)?,
            None => self.fresh_id("s:"),
        };
        let s = match self.shapes.iter_mut().find(|s| s.id == id) {
            Some(s) => s,
            None => {
                self.shapes.push(CanvasShape {
                    id: id.clone(),
                    kind: ShapeKind::Rect,
                    at: Pt::default(),
                    w: 160,
                    h: 64,
                    text: String::new(),
                    color: None,
                    fill: true,
                    pattern: LinePattern::Solid,
                    by,
                });
                self.shapes.last_mut().unwrap()
            }
        };
        if let Some(k) = op.kind {
            s.kind = k;
        }
        if let Some(x) = op.x {
            s.at.x = x;
        }
        if let Some(y) = op.y {
            s.at.y = y;
        }
        if let Some(w) = op.w {
            s.w = w.max(20);
        }
        if let Some(h) = op.h {
            s.h = h.max(20);
        }
        if let Some(t) = op.text {
            s.text = t;
        }
        if op.color.is_some() {
            s.color = op.color;
        }
        if let Some(f) = op.fill {
            s.fill = f;
        }
        if let Some(p) = op.pattern {
            s.pattern = p;
        }
        s.by = by;
        Ok(id)
    }
}

/// A shape op's fields.
struct ShapeOp {
    id: Option<String>,
    kind: Option<ShapeKind>,
    x: Option<i32>,
    y: Option<i32>,
    w: Option<i32>,
    h: Option<i32>,
    text: Option<String>,
    color: Option<Swatch>,
    fill: Option<bool>,
    pattern: Option<LinePattern>,
}

impl Canvas {
    /// Applies an agent's `ops` to view `view`, recording them as the last agent pass.
    pub fn apply(&mut self, view: &str, ops: Vec<CanvasOp>) -> anyhow::Result<()> {
        let mut touched = Vec::new();
        let changed = ops.len() as u32;
        let v = self.views.entry(view.to_string()).or_default();
        let by = CanvasAuthor::Agent;
        for (n, op) in ops.into_iter().enumerate() {
            let at = || format!("op {}", n + 1);
            match op {
                CanvasOp::Move { target, x, y } => {
                    let to = Pt { x, y };
                    if let Some(s) = v.shapes.iter_mut().find(|s| s.id == target) {
                        s.at = to;
                    } else if let Some(n) = v.notes.iter_mut().find(|n| n.id == target) {
                        n.at = to;
                    } else if is_part(&target) {
                        v.positions.insert(target.clone(), to);
                    } else {
                        bail!("{}: no part, shape or note `{target}`", at());
                    }
                    touched.push(target);
                }
                CanvasOp::Color { target, color } => {
                    if let Some(s) = v.shapes.iter_mut().find(|s| s.id == target) {
                        s.color = color;
                        s.by = by;
                    } else if let Some(g) = v.groups.iter_mut().find(|g| g.id == target) {
                        g.color = color;
                        g.by = by;
                    } else if let Some(n) = v.notes.iter_mut().find(|n| n.id == target) {
                        n.color = color;
                        n.by = by;
                    } else if let Some(a) = v.arrows.iter_mut().find(|a| a.id == target) {
                        a.color = color;
                        a.by = by;
                    } else if is_part(&target) {
                        match color {
                            Some(c) => self.colors.insert(target.clone(), c),
                            None => self.colors.remove(&target),
                        };
                    } else {
                        bail!("{}: nothing has id `{target}`", at());
                    }
                    touched.push(target);
                }
                CanvasOp::Shape {
                    id,
                    kind,
                    x,
                    y,
                    w,
                    h,
                    text,
                    color,
                    fill,
                    pattern,
                } => {
                    let op = ShapeOp {
                        id,
                        kind,
                        x,
                        y,
                        w,
                        h,
                        text,
                        color,
                        fill,
                        pattern,
                    };
                    touched.push(v.set_shape(op, by).with_context(at)?);
                }
                CanvasOp::Group {
                    id,
                    label,
                    color,
                    members,
                } => {
                    let id = match id {
                        Some(id) => id,
                        None => v.fresh_id("g"),
                    };
                    check_id(&id).with_context(at)?;
                    v.set_group(CanvasGroup {
                        id: id.clone(),
                        label,
                        color,
                        members,
                        by,
                    })
                    .with_context(at)?;
                    touched.push(id);
                }
                CanvasOp::Ungroup { id } => {
                    if !v.ungroup(&id) {
                        bail!("{}: no group `{id}`", at());
                    }
                }
                CanvasOp::StyleEdge {
                    from,
                    to,
                    pattern,
                    color,
                    weight,
                    route,
                } => {
                    let key = format!("{from}>{to}");
                    let bend = v.edges.get(&key).and_then(|e| e.bend);
                    v.edges.insert(
                        key.clone(),
                        EdgeStyle {
                            pattern,
                            color,
                            weight,
                            route,
                            bend,
                            by,
                        },
                    );
                    touched.push(key);
                }
                CanvasOp::ClearEdge { from, to } => {
                    v.edges.remove(&format!("{from}>{to}"));
                }
                CanvasOp::Note {
                    id,
                    text,
                    x,
                    y,
                    size,
                    color,
                    sticky,
                } => {
                    let id = match id {
                        Some(id) => id,
                        None => v.fresh_id("n"),
                    };
                    check_id(&id).with_context(at)?;
                    let note = CanvasNote {
                        id: id.clone(),
                        text,
                        at: Pt { x, y },
                        size,
                        color,
                        sticky,
                        by,
                    };
                    match v.notes.iter_mut().find(|n| n.id == id) {
                        Some(n) => *n = note,
                        None => v.notes.push(note),
                    }
                    touched.push(id);
                }
                CanvasOp::Arrow {
                    id,
                    from,
                    to,
                    color,
                    pattern,
                    head,
                } => {
                    let id = match id {
                        Some(id) => id,
                        None => v.fresh_id("a"),
                    };
                    check_id(&id).with_context(at)?;
                    let arrow = CanvasArrow {
                        id: id.clone(),
                        from,
                        to,
                        color,
                        pattern,
                        head: head.unwrap_or(true),
                        by,
                    };
                    match v.arrows.iter_mut().find(|a| a.id == id) {
                        Some(a) => *a = arrow,
                        None => v.arrows.push(arrow),
                    }
                    touched.push(id);
                }
                CanvasOp::Remove { id } => {
                    if !v.remove(&id) {
                        bail!("{}: no shape, note, arrow or group `{id}`", at());
                    }
                }
                CanvasOp::ResetLayout => {
                    v.positions.clear();
                    v.frozen = false;
                }
            }
        }
        touched.sort();
        touched.dedup();
        self.agent = Some(AgentPass {
            changed,
            touched,
            view: view.to_string(),
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
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
        v.groups.iter().find(|g| g.id == id).unwrap().members.clone()
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

        let bad = ops(serde_json::json!([{"op": "note", "id": "has space", "text": "", "x": 0, "y": 0}]));
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
        c.apply("main", ops(serde_json::json!([{"op": "remove", "id": "s:a"}])))
            .unwrap();
        let v = &c.views["main"];
        assert_eq!(members(v, "pair"), ["s:b"]);
        assert_eq!(v.tags.keys().collect::<Vec<_>>(), ["4"]);
    }
}
