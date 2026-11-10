//! Moving spaces between layouts: into the shown layout, into a layout of their own, or beside
//! another space.

use super::{Axis, Layout, LayoutId, Model, Space, SpaceId, Tree};

/// A side of a space or of the whole layout.
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
    /// Beside a space of the shown layout.
    Space(SpaceId, Side),
    /// Along an edge of the whole shown layout.
    Edge(Side),
}

impl Model {
    fn layout_of(&self, space: SpaceId) -> Option<usize> {
        self.layouts.iter().position(|l| l.spaces.contains(space))
    }

    fn index_of(&self, id: LayoutId) -> Option<usize> {
        self.layouts.iter().position(|l| l.id == id)
    }

    /// Whether `space` is the only space in its layout.
    pub fn is_alone(&self, space: SpaceId) -> bool {
        self.layout_of(space)
            .is_some_and(|i| matches!(self.layouts[i].spaces, Tree::Leaf(_)))
    }

    /// Takes a space out of its layout, dropping the layout when it was the last one. The shown
    /// layout stays shown if it's still there.
    fn take_space(&mut self, id: SpaceId) -> Option<Space> {
        let i = self.layout_of(id)?;
        let shown = self.layouts[self.active].id;
        let layout = &mut self.layouts[i];
        let space = match layout.spaces {
            Tree::Leaf(_) => {
                let Tree::Leaf(space) = self.layouts.remove(i).spaces else {
                    unreachable!()
                };
                space
            }
            Tree::Branch { .. } => {
                let space = layout.spaces.remove(id)?;
                if layout.focus == id {
                    layout.focus = layout.spaces.first().id;
                }
                if layout.zoom.is_some_and(|z| space.splits.contains(z)) {
                    layout.zoom = None;
                }
                space
            }
        };
        self.active = self.index_of(shown).unwrap_or(0);
        Some(space)
    }

    /// Moves a space into the shown layout and focuses it.
    pub fn drop_in_window(&mut self, id: SpaceId, at: WindowDrop) {
        let shown = self.layouts[self.active].id;
        let target = match at {
            WindowDrop::Space(target, _) => Some(target),
            WindowDrop::Edge(_) => None,
        };
        let shown_spaces = &self.layouts[self.active].spaces;
        if target == Some(id)
            || target.is_some_and(|t| !shown_spaces.contains(t))
            || matches!(shown_spaces, Tree::Leaf(s) if s.id == id)
        {
            return;
        }
        let Some(space) = self.take_space(id) else {
            return;
        };
        let i = self.index_of(shown).expect("shown layout keeps a space");
        let layout = &mut self.layouts[i];
        match at {
            WindowDrop::Space(target, side) => {
                let _ = layout
                    .spaces
                    .insert_beside(target, space, side.axis(), side.before());
            }
            WindowDrop::Edge(side) => layout.spaces.wrap(space, side.axis(), side.before()),
        }
        layout.focus = id;
        layout.zoom = None;
        self.active = i;
    }

    /// Moves a space into a layout of its own, placed before the layout at `index` (or last).
    /// When the shown layout was only that space, the new layout is shown.
    pub fn drop_as_layout(&mut self, id: SpaceId, index: usize) {
        let Some(from) = self.layout_of(id) else {
            return;
        };
        if self.is_alone(id) {
            let to = if index > from { index - 1 } else { index };
            self.move_layout(from, to.min(self.layouts.len() - 1));
            return;
        }
        let before = self.layouts.get(index).map(|l| l.id);
        let Some(space) = self.take_space(id) else {
            return;
        };
        let shown = self.layouts[self.active].id;
        let at = before
            .and_then(|b| self.index_of(b))
            .unwrap_or(self.layouts.len());
        let layout = Layout {
            id: LayoutId(self.id()),
            name: None,
            focus: id,
            spaces: Tree::Leaf(space),
            zoom: None,
        };
        self.layouts.insert(at, layout);
        self.active = self.index_of(shown).unwrap_or(at);
    }

    /// Moves a space to the right of `target`, in `target`'s layout. When the shown layout was
    /// only that space, `target`'s layout is shown with the space focused.
    pub fn drop_beside(&mut self, id: SpaceId, target: SpaceId) {
        if id == target || self.layout_of(target).is_none() {
            return;
        }
        let follow = matches!(&self.layouts[self.active].spaces, Tree::Leaf(s) if s.id == id);
        let Some(space) = self.take_space(id) else {
            return;
        };
        let i = self.layout_of(target).expect("target is in a layout");
        let layout = &mut self.layouts[i];
        let _ = layout.spaces.insert_beside(target, space, Axis::Row, false);
        layout.zoom = None;
        if follow {
            layout.focus = id;
            self.active = i;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Three layouts of one space each, and their space ids.
    fn three() -> (Model, [SpaceId; 3]) {
        let (mut m, _) = Model::new();
        m.new_layout();
        m.new_layout();
        m.active = 0;
        let ids = [0, 1, 2].map(|i| m.layouts[i].spaces.first().id);
        (m, ids)
    }

    fn spaces(m: &Model) -> Vec<Vec<SpaceId>> {
        m.layouts
            .iter()
            .map(|l| l.spaces.leaves().iter().map(|s| s.id).collect())
            .collect()
    }

    #[test]
    fn dropping_in_the_window_splits_the_shown_layout_and_drops_the_emptied_one() {
        let (mut m, [a, b, c]) = three();
        m.drop_in_window(b, WindowDrop::Space(a, Side::Right));
        assert_eq!(spaces(&m), [vec![a, b], vec![c]]);
        assert_eq!((m.active, m.layout().focus), (0, b));

        m.drop_in_window(c, WindowDrop::Edge(Side::Top));
        assert_eq!(spaces(&m), [vec![c, a, b]]);
        let Tree::Branch { axis, .. } = &m.layout().spaces else {
            panic!("branch")
        };
        assert_eq!(*axis, Axis::Column);
        assert!(m.is_valid());
    }

    #[test]
    fn dropping_a_space_on_itself_or_the_only_space_does_nothing() {
        let (mut m, [a, b, _]) = three();
        m.drop_in_window(a, WindowDrop::Edge(Side::Left));
        m.drop_in_window(a, WindowDrop::Space(a, Side::Left));
        m.drop_in_window(a, WindowDrop::Space(b, Side::Left));
        assert_eq!(m.layouts.len(), 3);
    }

    #[test]
    fn a_space_dropped_in_the_sidebar_gets_its_own_layout_there() {
        let (mut m, [a, b, c]) = three();
        m.drop_in_window(b, WindowDrop::Space(a, Side::Right));
        m.drop_as_layout(a, 2);
        assert_eq!(spaces(&m), [vec![b], vec![c], vec![a]]);
        assert_eq!(m.active, 0);
        assert!(m.is_valid());
    }

    #[test]
    fn a_lone_space_dropped_in_the_sidebar_moves_its_layout() {
        let (mut m, [a, b, c]) = three();
        m.drop_as_layout(a, 3);
        assert_eq!(spaces(&m), [vec![b], vec![c], vec![a]]);
        assert_eq!(m.active, 2);
        m.drop_as_layout(c, 0);
        assert_eq!(spaces(&m), [vec![c], vec![b], vec![a]]);
    }

    #[test]
    fn dropping_the_shown_lone_space_beside_another_follows_it() {
        let (mut m, [a, b, c]) = three();
        m.drop_beside(a, c);
        assert_eq!(spaces(&m), [vec![b], vec![c, a]]);
        assert_eq!((m.active, m.layout().focus), (1, a));

        m.drop_beside(b, a);
        assert_eq!(spaces(&m), [vec![c, a, b]]);
        assert_eq!((m.active, m.layout().focus), (0, a));
        assert!(m.is_valid());
    }

    #[test]
    fn taking_the_zoomed_space_clears_the_zoom() {
        let (mut m, [a, b, _]) = three();
        m.drop_in_window(b, WindowDrop::Space(a, Side::Right));
        m.layouts[0].focus = b;
        m.toggle_zoom();
        m.drop_as_layout(b, 0);
        assert_eq!(m.layouts[1].zoom, None);
        assert_eq!(m.layouts[1].focus, a);
    }
}
