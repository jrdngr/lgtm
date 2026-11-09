//! The main area: the active layout's spaces, each a tree of split cards with tabs.

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, Context, CursorStyle, DragMoveEvent, FontWeight, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Render, StatefulInteractiveElement, Styled, Window,
    div, px, rgb,
};

use super::Workspace;
use super::theme;
use crate::model::{Axis, Space, Split, Tree, tree};

/// Which tree a branch belongs to: a layout's spaces or a space's splits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Owner {
    Layout(u64),
    Space(u64),
}

/// A divider being dragged: the boundary after child `index` of the branch at `path`.
#[derive(Clone, Debug, PartialEq)]
struct Divider {
    owner: Owner,
    path: Vec<usize>,
    index: usize,
}

/// What a divider drag shows under the pointer: nothing.
struct Nothing;

impl Render for Nothing {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

impl Workspace {
    pub(super) fn render_main(&mut self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let layout = self.model.layout();
        let body = match layout.zoom.and_then(|z| {
            layout
                .spaces
                .leaves()
                .into_iter()
                .find_map(|s| s.splits.find(z))
        }) {
            Some(split) => self.card(split, true, cx),
            None => self.space_tree(&layout.spaces, Owner::Layout(layout.id), vec![], cx),
        };
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .pt(px(if self.sidebar { 10. } else { theme::TITLEBAR }))
            .px(px(theme::GAP))
            .pb(px(theme::GAP))
            .child(body)
            .into_any_element()
    }

    fn space_tree(
        &self,
        t: &Tree<Space>,
        owner: Owner,
        path: Vec<usize>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match t {
            Tree::Leaf(space) => self.space(space, cx),
            Tree::Branch { axis, children } => {
                let kids = children
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        let mut path = path.clone();
                        path.push(i);
                        (c.weight, self.space_tree(&c.tree, owner, path, cx))
                    })
                    .collect();
                self.branch(*axis, kids, owner, path, cx)
            }
        }
    }

    fn space(&self, space: &Space, cx: &mut Context<Self>) -> AnyElement {
        div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .gap(px(theme::GAP))
            .child(
                div()
                    .h(px(20.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .px(px(4.))
                    .text_size(px(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT_SPACE))
                    .child(self.space_label(space, cx)),
            )
            .child(div().flex_1().min_h_0().flex().child(self.split_tree(
                &space.splits,
                Owner::Space(space.id),
                vec![],
                cx,
            )))
            .into_any_element()
    }

    fn split_tree(
        &self,
        t: &Tree<Split>,
        owner: Owner,
        path: Vec<usize>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match t {
            Tree::Leaf(split) => {
                let focused = split.id == self.model.split().id;
                self.card(split, focused, cx)
            }
            Tree::Branch { axis, children } => {
                let kids = children
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        let mut path = path.clone();
                        path.push(i);
                        (c.weight, self.split_tree(&c.tree, owner, path, cx))
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
                        .and_then(|l| l.spaces.branch_mut(&here))
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

    fn card(&self, split: &Split, focused: bool, cx: &mut Context<Self>) -> AnyElement {
        let split_id = split.id;
        let tabs = split.tabs.iter().enumerate().map(|(i, &term)| {
            let active = i == split.active;
            div()
                .id(("tab", term))
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
                .child(self.term_label(term, cx))
                .when(active, |d| {
                    d.child(
                        div()
                            .id(("close", term))
                            .text_size(px(13.))
                            .text_color(rgb(theme::ICON))
                            .child("×")
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(move |w, _, window, cx| {
                                cx.stop_propagation();
                                w.close(term, window, cx)
                            })),
                    )
                })
                .on_click(cx.listener(move |w, _, window, cx| {
                    w.model.focus_term(term);
                    w.focus_current(window, cx);
                }))
        });
        let plus = div()
            .id(("new-tab", split_id))
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
                w.apply(window, cx, |m| Some(m.new_tab()));
            }));
        let view = self.terms.get(&split.term()).map(|t| t.view.clone());
        div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(rgb(theme::CARD))
            .border_1()
            .border_color(rgb(if focused {
                theme::FOCUS
            } else {
                theme::CARD_BORDER
            }))
            .rounded(px(10.))
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |w, _, window, cx| {
                    if w.model.split().id != split_id {
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
                    .children(tabs)
                    .child(plus),
            )
            .child(div().flex_1().min_h_0().children(view))
            .into_any_element()
    }
}
