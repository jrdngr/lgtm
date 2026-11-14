//! Splits side by side or stacked, with draggable dividers between them.

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, Context, CursorStyle, DragMoveEvent, InteractiveElement, IntoElement,
    ParentElement, Render, StatefulInteractiveElement, Styled, Window, div, px,
};

use super::Workspace;
use crate::model::{Axis, LayoutId, SpaceId, tree};
use crate::theme;

/// The smallest a split or slot gets when a divider is dragged.
const MIN: f32 = 120.;

/// Which tree a branch belongs to: a layout's slots or a space's splits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Owner {
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
    /// Children side by side or stacked, with draggable dividers between them. Each child is
    /// its index among the branch's children, its weight and its element; the children shown
    /// share the length by weight.
    pub(super) fn branch(
        &self,
        axis: Axis,
        children: Vec<(usize, f32, AnyElement)>,
        owner: Owner,
        path: Vec<usize>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let row = axis == Axis::Row;
        let count = children.len();
        let shown: Vec<usize> = children.iter().map(|(i, ..)| *i).collect();
        let mut items = Vec::with_capacity(count * 2);
        for (i, (_, weight, child)) in children.into_iter().enumerate() {
            items.push(
                div()
                    .min_w_0()
                    .min_h_0()
                    .flex()
                    .flex_grow(weight)
                    .flex_basis(px(0.))
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
                let min = MIN / len.as_f32().max(1.);
                let index = d.index;
                let moved = match owner {
                    Owner::Layout(id) => w
                        .model
                        .find_layout_mut(id)
                        .and_then(|l| l.slots.branch_mut(&here))
                        .map(|c| tree::move_boundary(c, &shown, index, at, min)),
                    Owner::Space(id) => w
                        .model
                        .find_space_mut(id)
                        .and_then(|s| s.splits.branch_mut(&here))
                        .map(|c| tree::move_boundary(c, &shown, index, at, min)),
                };
                if moved.is_some() {
                    cx.notify();
                }
            }))
            .children(items)
            .into_any_element()
    }
}
