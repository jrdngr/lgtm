//! Dropping a space in the window: into a slot, beside one, or along an edge of the layout.
//! Dropping a tab on a split: into its tabs, or beside it in a new split.

use super::{Axis, Model, SlotId, SpaceId, Split, SplitId, Tab};

/// A side of a slot or of the whole layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

impl Side {
    pub fn axis(self) -> Axis {
        match self {
            Side::Left | Side::Right => Axis::Row,
            Side::Top | Side::Bottom => Axis::Column,
        }
    }

    pub fn before(self) -> bool {
        matches!(self, Side::Left | Side::Top)
    }
}

/// Where a space dropped in the window goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowDrop {
    /// Into a slot, or beside it in a new slot.
    Slot(SlotId, Option<Side>),
    /// Along an edge of the whole layout, in a new slot.
    Edge(Side),
}

impl Model {
    /// Puts a space where it was dropped and focuses it. Dropped into a slot, it swaps with
    /// what's there when it was already shown elsewhere. Dropped in a new slot, the slot it
    /// leaves closes.
    pub fn drop_in_window(&mut self, id: SpaceId, at: WindowDrop) {
        let from = self.layout().slot_of(id);
        match at {
            WindowDrop::Slot(target, None) => {
                if from == Some(target) {
                    return;
                }
                let layout = self.layout_mut();
                let Some(old) = layout.slots.find(target).map(|s| s.space) else {
                    return;
                };
                if let Some(from) = from {
                    layout.slots.find_mut(from).expect("slot").space = old;
                    layout.slots.find_mut(target).expect("slot").space = None;
                }
                self.assign(target, id);
            }
            WindowDrop::Slot(target, Some(side)) => {
                if from == Some(target) || !self.layout().slots.contains(target) {
                    return;
                }
                self.leave(from);
                let slot = self.new_slot_for(id);
                let layout = self.layout_mut();
                let _ = layout
                    .slots
                    .insert_beside(target, slot, side.axis(), side.before());
                self.focus_new(id);
            }
            WindowDrop::Edge(side) => {
                if from.is_some() && self.layout().slots.leaves().len() == 1 {
                    return;
                }
                self.leave(from);
                let slot = self.new_slot_for(id);
                self.layout_mut()
                    .slots
                    .wrap(slot, side.axis(), side.before());
                self.focus_new(id);
            }
        }
    }

    /// Moves a tab into `to`'s tabs, or into a new split on `side` of it, in any space, and
    /// focuses it. What it leaves empty closes, as when it's closed.
    pub fn move_tab(&mut self, tab: Tab, to: SplitId, side: Option<Side>) {
        let Some(side) = side else {
            return self.move_tab_to(tab, to, usize::MAX);
        };
        let Some(from) = self.locate(tab) else { return };
        let alone = self
            .find_space(from.space)
            .and_then(|s| s.splits.find(from.split))
            .is_some_and(|s| s.tabs.len() == 1);
        if from.split == to && alone {
            return;
        }
        if !self.spaces.iter().any(|s| s.splits.contains(to)) || !self.remove_tab(tab) {
            return;
        }
        let id = SplitId(self.id());
        let Some(space) = self.spaces.iter_mut().find(|s| s.splits.contains(to)) else {
            return;
        };
        space.zoom = None;
        let split = Split {
            id,
            tabs: vec![tab],
            active: 0,
            stacked: false,
            stacked_width: None,
        };
        let _ = space
            .splits
            .insert_beside(to, split, side.axis(), side.before());
        self.focus_tab(tab);
    }

    /// Moves a tab into `to`'s tabs before the one at `at` (after the last when `at` is past
    /// it), in any space, and focuses it. What it leaves empty closes, as when it's closed.
    pub fn move_tab_to(&mut self, tab: Tab, to: SplitId, at: usize) {
        let Some(from) = self.locate(tab) else { return };
        if from.split == to {
            let split = self
                .find_space_mut(from.space)
                .and_then(|s| s.splits.find_mut(to))
                .expect("split");
            let at = at.min(split.tabs.len());
            let at = if at > from.tab { at - 1 } else { at };
            let t = split.tabs.remove(from.tab);
            split.tabs.insert(at, t);
        } else {
            if !self.spaces.iter().any(|s| s.splits.contains(to)) || !self.remove_tab(tab) {
                return;
            }
            let Some(space) = self.spaces.iter_mut().find(|s| s.splits.contains(to)) else {
                return;
            };
            space.zoom = None;
            let split = space.splits.find_mut(to).expect("split");
            split.tabs.insert(at.min(split.tabs.len()), tab);
        }
        self.focus_tab(tab);
    }

    /// Moves a tab into a new split along `side` of the largest part of its space that split
    /// `beside` borders on that side, and focuses it.
    pub fn move_tab_to_edge(&mut self, tab: Tab, beside: SplitId, side: Side) {
        let Some(from) = self.locate(tab) else { return };
        let alone = self
            .find_space(from.space)
            .and_then(|s| s.splits.find(from.split))
            .is_some_and(|s| s.tabs.len() == 1);
        if (from.split == beside && alone)
            || !self.spaces.iter().any(|s| s.splits.contains(beside))
            || !self.remove_tab(tab)
        {
            return;
        }
        let id = SplitId(self.id());
        let Some(space) = self.spaces.iter_mut().find(|s| s.splits.contains(beside)) else {
            return;
        };
        let (axis, before) = (side.axis(), side.before());
        let Some(path) = space.splits.edge_of(beside, axis, before) else {
            return;
        };
        space.zoom = None;
        let split = Split {
            id,
            tabs: vec![tab],
            active: 0,
            stacked: false,
            stacked_width: None,
        };
        space.splits.insert_at_edge(&path, split, axis, before);
        self.focus_tab(tab);
    }

    fn new_slot_for(&mut self, space: SpaceId) -> super::Slot {
        super::Slot {
            id: SlotId(self.id()),
            space: Some(space),
        }
    }

    /// Closes the slot a dragged space leaves.
    fn leave(&mut self, from: Option<SlotId>) {
        let Some(from) = from else { return };
        let layout = self.layout_mut();
        if layout.slots.remove(from).is_none() {
            layout.slots.find_mut(from).expect("slot").space = None;
        }
        if layout.focus == from {
            layout.focus = layout.slots.first().id;
        }
    }

    fn focus_new(&mut self, space: SpaceId) {
        let layout = self.layout_mut();
        if let Some(slot) = layout.slot_of(space) {
            layout.focus = slot;
        }
        layout.expanded = None;
    }
}
