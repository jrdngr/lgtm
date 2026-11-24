//! Splits side by side or stacked, with draggable dividers between them.

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, Context, CursorStyle, DragMoveEvent, InteractiveElement, IntoElement,
    ParentElement, Pixels, Render, StatefulInteractiveElement, Styled, Window, div, px,
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

/// A branch's child: its index among the branch's children, its weight, and a fixed length
/// when it's a folded strip that takes no share of the branch.
pub(super) struct Kid {
    pub index: usize,
    pub weight: f32,
    pub fixed: Option<Pixels>,
    pub el: AnyElement,
}

impl Workspace {
    pub(super) fn branch(
        &self,
        axis: Axis,
        children: Vec<Kid>,
        owner: Owner,
        path: Vec<usize>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let row = axis == Axis::Row;
        let count = children.len();
        // Folded strips keep their weights and take a fixed length, so dividers trade length
        // only between the children that share the rest.
        let shown: Vec<usize> = children
            .iter()
            .filter(|k| k.fixed.is_none())
            .map(|k| k.index)
            .collect();
        let fixed: Vec<Option<Pixels>> = children.iter().map(|k| k.fixed).collect();
        // Flex leaves room unfilled when the grow factors sum under 1, as they do when
        // folded siblings keep their weights, so the shown children split it all.
        let total: f32 = children
            .iter()
            .filter(|k| k.fixed.is_none())
            .map(|k| k.weight)
            .sum::<f32>()
            .max(f32::EPSILON);
        let mut items = Vec::with_capacity(count * 2);
        let mut flexible = 0;
        for (i, kid) in children.into_iter().enumerate() {
            let cell = div().min_w_0().min_h_0().flex().child(kid.el);
            items.push(match kid.fixed {
                Some(len) => cell
                    .flex_none()
                    .when(row, |d| d.w(len))
                    .when(!row, |d| d.h(len))
                    .into_any_element(),
                None => cell
                    .flex_grow(kid.weight / total)
                    .flex_basis(px(0.))
                    .into_any_element(),
            });
            if kid.fixed.is_none() {
                flexible += 1;
            }
            if i + 1 >= count {
                continue;
            }
            let gap = div()
                .flex_none()
                .when(row, |d| d.w(px(theme::GAP)).h_full())
                .when(!row, |d| d.h(px(theme::GAP)).w_full());
            if fixed[i].is_some() || fixed[i + 1].is_some() {
                items.push(gap.into_any_element());
                continue;
            }
            let drag = Divider {
                owner,
                path: path.clone(),
                index: flexible - 1,
            };
            items.push(
                gap.id(gpui::ElementId::Name(
                    format!("divider-{owner:?}-{path:?}-{i}").into(),
                ))
                .cursor(if row {
                    CursorStyle::ResizeLeftRight
                } else {
                    CursorStyle::ResizeUpDown
                })
                .on_drag(drag, |_, _, _, cx| cx.new(|_| Nothing))
                .into_any_element(),
            );
        }
        let strips: f32 = fixed.iter().flatten().map(|l| l.as_f32()).sum();
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
                let (from, len) = if row {
                    ((pos.x - b.origin.x).as_f32(), b.size.width.as_f32())
                } else {
                    ((pos.y - b.origin.y).as_f32(), b.size.height.as_f32())
                };
                // Strips before the divider take their length off where it lands.
                let before: f32 = fixed
                    .iter()
                    .scan(0, |seen, f| {
                        let left = *seen <= d.index;
                        if f.is_none() {
                            *seen += 1;
                        }
                        Some((left, f))
                    })
                    .filter_map(|(left, f)| f.filter(|_| left).map(|l| l.as_f32()))
                    .sum();
                let len = (len - strips).max(1.);
                let at = (from - before) / len;
                let min = MIN / len;
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
