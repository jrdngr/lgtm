//! Alerts shown inside the window, in the bottom right corner, until clicked, closed, or timed out.

use std::time::Duration;

use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement, Styled, div, px, rgb,
};

use super::Workspace;
use crate::agents::Status;
use crate::model::TermId;
use crate::theme;

/// How long a toast stays up.
const SHOWN_FOR: Duration = Duration::from_secs(8);
/// The most toasts shown at once; older ones go first.
const MAX: usize = 4;

pub(super) struct Toast {
    id: u64,
    term: TermId,
    status: Status,
    title: String,
    body: String,
}

impl Workspace {
    pub(super) fn toast(
        &mut self,
        term: TermId,
        status: Status,
        title: String,
        body: String,
        cx: &mut Context<Self>,
    ) {
        self.toast_id += 1;
        let id = self.toast_id;
        self.toasts.retain(|t| t.term != term);
        self.toasts.push(Toast {
            id,
            term,
            status,
            title,
            body,
        });
        if self.toasts.len() > MAX {
            self.toasts.remove(0);
        }
        cx.spawn(async move |w, cx| {
            cx.background_executor().timer(SHOWN_FOR).await;
            let _ = w.update(cx, |w, cx| w.close_toast(id, cx));
        })
        .detach();
        cx.notify();
    }

    fn close_toast(&mut self, id: u64, cx: &mut Context<Self>) {
        let before = self.toasts.len();
        self.toasts.retain(|t| t.id != id);
        if self.toasts.len() != before {
            cx.notify();
        }
    }

    pub(super) fn render_toasts(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.toasts.is_empty() {
            return None;
        }
        let cards = self.toasts.iter().map(|t| {
            let (id, term) = (t.id, t.term);
            div()
                .id(("toast", id))
                .w(px(300.))
                .flex()
                .items_center()
                .gap(px(10.))
                .px(px(14.))
                .py(px(10.))
                .bg(rgb(theme::DIALOG))
                .border_1()
                .border_color(rgb(theme::CARD_BORDER))
                .rounded(px(10.))
                .shadow_md()
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |w, _, window, cx| {
                    w.close_toast(id, cx);
                    w.model.focus_term(term);
                    w.focus_current(window, cx);
                }))
                .child(self.mark(t.status))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(theme::TEXT_STRONG))
                                .overflow_hidden()
                                .text_ellipsis()
                                .whitespace_nowrap()
                                .child(t.title.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(rgb(theme::TEXT))
                                .overflow_hidden()
                                .text_ellipsis()
                                .whitespace_nowrap()
                                .child(t.body.clone()),
                        ),
                )
                .child(
                    div()
                        .id(("toast-close", id))
                        .text_size(px(15.))
                        .text_color(rgb(theme::ICON))
                        .child("×")
                        .on_click(cx.listener(move |w, _, _, cx| {
                            cx.stop_propagation();
                            w.close_toast(id, cx);
                        })),
                )
        });
        Some(
            div()
                .absolute()
                .bottom(px(theme::GAP * 2.))
                .right(px(theme::GAP * 2.))
                .flex()
                .flex_col()
                .gap(px(8.))
                .children(cards)
                .into_any_element(),
        )
    }
}
