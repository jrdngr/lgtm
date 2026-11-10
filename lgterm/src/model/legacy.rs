//! Reading states saved before spaces left layouts: each old layout held a tree of spaces. Its
//! spaces join the list in order, and it becomes a layout of slots in the same places.

use serde::Deserialize;

use super::tree::Node;
use super::{COLORS, Layout, LayoutId, Model, Slot, SlotId, Space, SpaceId, Tree};

#[derive(Deserialize)]
pub struct OldModel {
    layouts: Vec<OldLayout>,
    active: usize,
    next: u64,
}

#[derive(Deserialize)]
struct OldLayout {
    id: LayoutId,
    name: Option<String>,
    spaces: Tree<Space>,
    focus: SpaceId,
}

impl From<OldModel> for Model {
    fn from(old: OldModel) -> Model {
        let mut next = old.next;
        let mut spaces = Vec::new();
        let layouts = old
            .layouts
            .into_iter()
            .map(|l| {
                let mut focus = None;
                let slots = map(l.spaces, &mut |mut space| {
                    let id = SlotId(next);
                    next += 1;
                    if space.id == l.focus {
                        focus = Some(id);
                    }
                    space.color = spaces.len() % COLORS;
                    let slot = Slot {
                        id,
                        space: Some(space.id),
                    };
                    spaces.push(space);
                    slot
                });
                Layout {
                    id: l.id,
                    name: l.name,
                    focus: focus.unwrap_or_else(|| slots.first().id),
                    slots,
                    expanded: None,
                }
            })
            .collect();
        Model {
            spaces,
            layouts,
            active: old.active,
            next,
        }
    }
}

fn map<A, B>(tree: Tree<A>, f: &mut impl FnMut(A) -> B) -> Tree<B> {
    match tree {
        Tree::Leaf(a) => Tree::Leaf(f(a)),
        Tree::Branch { axis, children } => Tree::Branch {
            axis,
            children: children
                .into_iter()
                .map(|c| Node {
                    weight: c.weight,
                    tree: map(c.tree, f),
                })
                .collect(),
        },
    }
}
