//! The window's root element, and what each action does.

use gpui::prelude::FluentBuilder;
use gpui::{
    Context, DragMoveEvent, InteractiveElement, IntoElement, ParentElement, Render, Styled, Window,
    div, rgb,
};

use super::actions::*;
use super::sidebar::{self, SidebarDrag};
use super::{Workspace, theme};
use crate::model::{Axis, Dir, Model};
use crate::terminal::FontSize;

/// A listener for an action that only changes the model.
macro_rules! on_model {
    ($el:expr, $cx:expr, $action:ty, $change:expr) => {
        $el.on_action($cx.listener(|w, _: &$action, win, cx| {
            let change: fn(&mut Model) = $change;
            w.apply(win, cx, |m| {
                change(m);
                None
            })
        }))
    };
}

/// A listener for an action that opens a terminal.
macro_rules! on_open {
    ($el:expr, $cx:expr, $action:ty, $open:expr) => {
        $el.on_action($cx.listener(|w, _: &$action, win, cx| {
            let open: fn(&mut Model) -> u64 = $open;
            w.apply(win, cx, |m| Some(open(m)))
        }))
    };
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut el = div()
            .id("workspace")
            .key_context("Workspace")
            .size_full()
            .relative()
            .flex()
            .flex_row()
            .bg(rgb(theme::WINDOW))
            .font_family(theme::UI_FONT)
            .on_key_down(cx.listener(Self::key_down))
            .on_drag_move(cx.listener(|w, ev: &DragMoveEvent<SidebarDrag>, _, cx| {
                let width = sidebar::snap(ev.event.position.x.as_f32());
                if width != w.sidebar_width {
                    w.sidebar_width = width;
                    cx.notify();
                }
            }));
        el = on_open!(el, cx, NewTab, |m| m.new_tab());
        el = on_open!(el, cx, SplitRight, |m| m.split_split(Axis::Row));
        el = on_open!(el, cx, SplitDown, |m| m.split_split(Axis::Column));
        el = on_open!(el, cx, SplitSpaceRight, |m| m.split_space(Axis::Row));
        el = on_open!(el, cx, SplitSpaceDown, |m| m.split_space(Axis::Column));
        el = on_open!(el, cx, NewLayout, |m| m.new_layout());
        el = on_model!(el, cx, NextTab, |m| m.cycle_tab(1));
        el = on_model!(el, cx, PrevTab, |m| m.cycle_tab(-1));
        el = on_model!(el, cx, ZoomSplit, |m| m.toggle_zoom());
        el = on_model!(el, cx, NextSplit, |m| m.cycle_split(1));
        el = on_model!(el, cx, PrevSplit, |m| m.cycle_split(-1));
        el = on_model!(el, cx, MoveTabToNewSplit, |m| m.move_tab_to_new_split());
        el = on_model!(el, cx, FocusLeft, |m| m.focus_dir(Dir::Left));
        el = on_model!(el, cx, FocusRight, |m| m.focus_dir(Dir::Right));
        el = on_model!(el, cx, FocusUp, |m| m.focus_dir(Dir::Up));
        el = on_model!(el, cx, FocusDown, |m| m.focus_dir(Dir::Down));
        el = on_model!(el, cx, NextLayout, |m| m.cycle_layout(1));
        el = on_model!(el, cx, PrevLayout, |m| m.cycle_layout(-1));
        el.on_action(cx.listener(|w, _: &CloseTab, win, cx| w.close(w.model.term(), win, cx)))
            .on_action(cx.listener(|w, _: &CloseSplit, win, cx| w.close_split(win, cx)))
            .on_action(cx.listener(|w, _: &NextWaiting, win, cx| {
                if let Some(term) = w.next_waiting() {
                    w.model.focus_term(term);
                    w.focus_current(win, cx);
                }
            }))
            .on_action(cx.listener(|w, _: &ShowMessages, win, cx| w.show_messages(win, cx)))
            .on_action(cx.listener(|w, _: &TogglePalette, win, cx| w.toggle_palette(win, cx)))
            .on_action(cx.listener(|w, _: &OpenSettings, win, cx| w.open_settings(win, cx)))
            .on_action(cx.listener(|w, _: &Find, win, cx| {
                if let Some(t) = w.terms.get(&w.model.term()) {
                    t.view.update(cx, |v, cx| v.open_find(win, cx));
                }
            }))
            .on_action(cx.listener(|w, _: &ToggleSidebar, win, cx| {
                w.sidebar = !w.sidebar;
                if w.rename.is_some() {
                    w.focus_current(win, cx);
                }
                cx.notify();
            }))
            .on_action(cx.listener(|w, _: &FontBigger, _, cx| {
                w.set_font_size(cx.global::<FontSize>().0 + 1., cx)
            }))
            .on_action(cx.listener(|w, _: &FontSmaller, _, cx| {
                w.set_font_size(cx.global::<FontSize>().0 - 1., cx)
            }))
            .on_action(
                cx.listener(|w, _: &FontReset, _, cx| {
                    w.set_font_size(theme::DEFAULT_FONT_SIZE, cx)
                }),
            )
            .when(self.sidebar, |d| d.child(self.render_sidebar(window, cx)))
            .child(self.render_main(window, cx))
            .children(self.render_palette(cx))
            .children(self.render_settings(cx))
            .children(self.render_tab_menu(cx))
    }
}
