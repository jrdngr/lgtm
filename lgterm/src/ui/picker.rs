//! The layout picker at the bottom of the sidebar: the shown layout, which opens the list of
//! layouts, and a button for a new one.

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, StatefulInteractiveElement, Styled, div, px, rgb,
};

use super::Workspace;
use super::menu::For;
use super::rename::Target;
use crate::theme;

impl Workspace {
    pub(super) fn layout_picker(&self, icons: bool, cx: &mut Context<Self>) -> AnyElement {
        let active = self.model.active;
        let id = self.model.layout().id;
        let label = self.layout_label(active);
        let open = cx.listener(|w, ev: &gpui::ClickEvent, window, cx| {
            let at = gpui::point(px(8.), ev.position().y);
            w.open_menu(For::Layouts, at, window, cx)
        });
        let field = |d: gpui::Stateful<gpui::Div>| {
            d.h(px(28.))
                .flex()
                .items_center()
                .rounded(px(6.))
                .border_1()
                .border_color(rgb(theme::CARD_BORDER))
                .bg(rgb(theme::CARD))
                .text_size(px(13.))
                .text_color(rgb(theme::TEXT))
                .cursor_pointer()
        };
        let row = div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(6.))
            .p(px(8.))
            .border_t_1()
            .border_color(rgb(theme::SIDEBAR_BORDER));
        if icons {
            return row
                .justify_center()
                .child(
                    field(div().id("layout-picker"))
                        .w(px(36.))
                        .justify_center()
                        .child(format!("{}", active + 1))
                        .on_click(open),
                )
                .into_any_element();
        }
        let plus = field(div().id("new-layout"))
            .w(px(28.))
            .flex_none()
            .justify_center()
            .text_size(px(16.))
            .text_color(rgb(theme::ICON))
            .child("+")
            .on_click(cx.listener(|w, _, window, cx| w.new_layout(window, cx)));
        let picker = field(div().id("layout-picker"))
            .flex_1()
            .min_w_0()
            .justify_between()
            .gap(px(6.))
            .px(px(10.))
            .child(self.row_label(Target::Layout(id), label, cx))
            .child(
                div()
                    .text_size(px(10.))
                    .text_color(rgb(theme::ICON))
                    .child("▾"),
            )
            .on_click(open)
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |w, ev: &MouseDownEvent, window, cx| {
                    w.open_menu(For::Layout(id), ev.position, window, cx)
                }),
            )
            .when(self.renaming().is_some(), |d| d.cursor_text());
        row.child(picker).child(plus).into_any_element()
    }
}
