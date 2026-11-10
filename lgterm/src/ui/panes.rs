//! The main area: the shown layout's slots, each empty or a space in its color holding a tree
//! of split cards with tabs.

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, Context, CursorStyle, DragMoveEvent, FontWeight, InteractiveElement,
    IntoElement, MouseButton, MouseDownEvent, ParentElement, Render, StatefulInteractiveElement,
    StyleRefinement, Styled, Window, div, px, rgb, rgba,
};

use super::menu::For;
use super::space_drag::{self, DragsSpace};
use super::{MovesWindow, Workspace, badge, expand};
use crate::ipc::Place;
use crate::model::{
    Axis, LayoutId, Rect, Slot, SlotId, Space, SpaceId, Split, SplitId, Tab, Tree, tree,
};
use crate::theme;

/// A space's title height.
const SPACE_TITLE: f32 = 26.;
/// The tinted border around a space's splits.
const SPACE_PAD: f32 = 5.;

/// Which tree a branch belongs to: a layout's slots or a space's splits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Owner {
    Layout(LayoutId),
    Space(SpaceId),
}

/// A divider being dragged: the boundary after child `index` of the branch at `path`.
#[derive(Clone, Debug, PartialEq)]
struct Divider {
    owner: Owner,
    path: Vec<usize>,
    index: usize,
}

/// What a divider drag shows under the pointer: nothing.
pub(super) struct Nothing;

impl Render for Nothing {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

impl Workspace {
    pub(super) fn render_main(&mut self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let layout = self.model.layout();
        let expanded = layout
            .expanded
            .and_then(|e| Some((layout.slot_of(e)?, self.model.find_space(e)?)));
        let body = match expanded {
            Some((slot, space)) => {
                let focused = slot == layout.focus;
                let el = self.space(space, focused, true, cx);
                space_drag::track_slot(el, slot, cx).into_any_element()
            }
            None => {
                let top: Vec<SlotId> = layout
                    .slots
                    .rects(Rect::UNIT)
                    .into_iter()
                    .filter(|(_, r)| r.y == 0.)
                    .map(|(id, _)| id)
                    .collect();
                self.slot_tree(&layout.slots, &top, Owner::Layout(layout.id), vec![], cx)
            }
        };
        let titlebar = div()
            .id("main-titlebar")
            .absolute()
            .top_0()
            .left_0()
            .w_full()
            .h(px(theme::TITLEBAR))
            .moves_window();
        let main = div()
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .pt(px(if self.sidebar { 10. } else { theme::TITLEBAR }))
            .px(px(theme::GAP))
            .pb(px(theme::GAP))
            .child(titlebar)
            .child(body);
        space_drag::track_main(main, cx).into_any_element()
    }

    /// The slots; `top` are those along the window's top edge, whose titles move the window.
    fn slot_tree(
        &self,
        t: &Tree<Slot>,
        top: &[SlotId],
        owner: Owner,
        path: Vec<usize>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match t {
            Tree::Leaf(slot) => {
                let focused = slot.id == self.model.layout().focus;
                let space = slot.space.and_then(|s| self.model.find_space(s));
                let el = match space {
                    Some(space) => self.space(space, focused, top.contains(&slot.id), cx),
                    None => self.empty_slot(slot.id, focused, cx),
                };
                space_drag::track_slot(el, slot.id, cx).into_any_element()
            }
            Tree::Branch { axis, children } => {
                let kids = children
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        let mut path = path.clone();
                        path.push(i);
                        (c.weight, self.slot_tree(&c.tree, top, owner, path, cx))
                    })
                    .collect();
                self.branch(*axis, kids, owner, path, cx)
            }
        }
    }

    fn empty_slot(
        &self,
        slot: SlotId,
        focused: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(("empty-slot", slot.0))
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .pt(px(SPACE_TITLE))
            .child(
                div()
                    .size_full()
                    .bg(rgb(theme::CARD))
                    .border_1()
                    .border_color(rgb(if focused {
                        theme::FOCUS
                    } else {
                        theme::CARD_BORDER
                    }))
                    .rounded(px(10.)),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |w, _, window, cx| {
                    cx.stop_propagation();
                    w.apply(window, cx, |m| m.focus_slot(slot));
                }),
            )
    }

    /// A space in its color: a tinted title and background around its splits, brighter when
    /// focused.
    fn space(
        &self,
        space: &Space,
        focused: bool,
        top: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let label = self.space_label(space);
        let color = theme::space_color(space.color);
        let id = space.id;
        let expanded = self.model.layout().expanded == Some(id);
        let title = div()
            .id(("space-title", space.id.0))
            .h(px(SPACE_TITLE))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(6.))
            .px(px(10.))
            .rounded_t(px(12.))
            .bg(rgba(theme::tint(color, if focused { 0x66 } else { 0x33 })))
            .text_size(px(12.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(if focused { theme::TEXT_STRONG } else { color }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |w, ev: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    w.apply(window, cx, |m| m.show_space(id));
                    if top {
                        if ev.click_count == 2 {
                            window.titlebar_double_click();
                        } else {
                            window.start_window_move();
                        }
                    }
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |w, ev: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    w.open_menu(For::Space(id), ev.position, window, cx)
                }),
            )
            .map(|d| match self.space_number(id) {
                Some(n) => d.child(badge::chip(n)),
                None => d.child(
                    div()
                        .id(("space-grip", space.id.0))
                        .cursor_grab()
                        .text_size(px(11.))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child("⠿")
                        .drags_space(space.id, label.clone()),
                ),
            })
            .child(div().flex_1().min_w_0().overflow_hidden().child(label))
            .child(expand::button(
                ("expand-space", id.0),
                expanded,
                cx.listener(move |w, _, window, cx| w.apply(window, cx, |m| m.toggle_expand(id))),
            ));
        let focused_split = focused.then_some(space.focus);
        let zoomed = space.zoom.and_then(|z| space.splits.find(z));
        let splits = match zoomed {
            Some(split) => {
                let outline = (focused_split == Some(split.id)).then_some(space.color);
                self.card(split, outline, cx)
            }
            None => self.split_tree(
                &space.splits,
                space.color,
                focused_split,
                Owner::Space(space.id),
                vec![],
                cx,
            ),
        };
        div()
            .id(("space", space.id.0))
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .relative()
            .rounded(px(12.))
            .bg(rgba(theme::tint(color, if focused { 0x38 } else { 0x1c })))
            .when(expanded, |d| d.border_2().border_color(rgb(color)))
            .when(self.dragged_space(cx) == Some(space.id), |d| {
                d.opacity(0.35)
            })
            .child(title)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .p(px(SPACE_PAD))
                    .child(splits),
            )
            .children(self.render_toasts(Some(id), cx))
    }

    #[allow(clippy::too_many_arguments)]
    fn split_tree(
        &self,
        t: &Tree<Split>,
        color: usize,
        focused: Option<SplitId>,
        owner: Owner,
        path: Vec<usize>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match t {
            Tree::Leaf(split) => {
                let outline = (Some(split.id) == focused).then_some(color);
                self.card(split, outline, cx)
            }
            Tree::Branch { axis, children } => {
                let kids = children
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        let mut path = path.clone();
                        path.push(i);
                        (
                            c.weight,
                            self.split_tree(&c.tree, color, focused, owner, path, cx),
                        )
                    })
                    .collect();
                self.branch(*axis, kids, owner, path, cx)
            }
        }
    }

    /// Children side by side or stacked, with draggable dividers between them.
    fn branch(
        &self,
        axis: Axis,
        children: Vec<(f32, AnyElement)>,
        owner: Owner,
        path: Vec<usize>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let row = axis == Axis::Row;
        let count = children.len();
        let mut items = Vec::with_capacity(count * 2);
        for (i, (weight, child)) in children.into_iter().enumerate() {
            items.push(
                div()
                    .flex_grow(weight)
                    .flex_basis(px(0.))
                    .min_w_0()
                    .min_h_0()
                    .flex()
                    .child(child)
                    .into_any_element(),
            );
            if i + 1 < count {
                let drag = Divider {
                    owner,
                    path: path.clone(),
                    index: i,
                };
                items.push(
                    div()
                        .id(gpui::ElementId::Name(
                            format!("divider-{owner:?}-{path:?}-{i}").into(),
                        ))
                        .flex_none()
                        .when(row, |d| d.w(px(theme::GAP)).h_full())
                        .when(!row, |d| d.h(px(theme::GAP)).w_full())
                        .cursor(if row {
                            CursorStyle::ResizeLeftRight
                        } else {
                            CursorStyle::ResizeUpDown
                        })
                        .on_drag(drag, |_, _, _, cx| cx.new(|_| Nothing))
                        .into_any_element(),
                );
            }
        }
        let here = path;
        div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .when(!row, |d| d.flex_col())
            .on_drag_move(cx.listener(move |w, ev: &DragMoveEvent<Divider>, _, cx| {
                let d = ev.drag(cx);
                if d.owner != owner || d.path != here {
                    return;
                }
                let b = ev.bounds;
                let pos = ev.event.position;
                let (at, len) = if row {
                    ((pos.x - b.origin.x) / b.size.width, b.size.width)
                } else {
                    ((pos.y - b.origin.y) / b.size.height, b.size.height)
                };
                let min = 120. / len.as_f32().max(1.);
                let index = d.index;
                let moved = match owner {
                    Owner::Layout(id) => w
                        .model
                        .find_layout_mut(id)
                        .and_then(|l| l.slots.branch_mut(&here))
                        .map(|c| tree::move_boundary(c, index, at, min)),
                    Owner::Space(id) => w
                        .model
                        .find_space_mut(id)
                        .and_then(|s| s.splits.branch_mut(&here))
                        .map(|c| tree::move_boundary(c, index, at, min)),
                };
                if moved.is_some() {
                    cx.notify();
                }
            }))
            .children(items)
            .into_any_element()
    }

    /// A split's card; `outline` is the space color when it's the focused split.
    fn card(&self, split: &Split, outline: Option<usize>, cx: &mut Context<Self>) -> AnyElement {
        let split_id = split.id;
        let zoomed = self.model.spaces.iter().any(|s| s.zoom == Some(split_id));
        let numbered = outline.is_some() && self.held == super::Held::Ctrl;
        let tabs = split.tabs.iter().enumerate().map(|(i, &tab)| {
            let active = i == split.active;
            let key = match tab {
                Tab::Term(t) => t.0,
                Tab::Messages => 0,
            };
            div()
                .id(("tab", key))
                .flex()
                .items_center()
                .gap(px(7.))
                .h(px(26.))
                .px(px(10.))
                .rounded(px(7.))
                .text_size(px(12.5))
                .whitespace_nowrap()
                .border_1()
                .border_color(gpui::transparent_black())
                .text_color(rgb(theme::TEXT_MUTED))
                .when(active, |d| {
                    d.bg(rgb(theme::TAB_ACTIVE))
                        .border_color(rgb(theme::CARD_BORDER))
                        .text_color(rgb(theme::TEXT_STRONG))
                })
                .when(numbered && i < 9, |d| {
                    d.child(badge::chip(i + 1).ml(px(-3.)))
                })
                .child(self.term_label(tab))
                .children(
                    tab.term()
                        .and_then(|t| self.agents.status(t))
                        .map(|s| self.mark(s)),
                )
                .when(active, |d| {
                    d.child(
                        div()
                            .id(("close", key))
                            .text_size(px(13.))
                            .text_color(rgb(theme::ICON))
                            .child("×")
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(move |w, _, window, cx| {
                                cx.stop_propagation();
                                w.close(tab, window, cx)
                            })),
                    )
                })
                .on_click(cx.listener(move |w, _, window, cx| {
                    w.model.focus_tab(tab);
                    w.focus_current(window, cx);
                }))
        });
        let plus = div()
            .id(("new-tab", split_id.0))
            .w(px(26.))
            .h(px(26.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(6.))
            .text_size(px(16.))
            .text_color(rgb(theme::ICON))
            .child("+")
            .on_click(cx.listener(move |w, _, window, cx| {
                w.model.focus_split(split_id);
                let _ = w.open(None, Place::Tab, None, &[], true, window, cx);
            }));
        let body = match split.tab() {
            Tab::Messages => Some(
                div()
                    .size_full()
                    .track_focus(&self.messages_focus)
                    .child(self.render_messages())
                    .into_any_element(),
            ),
            Tab::Term(t) => self.terms.get(&t).map(|t| {
                t.view
                    .clone()
                    .cached(StyleRefinement::default().size_full())
                    .into_any_element()
            }),
        };
        div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(rgb(theme::CARD))
            .map(|d| if zoomed { d.border_2() } else { d.border_1() })
            .border_color(rgb(outline.map_or(theme::CARD_BORDER, theme::space_color)))
            .rounded(px(10.))
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |w, _, window, cx| {
                    cx.stop_propagation();
                    if w.model.split().map(|s| s.id) != Some(split_id) {
                        w.model.focus_split(split_id);
                        w.focus_current(window, cx);
                    }
                }),
            )
            .child(
                div()
                    .h(px(36.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(3.))
                    .px(px(6.))
                    .border_b_1()
                    .border_color(rgb(theme::TAB_STRIP_BORDER))
                    .overflow_hidden()
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |w, ev: &MouseDownEvent, window, cx| {
                            w.open_menu(For::Tabs(split_id), ev.position, window, cx)
                        }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .items_center()
                            .gap(px(3.))
                            .overflow_hidden()
                            .children(tabs)
                            .child(plus),
                    )
                    .child(expand::button(
                        ("zoom-split", split_id.0),
                        zoomed,
                        cx.listener(move |w, _, window, cx| {
                            w.model.focus_split(split_id);
                            w.apply(window, cx, |m| m.toggle_zoom())
                        }),
                    )),
            )
            .child(div().flex_1().min_h_0().children(body))
            .into_any_element()
    }
}
