//! The layout box at the bottom of the sidebar: every layout as a map of its slots, to pick,
//! add, rename, delete, and drag into a new order. Also the list of layouts shown over the
//! window while ⌃⌘ is held.

use std::collections::HashMap;

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, Bounds, ClickEvent, Context, DragMoveEvent, FontWeight,
    InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement, Pixels, Render,
    StatefulInteractiveElement, Styled, Window, div, px, rgb,
};

use super::menu::For;
use super::rename::Target;
use super::{Held, Workspace, badge};
use crate::model::{Layout, LayoutId, Rect};
use crate::theme;

/// A layout being dragged to a new place in the list.
pub(super) struct LayoutDrag {
    layout: LayoutId,
    label: String,
}

impl Render for LayoutDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .h(px(28.))
            .px(px(10.))
            .rounded(px(6.))
            .bg(rgb(theme::ROW_SELECTED))
            .border_1()
            .border_color(rgb(theme::CARD_BORDER))
            .text_size(px(13.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(theme::TEXT))
            .child(self.label.clone())
    }
}

/// The rows' places while a layout is dragged, and where it would land.
#[derive(Default)]
pub(super) struct LayoutDrop {
    rows: HashMap<LayoutId, Bounds<Pixels>>,
    dragging: Option<LayoutId>,
    /// Before the layout at this place in the list, or last.
    target: Option<usize>,
}

impl Workspace {
    pub(super) fn layout_box(&self, icons: bool, cx: &mut Context<Self>) -> AnyElement {
        let plus = div()
            .id("new-layout")
            .text_size(px(16.))
            .cursor_pointer()
            .child("+")
            .on_click(cx.listener(|w, _, window, cx| w.new_layout(window, cx)));
        let header = div()
            .flex()
            .items_center()
            .pt(px(8.))
            .pb(px(6.))
            .text_size(px(11.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(theme::TEXT_MUTED))
            .when(icons, |d| d.justify_center())
            .when(!icons, |d| {
                d.justify_between().pl(px(20.)).pr(px(18.)).child("LAYOUTS")
            })
            .child(plus);
        let rows = (0..self.model.layouts.len()).map(|i| self.layout_row(i, icons, cx));
        div()
            .id("layout-box")
            .flex_none()
            .flex()
            .flex_col()
            .pb(px(8.))
            .border_t_1()
            .border_color(rgb(theme::SIDEBAR_BORDER))
            .child(header)
            .child(
                div()
                    .id("layout-rows")
                    .max_h(px(220.))
                    .overflow_y_scroll()
                    // Room for the drop line above the first row and below the last.
                    .py(px(2.))
                    .flex()
                    .flex_col()
                    .when(icons, |d| d.items_center().gap(px(6.)))
                    .when(!icons, |d| d.gap(px(1.)).px(px(8.)))
                    .children(rows),
            )
            .on_drag_move(
                cx.listener(|w, ev: &DragMoveEvent<LayoutDrag>, _, cx| w.layout_drag_move(ev, cx)),
            )
            .on_drop(cx.listener(|w, d: &LayoutDrag, window, cx| {
                if let Some(to) = w.layout_drop.target.take() {
                    w.apply(window, cx, |m| m.move_layout(d.layout, to));
                }
            }))
            .into_any_element()
    }

    fn layout_row(&self, i: usize, icons: bool, cx: &mut Context<Self>) -> AnyElement {
        let layouts = &self.model.layouts;
        let layout = &layouts[i];
        let id = layout.id;
        let on = i == self.model.active;
        let label = self.layout_label(i);
        let dragging = self.dragged_layout(cx) == Some(id);
        let target = self.dragged_layout(cx).and(self.layout_drop.target);
        let chip = (self.held == Held::CmdCtrl && i < 9).then(|| badge::chip(i + 1));
        let line = |d: gpui::Div| {
            d.absolute()
                .left_0()
                .right_0()
                .h(px(2.))
                .bg(rgb(theme::FOCUS))
        };
        div()
            .id(("layout-row", id.0))
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .h(px(28.))
            .rounded(px(6.))
            .text_size(px(13.))
            .text_color(rgb(theme::TEXT))
            .map(|d| {
                if icons {
                    d.w(px(36.)).h(px(32.)).justify_center()
                } else {
                    d.gap(px(9.)).pl(px(10.)).pr(px(10.))
                }
            })
            .when(on, |d| {
                d.bg(rgb(theme::ROW_SELECTED))
                    .font_weight(FontWeight::SEMIBOLD)
            })
            .when(!on, |d| d.hover(|s| s.bg(rgb(theme::ROW_HOVER))))
            .when(dragging, |d| d.opacity(0.35))
            .cursor_pointer()
            .child(map(&self.model, layout, 22., 15.))
            .when(!icons, |d| {
                d.child(self.row_label(Target::Layout(id), label.clone(), cx))
            })
            .map(|d| match (chip, icons) {
                (Some(c), true) => d.child(c.absolute().top(px(-3.)).right(px(-5.))),
                (Some(c), false) => d.child(c),
                (None, _) => d,
            })
            .when(target == Some(i), |d| d.child(line(div()).top(px(-2.))))
            .when(
                target == Some(layouts.len()) && i + 1 == layouts.len(),
                |d| d.child(line(div()).bottom(px(-2.))),
            )
            .on_click(cx.listener(move |w, ev: &ClickEvent, window, cx| {
                let Some(i) = w.model.layouts.iter().position(|l| l.id == id) else {
                    return;
                };
                if ev.click_count() == 2 && !w.sidebar_icons() {
                    let text = w.layout_label(i);
                    w.start_rename(Target::Layout(id), text, window, cx);
                    return;
                }
                w.apply(window, cx, |m| m.select_layout(i));
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |w, ev: &MouseDownEvent, window, cx| {
                    w.open_menu(For::Layout(id), ev.position, window, cx)
                }),
            )
            .on_drag(
                LayoutDrag {
                    layout: id,
                    label: label.clone(),
                },
                |d, _, _, cx| {
                    cx.new(|_| LayoutDrag {
                        layout: d.layout,
                        label: d.label.clone(),
                    })
                },
            )
            .on_drag_move(cx.listener(move |w, ev: &DragMoveEvent<LayoutDrag>, _, _| {
                w.layout_drop.rows.insert(id, ev.bounds);
            }))
            .into_any_element()
    }

    /// Works out where the dragged layout would land under the pointer.
    fn layout_drag_move(&mut self, ev: &DragMoveEvent<LayoutDrag>, cx: &mut Context<Self>) {
        let id = ev.drag(cx).layout;
        let p = ev.event.position;
        let layouts = &self.model.layouts;
        let rows = &self.layout_drop.rows;
        let target = ev.bounds.contains(&p).then(|| {
            layouts
                .iter()
                .position(|l| {
                    rows.get(&l.id)
                        .is_some_and(|b| p.y < b.origin.y + b.size.height / 2.)
                })
                .unwrap_or(layouts.len())
        });
        let own = layouts.iter().position(|l| l.id == id);
        let target = target.filter(|&t| own.is_some_and(|o| t != o && t != o + 1));
        let d = &mut self.layout_drop;
        if target != d.target || d.dragging != Some(id) {
            d.target = target;
            d.dragging = Some(id);
            cx.notify();
        }
    }

    /// The layout being dragged, if any.
    fn dragged_layout(&self, cx: &gpui::App) -> Option<LayoutId> {
        if cx.has_active_drag() {
            self.layout_drop.dragging
        } else {
            None
        }
    }

    /// While ⌃⌘ is held, the layouts with the digits that pick them, over the window.
    pub(super) fn render_layout_list(&self) -> Option<AnyElement> {
        if self.held != Held::CmdCtrl {
            return None;
        }
        let rows = self.model.layouts.iter().enumerate().take(9).map(|(i, l)| {
            let on = i == self.model.active;
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .h(px(34.))
                .px(px(10.))
                .rounded(px(7.))
                .when(on, |d| {
                    d.bg(rgb(theme::ROW_SELECTED))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(theme::TEXT_STRONG))
                })
                .child(badge::chip(i + 1))
                .child(map(&self.model, l, 30., 20.))
                .child(self.layout_label(i))
        });
        let list = div()
            .min_w(px(240.))
            .p(px(6.))
            .flex()
            .flex_col()
            .gap(px(2.))
            .bg(rgb(theme::DIALOG))
            .border_1()
            .border_color(rgb(theme::CARD_BORDER))
            .rounded(px(10.))
            .shadow_lg()
            .text_size(px(13.))
            .text_color(rgb(theme::TEXT))
            .children(rows);
        Some(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .child(list)
                .into_any_element(),
        )
    }
}

/// A layout's slots, each in its space's color, gray when empty, in a `w` by `h` box.
fn map(model: &crate::model::Model, layout: &Layout, w: f32, h: f32) -> AnyElement {
    let cells = layout.slots.rects(Rect::UNIT).into_iter().map(|(slot, r)| {
        let color = layout
            .slots
            .find(slot)
            .and_then(|s| s.space)
            .and_then(|s| model.find_space(s))
            .map_or(theme::CARD_BORDER, |s| theme::space_color(s.color));
        div()
            .absolute()
            .left(px(r.x * w))
            .top(px(r.y * h))
            .w(px(r.w * w - 2.))
            .h(px(r.h * h - 2.))
            .rounded(px(2.))
            .bg(rgb(color))
    });
    div()
        .relative()
        .flex_none()
        .w(px(w))
        .h(px(h))
        .children(cells)
        .into_any_element()
}
