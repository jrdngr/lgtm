//! Dragging a space by a sidebar row or its title: into a slot, onto another space's title to
//! swap with it, beside a slot, or along an edge of the window; or to a new place in the
//! sidebar list.

use std::collections::HashMap;

use gpui::{
    AnyElement, App, AppContext, Bounds, Context, DragMoveEvent, FontWeight, InteractiveElement,
    IntoElement, ParentElement, Pixels, Point, Render, Stateful, StatefulInteractiveElement,
    Styled, Window, div, px, rgb, rgba,
};

use super::Workspace;
use crate::model::{Side, SlotId, SpaceId, Tree, WindowDrop};
use crate::theme;

/// How close to the window's edge a drop splits the whole layout.
const EDGE: f32 = 26.;
/// The share of a slot's width or height, from each side, where a drop splits it.
const SIDE: f32 = 0.25;

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

#[derive(Clone, Copy, Debug, PartialEq)]
enum Target {
    Window(WindowDrop),
    /// Before the space at this place in the sidebar list, or last.
    List(usize),
}

/// Where things are while a space is dragged, and where it would land.
#[derive(Default)]
pub(super) struct SpaceDrop {
    dragging: Option<SpaceId>,
    target: Option<Target>,
    main: Option<Bounds<Pixels>>,
    sidebar: Option<Bounds<Pixels>>,
    slots: HashMap<SlotId, Bounds<Pixels>>,
    rows: HashMap<SpaceId, Bounds<Pixels>>,
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

/// Records a space's sidebar row.
pub(super) fn track_row<E: InteractiveElement>(
    el: E,
    space: SpaceId,
    cx: &mut Context<Workspace>,
) -> E {
    track(el, cx, move |d, b| {
        d.rows.insert(space, b);
    })
}

pub(super) fn track_slot<E: InteractiveElement>(
    el: E,
    slot: SlotId,
    cx: &mut Context<Workspace>,
) -> E {
    track(el, cx, move |d, b| {
        d.slots.insert(slot, b);
    })
}

fn f(p: Pixels) -> f32 {
    p.as_f32()
}

/// The side of `b` nearest `p`, measured as a share of its width or height, when it's within
/// `SIDE` of it; none in the middle.
fn nearest_side(b: &Bounds<Pixels>, p: Point<Pixels>) -> Option<Side> {
    let (x, y) = (f(p.x - b.origin.x), f(p.y - b.origin.y));
    let (w, h) = (f(b.size.width), f(b.size.height));
    [
        (x / w, Side::Left),
        ((w - x) / w, Side::Right),
        (y / h, Side::Top),
        ((h - y) / h, Side::Bottom),
    ]
    .into_iter()
    .filter(|(share, _)| *share < SIDE)
    .min_by(|a, b| a.0.total_cmp(&b.0))
    .map(|(_, s)| s)
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
        let layout = self.model.layout();
        if d.main.is_some_and(|b| b.contains(&p)) {
            let from = layout.slot_of(id);
            let alone = matches!(layout.slots, Tree::Leaf(_));
            // Over another space's title, it swaps with that space.
            let title = d.slots.iter().find(|(s, b)| {
                layout.slots.contains(**s)
                    && b.contains(&p)
                    && f(p.y - b.origin.y) < super::panes::SPACE_TITLE
            });
            if let Some((&slot, _)) = title {
                return (from != Some(slot))
                    .then_some(Target::Window(WindowDrop::Slot(slot, None)));
            }
            if let Some(side) = edge(&d.main?, p) {
                return (!(alone && from.is_some()))
                    .then_some(Target::Window(WindowDrop::Edge(side)));
            }
            let (&slot, b) = d
                .slots
                .iter()
                .find(|(s, b)| layout.slots.contains(**s) && b.contains(&p))?;
            return (from != Some(slot))
                .then(|| Target::Window(WindowDrop::Slot(slot, nearest_side(b, p))));
        }
        if !d.sidebar.is_some_and(|b| b.contains(&p)) {
            return None;
        }
        let spaces = &self.model.spaces;
        let index = spaces
            .iter()
            .position(|s| {
                d.rows
                    .get(&s.id)
                    .is_some_and(|b| p.y < b.origin.y + b.size.height / 2.)
            })
            .unwrap_or(spaces.len());
        let own = spaces.iter().position(|s| s.id == id)?;
        (index != own && index != own + 1).then_some(Target::List(index))
    }

    pub(super) fn space_drop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let d = std::mem::take(&mut self.space_drop);
        if let (Some(id), Some(target)) = (d.dragging, d.target) {
            match target {
                Target::Window(at) => self.model.drop_in_window(id, at),
                Target::List(index) => self.model.move_space(id, index),
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
            Target::Window(WindowDrop::Slot(s, side)) => {
                let b = *d.slots.get(&s)?;
                (side.map_or(b, |side| half(b, side)), false)
            }
            Target::Window(WindowDrop::Edge(side)) => {
                let layout = &self.model.layout().slots;
                let all = d
                    .slots
                    .iter()
                    .filter(|(s, _)| layout.contains(**s))
                    .map(|(_, b)| *b)
                    .reduce(|a, b| a.union(&b))?;
                (half(all, side), false)
            }
            Target::List(index) => {
                let side = d.sidebar?;
                let y = match self.model.spaces.get(index).and_then(|s| d.rows.get(&s.id)) {
                    Some(b) => b.origin.y - px(4.),
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
    fn the_nearest_side_is_measured_by_share_and_the_middle_has_none() {
        assert_eq!(
            nearest_side(&b(), point(px(450.), px(200.))),
            Some(Side::Right)
        );
        assert_eq!(
            nearest_side(&b(), point(px(300.), px(120.))),
            Some(Side::Top)
        );
        assert_eq!(
            nearest_side(&b(), point(px(130.), px(200.))),
            Some(Side::Left)
        );
        assert_eq!(
            nearest_side(&b(), point(px(300.), px(290.))),
            Some(Side::Bottom)
        );
        assert_eq!(nearest_side(&b(), point(px(300.), px(200.))), None);
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
