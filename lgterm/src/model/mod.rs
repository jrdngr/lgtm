//! Spaces hold a tree of splits, and a split holds tabs. A layout is a saved tree of slots, each
//! empty or showing one space; the window shows one layout at a time. A space can be in any
//! number of layouts, but in at most one slot of each.

mod ids;
mod layouts;
pub mod legacy;
mod moves;
pub mod tree;

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

pub use ids::{LayoutId, SlotId, SpaceId, SplitId, Tab, TermId};
pub use moves::{Side, WindowDrop};
pub use tree::{Axis, Dir, Keyed, Rect, Tree};

/// How many space colors there are.
pub const COLORS: usize = 8;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Split {
    pub id: SplitId,
    pub tabs: Vec<Tab>,
    pub active: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Space {
    pub id: SpaceId,
    /// A name the user gave it. Unnamed spaces follow their first terminal's folder.
    pub name: Option<String>,
    /// Its color, an index into the space colors.
    #[serde(default)]
    pub color: usize,
    pub splits: Tree<Split>,
    pub focus: SplitId,
    /// A split filling the space.
    #[serde(default)]
    pub zoom: Option<SplitId>,
}

/// A place in a layout for one space.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Slot {
    pub id: SlotId,
    pub space: Option<SpaceId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Layout {
    pub id: LayoutId,
    pub name: Option<String>,
    pub slots: Tree<Slot>,
    pub focus: SlotId,
    /// A space filling the window.
    #[serde(default)]
    pub expanded: Option<SpaceId>,
}

impl Keyed for Split {
    type Key = SplitId;
    fn key(&self) -> SplitId {
        self.id
    }
}

impl Keyed for Space {
    type Key = SpaceId;
    fn key(&self) -> SpaceId {
        self.id
    }
}

impl Keyed for Slot {
    type Key = SlotId;
    fn key(&self) -> SlotId {
        self.id
    }
}

impl Split {
    pub fn tab(&self) -> Tab {
        self.tabs[self.active]
    }
}

impl Space {
    pub fn split(&self) -> &Split {
        self.splits.find(self.focus).expect("focused split")
    }

    fn split_mut(&mut self) -> &mut Split {
        self.splits.find_mut(self.focus).expect("focused split")
    }

    pub fn tabs(&self) -> impl Iterator<Item = Tab> + '_ {
        self.splits
            .leaves()
            .into_iter()
            .flat_map(|s| s.tabs.iter().copied())
    }

    pub fn terms(&self) -> impl Iterator<Item = TermId> + '_ {
        self.tabs().filter_map(Tab::term)
    }
}

impl Layout {
    pub fn slot(&self) -> &Slot {
        self.slots.find(self.focus).expect("focused slot")
    }

    /// The slot showing `space`.
    pub fn slot_of(&self, space: SpaceId) -> Option<SlotId> {
        self.slots
            .leaves()
            .into_iter()
            .find(|s| s.space == Some(space))
            .map(|s| s.id)
    }

    /// The spaces it shows, in reading order.
    pub fn spaces(&self) -> Vec<SpaceId> {
        self.slots
            .leaves()
            .into_iter()
            .filter_map(|s| s.space)
            .collect()
    }
}

/// Something focus can move to: a split, or an empty slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    Split(SplitId),
    Slot(SlotId),
}

/// Where a tab is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    pub space: SpaceId,
    pub split: SplitId,
    pub tab: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Model {
    /// Every space, in sidebar order.
    pub spaces: Vec<Space>,
    pub layouts: Vec<Layout>,
    /// The shown layout.
    pub active: usize,
    next: u64,
}

impl Model {
    /// One layout showing one space with one terminal, and that terminal's id.
    pub fn new() -> (Self, TermId) {
        let mut model = Model {
            spaces: vec![],
            layouts: vec![],
            active: 0,
            next: 1,
        };
        let (space, term) = model.make_space();
        let slot = Slot {
            id: SlotId(model.id()),
            space: Some(space),
        };
        let id = LayoutId(model.id());
        model.layouts.push(Layout {
            id,
            name: None,
            focus: slot.id,
            slots: Tree::Leaf(slot),
            expanded: None,
        });
        model.name_layouts();
        (model, term)
    }

    fn id(&mut self) -> u64 {
        self.next += 1;
        self.next - 1
    }

    fn new_split(&mut self) -> (Split, TermId) {
        let term = TermId(self.id());
        let split = Split {
            id: SplitId(self.id()),
            tabs: vec![Tab::Term(term)],
            active: 0,
        };
        (split, term)
    }

    /// The first color no space has, or the next one round when all are taken.
    fn free_color(&self) -> usize {
        (0..COLORS)
            .find(|c| self.spaces.iter().all(|s| s.color != *c))
            .unwrap_or(self.spaces.len() % COLORS)
    }

    /// Adds a space with one terminal to the end of the list, showing it nowhere.
    fn make_space(&mut self) -> (SpaceId, TermId) {
        let (split, term) = self.new_split();
        let space = Space {
            id: SpaceId(self.id()),
            name: None,
            color: self.free_color(),
            focus: split.id,
            splits: Tree::Leaf(split),
            zoom: None,
        };
        let id = space.id;
        self.spaces.push(space);
        (id, term)
    }

    pub fn layout(&self) -> &Layout {
        &self.layouts[self.active]
    }

    fn layout_mut(&mut self) -> &mut Layout {
        &mut self.layouts[self.active]
    }

    /// The space in the focused slot, if it has one.
    pub fn space(&self) -> Option<&Space> {
        self.layout().slot().space.and_then(|s| self.find_space(s))
    }

    fn space_mut(&mut self) -> Option<&mut Space> {
        let id = self.layout().slot().space?;
        self.find_space_mut(id)
    }

    pub fn split(&self) -> Option<&Split> {
        self.space().map(Space::split)
    }

    fn split_mut(&mut self) -> Option<&mut Split> {
        self.space_mut().map(Space::split_mut)
    }

    /// The focused tab, unless an empty slot is focused.
    pub fn tab(&self) -> Option<Tab> {
        self.split().map(Split::tab)
    }

    /// The focused terminal, unless Messages or an empty slot is focused.
    pub fn term(&self) -> Option<TermId> {
        self.tab().and_then(Tab::term)
    }

    pub fn find_space(&self, id: SpaceId) -> Option<&Space> {
        self.spaces.iter().find(|s| s.id == id)
    }

    pub fn find_space_mut(&mut self, id: SpaceId) -> Option<&mut Space> {
        self.spaces.iter_mut().find(|s| s.id == id)
    }

    /// Makes a space and puts it in the focused slot.
    pub fn add_space(&mut self) -> TermId {
        let (space, term) = self.make_space();
        let slot = self.layout().focus;
        self.assign(slot, space);
        term
    }

    /// Makes a space that isn't shown anywhere.
    pub fn add_space_behind(&mut self) -> TermId {
        self.make_space().1
    }

    /// Opens a tab after the focused one; in an empty slot, a new space.
    pub fn new_tab(&mut self) -> TermId {
        if self.space().is_none() {
            return self.add_space();
        }
        let term = TermId(self.id());
        self.insert_tab(Tab::Term(term));
        term
    }

    /// Shows the Messages tab, opening it after the focused tab if it isn't open.
    pub fn show_messages(&mut self) {
        if self.locate(Tab::Messages).is_none() {
            if self.space().is_none() {
                self.add_space();
            }
            self.insert_tab(Tab::Messages);
        }
        self.focus_tab(Tab::Messages);
    }

    fn insert_tab(&mut self, tab: Tab) {
        if let Some(split) = self.split_mut() {
            split.active += 1;
            split.tabs.insert(split.active, tab);
        }
    }

    /// Splits the focused split, focusing the new one; in an empty slot, a new space.
    pub fn split_split(&mut self, axis: Axis) -> TermId {
        if self.space().is_none() {
            return self.add_space();
        }
        let (split, term) = self.new_split();
        let id = split.id;
        let space = self.space_mut().expect("focused space");
        space.zoom = None;
        let at = space.focus;
        space.splits.insert(at, split, axis).expect("focused split");
        space.focus = id;
        term
    }

    /// Opens a tab after `at`, leaving focus and zoom as they are.
    pub fn new_tab_at(&mut self, at: Tab) -> Option<TermId> {
        let p = self.locate(at)?;
        let new = TermId(self.id());
        let split = self.find_space_mut(p.space)?.splits.find_mut(p.split)?;
        split.tabs.insert(p.tab + 1, Tab::Term(new));
        if split.active > p.tab {
            split.active += 1;
        }
        Some(new)
    }

    /// Splits `at`'s split, leaving focus and zoom as they are.
    pub fn split_split_at(&mut self, at: Tab, axis: Axis) -> Option<TermId> {
        let p = self.locate(at)?;
        let (split, new) = self.new_split();
        let space = self.find_space_mut(p.space)?;
        space.splits.insert(p.split, split, axis).ok()?;
        Some(new)
    }

    pub fn locate(&self, tab: Tab) -> Option<Place> {
        self.spaces.iter().find_map(|space| {
            space.splits.leaves().into_iter().find_map(|split| {
                let i = split.tabs.iter().position(|t| *t == tab)?;
                Some(Place {
                    space: space.id,
                    split: split.id,
                    tab: i,
                })
            })
        })
    }

    /// Shows and focuses a tab, bringing its space into the window if it isn't there.
    pub fn focus_tab(&mut self, tab: Tab) {
        let Some(p) = self.locate(tab) else { return };
        self.show_space(p.space);
        let space = self.find_space_mut(p.space).expect("space");
        if space.zoom.is_some_and(|z| z != p.split) {
            space.zoom = None;
        }
        space.focus = p.split;
        space.split_mut().active = p.tab;
    }

    pub fn focus_term(&mut self, term: TermId) {
        self.focus_tab(Tab::Term(term));
    }

    /// Focuses a space's slot in the shown layout, first putting it in the focused slot when
    /// it isn't shown.
    pub fn show_space(&mut self, space: SpaceId) {
        let layout = self.layout();
        let slot = layout.slot_of(space).unwrap_or(layout.focus);
        self.assign(slot, space);
    }

    pub fn focus_split(&mut self, split: SplitId) {
        let tab = self
            .spaces
            .iter()
            .find_map(|s| s.splits.find(split))
            .map(Split::tab);
        if let Some(tab) = tab {
            self.focus_tab(tab);
        }
    }

    /// Takes a tab out, closing whatever is left empty: its split, and its space, which leaves
    /// its slots empty. Returns false, leaving the model as it is, when `tab` is the last one.
    pub fn remove_tab(&mut self, tab: Tab) -> bool {
        let Some(p) = self.locate(tab) else {
            return true;
        };
        let last_space = self.spaces.len() == 1;
        let space = self.find_space_mut(p.space).expect("space");
        let split = space.splits.find_mut(p.split).expect("split");
        if split.tabs.len() > 1 {
            split.tabs.remove(p.tab);
            if split.active > p.tab || split.active == split.tabs.len() {
                split.active -= 1;
            }
            return true;
        }
        if space.splits.remove(p.split).is_some() {
            if space.focus == p.split {
                space.focus = space.splits.first().id;
            }
            if space.zoom == Some(p.split) {
                space.zoom = None;
            }
            return true;
        }
        if last_space {
            return false;
        }
        self.remove_space(p.space);
        true
    }

    /// Drops a space, emptying the slots that showed it.
    fn remove_space(&mut self, id: SpaceId) {
        let Some(i) = self.spaces.iter().position(|s| s.id == id) else {
            return;
        };
        self.spaces.remove(i);
        for layout in &mut self.layouts {
            if let Some(slot) = layout.slot_of(id) {
                layout.slots.find_mut(slot).expect("slot").space = None;
            }
            if layout.expanded == Some(id) {
                layout.expanded = None;
            }
        }
    }

    /// Moves the focused tab into a new split to the right of its own.
    pub fn move_tab_to_new_split(&mut self) {
        if self.split().is_none_or(|s| s.tabs.len() < 2) {
            return;
        }
        let id = SplitId(self.id());
        let space = self.space_mut().expect("focused space");
        space.zoom = None;
        let from = space.split_mut();
        let tab = from.tabs.remove(from.active);
        from.active = from.active.min(from.tabs.len() - 1);
        let split = Split {
            id,
            tabs: vec![tab],
            active: 0,
        };
        let at = space.focus;
        space
            .splits
            .insert(at, split, Axis::Row)
            .expect("focused split");
        space.focus = id;
    }

    /// Every terminal, in sidebar order.
    pub fn terms(&self) -> Vec<TermId> {
        self.spaces.iter().flat_map(Space::terms).collect()
    }

    /// Whether a loaded model holds together: every tree is well formed, everything focused
    /// exists, every id is unique and below the next one handed out, no layout shows a space
    /// twice or one that doesn't exist, and there's at most one Messages tab.
    pub fn is_valid(&self) -> bool {
        if self.active >= self.layouts.len() || self.spaces.is_empty() {
            return false;
        }
        let mut terms = HashSet::new();
        let mut messages = 0;
        let mut tab_ok = |t: &Tab| match t {
            Tab::Term(t) => t.0 < self.next && terms.insert(t.0),
            Tab::Messages => {
                messages += 1;
                messages == 1
            }
        };
        let mut seen: [HashSet<u64>; 4] = Default::default();
        let spaces_ok = self.spaces.iter().all(|s| {
            s.id.0 < self.next
                && seen[0].insert(s.id.0)
                && s.color < COLORS
                && s.splits.well_formed()
                && s.splits.contains(s.focus)
                && s.splits.leaves().into_iter().all(|sp| {
                    sp.id.0 < self.next
                        && seen[1].insert(sp.id.0)
                        && sp.active < sp.tabs.len()
                        && sp.tabs.iter().all(&mut tab_ok)
                })
        });
        spaces_ok
            && self.layouts.iter().all(|l| {
                let mut shown = HashSet::new();
                l.id.0 < self.next
                    && seen[2].insert(l.id.0)
                    && l.slots.well_formed()
                    && l.slots.contains(l.focus)
                    && l.slots.leaves().into_iter().all(|slot| {
                        slot.id.0 < self.next
                            && seen[3].insert(slot.id.0)
                            && slot
                                .space
                                .is_none_or(|s| seen[0].contains(&s.0) && shown.insert(s.0))
                    })
            })
    }

    /// Every tab in the focused split.
    pub fn split_tabs(&self) -> Vec<Tab> {
        self.split().map(|s| s.tabs.clone()).unwrap_or_default()
    }

    pub fn select_tab(&mut self, index: usize) {
        if let Some(split) = self.split_mut()
            && index < split.tabs.len()
        {
            split.active = index;
        }
    }

    pub fn cycle_tab(&mut self, delta: isize) {
        if let Some(split) = self.split_mut() {
            split.active = cycle(split.active, split.tabs.len(), delta);
        }
    }

    /// Each split and empty slot the window shows, with its rectangle: only the expanded
    /// space's, and only a space's zoomed split.
    pub fn cells(&self) -> Vec<(Cell, Rect)> {
        let layout = self.layout();
        let slots = match layout.expanded.and_then(|e| layout.slot_of(e)) {
            Some(slot) => vec![(slot, Rect::UNIT)],
            None => layout.slots.rects(Rect::UNIT),
        };
        slots
            .into_iter()
            .flat_map(|(slot, area)| {
                let slot = layout.slots.find(slot).expect("slot");
                match slot.space.and_then(|s| self.find_space(s)) {
                    Some(space) => match space.zoom.filter(|&z| space.splits.contains(z)) {
                        Some(z) => vec![(Cell::Split(z), area)],
                        None => space
                            .splits
                            .rects(area)
                            .into_iter()
                            .map(|(id, r)| (Cell::Split(id), r))
                            .collect(),
                    },
                    None => vec![(Cell::Slot(slot.id), area)],
                }
            })
            .collect()
    }

    /// What has focus: the focused split, or the focused slot when it's empty.
    fn cell(&self) -> Cell {
        match self.split() {
            Some(split) => Cell::Split(split.id),
            None => Cell::Slot(self.layout().focus),
        }
    }

    fn focus_cell(&mut self, cell: Cell) {
        match cell {
            Cell::Split(split) => self.focus_split(split),
            Cell::Slot(slot) => self.focus_slot(slot),
        }
    }

    /// Focuses the next or previous split or empty slot, in reading order.
    pub fn cycle_split(&mut self, delta: isize) {
        let cells = self.cells();
        let here = self.cell();
        let i = cells.iter().position(|(k, _)| *k == here).unwrap_or(0);
        let next = cells[cycle(i, cells.len(), delta)].0;
        self.focus_cell(next);
    }

    pub fn focus_dir(&mut self, dir: Dir) {
        let cells = self.cells();
        if let Some(next) = tree::neighbor(&cells, self.cell(), dir) {
            self.focus_cell(next);
        }
    }

    /// Fills the focused space with its focused split, or puts its splits back.
    pub fn toggle_zoom(&mut self) {
        if let Some(space) = self.space_mut() {
            space.zoom = match space.zoom {
                Some(_) => None,
                None => Some(space.focus),
            };
        }
    }

    /// Fills the window with `space`, or puts the layout back when it already does.
    pub fn toggle_expand(&mut self, space: SpaceId) {
        if self.layout().expanded == Some(space) {
            self.layout_mut().expanded = None;
            return;
        }
        self.show_space(space);
        self.layout_mut().expanded = Some(space);
    }

    /// Puts `to` where `from` is.
    pub fn replace_tab(&mut self, from: Tab, to: Tab) {
        let Some(p) = self.locate(from) else { return };
        let split = self
            .find_space_mut(p.space)
            .and_then(|s| s.splits.find_mut(p.split));
        if let Some(split) = split {
            split.tabs[p.tab] = to;
        }
    }

    /// The space a tab is in.
    pub fn space_of(&self, tab: Tab) -> Option<&Space> {
        let p = self.locate(tab)?;
        self.find_space(p.space)
    }
}

fn cycle(i: usize, len: usize, delta: isize) -> usize {
    (i as isize + delta).rem_euclid(len as isize) as usize
}

#[cfg(test)]
mod tests;
