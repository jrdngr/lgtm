//! Layouts and their slots: putting spaces in slots, splitting and closing slots, and adding,
//! picking and deleting layouts.

use super::{Axis, COLORS, Layout, LayoutId, Model, Slot, SlotId, SpaceId, TermId, Tree, cycle};

impl Model {
    /// Puts `space` in `slot` of the shown layout and focuses it. A slot already showing it
    /// goes empty, since a layout shows a space at most once.
    pub fn assign(&mut self, slot: SlotId, space: SpaceId) {
        let layout = self.layout_mut();
        if let Some(old) = layout.slot_of(space).filter(|&s| s != slot) {
            layout.slots.find_mut(old).expect("slot").space = None;
        }
        let Some(target) = layout.slots.find_mut(slot) else {
            return;
        };
        target.space = Some(space);
        layout.focus = slot;
        if layout.expanded.is_some_and(|e| e != space) {
            layout.expanded = None;
        }
    }

    /// The shown layout's slots in reading order: left to right along a row, then rows top to
    /// bottom.
    pub fn slots_in_order(&self) -> Vec<SlotId> {
        let mut rects = self.layout().slots.rects(super::Rect::UNIT);
        rects.sort_by(|(_, a), (_, b)| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
        rects.into_iter().map(|(id, _)| id).collect()
    }

    /// Focuses the slot at `index` in reading order.
    pub fn select_slot(&mut self, index: usize) {
        if let Some(&slot) = self.slots_in_order().get(index) {
            self.focus_slot(slot);
        }
    }

    /// Focuses the slot `delta` away from the focused one in reading order, wrapping.
    pub fn cycle_slot(&mut self, delta: isize) {
        let order = self.slots_in_order();
        let at = order
            .iter()
            .position(|&s| s == self.layout().focus)
            .unwrap_or(0);
        self.focus_slot(order[cycle(at, order.len(), delta)]);
    }

    pub fn focus_slot(&mut self, slot: SlotId) {
        let layout = self.layout_mut();
        if layout.slots.contains(slot) {
            layout.focus = slot;
            if layout
                .expanded
                .is_some_and(|e| layout.slot_of(e) != Some(slot))
            {
                layout.expanded = None;
            }
        }
    }

    fn new_slot(&mut self, space: Option<SpaceId>) -> Slot {
        Slot {
            id: SlotId(self.id()),
            space,
        }
    }

    /// Splits the focused slot, focusing the new, empty one.
    pub fn split_slot(&mut self, axis: Axis) {
        let slot = self.new_slot(None);
        let id = slot.id;
        let layout = self.layout_mut();
        let at = layout.focus;
        layout.slots.insert(at, slot, axis).expect("focused slot");
        layout.focus = id;
        layout.expanded = None;
    }

    /// Splits the focused slot and puts a new space in the new one.
    pub fn split_slot_with_space(&mut self, axis: Axis) -> TermId {
        self.split_slot(axis);
        self.add_space()
    }

    /// Puts a new space in a new slot beside `at`'s, leaving focus as it is. When `at`'s space
    /// isn't in the shown layout, the new space isn't shown anywhere.
    pub fn split_slot_at(&mut self, at: super::Tab, axis: Axis) -> Option<TermId> {
        let p = self.locate(at)?;
        let (space, term) = self.make_space();
        let slot = self.new_slot(Some(space));
        if let Some(beside) = self.layout().slot_of(p.space) {
            let layout = self.layout_mut();
            let _ = layout.slots.insert(beside, slot, axis);
        }
        Some(term)
    }

    /// Closes the focused slot when it's empty and not the layout's only one.
    pub fn close_slot(&mut self) -> bool {
        let layout = self.layout_mut();
        if layout.slot().space.is_some() || matches!(layout.slots, Tree::Leaf(_)) {
            return false;
        }
        let focus = layout.focus;
        let next = layout
            .slots
            .rects(super::Rect::UNIT)
            .into_iter()
            .map(|(id, _)| id)
            .take_while(|id| *id != focus)
            .last();
        layout.slots.remove(focus);
        layout.focus = next.unwrap_or_else(|| layout.slots.first().id);
        true
    }

    /// Empties the slot showing `space` in the shown layout.
    pub fn unassign(&mut self, space: SpaceId) {
        let layout = self.layout_mut();
        if let Some(slot) = layout.slot_of(space) {
            layout.slots.find_mut(slot).expect("slot").space = None;
            layout.focus = slot;
            if layout.expanded == Some(space) {
                layout.expanded = None;
            }
        }
    }

    /// Adds a layout of one empty slot after the shown one and shows it.
    pub fn new_layout(&mut self) {
        let slot = self.new_slot(None);
        let layout = Layout {
            id: LayoutId(self.id()),
            name: None,
            focus: slot.id,
            slots: Tree::Leaf(slot),
            expanded: None,
        };
        self.layouts.insert(self.active + 1, layout);
        self.active += 1;
        self.name_layouts();
    }

    /// Names each unnamed layout "Layout N" with the lowest N no layout has, so names stay put
    /// when others are deleted.
    pub fn name_layouts(&mut self) {
        for i in 0..self.layouts.len() {
            if self.layouts[i].name.is_some() {
                continue;
            }
            let n = (1..)
                .find(|n| {
                    let name = format!("Layout {n}");
                    self.layouts
                        .iter()
                        .all(|l| l.name.as_deref() != Some(&name))
                })
                .expect("a free number");
            self.layouts[i].name = Some(format!("Layout {n}"));
        }
    }

    /// Evens out the shown layout's slots: in each branch, the children holding two or more
    /// of `spaces` (every slot, when `None`) share what they had equally. Others keep theirs.
    pub fn distribute(&mut self, spaces: Option<&[SpaceId]>) {
        fn walk(t: &mut Tree<Slot>, pick: &dyn Fn(&Slot) -> bool) {
            let Tree::Branch { children, .. } = t else {
                return;
            };
            let has: Vec<bool> = children
                .iter()
                .map(|c| c.tree.leaves().into_iter().any(pick))
                .collect();
            let n = has.iter().filter(|&&h| h).count();
            if n >= 2 {
                let total: f32 = children
                    .iter()
                    .zip(&has)
                    .filter(|(_, h)| **h)
                    .map(|(c, _)| c.weight)
                    .sum();
                for (c, _) in children.iter_mut().zip(&has).filter(|(_, h)| **h) {
                    c.weight = total / n as f32;
                }
            }
            for c in children {
                walk(&mut c.tree, pick);
            }
        }
        let pick = |s: &Slot| spaces.is_none_or(|ids| s.space.is_some_and(|id| ids.contains(&id)));
        walk(&mut self.layout_mut().slots, &pick);
    }

    /// Deletes a layout, unless it's the last one. Spaces stay.
    pub fn delete_layout(&mut self, id: LayoutId) {
        if self.layouts.len() < 2 {
            return;
        }
        let Some(i) = self.layouts.iter().position(|l| l.id == id) else {
            return;
        };
        self.layouts.remove(i);
        if self.active > i || self.active == self.layouts.len() {
            self.active -= 1;
        }
    }

    pub fn find_layout_mut(&mut self, id: LayoutId) -> Option<&mut Layout> {
        self.layouts.iter_mut().find(|l| l.id == id)
    }

    pub fn cycle_layout(&mut self, delta: isize) {
        self.active = cycle(self.active, self.layouts.len(), delta);
    }

    pub fn select_layout(&mut self, index: usize) {
        if index < self.layouts.len() {
            self.active = index;
        }
    }

    pub fn set_color(&mut self, space: SpaceId, color: usize) {
        if let Some(s) = self.find_space_mut(space)
            && color < COLORS
        {
            s.color = color;
        }
    }

    /// Moves a space in the sidebar list so it ends up before the space now at `to` (or last).
    pub fn move_space(&mut self, space: SpaceId, to: usize) {
        let Some(from) = self.spaces.iter().position(|s| s.id == space) else {
            return;
        };
        let to = if to > from { to - 1 } else { to };
        let s = self.spaces.remove(from);
        self.spaces.insert(to.min(self.spaces.len()), s);
    }
}
