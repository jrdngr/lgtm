//! Layouts hold a tree of spaces, a space holds a tree of splits, and a split holds tabs. The
//! window shows one layout at a time.

mod ids;
pub mod tree;

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

pub use ids::{LayoutId, SpaceId, SplitId, Tab, TermId};
pub use tree::{Axis, Dir, Keyed, Rect, Tree};

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
    pub splits: Tree<Split>,
    pub focus: SplitId,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Layout {
    pub id: LayoutId,
    pub name: Option<String>,
    pub spaces: Tree<Space>,
    pub focus: SpaceId,
    /// A split filling the whole layout.
    pub zoom: Option<SplitId>,
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
    pub fn space(&self) -> &Space {
        self.spaces.find(self.focus).expect("focused space")
    }

    fn space_mut(&mut self) -> &mut Space {
        self.spaces.find_mut(self.focus).expect("focused space")
    }

    /// Every split's rectangle across the layout's spaces.
    pub fn split_rects(&self) -> Vec<(SplitId, Rect)> {
        self.spaces
            .rects(Rect::UNIT)
            .into_iter()
            .flat_map(|(space, area)| {
                let space = self.spaces.find(space).expect("space");
                space.splits.rects(area)
            })
            .collect()
    }

    pub fn terms(&self) -> impl Iterator<Item = TermId> + '_ {
        self.spaces.leaves().into_iter().flat_map(Space::terms)
    }
}

/// Where a tab is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    pub layout: usize,
    pub space: SpaceId,
    pub split: SplitId,
    pub tab: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Model {
    pub layouts: Vec<Layout>,
    pub active: usize,
    next: u64,
}

impl Model {
    /// One layout with one terminal, and that terminal's id.
    pub fn new() -> (Self, TermId) {
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

    fn new_split(&mut self) -> (Split, TermId) {
        let term = TermId(self.id());
        let split = Split {
            id: SplitId(self.id()),
            tabs: vec![Tab::Term(term)],
            active: 0,
        };
        (split, term)
    }

    fn new_space(&mut self) -> (Space, TermId) {
        let (split, term) = self.new_split();
        let space = Space {
            id: SpaceId(self.id()),
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

    /// The focused tab.
    pub fn tab(&self) -> Tab {
        self.split().tab()
    }

    /// The focused terminal, unless Messages is focused.
    pub fn term(&self) -> Option<TermId> {
        self.tab().term()
    }

    /// Opens a layout after the current one and switches to it.
    pub fn new_layout(&mut self) -> TermId {
        let (space, term) = self.new_space();
        let layout = Layout {
            id: LayoutId(self.id()),
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

    pub fn new_tab(&mut self) -> TermId {
        let term = TermId(self.id());
        self.insert_tab(Tab::Term(term));
        term
    }

    /// Shows the Messages tab, opening it after the focused tab if it isn't open.
    pub fn show_messages(&mut self) {
        if self.locate(Tab::Messages).is_none() {
            self.insert_tab(Tab::Messages);
        }
        self.focus_tab(Tab::Messages);
    }

    fn insert_tab(&mut self, tab: Tab) {
        let split = self.layout_mut().space_mut().split_mut();
        split.active += 1;
        split.tabs.insert(split.active, tab);
    }

    /// Splits the focused split, focusing the new one.
    pub fn split_split(&mut self, axis: Axis) -> TermId {
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
    pub fn split_space(&mut self, axis: Axis) -> TermId {
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

    /// Opens a tab after `at`, leaving focus and zoom as they are.
    pub fn new_tab_at(&mut self, at: Tab) -> Option<TermId> {
        let p = self.locate(at)?;
        let new = TermId(self.id());
        let split = self.layouts[p.layout]
            .spaces
            .find_mut(p.space)?
            .splits
            .find_mut(p.split)?;
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
        let space = self.layouts[p.layout].spaces.find_mut(p.space)?;
        space.splits.insert(p.split, split, axis).ok()?;
        Some(new)
    }

    /// Opens a space beside `at`'s, leaving focus and zoom as they are.
    pub fn split_space_at(&mut self, at: Tab, axis: Axis) -> Option<TermId> {
        let p = self.locate(at)?;
        let (space, new) = self.new_space();
        self.layouts[p.layout]
            .spaces
            .insert(p.space, space, axis)
            .ok()?;
        Some(new)
    }

    /// Opens a layout after the current one, leaving the current one shown.
    pub fn new_layout_behind(&mut self) -> TermId {
        let active = self.active;
        let term = self.new_layout();
        self.active = active;
        term
    }

    pub fn locate(&self, tab: Tab) -> Option<Place> {
        self.layouts.iter().enumerate().find_map(|(layout, l)| {
            l.spaces.leaves().into_iter().find_map(|space| {
                space.splits.leaves().into_iter().find_map(|split| {
                    let i = split.tabs.iter().position(|t| *t == tab)?;
                    Some(Place {
                        layout,
                        space: space.id,
                        split: split.id,
                        tab: i,
                    })
                })
            })
        })
    }

    /// Shows and focuses a tab, switching layouts if needed.
    pub fn focus_tab(&mut self, tab: Tab) {
        let Some(p) = self.locate(tab) else { return };
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

    pub fn focus_term(&mut self, term: TermId) {
        self.focus_tab(Tab::Term(term));
    }

    /// Shows a space and focuses its focused split.
    pub fn focus_space(&mut self, space: SpaceId) {
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

    pub fn focus_split(&mut self, split: SplitId) {
        let Some(tab) = self.split_tab(split) else {
            return;
        };
        self.focus_tab(tab);
    }

    fn split_tab(&self, split: SplitId) -> Option<Tab> {
        self.layout()
            .spaces
            .leaves()
            .into_iter()
            .find_map(|s| s.splits.find(split))
            .map(Split::tab)
    }

    /// Takes a tab out, closing whatever is left empty: its split, space and layout. Returns
    /// false, leaving the model as it is, when `tab` is the last one.
    pub fn remove_tab(&mut self, tab: Tab) -> bool {
        let Some(p) = self.locate(tab) else {
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
        if self.layouts.len() == 1 {
            return false;
        }
        self.layouts.remove(p.layout);
        if self.active > p.layout || self.active == self.layouts.len() {
            self.active = self.active.saturating_sub(1);
        }
        true
    }

    /// Moves the focused tab into a new split to the right of its own.
    pub fn move_tab_to_new_split(&mut self) {
        if self.split().tabs.len() < 2 {
            return;
        }
        let id = SplitId(self.id());
        let layout = self.layout_mut();
        layout.zoom = None;
        let space = layout.space_mut();
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

    /// Every terminal in every layout, in sidebar order.
    pub fn terms(&self) -> Vec<TermId> {
        self.layouts.iter().flat_map(Layout::terms).collect()
    }

    /// Whether a loaded model holds together: every tree is well formed, everything focused
    /// exists, every id is unique and below the next one handed out, and there's at most one
    /// Messages tab.
    pub fn is_valid(&self) -> bool {
        if self.active >= self.layouts.len() {
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
        let mut seen: [HashSet<u64>; 3] = Default::default();
        self.layouts.iter().all(|l| {
            l.id.0 < self.next
                && seen[0].insert(l.id.0)
                && l.spaces.well_formed()
                && l.spaces.contains(l.focus)
                && l.spaces.leaves().into_iter().all(|s| {
                    s.id.0 < self.next
                        && seen[1].insert(s.id.0)
                        && s.splits.well_formed()
                        && s.splits.contains(s.focus)
                        && s.splits.leaves().into_iter().all(|sp| {
                            sp.id.0 < self.next
                                && seen[2].insert(sp.id.0)
                                && sp.active < sp.tabs.len()
                                && sp.tabs.iter().all(&mut tab_ok)
                        })
                })
        })
    }

    /// Every tab in the focused split.
    pub fn split_tabs(&self) -> Vec<Tab> {
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

    pub fn find_space(&self, id: SpaceId) -> Option<&Space> {
        self.layouts.iter().find_map(|l| l.spaces.find(id))
    }

    pub fn find_space_mut(&mut self, id: SpaceId) -> Option<&mut Space> {
        self.layouts.iter_mut().find_map(|l| l.spaces.find_mut(id))
    }

    pub fn find_layout_mut(&mut self, id: LayoutId) -> Option<&mut Layout> {
        self.layouts.iter_mut().find(|l| l.id == id)
    }

    /// Puts `to` where `from` is.
    pub fn replace_tab(&mut self, from: Tab, to: Tab) {
        let Some(p) = self.locate(from) else { return };
        let split = self.layouts[p.layout]
            .spaces
            .find_mut(p.space)
            .and_then(|s| s.splits.find_mut(p.split));
        if let Some(split) = split {
            split.tabs[p.tab] = to;
        }
    }

    /// The space a tab is in.
    pub fn space_of(&self, tab: Tab) -> Option<&Space> {
        let p = self.locate(tab)?;
        self.layouts[p.layout].spaces.find(p.space)
    }
}

fn cycle(i: usize, len: usize, delta: isize) -> usize {
    (i as isize + delta).rem_euclid(len as isize) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tabs(terms: &[TermId]) -> Vec<Tab> {
        terms.iter().map(|t| Tab::Term(*t)).collect()
    }

    #[test]
    fn closing_the_last_tab_closes_its_split_space_and_layout() {
        let (mut m, first) = Model::new();
        let right = m.split_space(Axis::Row);
        let below = m.split_split(Axis::Column);
        let terms: Vec<TermId> = m
            .layout()
            .spaces
            .leaves()
            .iter()
            .flat_map(|s| s.terms())
            .collect();
        assert_eq!(terms, [first, right, below]);

        assert!(m.remove_tab(below.into()));
        assert_eq!(m.term(), Some(right));
        assert!(m.remove_tab(right.into()));
        assert_eq!(m.term(), Some(first));
        assert_eq!(m.layout().spaces.leaves().len(), 1);

        let other = m.new_layout();
        assert_eq!(m.active, 1);
        assert!(m.remove_tab(other.into()));
        assert_eq!(m.active, 0);
        assert!(!m.remove_tab(first.into()));
        assert_eq!(m.term(), Some(first));
    }

    #[test]
    fn closing_a_tab_keeps_the_active_tab_in_range() {
        let (mut m, a) = Model::new();
        let b = m.new_tab();
        let c = m.new_tab();
        assert_eq!(m.split().tabs, tabs(&[a, b, c]));
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
        m.new_layout();
        let json = serde_json::to_string(&m).unwrap();
        let back: Model = serde_json::from_str(&json).unwrap();
        assert!(back.is_valid());
        assert_eq!(back.terms(), m.terms());
        assert_eq!(back.term(), m.term());
        let broken = json.replace("\"active\":1", "\"active\":7");
        let broken: Model = serde_json::from_str(&broken).unwrap();
        assert!(!broken.is_valid());
    }

    #[test]
    fn degenerate_trees_and_duplicate_ids_are_invalid() {
        let (mut m, _) = Model::new();
        m.split_split(Axis::Row);
        assert!(m.is_valid());

        let mut lone = m.clone();
        let focus = lone.layouts[0].focus;
        let space = lone.layouts[0].spaces.find_mut(focus).unwrap();
        let Tree::Branch { children, .. } = &mut space.splits else {
            panic!("split space");
        };
        children.pop();
        space.focus = children[0].tree.first().id;
        assert!(!lone.is_valid());

        let mut twin = m.clone();
        let focus = twin.layouts[0].focus;
        let space = twin.layouts[0].spaces.find_mut(focus).unwrap();
        let Tree::Branch { children, .. } = &mut space.splits else {
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
    }

    #[test]
    fn a_tab_moves_into_a_split_of_its_own() {
        let (mut m, a) = Model::new();
        let b = m.new_tab();
        m.move_tab_to_new_split();
        assert_eq!(m.term(), Some(b));
        assert_eq!(m.split().tabs, tabs(&[b]));
        let splits = m.space().splits.leaves();
        assert_eq!(splits.len(), 2);
        assert_eq!(splits[0].tabs, tabs(&[a]));
        m.move_tab_to_new_split();
        assert_eq!(m.space().splits.leaves().len(), 2);
    }

    #[test]
    fn new_tabs_open_after_the_active_one() {
        let (mut m, a) = Model::new();
        let b = m.new_tab();
        m.select_tab(0);
        let c = m.new_tab();
        assert_eq!(m.split().tabs, tabs(&[a, c, b]));
        m.cycle_tab(-2);
        assert_eq!(m.term(), Some(b));
    }

    #[test]
    fn focus_term_switches_layout_and_moves_focus_down_the_tree() {
        let (mut m, a) = Model::new();
        let b = m.split_split(Axis::Row);
        m.new_layout();
        m.focus_term(a);
        assert_eq!((m.active, m.term()), (0, Some(a)));
        m.focus_dir(Dir::Right);
        assert_eq!(m.term(), Some(b));
    }

    #[test]
    fn moving_focus_crosses_spaces() {
        let (mut m, a) = Model::new();
        let b = m.split_space(Axis::Row);
        m.focus_dir(Dir::Left);
        assert_eq!(m.term(), Some(a));
        m.cycle_split(1);
        assert_eq!(m.term(), Some(b));
    }

    #[test]
    fn zoom_ends_when_focus_leaves_the_split_or_the_layout_splits() {
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
        let space = m.split_space_at(a.into(), Axis::Row).unwrap();
        assert_eq!((m.term(), m.layout().zoom), (Some(b), zoom));
        assert_eq!(
            m.locate(tab.into()).unwrap().split,
            m.locate(a.into()).unwrap().split
        );
        assert_eq!(m.split().tabs, tabs(&[b]));
        assert!(m.locate(split.into()).is_some() && m.locate(space.into()).is_some());
        assert!(m.is_valid());
    }
}
