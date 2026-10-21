//! A design's canvas: what the user and the agent draw over its map. Each view (a level, and
//! for components and interfaces the container or component it shows) has its own positions,
//! groups, edge styles, notes and arrows; a part's color follows it into every view.

use std::collections::BTreeMap;

use anyhow::{Context, bail};
use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Swatch {
    Red,
    Orange,
    Yellow,
    Green,
    Cyan,
    Blue,
    Purple,
    Pink,
}

/// Who last made or changed an element.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum CanvasAuthor {
    #[default]
    User,
    Agent,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type, JsonSchema)]
pub struct Pt {
    pub x: i32,
    pub y: i32,
}

/// A labeled frame around parts and other groups.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CanvasGroup {
    pub id: String,
    pub label: String,
    pub color: Option<Swatch>,
    /// Part ids, which have a `:` (or are `sys`), and group ids, which don't.
    pub members: Vec<String>,
    #[serde(default)]
    pub by: CanvasAuthor,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum LinePattern {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum LineWeight {
    #[default]
    Thin,
    Thick,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Route {
    #[default]
    Curved,
    Elbow,
}

/// How a styled edge draws. A styled edge shows at rest, with its label.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EdgeStyle {
    pub pattern: LinePattern,
    pub color: Option<Swatch>,
    pub weight: LineWeight,
    pub route: Route,
    /// Where its middle is dragged to, from halfway between its ends' centers.
    pub bend: Option<Pt>,
    #[serde(default)]
    pub by: CanvasAuthor,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum NoteSize {
    S,
    #[default]
    M,
    L,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CanvasNote {
    pub id: String,
    pub text: String,
    /// Its top left.
    pub at: Pt,
    pub size: NoteSize,
    pub color: Option<Swatch>,
    #[serde(default)]
    pub by: CanvasAuthor,
}

/// An arrow's end: stuck to a part, group or note, or free at `at`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArrowEnd {
    /// A part, group or note id; none when the end is free.
    pub target: Option<String>,
    /// Where a free end is, and where an attached one was last drawn.
    #[serde(default)]
    pub at: Pt,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CanvasArrow {
    pub id: String,
    pub from: ArrowEnd,
    pub to: ArrowEnd,
    pub color: Option<Swatch>,
    pub pattern: LinePattern,
    #[serde(default)]
    pub by: CanvasAuthor,
}

/// One view's drawing.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct ViewCanvas {
    /// Set once the user moves something: every part keeps a position, and a part without
    /// one is new since.
    pub frozen: bool,
    /// Parts' top lefts, by part id.
    pub positions: BTreeMap<String, Pt>,
    pub groups: Vec<CanvasGroup>,
    /// By `from>to` part ids.
    pub edges: BTreeMap<String, EdgeStyle>,
    pub notes: Vec<CanvasNote>,
    pub arrows: Vec<CanvasArrow>,
}

/// The last agent pass: how many changes it made and what it touched.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentPass {
    pub changed: u32,
    /// Part, group, note and arrow ids, and edges as `from>to`.
    pub touched: Vec<String>,
    pub view: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct DesignCanvas {
    /// By view: `context`, `containers`, `components/<container id>` or
    /// `interfaces/<component id>`.
    pub views: BTreeMap<String, ViewCanvas>,
    /// Part colors, by part id, in every view.
    pub colors: BTreeMap<String, Swatch>,
    pub agent: Option<AgentPass>,
}

/// An agent's change to a view's canvas.
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum CanvasOp {
    /// Puts a part's top left at (x, y). Parts are about 210 by 150 (containers) or 140 by 48
    /// (components); leave 40 or more between them.
    Move { part: String, x: i32, y: i32 },
    /// Colors a part in every view, or a group; null clears it.
    Color {
        target: String,
        color: Option<Swatch>,
    },
    /// Makes a group, or with an existing id replaces its label, color and members. Members
    /// are part ids and other groups' ids; each leaves any group it was in.
    Group {
        /// Letters, digits, `-` and `_`. Made up when left out.
        id: Option<String>,
        label: String,
        color: Option<Swatch>,
        members: Vec<String>,
    },
    /// Removes a group, keeping its members, which join its parent group if it had one.
    Ungroup { id: String },
    /// Styles the edge between two parts, which then shows at rest with its label.
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
    /// Adds a note, or with an existing id changes it.
    Note {
        id: Option<String>,
        text: String,
        x: i32,
        y: i32,
        #[serde(default)]
        size: NoteSize,
        color: Option<Swatch>,
    },
    /// Adds an arrow, or with an existing id changes it. An end with a target sticks to that
    /// part, group or note.
    Arrow {
        id: Option<String>,
        from: ArrowEnd,
        to: ArrowEnd,
        color: Option<Swatch>,
        #[serde(default)]
        pattern: LinePattern,
    },
    /// Removes a note, arrow or group (keeping its members).
    Remove { id: String },
    /// Drops every position, so the view lays itself out again.
    ResetLayout,
}

/// A part id has a `:`, as `c:api` or `k:api/auth`, or is `sys`, the system.
pub fn is_part(id: &str) -> bool {
    id.contains(':') || id == "sys"
}

fn check_id(id: &str) -> anyhow::Result<()> {
    let chars = id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if id.is_empty() || !chars || is_part(id) {
        bail!("`{id}` isn't an id: use letters, digits, `-` and `_`");
    }
    Ok(())
}

impl ViewCanvas {
    fn fresh_id(&self, prefix: &str) -> String {
        let taken = |id: &str| {
            self.groups.iter().any(|g| g.id == id)
                || self.notes.iter().any(|n| n.id == id)
                || self.arrows.iter().any(|a| a.id == id)
        };
        (1..)
            .map(|n| format!("{prefix}{n}"))
            .find(|id| !taken(id))
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

    /// Removes a note, an arrow or a group.
    fn remove(&mut self, id: &str) -> bool {
        let (notes, arrows) = (self.notes.len(), self.arrows.len());
        self.notes.retain(|n| n.id != id);
        self.arrows.retain(|a| a.id != id);
        notes != self.notes.len() || arrows != self.arrows.len() || self.ungroup(id)
    }
}

impl DesignCanvas {
    /// Applies an agent's `ops` to view `view`, recording them as the last agent pass.
    pub fn apply(&mut self, view: &str, ops: Vec<CanvasOp>) -> anyhow::Result<()> {
        check_view(view)?;
        let mut touched = Vec::new();
        let changed = ops.len() as u32;
        let v = self.views.entry(view.to_string()).or_default();
        let by = CanvasAuthor::Agent;
        for (n, op) in ops.into_iter().enumerate() {
            let at = || format!("op {}", n + 1);
            match op {
                CanvasOp::Move { part, x, y } => {
                    v.positions.insert(part.clone(), Pt { x, y });
                    touched.push(part);
                }
                CanvasOp::Color { target, color } => {
                    if is_part(&target) {
                        match color {
                            Some(c) => self.colors.insert(target.clone(), c),
                            None => self.colors.remove(&target),
                        };
                    } else {
                        let g = v
                            .groups
                            .iter_mut()
                            .find(|g| g.id == target)
                            .with_context(|| format!("{}: no group `{target}`", at()))?;
                        g.color = color;
                        g.by = by;
                    }
                    touched.push(target);
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
                        bail!("{}: no note, arrow or group `{id}`", at());
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

fn check_view(view: &str) -> anyhow::Result<()> {
    let ok = matches!(view, "context" | "containers")
        || view
            .strip_prefix("components/")
            .or_else(|| view.strip_prefix("interfaces/"))
            .is_some_and(|rest| !rest.is_empty());
    if !ok {
        bail!(
            "no view `{view}`: use context, containers, components/<container id> or \
             interfaces/<component id>"
        );
    }
    Ok(())
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
    fn an_agent_pass_records_what_it_touched_and_rejects_bad_views_and_ids() {
        let mut c = DesignCanvas::default();
        let ops: Vec<CanvasOp> = serde_json::from_value(serde_json::json!([
            {"op": "move", "part": "c:api", "x": 10, "y": 20},
            {"op": "group", "label": "Core", "members": ["c:api", "c:web"]},
            {"op": "color", "target": "c:api", "color": "orange"},
            {"op": "note", "text": "Phase 1", "x": 0, "y": 0},
            {"op": "arrow", "from": {"target": "n1"}, "to": {"target": "g1"}},
            {"op": "style_edge", "from": "c:web", "to": "c:api", "color": "blue"},
        ]))
        .unwrap();
        c.apply("containers", ops).unwrap();
        let v = &c.views["containers"];
        assert_eq!(v.positions["c:api"], Pt { x: 10, y: 20 });
        assert_eq!(c.colors["c:api"], Swatch::Orange);
        assert_eq!(v.groups[0].by, CanvasAuthor::Agent);
        assert_eq!(v.arrows[0].to.target.as_deref(), Some("g1"));
        let pass = c.agent.as_ref().unwrap();
        assert_eq!(pass.touched, ["a1", "c:api", "c:web>c:api", "g1", "n1"]);

        assert!(c.apply("components", vec![]).is_err());
        let bad = vec![CanvasOp::Note {
            id: Some("has space".into()),
            text: String::new(),
            x: 0,
            y: 0,
            size: NoteSize::S,
            color: None,
        }];
        assert!(c.apply("context", bad).is_err());
    }
}
