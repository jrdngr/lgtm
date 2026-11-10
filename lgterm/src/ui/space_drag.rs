//! Dragging a space: from a sidebar row or its title in the window, into the window beside a
//! space or along an edge, into the sidebar as a layout of its own, or onto another space's row.

use std::collections::HashMap;

use gpui::{
    AnyElement, App, AppContext, Bounds, Context, DragMoveEvent, FontWeight, InteractiveElement,
    IntoElement, ParentElement, Pixels, Point, Render, Stateful, StatefulInteractiveElement,
    Styled, Window, div, px, rgb, rgba,
};

use super::Workspace;
use crate::model::{Side, SpaceId, Tree, WindowDrop};
use crate::theme;

/// How close to the window's edge a drop splits the whole layout.
const EDGE: f32 = 26.;
/// How far into a layout row a drop still lands before it.
const ROW_TOP: f32 = 7.;

/// A space being dragged.
pub(super) struct SpaceDrag {
    pub space: SpaceId,
    pub label: String,
}

impl Render for SpaceDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(28.))
            .px(px(10.))
            .rounded(px(6.))
            .bg(rgb(theme::ROW_SELECTED))
            .border_1()
            .border_color(rgb(theme::CARD_BORDER))
            .text_size(px(13.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(theme::TEXT))
            .child(div().text_color(rgb(theme::ICON)).child("⠿"))
            .child(self.label.clone())
    }
}

/// A sidebar row that takes part in a space drop.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum DropRow {
    /// A layout's own row, with its space when it has only one.
    Layout(usize, Option<SpaceId>),
    /// A space row under a layout of several.
    Space(SpaceId),
    /// Any other row.
    Other(u64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Target {
    Window(WindowDrop),
    Layout(usize),
    Beside(SpaceId),
}

/// Where things are while a space is dragged, and where it would land.
#[derive(Default)]
pub(super) struct SpaceDrop {
    dragging: Option<SpaceId>,
    target: Option<Target>,
    main: Option<Bounds<Pixels>>,
    sidebar: Option<Bounds<Pixels>>,
    spaces: HashMap<SpaceId, Bounds<Pixels>>,
    rows: HashMap<DropRow, Bounds<Pixels>>,
}

pub(super) trait DragsSpace {
    /// Drags `space` out from here.
    fn drags_space(self, space: SpaceId, label: String) -> Self;
}

impl DragsSpace for Stateful<gpui::Div> {
    fn drags_space(self, space: SpaceId, label: String) -> Self {
        self.on_drag(SpaceDrag { space, label }, |d, _, _, cx| {
            cx.new(|_| SpaceDrag {
                space: d.space,
                label: d.label.clone(),
            })
        })
    }
}

/// Records an element's bounds while a space is dragged.
fn track<E: InteractiveElement>(
    el: E,
    cx: &mut Context<Workspace>,
    put: impl Fn(&mut SpaceDrop, Bounds<Pixels>) + 'static,
) -> E {
    el.on_drag_move(
        cx.listener(move |w, ev: &DragMoveEvent<SpaceDrag>, _, _| {
            put(&mut w.space_drop, ev.bounds)
        }),
    )
}

pub(super) fn track_main<E: InteractiveElement>(el: E, cx: &mut Context<Workspace>) -> E {
    track(el, cx, |d, b| d.main = Some(b))
}

pub(super) fn track_sidebar<E: InteractiveElement>(el: E, cx: &mut Context<Workspace>) -> E {
    track(el, cx, |d, b| d.sidebar = Some(b))
}

pub(super) fn track_row<E: InteractiveElement>(
    el: E,
    row: DropRow,
    cx: &mut Context<Workspace>,
) -> E {
    track(el, cx, move |d, b| {
        d.rows.insert(row, b);
    })
}

pub(super) fn track_space<E: InteractiveElement>(
    el: E,
    space: SpaceId,
    cx: &mut Context<Workspace>,
) -> E {
    track(el, cx, move |d, b| {
        d.spaces.insert(space, b);
    })
}

fn f(p: Pixels) -> f32 {
    p.as_f32()
}

/// The side of `b` nearest `p`, measured as a share of its width or height.
fn nearest_side(b: &Bounds<Pixels>, p: Point<Pixels>) -> Side {
    let (x, y) = (f(p.x - b.origin.x), f(p.y - b.origin.y));
    let (w, h) = (f(b.size.width), f(b.size.height));
    [
        (x / w, Side::Left),
        ((w - x) / w, Side::Right),
        (y / h, Side::Top),
        ((h - y) / h, Side::Bottom),
    ]
    .into_iter()
    .min_by(|a, b| a.0.total_cmp(&b.0))
    .map(|(_, s)| s)
    .unwrap_or(Side::Right)
}

/// The side of `b` within `EDGE` of `p`.
fn edge(b: &Bounds<Pixels>, p: Point<Pixels>) -> Option<Side> {
    let (x, y) = (f(p.x - b.origin.x), f(p.y - b.origin.y));
    let (w, h) = (f(b.size.width), f(b.size.height));
    if x < EDGE {
        Some(Side::Left)
    } else if w - x < EDGE {
        Some(Side::Right)
    } else if y < EDGE {
        Some(Side::Top)
    } else if h - y < EDGE {
        Some(Side::Bottom)
    } else {
        None
    }
}

fn half(b: Bounds<Pixels>, side: Side) -> Bounds<Pixels> {
    let mut b = b;
    match side {
        Side::Left => b.size.width /= 2.,
        Side::Right => {
            b.size.width /= 2.;
            b.origin.x += b.size.width;
        }
        Side::Top => b.size.height /= 2.,
        Side::Bottom => {
            b.size.height /= 2.;
            b.origin.y += b.size.height;
        }
    }
    b
}

impl Workspace {
    /// Works out where the dragged space would land under the pointer.
    pub(super) fn space_drag_move(
        &mut self,
        ev: &DragMoveEvent<SpaceDrag>,
        cx: &mut Context<Self>,
    ) {
        let id = ev.drag(cx).space;
        let p = ev.event.position;
        let target = self.space_target(id, p);
        let d = &mut self.space_drop;
        if d.dragging != Some(id) || d.target != target {
            d.dragging = Some(id);
            d.target = target;
            cx.notify();
        }
    }

    fn space_target(&self, id: SpaceId, p: Point<Pixels>) -> Option<Target> {
        let d = &self.space_drop;
        if d.main.is_some_and(|b| b.contains(&p)) {
            let shown = &self.model.layout().spaces;
            if matches!(shown, Tree::Leaf(s) if s.id == id) {
                return None;
            }
            if let Some(side) = edge(&d.main?, p) {
                return Some(Target::Window(WindowDrop::Edge(side)));
            }
            let (&space, b) = d
                .spaces
                .iter()
                .find(|(s, b)| shown.contains(**s) && b.contains(&p))?;
            return (space != id)
                .then(|| Target::Window(WindowDrop::Space(space, nearest_side(b, p))));
        }
        if !d.sidebar.is_some_and(|b| b.contains(&p)) {
            return None;
        }
        let hit = d.rows.iter().find(|(_, b)| b.contains(&p));
        if let Some((row, b)) = hit {
            let space = match *row {
                DropRow::Space(s) => Some(s),
                DropRow::Layout(_, s) if f(p.y - b.origin.y) >= ROW_TOP => s,
                _ => None,
            };
            if let Some(space) = space {
                return (space != id).then_some(Target::Beside(space));
            }
        }
        let mut layouts: Vec<(usize, Bounds<Pixels>)> = d
            .rows
            .iter()
            .filter_map(|(row, b)| match row {
                DropRow::Layout(i, _) => Some((*i, *b)),
                _ => None,
            })
            .collect();
        layouts.sort_by_key(|l| l.0);
        let index = layouts
            .iter()
            .find(|(_, b)| p.y < b.bottom())
            .map_or(self.model.layouts.len(), |l| l.0);
        let own = self
            .model
            .layouts
            .iter()
            .position(|l| l.spaces.contains(id));
        if self.model.is_alone(id) && own.is_some_and(|o| index == o || index == o + 1) {
            return None;
        }
        Some(Target::Layout(index))
    }

    pub(super) fn space_drop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let d = std::mem::take(&mut self.space_drop);
        if let (Some(id), Some(target)) = (d.dragging, d.target) {
            match target {
                Target::Window(at) => self.model.drop_in_window(id, at),
                Target::Layout(index) => self.model.drop_as_layout(id, index),
                Target::Beside(space) => self.model.drop_beside(id, space),
            }
        }
        self.focus_current(window, cx);
    }

    /// The space being dragged, if any.
    pub(super) fn dragged_space(&self, cx: &App) -> Option<SpaceId> {
        if cx.has_active_drag() {
            self.space_drop.dragging
        } else {
            None
        }
    }

    /// Where the dragged space would land, drawn over the window.
    pub(super) fn render_space_drop(&mut self, cx: &App) -> Option<AnyElement> {
        if !cx.has_active_drag() {
            if self.space_drop.dragging.is_some() {
                self.space_drop = SpaceDrop::default();
            }
            return None;
        }
        let d = &self.space_drop;
        let (bounds, line) = match d.target? {
            Target::Window(WindowDrop::Space(s, side)) => (half(*d.spaces.get(&s)?, side), false),
            Target::Window(WindowDrop::Edge(side)) => {
                let shown = &self.model.layout().spaces;
                let all = d
                    .spaces
                    .iter()
                    .filter(|(s, _)| shown.contains(**s))
                    .map(|(_, b)| *b)
                    .reduce(|a, b| a.union(&b))?;
                (half(all, side), false)
            }
            Target::Beside(s) => {
                let row = d
                    .rows
                    .iter()
                    .find(|(r, _)| matches!(r, DropRow::Space(x) | DropRow::Layout(_, Some(x)) if *x == s))?;
                (*row.1, false)
            }
            Target::Layout(index) => {
                let side = d.sidebar?;
                let y = match d
                    .rows
                    .iter()
                    .find(|(r, _)| matches!(r, DropRow::Layout(i, _) if *i == index))
                {
                    Some((_, b)) => b.origin.y - px(4.),
                    None => d.rows.values().map(|b| b.bottom()).max()? + px(3.),
                };
                let b = Bounds::new(
                    gpui::point(side.origin.x + px(8.), y),
                    gpui::size(side.size.width - px(16.), px(2.)),
                );
                (b, true)
            }
        };
        let el = div()
            .absolute()
            .left(bounds.origin.x)
            .top(bounds.origin.y)
            .w(bounds.size.width)
            .h(bounds.size.height);
        Some(if line {
            el.rounded(px(1.)).bg(rgb(theme::FOCUS)).into_any_element()
        } else {
            el.rounded(px(8.))
                .border(px(1.5))
                .border_color(rgb(theme::FOCUS))
                .bg(rgba((theme::FOCUS << 8) | 0x2e))
                .into_any_element()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, size};

    fn b() -> Bounds<Pixels> {
        Bounds::new(point(px(100.), px(100.)), size(px(400.), px(200.)))
    }

    #[test]
    fn the_nearest_side_is_measured_by_share() {
        assert_eq!(nearest_side(&b(), point(px(450.), px(200.))), Side::Right);
        assert_eq!(nearest_side(&b(), point(px(300.), px(120.))), Side::Top);
        assert_eq!(nearest_side(&b(), point(px(130.), px(200.))), Side::Left);
        assert_eq!(nearest_side(&b(), point(px(300.), px(290.))), Side::Bottom);
    }

    #[test]
    fn only_a_band_along_the_edge_is_an_edge() {
        assert_eq!(edge(&b(), point(px(110.), px(200.))), Some(Side::Left));
        assert_eq!(edge(&b(), point(px(300.), px(290.))), Some(Side::Bottom));
        assert_eq!(edge(&b(), point(px(300.), px(200.))), None);
    }

    #[test]
    fn half_keeps_the_side_dropped_on() {
        let r = half(b(), Side::Right);
        assert_eq!((r.origin.x, r.size.width), (px(300.), px(200.)));
        let t = half(b(), Side::Bottom);
        assert_eq!((t.origin.y, t.size.height), (px(200.), px(100.)));
    }
}
