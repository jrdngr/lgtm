//! Sending messages between agents, and the Messages tab that lists them.

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, StyledText, TextRun, Window, div, font, px, rgb,
};

use super::Workspace;
use crate::agents::Message;
use crate::agents::messages::clock;
use crate::fonts;
use crate::ipc::Reply;
use crate::model::{Tab, TermId};
use crate::theme;

impl Workspace {
    pub(super) fn send_message(
        &mut self,
        from: Option<TermId>,
        to: TermId,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Reply {
        let message = Message {
            time: clock(),
            from,
            from_label: from
                .map(|f| self.term_label(Tab::Term(f)))
                .unwrap_or_default(),
            to,
            to_label: self.term_label(Tab::Term(to)),
            text,
            queued: true,
            dropped: false,
        };
        match self.with_agents(window, cx, |a, env| a.send(message, env)) {
            Ok(outcome) => Reply::Ok(outcome.into()),
            Err(e) => Reply::Err(e),
        }
    }

    pub(super) fn render_messages(&self) -> AnyElement {
        let rows = self.agents.messages.iter().enumerate().map(|(i, m)| {
            let who = |label: &str, id: Option<TermId>| {
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .min_w_0()
                    .flex_shrink(1.)
                    .child(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(SharedString::from(label.to_owned())),
                    )
                    .when_some(id, |d, id| {
                        d.child(
                            div()
                                .flex_none()
                                .font_family(fonts::mono())
                                .text_size(px(11.5))
                                .text_color(rgb(theme::TEXT_MUTED))
                                .child(id.to_string()),
                        )
                    })
            };
            div()
                .id(("message", i))
                .flex()
                .items_start()
                .gap(px(14.))
                .px(px(16.))
                .py(px(11.))
                .border_b_1()
                .border_color(rgb(theme::ROW_RULE))
                .text_size(px(13.))
                .child(
                    div()
                        .w(px(48.))
                        .flex_none()
                        .font_family(fonts::mono())
                        .text_size(px(11.5))
                        .text_color(rgb(theme::TEXT_MUTED))
                        .child(m.time.clone()),
                )
                .child(
                    div()
                        .w(px(260.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_color(rgb(theme::TEXT))
                        .when(m.from.is_some(), |d| {
                            d.child(who(&m.from_label, m.from))
                                .child(div().flex_none().text_color(rgb(theme::ICON)).child("→"))
                        })
                        .child(who(&m.to_label, Some(m.to))),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .items_start()
                        .justify_between()
                        .gap(px(12.))
                        .child(body(&m.text))
                        .when(m.queued || m.dropped, |d| {
                            d.child(
                                div()
                                    .flex_none()
                                    .px(px(6.))
                                    .border_1()
                                    .border_color(rgb(theme::BLOCKED))
                                    .rounded(px(5.))
                                    .text_size(px(11.))
                                    .text_color(rgb(theme::BLOCKED))
                                    .child(if m.queued { "Queued" } else { "Undelivered" }),
                            )
                        }),
                )
        });
        div()
            .id("messages")
            .size_full()
            .overflow_y_scroll()
            .font_weight(FontWeight::NORMAL)
            .children(rows)
            .into_any_element()
    }
}

/// Message text with `code` spans in the mono font.
fn body(text: &str) -> AnyElement {
    let plain = font(fonts::ui());
    let mono = font(fonts::mono());
    let mut shown = String::new();
    let mut runs = Vec::new();
    for (i, part) in text.split('`').enumerate() {
        if part.is_empty() {
            continue;
        }
        let code = i % 2 == 1;
        shown.push_str(part);
        runs.push(TextRun {
            len: part.len(),
            font: if code { mono.clone() } else { plain.clone() },
            color: rgb(if code { theme::CODE } else { 0xcfcfcf }).into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        });
    }
    div()
        .min_w_0()
        .line_height(px(19.))
        .child(StyledText::new(shown).with_runs(runs))
        .into_any_element()
}
