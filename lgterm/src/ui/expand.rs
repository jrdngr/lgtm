//! The button at the right of a space's title or a split's tabs that fills the window or the
//! space with it, and puts the layout back.

use gpui::prelude::FluentBuilder;
use gpui::{
    App, ClickEvent, ElementId, InteractiveElement, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, px, rgb, svg,
};

use crate::theme;

/// The expand button, shown while the pointer is over the element of group `parent`, or the
/// collapse button, always shown, when `expanded`.
pub fn button(
    id: impl Into<ElementId>,
    parent: String,
    expanded: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let id = id.into();
    let group = format!("{id:?}");
    div()
        .id(id)
        .flex_none()
        .size(px(22.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(5.))
        .when(!expanded, |d| {
            d.opacity(0.).group_hover(parent, |s| s.opacity(1.))
        })
        .cursor_pointer()
        .group(group.clone())
        .hover(|s| s.bg(rgb(theme::ROW_HOVER)))
        .child(
            svg()
                .path(if expanded {
                    "ui/collapse.svg"
                } else {
                    "ui/expand.svg"
                })
                .size(px(14.))
                .text_color(rgb(theme::ICON))
                .group_hover(group, |s| s.text_color(rgb(theme::TEXT))),
        )
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |ev, window, cx| {
            cx.stop_propagation();
            on_click(ev, window, cx)
        })
}
