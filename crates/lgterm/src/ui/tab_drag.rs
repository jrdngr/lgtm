//! Dragging tabs onto any split in the window: between its tabs, beside it in a new split, or
//! along the side of a larger part of its space. A pill drags its tab, or every picked tab when
//! it's picked; a split's tab strip drags all its tabs.

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

/// Tabs being dragged, in the order they land.
pub(super) struct TabDrag {
    pub tabs: Vec<Tab>,
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
    /// Into a split's tabs, before the one at the index.
    Tabs(SplitId, usize),
    /// Beside a split in a new split.
    Split(SplitId, Side),
    /// In a new split along the side of the largest part of the space that split borders.
    Edge(SplitId, Side),
}

/// Where things are while a tab is dragged, and where it would land.
#[derive(Default)]
pub(super) struct TabDrop {
    target: Option<Target>,
    cards: HashMap<SplitId, Bounds<Pixels>>,
    pills: HashMap<Tab, Bounds<Pixels>>,
}

pub(super) trait DragsTab {
    /// Drags `tabs` out from here.
    fn drags_tabs(self, tabs: Vec<Tab>, label: String) -> Self;
}

impl DragsTab for Stateful<gpui::Div> {
    fn drags_tabs(self, tabs: Vec<Tab>, label: String) -> Self {
        self.on_drag(TabDrag { tabs, label }, |d, _, _, cx| {
            cx.new(|_| TabDrag {
                tabs: d.tabs.clone(),
                label: d.label.clone(),
            })
        })
    }
}

/// Records a tab's pill while a tab is dragged.
pub(super) fn track_pill(
    el: Stateful<gpui::Div>,
    tab: Tab,
    cx: &mut Context<Workspace>,
) -> Stateful<gpui::Div> {
    el.on_drag_move(cx.listener(move |w, ev: &DragMoveEvent<TabDrag>, _, _| {
        w.tab_drop.pills.insert(tab, ev.bounds);
    }))
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
        let tabs = ev.drag(cx).tabs.clone();
        let Some(&tab) = tabs.first() else { return };
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
            let from = self.model.locate(tab)?;
            if in_strip {
                let at = self
                    .pills_of(split)
                    .iter()
                    .filter(|(_, b)| b.center().x < p.x)
                    .count();
                let same = tabs.len() == 1
                    && from.split == split
                    && (at == from.tab || at == from.tab + 1);
                return (!same).then_some(Target::Tabs(split, at));
            }
            let side = nearest_side(b, p)?;
            let whole = self.split_tabs(split).iter().all(|t| tabs.contains(t));
            (!whole).then_some(Target::Split(split, side))
        });
        if self.tab_drop.target != target {
            self.tab_drop.target = target;
            cx.notify();
        }
    }

    /// Picks a tab to drag with the others picked, or unpicks it.
    pub(super) fn toggle_pick(&mut self, tab: Tab, cx: &mut Context<Self>) {
        match self.picked.iter().position(|t| *t == tab) {
            Some(i) => {
                self.picked.remove(i);
            }
            None => self.picked.push(tab),
        }
        cx.notify();
    }

    /// What a drag of `tabs` shows: their names.
    pub(super) fn drag_label(&self, tabs: &[Tab]) -> String {
        tabs.iter()
            .map(|t| self.term_label(*t))
            .collect::<Vec<_>>()
            .join(" · ")
    }

    /// A split's tabs in order.
    fn split_tabs(&self, split: SplitId) -> Vec<Tab> {
        self.model
            .spaces
            .iter()
            .find_map(|s| s.splits.find(split))
            .map(|s| s.tabs.clone())
            .unwrap_or_default()
    }

    /// A split's tabs in order, with their pills' bounds.
    fn pills_of(&self, split: SplitId) -> Vec<(Tab, Bounds<Pixels>)> {
        self.model
            .spaces
            .iter()
            .find_map(|s| s.splits.find(split))
            .map(|s| {
                s.tabs
                    .iter()
                    .filter_map(|t| Some((*t, *self.tab_drop.pills.get(t)?)))
                    .collect()
            })
            .unwrap_or_default()
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
        if let Some(target) = d.target {
            self.move_tabs(&drag.tabs, target);
            self.picked.clear();
        }
        self.focus_current(window, cx);
    }

    /// Moves `tabs` to `target`, keeping their order; ones that start a new split take the rest
    /// along.
    fn move_tabs(&mut self, tabs: &[Tab], target: Target) {
        let Some((&first, rest)) = tabs.split_first() else {
            return;
        };
        match target {
            Target::Tabs(split, at) => {
                let anchor = self
                    .split_tabs(split)
                    .into_iter()
                    .skip(at)
                    .find(|t| !tabs.contains(t));
                for &tab in tabs {
                    let at = anchor
                        .and_then(|a| self.split_tabs(split).iter().position(|t| *t == a))
                        .unwrap_or(usize::MAX);
                    self.model.move_tab_to(tab, split, at);
                }
                self.model.focus_tab(first);
                return;
            }
            Target::Split(split, side) => self.model.move_tab(first, split, Some(side)),
            Target::Edge(split, side) => self.model.move_tab_to_edge(first, split, side),
        }
        let Some(to) = self.model.locate(first).map(|p| p.split) else {
            return;
        };
        for &tab in rest {
            self.model.move_tab_to(tab, to, usize::MAX);
        }
        self.model.focus_tab(first);
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
            Target::Tabs(split, at) => return self.render_tab_slot(split, at),
            Target::Split(split, side) => half(*self.tab_drop.cards.get(&split)?, side),
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

    /// A bar where the dragged tab would go among a split's tabs.
    fn render_tab_slot(&self, split: SplitId, at: usize) -> Option<AnyElement> {
        let pills = self.pills_of(split);
        let (x, b) = match pills.get(at) {
            Some((_, b)) => (b.origin.x - px(2.5), *b),
            None => {
                let (_, b) = pills.last()?;
                (b.origin.x + b.size.width + px(0.5), *b)
            }
        };
        Some(
            div()
                .absolute()
                .left(x)
                .top(b.origin.y)
                .w(px(2.))
                .h(b.size.height)
                .rounded(px(1.))
                .bg(rgb(theme::FOCUS))
                .into_any_element(),
        )
    }
}
