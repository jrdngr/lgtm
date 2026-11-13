//! Asking before deleting a layout or restarting into an update: a question, Cancel, and the
//! action's button. Return confirms, Escape cancels.

use gpui::{
    AnyElement, Context, FocusHandle, FontWeight, InteractiveElement, IntoElement, KeyDownEvent,
    MouseButton, ParentElement, StatefulInteractiveElement, Styled, Window, div, px, rgb,
};

use super::Workspace;
use super::overlay::{Overlay, backdrop};
use crate::model::LayoutId;
use crate::theme;

pub struct Confirm {
    pub focus: FocusHandle,
    pub _blur: gpui::Subscription,
    pub what: Ask,
}

pub enum Ask {
    DeleteLayout(LayoutId),
    Restart,
}

impl Workspace {
    pub(super) fn confirm_delete_layout(
        &mut self,
        layout: LayoutId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_overlay(window, cx, |focus, _blur| {
            Overlay::Confirm(Confirm {
                focus,
                _blur,
                what: Ask::DeleteLayout(layout),
            })
        });
    }

    pub(super) fn confirm_restart(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_overlay(window, cx, |focus, _blur| {
            Overlay::Confirm(Confirm {
                focus,
                _blur,
                what: Ask::Restart,
            })
        });
    }

    fn confirmed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Overlay::Confirm(c)) = self.overlay.take() else {
            return;
        };
        match c.what {
            Ask::DeleteLayout(layout) => self.apply(window, cx, |m| m.delete_layout(layout)),
            Ask::Restart => self.restart(cx),
        }
    }

    fn confirm_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match ev.keystroke.key.as_str() {
            "enter" => self.confirmed(window, cx),
            "escape" => self.close_overlay(window, cx),
            _ => return,
        }
        cx.stop_propagation();
    }

    pub(super) fn render_confirm(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let Some(Overlay::Confirm(c)) = &self.overlay else {
            return None;
        };
        let (question, action, color) = match c.what {
            Ask::DeleteLayout(layout) => {
                let i = self.model.layouts.iter().position(|l| l.id == layout)?;
                (
                    format!("Delete “{}”?", self.layout_label(i)),
                    "Delete",
                    theme::DANGER,
                )
            }
            Ask::Restart => (
                "Restart lgterm to update?".to_owned(),
                "Restart",
                theme::UPDATE,
            ),
        };
        let button = |id: &'static str, label: &'static str, bg: u32| {
            div()
                .id(id)
                .h(px(28.))
                .px(px(16.))
                .flex()
                .items_center()
                .rounded(px(6.))
                .bg(rgb(bg))
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(theme::BADGE_TEXT))
                .cursor_pointer()
                .child(label)
        };
        let dialog = div()
            .id("confirm")
            .w(px(340.))
            .bg(rgb(theme::DIALOG))
            .border_1()
            .border_color(rgb(theme::CARD_BORDER))
            .rounded(px(12.))
            .p(px(20.))
            .flex()
            .flex_col()
            .gap(px(18.))
            .track_focus(&c.focus)
            .on_key_down(cx.listener(Self::confirm_key))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .text_size(px(15.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT_STRONG))
                    .child(question),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        button("cancel-confirm", "Cancel", theme::TAB_ACTIVE)
                            .on_click(cx.listener(|w, _, window, cx| w.close_overlay(window, cx))),
                    )
                    .child(
                        button("confirm", action, color)
                            .on_click(cx.listener(|w, _, window, cx| w.confirmed(window, cx))),
                    ),
            );
        Some(backdrop("confirm-overlay", dialog.into_any_element(), cx))
    }
}
