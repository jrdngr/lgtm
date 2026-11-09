//! Layouts hold a tree of spaces, a space holds a tree of splits, and a split holds terminals
//! shown as tabs. The window shows one layout at a time.

pub mod tree;

pub use tree::{Axis, Dir, Keyed, Rect, Tree};

#[derive(Debug)]
pub struct Split {
    pub id: u64,
    pub tabs: Vec<u64>,
    pub active: usize,
}

#[derive(Debug)]
pub struct Space {
    pub id: u64,
    /// A name the user gave it. Unnamed spaces follow their first terminal's folder.
    pub name: Option<String>,
    pub splits: Tree<Split>,
    pub focus: u64,
}

#[derive(Debug)]
pub struct Layout {
    pub id: u64,
    pub name: Option<String>,
    pub spaces: Tree<Space>,
    pub focus: u64,
    /// A split filling the whole layout.
    pub zoom: Option<u64>,
}

impl Keyed for Split {
    fn key(&self) -> u64 {
        self.id
    }
}

impl Keyed for Space {
    fn key(&self) -> u64 {
        self.id
    }
}

impl Split {
    pub fn term(&self) -> u64 {
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

    /// The terminal an unnamed space takes its name from.
    pub fn first_term(&self) -> u64 {
        self.splits.first().tabs[0]
    }

    pub fn terms(&self) -> impl Iterator<Item = u64> + '_ {
        self.splits
            .leaves()
            .into_iter()
            .flat_map(|s| s.tabs.iter().copied())
    }
}

impl Layout {
    pub fn space(&self) -> &Space {
        self.spaces.find(self.focus).expect("focused space")
    }

    fn space_mut(&mut self) -> &mut Space {
        self.spaces.find_mut(self.focus).expect("focused space")
    }

    /// Every split's rectangle across the layout's spaces.
    pub fn split_rects(&self) -> Vec<(u64, Rect)> {
        self.spaces
            .rects(Rect::UNIT)
            .into_iter()
            .flat_map(|(space, area)| {
                let space = self.spaces.find(space).expect("space");
                space.splits.rects(area)
            })
            .collect()
    }
}

/// Where a terminal is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    pub layout: usize,
    pub space: u64,
    pub split: u64,
    pub tab: usize,
}

#[derive(Debug)]
pub struct Model {
    pub layouts: Vec<Layout>,
    pub active: usize,
    next: u64,
}

impl Model {
    /// One layout with one terminal, and that terminal's id.
    pub fn new() -> (Self, u64) {
        let mut model = Model {
            layouts: vec![],
            active: 0,
            next: 1,
        };
        let term = model.new_layout();
        (model, term)
    }

    fn id(&mut self) -> u64 {
        self.next += 1;
        self.next - 1
    }

    fn new_split(&mut self) -> (Split, u64) {
        let term = self.id();
        let split = Split {
            id: self.id(),
            tabs: vec![term],
            active: 0,
        };
        (split, term)
    }

    fn new_space(&mut self) -> (Space, u64) {
        let (split, term) = self.new_split();
        let space = Space {
            id: self.id(),
            name: None,
            focus: split.id,
            splits: Tree::Leaf(split),
        };
        (space, term)
    }

    pub fn layout(&self) -> &Layout {
        &self.layouts[self.active]
    }

    fn layout_mut(&mut self) -> &mut Layout {
        &mut self.layouts[self.active]
    }

    pub fn space(&self) -> &Space {
        self.layout().space()
    }

    pub fn split(&self) -> &Split {
        self.space().split()
    }

    /// The focused terminal.
    pub fn term(&self) -> u64 {
        self.split().term()
    }

    /// Opens a layout after the current one and switches to it.
    pub fn new_layout(&mut self) -> u64 {
        let (space, term) = self.new_space();
        let layout = Layout {
            id: self.id(),
            name: None,
            focus: space.id,
            spaces: Tree::Leaf(space),
            zoom: None,
        };
        let at = if self.layouts.is_empty() {
            0
        } else {
            self.active + 1
        };
        self.layouts.insert(at, layout);
        self.active = at;
        term
    }

    pub fn new_tab(&mut self) -> u64 {
        let term = self.id();
        let split = self.layout_mut().space_mut().split_mut();
        split.active += 1;
        split.tabs.insert(split.active, term);
        term
    }

    /// Splits the focused split, focusing the new one.
    pub fn split_split(&mut self, axis: Axis) -> u64 {
        let (split, term) = self.new_split();
        let id = split.id;
        let layout = self.layout_mut();
        layout.zoom = None;
        let space = layout.space_mut();
        let at = space.focus;
        space.splits.insert(at, split, axis).expect("focused split");
        space.focus = id;
        term
    }

    /// Opens a space beside the focused one, focusing it.
    pub fn split_space(&mut self, axis: Axis) -> u64 {
        let (space, term) = self.new_space();
        let id = space.id;
        let layout = self.layout_mut();
        layout.zoom = None;
        let at = layout.focus;
        layout
            .spaces
            .insert(at, space, axis)
            .expect("focused space");
        layout.focus = id;
        term
    }

    pub fn locate(&self, term: u64) -> Option<Place> {
        self.layouts.iter().enumerate().find_map(|(layout, l)| {
            l.spaces.leaves().into_iter().find_map(|space| {
                space.splits.leaves().into_iter().find_map(|split| {
                    let tab = split.tabs.iter().position(|t| *t == term)?;
                    Some(Place {
                        layout,
                        space: space.id,
                        split: split.id,
                        tab,
                    })
                })
            })
        })
    }

    /// Shows and focuses a terminal, switching layouts if needed.
    pub fn focus_term(&mut self, term: u64) {
        let Some(p) = self.locate(term) else { return };
        self.active = p.layout;
        let layout = self.layout_mut();
        if layout.zoom.is_some_and(|z| z != p.split) {
            layout.zoom = None;
        }
        layout.focus = p.space;
        let space = layout.space_mut();
        space.focus = p.split;
        space.split_mut().active = p.tab;
    }

    /// Shows a space and focuses its focused split.
    pub fn focus_space(&mut self, space: u64) {
        let Some(i) = self.layouts.iter().position(|l| l.spaces.contains(space)) else {
            return;
        };
        self.active = i;
        let layout = self.layout_mut();
        layout.focus = space;
        let zoom_here = layout
            .zoom
            .is_some_and(|z| layout.space().splits.contains(z));
        if !zoom_here {
            layout.zoom = None;
        }
    }

    pub fn focus_split(&mut self, split: u64) {
        let Some(term) = self.split_term(split) else {
            return;
        };
        self.focus_term(term);
    }

    fn split_term(&self, split: u64) -> Option<u64> {
        self.layout()
            .spaces
            .leaves()
            .into_iter()
            .find_map(|s| s.splits.find(split))
            .map(Split::term)
    }

    /// Takes a terminal out, closing whatever is left empty: its split, space and layout.
    /// Returns false once no terminals are left.
    pub fn remove_term(&mut self, term: u64) -> bool {
        let Some(p) = self.locate(term) else {
            return true;
        };
        let layout = &mut self.layouts[p.layout];
        let space = layout.spaces.find_mut(p.space).expect("space");
        let split = space.splits.find_mut(p.split).expect("split");
        if split.tabs.len() > 1 {
            split.tabs.remove(p.tab);
            if split.active > p.tab || split.active == split.tabs.len() {
                split.active -= 1;
            }
            return true;
        }
        if layout.zoom == Some(p.split) {
            layout.zoom = None;
        }
        if space.splits.remove(p.split).is_some() {
            if space.focus == p.split {
                space.focus = space.splits.first().id;
            }
            return true;
        }
        if layout.spaces.remove(p.space).is_some() {
            if layout.focus == p.space {
                layout.focus = layout.spaces.first().id;
            }
            return true;
        }
        self.layouts.remove(p.layout);
        if self.active > p.layout || self.active == self.layouts.len() {
            self.active = self.active.saturating_sub(1);
        }
        !self.layouts.is_empty()
    }

    /// Every terminal in the focused split.
    pub fn split_terms(&self) -> Vec<u64> {
        self.split().tabs.clone()
    }

    pub fn select_tab(&mut self, index: usize) {
        let split = self.layout_mut().space_mut().split_mut();
        if index < split.tabs.len() {
            split.active = index;
        }
    }

    pub fn cycle_tab(&mut self, delta: isize) {
        let split = self.layout_mut().space_mut().split_mut();
        split.active = cycle(split.active, split.tabs.len(), delta);
    }

    /// Focuses the next or previous split in the layout, in reading order.
    pub fn cycle_split(&mut self, delta: isize) {
        let rects = self.layout().split_rects();
        let here = self.split().id;
        let i = rects.iter().position(|(k, _)| *k == here).unwrap_or(0);
        let next = rects[cycle(i, rects.len(), delta)].0;
        self.focus_split(next);
    }

    pub fn focus_dir(&mut self, dir: Dir) {
        if self.layout().zoom.is_some() {
            return;
        }
        let rects = self.layout().split_rects();
        if let Some(next) = tree::neighbor(&rects, self.split().id, dir) {
            self.focus_split(next);
        }
    }

    pub fn toggle_zoom(&mut self) {
        let split = self.split().id;
        let layout = self.layout_mut();
        layout.zoom = match layout.zoom {
            Some(_) => None,
            None => Some(split),
        };
    }

    pub fn cycle_layout(&mut self, delta: isize) {
        self.active = cycle(self.active, self.layouts.len(), delta);
    }

    pub fn select_layout(&mut self, index: usize) {
        if index < self.layouts.len() {
            self.active = index;
        }
    }

    pub fn find_space_mut(&mut self, id: u64) -> Option<&mut Space> {
        self.layouts.iter_mut().find_map(|l| l.spaces.find_mut(id))
    }

    pub fn find_layout_mut(&mut self, id: u64) -> Option<&mut Layout> {
        self.layouts.iter_mut().find(|l| l.id == id)
    }
}

fn cycle(i: usize, len: usize, delta: isize) -> usize {
    (i as isize + delta).rem_euclid(len as isize) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_the_last_tab_closes_its_split_space_and_layout() {
        let (mut m, first) = Model::new();
        let right = m.split_space(Axis::Row);
        let below = m.split_split(Axis::Column);
        let terms: Vec<u64> = m
            .layout()
            .spaces
            .leaves()
            .iter()
            .flat_map(|s| s.terms())
            .collect();
        assert_eq!(terms, [first, right, below]);

        assert!(m.remove_term(below));
        assert_eq!(m.term(), right);
        assert!(m.remove_term(right));
        assert_eq!(m.term(), first);
        assert_eq!(m.layout().spaces.leaves().len(), 1);

        let other = m.new_layout();
        assert_eq!(m.active, 1);
        assert!(m.remove_term(other));
        assert_eq!(m.active, 0);
        assert!(!m.remove_term(first));
    }

    #[test]
    fn closing_a_tab_keeps_the_active_tab_in_range() {
        let (mut m, a) = Model::new();
        let b = m.new_tab();
        let c = m.new_tab();
        assert_eq!(m.split().tabs, [a, b, c]);
        assert!(m.remove_term(c));
        assert_eq!(m.term(), b);
        m.select_tab(0);
        assert!(m.remove_term(b));
        assert_eq!(m.term(), a);
    }

    #[test]
    fn new_tabs_open_after_the_active_one() {
        let (mut m, a) = Model::new();
        let b = m.new_tab();
        m.select_tab(0);
        let c = m.new_tab();
        assert_eq!(m.split().tabs, [a, c, b]);
        m.cycle_tab(-2);
        assert_eq!(m.term(), b);
    }

    #[test]
    fn focus_term_switches_layout_and_moves_focus_down_the_tree() {
        let (mut m, a) = Model::new();
        let b = m.split_split(Axis::Row);
        m.new_layout();
        m.focus_term(a);
        assert_eq!((m.active, m.term()), (0, a));
        m.focus_dir(Dir::Right);
        assert_eq!(m.term(), b);
    }

    #[test]
    fn moving_focus_crosses_spaces() {
        let (mut m, a) = Model::new();
        let b = m.split_space(Axis::Row);
        m.focus_dir(Dir::Left);
        assert_eq!(m.term(), a);
        m.cycle_split(1);
        assert_eq!(m.term(), b);
    }

    #[test]
    fn zoom_ends_when_focus_leaves_the_split_or_the_layout_splits() {
        let (mut m, a) = Model::new();
        let b = m.split_split(Axis::Row);
        m.toggle_zoom();
        m.focus_dir(Dir::Left);
        assert_eq!(m.term(), b);
        m.focus_term(a);
        assert_eq!(m.layout().zoom, None);
        m.toggle_zoom();
        m.split_split(Axis::Row);
        assert_eq!(m.layout().zoom, None);
    }
}
