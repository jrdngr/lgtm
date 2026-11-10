//! The tab bar's right-click menu.

use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels,
    Point, StatefulInteractiveElement, Styled, Window, anchored, deferred, div, px, rgb,
};

use super::actions::{self, COMMANDS};
use super::{Workspace, theme};
use crate::model::Axis;

pub struct TabMenu {
    pub at: Point<Pixels>,
    pub split: u64,
}

#[derive(Clone, Copy)]
enum Item {
    NewTerminal,
    NewSpace,
    Messages,
}

const ITEMS: [(Item, &str, &str); 3] = [
    (Item::NewTerminal, "New terminal", "New tab"),
    (Item::NewSpace, "New space", "Split space right"),
    (Item::Messages, "Messages", "Messages"),
];

/// The first key of the palette command named `command`, as the palette shows it.
fn key(command: &str) -> String {
    COMMANDS
        .iter()
        .find(|c| c.name == command)
        .and_then(|c| c.keys.first())
        .map(|k| actions::display(k).concat())
        .unwrap_or_default()
}

impl Workspace {
    pub(super) fn open_tab_menu(&mut self, split: u64, at: Point<Pixels>, cx: &mut Context<Self>) {
        self.tab_menu = Some(TabMenu { at, split });
        cx.notify();
    }

    fn run_tab_menu(&mut self, item: Item, window: &mut Window, cx: &mut Context<Self>) {
        let Some(menu) = self.tab_menu.take() else {
            return;
        };
        self.model.focus_split(menu.split);
        match item {
            Item::NewTerminal => self.apply(window, cx, |m| Some(m.new_tab())),
            Item::NewSpace => self.apply(window, cx, |m| Some(m.split_space(Axis::Row))),
            Item::Messages => self.show_messages(window, cx),
        }
    }

    pub(super) fn render_tab_menu(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let menu = self.tab_menu.as_ref()?;
        let rows = ITEMS.iter().map(|&(item, name, command)| {
            div()
                .id(name)
                .group(name)
                .flex()
                .items_center()
                .justify_between()
                .gap(px(24.))
                .h(px(26.))
                .px(px(10.))
                .rounded(px(5.))
                .text_color(rgb(theme::TEXT))
                .hover(|s| s.bg(rgb(theme::FOCUS)).text_color(rgb(theme::BADGE_TEXT)))
                .cursor_pointer()
                .child(name)
                .child(
                    div()
                        .text_color(rgb(theme::TEXT_MUTED))
                        .group_hover(name, |s| s.text_color(rgb(theme::BADGE_TEXT)))
                        .child(key(command)),
                )
                .on_click(cx.listener(move |w, _, window, cx| w.run_tab_menu(item, window, cx)))
        });
        let list = div()
            .id("tab-menu")
            .min_w(px(200.))
            .p(px(4.))
            .flex()
            .flex_col()
            .bg(rgb(theme::DIALOG))
            .border_1()
            .border_color(rgb(theme::CARD_BORDER))
            .rounded(px(8.))
            .shadow_lg()
            .text_size(px(13.))
            .on_mouse_down_out(cx.listener(|w, _, _, cx| {
                w.tab_menu = None;
                cx.notify();
            }))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .children(rows);
        Some(deferred(anchored().position(menu.at).snap_to_window().child(list)).into_any_element())
    }
}
