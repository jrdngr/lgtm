//! The buttons in a space's title or a split's tabs: expand, which fills the window or the
//! space with it and puts the layout back, a space's lock against agents, a slot's magnet,
//! and an empty slot's add.

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
    let icon = if expanded {
        "ui/collapse.svg"
    } else {
        "ui/expand.svg"
    };
    icon_button(id, parent, icon, expanded, on_click)
}

/// A space's lock: open, shown while the pointer is over the element of group `parent`, or
/// closed and always shown when `locked`.
pub fn lock(
    id: impl Into<ElementId>,
    parent: String,
    locked: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let icon = if locked {
        "ui/lock.svg"
    } else {
        "ui/unlock.svg"
    };
    icon_button(id, parent, icon, locked, on_click)
}

/// A slot's magnet, shown while the pointer is over the element of group `parent`, or always
/// when `on`.
pub fn magnet(
    id: impl Into<ElementId>,
    parent: String,
    on: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    icon_button(id, parent, "ui/magnet.svg", on, on_click)
}

/// The button that adds a space to an empty slot.
pub fn add(
    id: impl Into<ElementId>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    icon_button(id, String::new(), "ui/plus.svg", true, on_click)
}

/// `icon` as a button: bright and always shown when `on`, else dim and shown while the pointer is
/// over `parent`.
fn icon_button(
    id: impl Into<ElementId>,
    parent: String,
    icon: &'static str,
    on: bool,
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
        .when(!on, |d| {
            d.opacity(0.)
                .group_hover(parent, |s| s.opacity(1.))
                .hover(|s| s.bg(rgb(theme::ROW_HOVER)))
        })
        .cursor_pointer()
        .group(group.clone())
        .child(svg().path(icon).size(px(14.)).map(|s| {
            if on {
                s.text_color(rgb(theme::ICON_ON))
            } else {
                s.text_color(rgb(theme::ICON))
                    .group_hover(group, |s| s.text_color(rgb(theme::TEXT)))
            }
        }))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |ev, window, cx| {
            cx.stop_propagation();
            on_click(ev, window, cx)
        })
}
