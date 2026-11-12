//! Mockup layouts of the Messages tab.

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px, rgb,
};

use super::super::message_body;
use crate::agents::Message;
use crate::fonts;
use crate::model::TermId;
use crate::theme;

const PANEL: u32 = 0x222222;

fn id_chip(id: TermId) -> impl IntoElement {
    div()
        .flex_none()
        .font_family(fonts::mono())
        .text_size(px(11.))
        .text_color(rgb(theme::TEXT_MUTED))
        .child(id.to_string())
}

fn name(label: &str, id: TermId, strong: bool) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .min_w_0()
        .child(
            div()
                .min_w_0()
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .when(strong, |d| d.font_weight(FontWeight::SEMIBOLD))
                .text_color(rgb(if strong { theme::TEXT_STRONG } else { theme::TEXT }))
                .child(SharedString::from(label.to_owned())),
        )
        .child(id_chip(id))
}

fn pill(m: &Message) -> Option<impl IntoElement> {
    (m.queued || m.dropped).then(|| {
        div()
            .flex_none()
            .px(px(6.))
            .border_1()
            .border_color(rgb(theme::BLOCKED))
            .rounded(px(5.))
            .text_size(px(11.))
            .text_color(rgb(theme::BLOCKED))
            .child(if m.queued { "Queued" } else { "Undelivered" })
    })
}

fn time(m: &Message) -> impl IntoElement {
    div()
        .flex_none()
        .font_family(fonts::mono())
        .text_size(px(11.5))
        .text_color(rgb(theme::TEXT_MUTED))
        .child(m.time.clone())
}

/// From → to, the pill and the time.
fn header(m: &Message) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .min_w_0()
        .text_size(px(13.))
        .when_some(m.from, |d, from| {
            d.child(name(&m.from_label, from, true))
                .child(div().flex_none().text_color(rgb(theme::ICON)).child("→"))
        })
        .child(name(&m.to_label, m.to, true))
        .child(div().flex_1())
        .children(pill(m))
        .child(time(m))
}

/// 1: every message a card, its header over its whole text.
pub fn cards(messages: &[Message]) -> AnyElement {
    div()
        .id("messages")
        .size_full()
        .overflow_y_scroll()
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .p(px(16.))
                .children(messages.iter().map(|m| {
                    div()
                        .max_w(px(860.))
                        .flex()
                        .flex_col()
                        .gap(px(12.))
                        .px(px(18.))
                        .py(px(14.))
                        .bg(rgb(PANEL))
                        .border_1()
                        .border_color(rgb(theme::ROW_RULE))
                        .rounded(px(8.))
                        .child(header(m))
                        .child(message_body::render(&m.text))
                })),
        )
        .into_any_element()
}

/// The text's first line, for a list row.
fn first_line(text: &str) -> String {
    text.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string()
}

fn reader(m: &Message) -> AnyElement {
    div()
        .id("reader")
        .flex_1()
        .min_w_0()
        .h_full()
        .overflow_y_scroll()
        .child(
            div()
                .max_w(px(860.))
                .flex()
                .flex_col()
                .gap(px(14.))
                .px(px(26.))
                .py(px(20.))
                .child(header(m))
                .child(div().h(px(1.)).bg(rgb(theme::ROW_RULE)))
                .child(message_body::render(&m.text)),
        )
        .into_any_element()
}

/// 2: one line per message on the left, the one picked in full on the right.
pub fn inbox(messages: &[Message]) -> AnyElement {
    let picked = 0;
    let rows = messages.iter().enumerate().map(|(i, m)| {
        div()
            .id(("message", i))
            .flex()
            .flex_col()
            .gap(px(3.))
            .px(px(14.))
            .py(px(10.))
            .border_b_1()
            .border_color(rgb(theme::ROW_RULE))
            .when(i == picked, |d| d.bg(rgb(theme::ROW_SELECTED)))
            .text_size(px(12.5))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .min_w_0()
                    .when_some(m.from, |d, from| {
                        d.child(name(&m.from_label, from, false))
                            .child(div().flex_none().text_color(rgb(theme::ICON)).child("→"))
                    })
                    .child(name(&m.to_label, m.to, false))
                    .child(div().flex_1())
                    .child(time(m)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child(first_line(&m.text)),
                    )
                    .children(pill(m)),
            )
    });
    div()
        .size_full()
        .flex()
        .child(
            div()
                .id("message-list")
                .w(px(340.))
                .flex_none()
                .h_full()
                .overflow_y_scroll()
                .border_r_1()
                .border_color(rgb(theme::ROW_RULE))
                .children(rows),
        )
        .children(messages.get(picked).map(reader))
        .into_any_element()
}

/// The two terminals a message is between, lowest id first.
fn pair(m: &Message) -> (Option<TermId>, TermId) {
    match m.from {
        Some(f) if f.0 > m.to.0 => (Some(m.to), f),
        f => (f, m.to),
    }
}

/// 3: conversations between two agents on the left, one's messages in order on the right.
pub fn conversations(messages: &[Message]) -> AnyElement {
    let mut pairs: Vec<(Option<TermId>, TermId)> = vec![];
    for m in messages.iter().rev() {
        let p = pair(m);
        if !pairs.contains(&p) {
            pairs.push(p);
        }
    }
    let label = |id: TermId| {
        messages
            .iter()
            .find_map(|m| {
                if m.to == id {
                    Some(m.to_label.clone())
                } else if m.from == Some(id) {
                    Some(m.from_label.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default()
    };
    let picked = 0;
    let rows = pairs.iter().enumerate().map(|(i, &(a, b))| {
        let of: Vec<&Message> = messages.iter().filter(|m| pair(m) == (a, b)).collect();
        let last = of.last().map(|m| m.time.clone()).unwrap_or_default();
        div()
            .id(("pair", i))
            .flex()
            .flex_col()
            .gap(px(4.))
            .px(px(14.))
            .py(px(10.))
            .border_b_1()
            .border_color(rgb(theme::ROW_RULE))
            .when(i == picked, |d| d.bg(rgb(theme::ROW_SELECTED)))
            .text_size(px(12.5))
            .when_some(a, |d, a| d.child(name(&label(a), a, false)))
            .child(name(&label(b), b, false))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .text_size(px(11.5))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(match of.len() {
                        1 => "1 message".to_string(),
                        n => format!("{n} messages"),
                    })
                    .child(div().flex_1())
                    .child(div().font_family(fonts::mono()).child(last)),
            )
    });
    let shown: Vec<&Message> = pairs
        .get(picked)
        .map(|&p| messages.iter().filter(|m| pair(m) == p).collect())
        .unwrap_or_default();
    let first_sender = shown.first().and_then(|m| m.from);
    let transcript = shown.into_iter().map(|m| {
        let who = m.from.unwrap_or(m.to);
        let color = if Some(who) == first_sender { 0x58a6ff } else { 0xd27a45 };
        div()
            .flex()
            .gap(px(12.))
            .child(div().w(px(3.)).flex_none().rounded(px(2.)).bg(rgb(color)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .max_w(px(820.))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .text_size(px(13.))
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(color))
                                    .child(SharedString::from(match m.from {
                                        Some(_) => m.from_label.clone(),
                                        None => "lgterm".to_string(),
                                    })),
                            )
                            .when_some(m.from, |d, f| d.child(id_chip(f)))
                            .child(time(m))
                            .children(pill(m)),
                    )
                    .child(message_body::render(&m.text)),
            )
    });
    div()
        .size_full()
        .flex()
        .child(
            div()
                .id("pairs")
                .w(px(300.))
                .flex_none()
                .h_full()
                .overflow_y_scroll()
                .border_r_1()
                .border_color(rgb(theme::ROW_RULE))
                .children(rows),
        )
        .child(
            div()
                .id("transcript")
                .flex_1()
                .min_w_0()
                .h_full()
                .overflow_y_scroll()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(22.))
                        .px(px(24.))
                        .py(px(20.))
                        .children(transcript),
                ),
        )
        .into_any_element()
}

/// Agent colors, one per open agent, kept for the agent's life.
const AGENT_COLORS: [u32; 8] = [
    0x58a6ff, 0xf0883e, 0xbc8cff, 0xf778ba, 0x56d364, 0x39c5cf, 0xe3b341, 0xff7b72,
];

/// The mockup's colors: by id order among the agents in the seeded state.
pub fn agent_color(id: TermId) -> u32 {
    let order = [1, 4, 9, 17];
    let i = order.iter().position(|&n| n == id.0).unwrap_or(id.0 as usize);
    AGENT_COLORS[i % AGENT_COLORS.len()]
}

pub fn on() -> bool {
    std::env::var("LGTERM_MSG_MOCK").is_ok()
}

/// `t17` in the agent's color, on a tint of it.
pub fn chip(id: TermId) -> gpui::Div {
    let color = agent_color(id);
    div()
        .flex_none()
        .px(px(5.))
        .rounded(px(4.))
        .bg(gpui::rgba(theme::tint(color, 0x2e)))
        .font_family(fonts::mono())
        .text_size(px(11.5))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(color))
        .child(id.to_string())
}

/// One agent in a pair row: its chip and its space.
fn agent_line(id: TermId, space: String) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .min_w_0()
        .child(chip(id))
        .child(
            div()
                .min_w_0()
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .text_color(rgb(theme::TEXT))
                .child(SharedString::from(space)),
        )
}

/// 4: conversations, agents named by id and space, in their colors, with unread counts.
pub fn conversations_v2(messages: &[Message], space: &dyn Fn(TermId) -> String) -> AnyElement {
    let mut pairs: Vec<(Option<TermId>, TermId)> = vec![];
    for m in messages.iter().rev() {
        let p = pair(m);
        if !pairs.contains(&p) {
            pairs.push(p);
        }
    }
    let picked = 0;
    let rows = pairs.iter().enumerate().map(|(i, &(a, b))| {
        let unread = if i == picked {
            0
        } else {
            messages
                .iter()
                .filter(|m| pair(m) == (a, b) && m.time.as_str() > "12:05")
                .count()
        };
        div()
            .id(("pair", i))
            .flex()
            .items_center()
            .gap(px(10.))
            .px(px(14.))
            .py(px(10.))
            .border_b_1()
            .border_color(rgb(theme::ROW_RULE))
            .when(i == picked, |d| d.bg(rgb(theme::ROW_SELECTED)))
            .text_size(px(13.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .when_some(a, |d, a| d.child(agent_line(a, space(a))))
                    .child(agent_line(b, space(b))),
            )
            .when(unread > 0, |d| {
                d.child(
                    div()
                        .flex_none()
                        .min_w(px(20.))
                        .h(px(20.))
                        .px(px(6.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(10.))
                        .bg(rgb(theme::FOCUS))
                        .text_size(px(11.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(theme::BADGE_TEXT))
                        .child(unread.to_string()),
                )
            })
    });
    let shown: Vec<&Message> = pairs
        .get(picked)
        .map(|&p| messages.iter().filter(|m| pair(m) == p).collect())
        .unwrap_or_default();
    let transcript = shown.into_iter().map(|m| {
        let who = m.from.unwrap_or(m.to);
        let color = agent_color(who);
        div()
            .flex()
            .gap(px(12.))
            .child(div().w(px(3.)).flex_none().rounded(px(2.)).bg(rgb(color)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .max_w(px(820.))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .text_size(px(13.))
                            .child(chip(who))
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(theme::TEXT_STRONG))
                                    .child(SharedString::from(space(who))),
                            )
                            .child(time(m))
                            .children(pill(m)),
                    )
                    .child(message_body::render(&m.text)),
            )
    });
    div()
        .size_full()
        .flex()
        .child(
            div()
                .id("pairs")
                .w(px(240.))
                .flex_none()
                .h_full()
                .overflow_y_scroll()
                .border_r_1()
                .border_color(rgb(theme::ROW_RULE))
                .children(rows),
        )
        .child(
            div()
                .id("transcript")
                .flex_1()
                .min_w_0()
                .h_full()
                .overflow_y_scroll()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(22.))
                        .px(px(24.))
                        .py(px(20.))
                        .children(transcript),
                ),
        )
        .into_any_element()
}
