//! Dropping a space in the window: into a slot, beside one, or along an edge of the layout.

use super::{Axis, Model, SlotId, SpaceId};

/// A side of a slot or of the whole layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

impl Side {
    fn axis(self) -> Axis {
        match self {
            Side::Left | Side::Right => Axis::Row,
            Side::Top | Side::Bottom => Axis::Column,
        }
    }

    fn before(self) -> bool {
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
