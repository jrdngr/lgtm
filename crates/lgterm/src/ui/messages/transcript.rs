//! The picked conversation: its spaces and agent menu over its messages, in runs from
//! one sender, a card each.

use std::collections::HashMap;

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, deferred, div, px, rgb,
};

use super::convo::{self, Convo, Pair};
use super::view::{Who, space_line};
use crate::agents::Message;
use crate::fonts;
use crate::model::TermId;
use crate::theme;
use crate::ui::{Workspace, message_body};

/// The terminal's name, or `t17` when it has none, on its space's color.
fn chip(t: TermId, w: &Who) -> gpui::Div {
    div()
        .flex_none()
        .px(px(5.))
        .rounded(px(4.))
        .bg(rgb(theme::space_color(w.color)))
        .font_family(fonts::mono())
        .text_size(px(11.5))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(theme::BADGE_TEXT))
        .child(w.named.clone().unwrap_or_else(|| t.to_string()))
}

/// The count of unread messages, when there are any.
pub(super) fn unread(n: usize) -> Option<AnyElement> {
    (n > 0).then(|| {
        div()
            .flex_none()
            .px(px(6.))
            .rounded(px(4.))
            .bg(rgb(theme::UNREAD))
            .text_size(px(11.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(theme::UNREAD_TEXT))
            .child(n.to_string())
            .into_any_element()
    })
}

fn arrow(s: &'static str) -> impl IntoElement {
    div()
        .text_size(px(12.))
        .text_color(rgb(theme::ICON))
        .child(s)
}

/// The pair with the agent in space `left` first.
fn left_first((a, b): Pair, left: u64, who: &HashMap<TermId, Who>) -> Pair {
    if who[&a].space == left {
        (a, b)
    } else {
        (b, a)
    }
}

fn pair_chips((a, b): Pair, who: &HashMap<TermId, Who>) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .child(chip(a, &who[&a]))
        .child(arrow("↔"))
        .child(chip(b, &who[&b]))
}

/// A chip and the terminal's tab name, when the chip doesn't show it.
fn agent_line(t: TermId, w: &Who, strong: bool) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .min_w_0()
        .child(chip(t, w))
        .when(w.named.is_none(), |d| {
            d.child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .text_color(rgb(if strong {
                        theme::TEXT_STRONG
                    } else {
                        theme::TEXT
                    }))
                    .child(SharedString::from(w.label.clone())),
            )
        })
}

impl Workspace {
    /// The conversation's spaces, and the button for the agent menu.
    pub(super) fn convo_header(
        &self,
        c: &Convo,
        pair: Option<Pair>,
        who: &HashMap<TermId, Who>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let space = |id: u64| who.values().find(|w| w.space == id).cloned();
        let open = self.messages_view.menu;
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .px(px(16.))
            .py(px(8.))
            .border_b_1()
            .border_color(rgb(theme::ROW_RULE))
            .text_size(px(13.))
            .children(space(c.spaces.0).map(|w| space_line(&w, true)))
            .when(c.spaces.0 != c.spaces.1, |d| {
                d.child(arrow("↔"))
                    .children(space(c.spaces.1).map(|w| space_line(&w, true)))
            })
            .child(
                div()
                    .relative()
                    .ml(px(10.))
                    .child(self.agents_button(
                        pair.map(|p| left_first(p, c.spaces.0, who)),
                        who,
                        cx,
                    ))
                    .when(open, |d| d.child(self.agents_menu(c, pair, who, cx))),
            )
    }

    fn agents_button(
        &self,
        pair: Option<Pair>,
        who: &HashMap<TermId, Who>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let open = self.messages_view.menu;
        div()
            .id("agents-button")
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(28.))
            .px(px(10.))
            .rounded(px(7.))
            .border_1()
            .border_color(rgb(theme::CARD_BORDER))
            .text_size(px(12.5))
            .text_color(rgb(theme::TEXT))
            .when(open, |d| d.bg(rgb(theme::TAB_ACTIVE)))
            .when(!open, |d| d.hover(|s| s.bg(rgb(theme::ROW_HOVER))))
            .map(|d| match pair {
                Some(p) => d.child(pair_chips(p, who)),
                None => d.child("All agents"),
            })
            .child(arrow("▾"))
            .on_click(cx.listener(|w, _, _, cx| {
                w.messages_view.menu = true;
                cx.notify();
            }))
    }

    /// Under the open menu: a click anywhere else closes it.
    pub(super) fn menu_backdrop(&self, cx: &mut Context<Self>) -> impl IntoElement {
        deferred(
            div()
                .id("agents-backdrop")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .occlude()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|w, _, _, cx| {
                        w.messages_view.menu = false;
                        cx.notify();
                    }),
                ),
        )
        .with_priority(1)
    }

    /// The agent pairs to pick from.
    fn agents_menu(
        &self,
        c: &Convo,
        pair: Option<Pair>,
        who: &HashMap<TermId, Who>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let unread_in = |p: Pair| {
            self.agents
                .messages
                .iter()
                .filter(|m| !m.read && convo::pair_of(m) == p)
                .count()
        };
        let row = |id: usize, on: bool, body: AnyElement, n: usize, pick: Option<Pair>| {
            div()
                .id(("agent-pair", id))
                .flex()
                .items_center()
                .gap(px(10.))
                .px(px(10.))
                .py(px(7.))
                .rounded(px(6.))
                .when(on, |d| d.bg(rgb(theme::ROW_SELECTED)))
                .when(!on, |d| d.hover(|s| s.bg(rgb(theme::ROW_HOVER))))
                .child(div().flex_1().min_w_0().child(body))
                .children(unread(n))
                .on_click(cx.listener(move |w, _, _, cx| {
                    w.messages_view.pair = pick;
                    w.messages_view.menu = false;
                    cx.notify();
                }))
        };
        let mut rows = vec![
            row(
                0,
                pair.is_none(),
                div()
                    .text_color(rgb(theme::TEXT_STRONG))
                    .child("All")
                    .into_any_element(),
                0,
                None,
            )
            .into_any_element(),
        ];
        rows.extend(c.pairs.iter().enumerate().map(|(j, &p)| {
            let (a, b) = left_first(p, c.spaces.0, who);
            let n = unread_in(p);
            let lines = div()
                .flex()
                .flex_col()
                .gap(px(5.))
                .child(agent_line(a, &who[&a], n > 0))
                .child(agent_line(b, &who[&b], n > 0));
            row(j + 1, pair == Some(p), lines.into_any_element(), n, Some(p)).into_any_element()
        }));
        deferred(
            div()
                .id("agents-menu")
                .absolute()
                .top(px(34.))
                .left_0()
                .w(px(340.))
                .max_h(px(380.))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap(px(2.))
                .p(px(5.))
                .rounded(px(8.))
                .border_1()
                .border_color(rgb(theme::CARD_BORDER))
                .bg(rgb(theme::DIALOG))
                .shadow_lg()
                .text_size(px(13.))
                .occlude()
                .children(rows),
        )
        .with_priority(2)
    }
}

/// Back-to-back messages from one agent to another: one header, a card each.
pub(super) fn run(run: &[&Message], who: &HashMap<TermId, Who>) -> AnyElement {
    let from = convo::sender(run[0]);
    let to = run[0].to;
    let cards = run.iter().map(|m| {
        div()
            .flex()
            .items_start()
            .gap(px(16.))
            .px(px(14.))
            .py(px(10.))
            .rounded(px(7.))
            .bg(rgb(theme::MESSAGE))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(message_body::render(&m.text)),
            )
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
            })
            .child(
                div()
                    .flex_none()
                    .font_family(fonts::mono())
                    .text_size(px(11.5))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(m.time.clone()),
            )
    });
    div()
        .flex()
        .gap(px(12.))
        .child(
            div()
                .w(px(3.))
                .flex_none()
                .rounded(px(2.))
                .bg(rgb(theme::space_color(who[&from].color))),
        )
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
                        .when(run[0].from.is_some(), |d| {
                            d.child(chip(from, &who[&from])).child(arrow("→"))
                        })
                        .child(chip(to, &who[&to])),
                )
                .children(cards),
        )
        .into_any_element()
}
