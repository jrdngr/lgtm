//! Messages between agents: queued until the receiver is ready, and listed in a Messages tab.

use std::time::{Duration, Instant};

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, StyledText, TextRun, div, font, px, rgb,
};
use serde::{Deserialize, Serialize};

use super::agents::DELIVERY_GAP;
use super::{Workspace, theme};
use crate::ipc::Reply;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// Local time sent, as HH:MM.
    pub time: String,
    pub from: Option<u64>,
    pub from_label: String,
    pub to: u64,
    pub to_label: String,
    pub text: String,
    pub queued: bool,
}

/// The oldest messages are dropped past this many.
const KEEP: usize = 500;

/// The local time as HH:MM.
fn clock() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as libc::time_t)
        .unwrap_or(0);
    // SAFETY: localtime_r only writes the struct it's given.
    let tm = unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&now, &mut tm);
        tm
    };
    format!("{:02}:{:02}", tm.tm_hour, tm.tm_min)
}

/// What the receiving agent reads.
pub fn envelope(m: &Message) -> String {
    match m.from {
        Some(from) => format!(
            "[lgterm message from t{from} · {}] {}\n(Reply with: lgterm send t{from} \"...\")",
            m.from_label, m.text
        ),
        None => format!("[lgterm message] {}", m.text),
    }
}

impl Workspace {
    pub(super) fn send_message(
        &mut self,
        from: Option<u64>,
        to: u64,
        text: String,
        cx: &mut Context<Self>,
    ) -> Reply {
        let Some(t) = self.terms.get(&to) else {
            return Reply::Err(format!("no terminal t{to}"));
        };
        if t.agent.is_none() {
            return Reply::Err(format!("t{to} isn't running an agent"));
        }
        let from_label = from.map(|f| self.term_label(f, cx)).unwrap_or_default();
        self.messages.push(Message {
            time: clock(),
            from,
            from_label,
            to,
            to_label: self.term_label(to, cx),
            text,
            queued: true,
        });
        if self.messages.len() > KEEP {
            self.messages.remove(0);
        }
        self.deliver(to, cx);
        cx.notify();
        let queued = self.messages.last().is_some_and(|m| m.queued);
        Reply::Ok(if queued { "queued" } else { "sent" }.into())
    }

    /// Hands `term` its oldest queued message, if it's ready for one.
    pub(super) fn deliver(&mut self, term: u64, cx: &mut Context<Self>) {
        let Some(t) = self.terms.get_mut(&term) else {
            return;
        };
        let Some(agent) = t.agent.as_mut().filter(|a| a.can_take_message()) else {
            return;
        };
        let Some(m) = self.messages.iter_mut().find(|m| m.queued && m.to == term) else {
            return;
        };
        m.queued = false;
        agent.busy_until = Some(Instant::now() + DELIVERY_GAP);
        let text = envelope(m);
        let view = t.view.clone();
        view.update(cx, |v, cx| v.paste_text(text, cx));
        cx.spawn(async move |_, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(150))
                .await;
            view.update(cx, |v, cx| v.input(b"\r", cx));
        })
        .detach();
        cx.notify();
    }

    pub(super) fn render_messages(&self) -> AnyElement {
        let rows = self.messages.iter().enumerate().map(|(i, m)| {
            let who = |label: &str, id: Option<u64>| {
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
                                .font_family(theme::MONO_FONT)
                                .text_size(px(11.5))
                                .text_color(rgb(theme::TEXT_MUTED))
                                .child(format!("t{id}")),
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
                        .font_family(theme::MONO_FONT)
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
                        .when(m.queued, |d| {
                            d.child(
                                div()
                                    .flex_none()
                                    .px(px(6.))
                                    .border_1()
                                    .border_color(rgb(theme::BLOCKED))
                                    .rounded(px(5.))
                                    .text_size(px(11.))
                                    .text_color(rgb(theme::BLOCKED))
                                    .child("Queued"),
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
    let plain = font(theme::UI_FONT);
    let mono = font(theme::MONO_FONT);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn message(from: Option<u64>) -> Message {
        Message {
            time: "14:31".into(),
            from,
            from_label: "Review keys".into(),
            to: 5,
            to_label: "Modal bar".into(),
            text: "tests pass".into(),
            queued: true,
        }
    }

    #[test]
    fn envelopes_say_who_to_reply_to() {
        let e = envelope(&message(Some(2)));
        assert!(e.starts_with("[lgterm message from t2 · Review keys] tests pass"));
        assert!(e.contains("lgterm send t2"));
        assert_eq!(envelope(&message(None)), "[lgterm message] tests pass");
    }

    #[test]
    fn the_clock_reads_hours_and_minutes() {
        let c = clock();
        assert_eq!(c.len(), 5);
        assert_eq!(&c[2..3], ":");
    }
}
