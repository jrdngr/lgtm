//! What sits over the window and has the keyboard: a rename, the palette, Settings, or a
//! delete to confirm. One at a time.

use gpui::{
    AnyElement, Context, FocusHandle, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Styled, Subscription, Window, div, px,
};

use super::Workspace;
use super::confirm::Confirm;
use super::menu::Menu;
use super::palette::Palette;
use super::rename::Rename;
use super::settings_commands::CommandBox;

pub enum Overlay {
    Rename(Rename),
    Palette(Palette),
    Settings {
        focus: FocusHandle,
        _blur: Subscription,
        commands: Vec<CommandBox>,
    },
    Confirm(Confirm),
    /// Leaves the keyboard where it is.
    Menu(Menu),
}

impl Workspace {
    /// Whether an overlay has taken the keyboard from the terminals.
    pub(super) fn overlay_has_keyboard(&self) -> bool {
        self.overlay
            .as_ref()
            .is_some_and(|o| !matches!(o, Overlay::Menu(_)))
    }

    /// Puts `make`'s overlay up with the keyboard, closing whatever was up. It closes when the
    /// keyboard leaves it while the window is active.
    pub(super) fn show_overlay(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        make: impl FnOnce(FocusHandle, Subscription) -> Overlay,
    ) {
        self.finish_rename(true);
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let blur = cx.on_focus_out(&focus, window, |w, _, window, cx| {
            if window.is_window_active() && !matches!(w.overlay, Some(Overlay::Rename(_))) {
                w.overlay = None;
                cx.notify();
            }
        });
        self.overlay = Some(make(focus, blur));
        cx.notify();
    }

    /// Takes the palette or Settings down and gives the keyboard back.
    pub(super) fn close_overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.overlay = None;
        self.focus_current(window, cx);
    }
}

/// A dialog over the window; a click outside it closes it and drops the picked spaces.
pub(super) fn backdrop(
    id: &'static str,
    dialog: AnyElement,
    cx: &mut Context<Workspace>,
) -> AnyElement {
    div()
        .id(id)
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .flex()
        .justify_center()
        .items_start()
        .pt(px(80.))
        .occlude()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|w, _, window, cx| {
                w.selected.clear();
                w.close_overlay(window, cx)
            }),
        )
        .child(dialog)
        .into_any_element()
}
