//! The Messages tab: conversations between spaces on the left, the picked one on the right.

use std::cell::Cell;
use std::collections::HashMap;

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, ScrollHandle,
    SharedString, StatefulInteractiveElement, Styled, div, px, rgb,
};

use super::convo::{self, Convo, Pair, Spaces};
use super::transcript::unread;
use crate::agents::Message;
use crate::model::{SpaceId, Tab, TermId};
use crate::theme;
use crate::ui::{Workspace, badge};

/// What the tab shows.
pub struct View {
    pub(super) spaces: Option<Spaces>,
    pub(super) pair: Option<Pair>,
    pub(super) menu: bool,
    scroll: ScrollHandle,
    /// The conversation, pair and message count shown last; the view goes to the end
    /// when they change.
    last: Cell<Option<(Spaces, Option<Pair>, usize)>>,
}

impl Default for View {
    fn default() -> Self {
        Self {
            spaces: None,
            pair: None,
            menu: false,
            scroll: ScrollHandle::new(),
            last: Cell::new(None),
        }
    }
}

/// A terminal's space, and the terminal's own color in it.
#[derive(Clone)]
pub(super) struct Who {
    pub space: u64,
    pub name: String,
    pub color: usize,
    /// The space's color for its first terminal, the colors after it for the rest.
    pub sub: usize,
    /// The terminal's tab name.
    pub label: String,
}

/// Where terminals that left no trace of their space go.
const UNKNOWN: u64 = u64::MAX;

impl Workspace {
    /// Where `t`, one end of `m`, is: its space now, or the one `m` saw it in.
    fn who(&self, m: &Message, t: TermId) -> Who {
        let label = if self.terms.contains_key(&t) {
            self.term_label(Tab::Term(t))
        } else if m.from == Some(t) {
            m.from_label.clone()
        } else {
            m.to_label.clone()
        };
        let live = |space: &crate::model::Space, sub: usize| Who {
            space: space.id.0,
            name: self.space_label(space),
            color: space.color,
            sub: space.color + sub,
            label: label.clone(),
        };
        if let Some(s) = self.model.space_of(Tab::Term(t)) {
            return live(s, s.terms().position(|x| x == t).unwrap_or(0));
        }
        let tag = if m.from == Some(t) {
            &m.from_space
        } else {
            &m.to_space
        };
        match tag {
            Some(tag) => match self.model.find_space(SpaceId(tag.id)) {
                Some(s) => live(s, 0),
                None => Who {
                    space: tag.id,
                    name: tag.name.clone(),
                    color: tag.color,
                    sub: tag.color,
                    label,
                },
            },
            None => Who {
                space: UNKNOWN,
                name: "Closed".into(),
                color: 0,
                sub: 0,
                label,
            },
        }
    }

    pub(in crate::ui) fn render_messages(&self, cx: &mut Context<Self>) -> AnyElement {
        let messages = &self.agents.messages;
        // Each terminal as its newest message saw it.
        let mut who: HashMap<TermId, Who> = HashMap::new();
        for m in messages.iter().rev() {
            for t in [convo::sender(m), m.to] {
                who.entry(t).or_insert_with(|| self.who(m, t));
            }
        }
        let space = |_: &Message, t: TermId| who[&t].space;
        let convos = convo::convos(messages, &space);
        let v = &self.messages_view;
        let Some(c) = convos
            .iter()
            .find(|c| Some(c.spaces) == v.spaces)
            .or(convos.first())
        else {
            return div().size_full().into_any_element();
        };
        let pair = v.pair.filter(|p| c.pairs.contains(p));
        let shown: Vec<(usize, &Message)> = messages
            .iter()
            .enumerate()
            .filter(|(_, m)| convo::spaces_of(m, &space) == c.spaces)
            .filter(|(_, m)| pair.is_none_or(|p| convo::pair_of(m) == p))
            .collect();
        let key = Some((c.spaces, pair, shown.len()));
        if v.last.replace(key) != key {
            v.scroll.scroll_to_bottom();
        }
        let seen: Vec<usize> = shown
            .iter()
            .filter(|(_, m)| !m.read)
            .map(|&(i, _)| i)
            .collect();
        if !seen.is_empty() {
            let this = cx.entity().downgrade();
            cx.defer(move |cx| {
                let _ = this.update(cx, |w, cx| {
                    for i in seen {
                        if let Some(m) = w.agents.messages.get_mut(i) {
                            m.read = true;
                        }
                    }
                    cx.notify();
                });
            });
        }
        let shown: Vec<&Message> = shown.into_iter().map(|(_, m)| m).collect();
        div()
            .relative()
            .size_full()
            .flex()
            .font_weight(FontWeight::NORMAL)
            .child(self.convo_list(&convos, c.spaces, &who, cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(self.convo_header(c, pair, &who, cx))
                    .child(
                        div()
                            .id("transcript")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .track_scroll(&v.scroll)
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(26.))
                                    .px(px(24.))
                                    .py(px(20.))
                                    .children(
                                        convo::runs(&shown)
                                            .iter()
                                            .map(|run| super::transcript::run(run, &who)),
                                    ),
                            ),
                    ),
            )
            .when(v.menu, |d| d.child(self.menu_backdrop(cx)))
            .into_any_element()
    }

    fn convo_list(
        &self,
        convos: &[Convo],
        picked: Spaces,
        who: &HashMap<TermId, Who>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let space = |id: u64| who.values().find(|w| w.space == id).cloned();
        div()
            .id("conversations")
            .w(px(240.))
            .flex_none()
            .h_full()
            .overflow_y_scroll()
            .border_r_1()
            .border_color(rgb(theme::ROW_RULE))
            .children(convos.iter().enumerate().map(|(i, c)| {
                let on = c.spaces == picked;
                let bold = c.unread > 0;
                let spaces = c.spaces;
                div()
                    .id(("conversation", i))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(14.))
                    .py(px(10.))
                    .border_b_1()
                    .border_color(rgb(theme::ROW_RULE))
                    .text_size(px(13.))
                    .when(on, |d| d.bg(rgb(theme::ROW_SELECTED)))
                    .when(!on, |d| d.hover(|s| s.bg(rgb(theme::ROW_HOVER))))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .children(space(c.spaces.0).map(|w| space_line(&w, bold)))
                            .when(c.spaces.0 != c.spaces.1, |d| {
                                d.children(space(c.spaces.1).map(|w| space_line(&w, bold)))
                            }),
                    )
                    .children(unread(c.unread))
                    .on_click(cx.listener(move |w, _, _, cx| {
                        let v = &mut w.messages_view;
                        v.spaces = Some(spaces);
                        v.pair = None;
                        v.menu = false;
                        cx.notify();
                    }))
            }))
    }
}

/// A space's badge and name, bold when it has unread messages.
pub(super) fn space_line(w: &Who, bold: bool) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .min_w_0()
        .child(badge::badge(w.color, &w.name, None, 0))
        .child(
            div()
                .min_w_0()
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .text_color(rgb(if bold {
                    theme::TEXT_STRONG
                } else {
                    theme::TEXT
                }))
                .when(bold, |d| d.font_weight(FontWeight::SEMIBOLD))
                .child(SharedString::from(w.name.clone())),
        )
}
