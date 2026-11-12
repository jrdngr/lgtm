//! Dragging a tab by its pill onto any split in the window: into its tabs, beside it in a
//! new split, or along the side of a larger part of its space.

use std::collections::HashMap;

use gpui::{
    AnyElement, App, AppContext, Bounds, Context, DragMoveEvent, InteractiveElement, IntoElement,
    ParentElement, Pixels, Render, Stateful, StatefulInteractiveElement, Styled, Window, div, px,
    rgb, rgba,
};

use super::Workspace;
use super::space_drag::{edge, half, nearest_side};
use crate::model::{Cell, Side, SplitId, Tab};
use crate::theme;

/// The tab strip's height; a drop on it joins the split's tabs.
pub(super) const TAB_STRIP: f32 = 36.;

/// A tab being dragged.
pub(super) struct TabDrag {
    pub tab: Tab,
    pub label: String,
}

impl Render for TabDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .h(px(26.))
            .px(px(10.))
            .rounded(px(7.))
            .bg(rgb(theme::TAB_ACTIVE))
            .border_1()
            .border_color(rgb(theme::CARD_BORDER))
            .text_size(px(12.5))
            .text_color(rgb(theme::TEXT_STRONG))
            .child(self.label.clone())
    }
}

/// Where a dragged tab lands.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Target {
    /// Into a split's tabs, or beside it in a new split.
    Split(SplitId, Option<Side>),
    /// In a new split along the side of the largest part of the space that split borders.
    Edge(SplitId, Side),
}

/// Where things are while a tab is dragged, and where it would land.
#[derive(Default)]
pub(super) struct TabDrop {
    target: Option<Target>,
    cards: HashMap<SplitId, Bounds<Pixels>>,
}

pub(super) trait DragsTab {
    /// Drags `tab` out from here.
    fn drags_tab(self, tab: Tab, label: String) -> Self;
}

impl DragsTab for Stateful<gpui::Div> {
    fn drags_tab(self, tab: Tab, label: String) -> Self {
        self.on_drag(TabDrag { tab, label }, |d, _, _, cx| {
            cx.new(|_| TabDrag {
                tab: d.tab,
                label: d.label.clone(),
            })
        })
    }
}

/// Records a split's card while a tab is dragged.
pub(super) fn track_card<E: InteractiveElement>(
    el: E,
    split: SplitId,
    cx: &mut Context<Workspace>,
) -> E {
    el.on_drag_move(cx.listener(move |w, ev: &DragMoveEvent<TabDrag>, _, _| {
        w.tab_drop.cards.insert(split, ev.bounds);
    }))
}

impl Workspace {
    /// Works out where the dragged tab would land under the pointer.
    pub(super) fn tab_drag_move(&mut self, ev: &DragMoveEvent<TabDrag>, cx: &mut Context<Self>) {
        let tab = ev.drag(cx).tab;
        let p = ev.event.position;
        let shown: Vec<SplitId> = self
            .model
            .cells()
            .into_iter()
            .filter_map(|(c, _)| match c {
                Cell::Split(s) => Some(s),
                Cell::Slot(_) => None,
            })
            .collect();
        let over = self
            .tab_drop
            .cards
            .iter()
            .find(|(s, b)| shown.contains(s) && b.contains(&p));
        let target = over.and_then(|(&split, b)| {
            let in_strip = (p.y - b.origin.y).as_f32() < TAB_STRIP;
            if !in_strip
                && let Some(side) = edge(b, p)
                && self.edge_bounds(split, side).is_some()
            {
                return Some(Target::Edge(split, side));
            }
            let side = if in_strip { None } else { nearest_side(b, p) };
            let from = self.model.locate(tab)?;
            let alone = self
                .model
                .find_space(from.space)
                .and_then(|s| s.splits.find(from.split))
                .is_some_and(|s| s.tabs.len() == 1);
            (from.split != split || (side.is_some() && !alone))
                .then_some(Target::Split(split, side))
        });
        if self.tab_drop.target != target {
            self.tab_drop.target = target;
            cx.notify();
        }
    }

    /// The bounds of the largest part of `split`'s space that it borders on `side`, when
    /// that's more than the split itself.
    fn edge_bounds(&self, split: SplitId, side: Side) -> Option<Bounds<Pixels>> {
        let space = self
            .model
            .spaces
            .iter()
            .find(|s| s.splits.contains(split))?;
        if space.zoom.is_some() {
            return None;
        }
        let path = space.splits.edge_of(split, side.axis(), side.before())?;
        let leaves = space.splits.at_path(&path)?.leaves();
        if leaves.len() < 2 {
            return None;
        }
        leaves
            .iter()
            .map(|s| self.tab_drop.cards.get(&s.id).copied())
            .collect::<Option<Vec<_>>>()?
            .into_iter()
            .reduce(|a, b| a.union(&b))
    }

    pub(super) fn tab_drop(&mut self, drag: &TabDrag, window: &mut Window, cx: &mut Context<Self>) {
        let d = std::mem::take(&mut self.tab_drop);
        match d.target {
            Some(Target::Split(split, side)) => self.model.move_tab(drag.tab, split, side),
            Some(Target::Edge(split, side)) => self.model.move_tab_to_edge(drag.tab, split, side),
            None => {}
        }
        self.focus_current(window, cx);
    }

    /// Where the dragged tab would land, drawn over the window.
    pub(super) fn render_tab_drop(&mut self, cx: &App) -> Option<AnyElement> {
        if !cx.has_active_drag() {
            if self.tab_drop.target.is_some() {
                self.tab_drop = TabDrop::default();
            }
            return None;
        }
        let b = match self.tab_drop.target? {
            Target::Split(split, side) => {
                let b = *self.tab_drop.cards.get(&split)?;
                side.map_or(b, |side| half(b, side))
            }
            Target::Edge(split, side) => half(self.edge_bounds(split, side)?, side),
        };
        Some(
            div()
                .absolute()
                .left(b.origin.x)
                .top(b.origin.y)
                .w(b.size.width)
                .h(b.size.height)
                .rounded(px(10.))
                .border(px(1.5))
                .border_color(rgb(theme::FOCUS))
                .bg(rgba((theme::FOCUS << 8) | 0x2e))
                .into_any_element(),
        )
    }
}
