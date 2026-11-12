//! Mockup round 4: ways to pick agents within a space pair, for long lists.
//! `LGTERM_MSG_FSEL`: 1 menu, 2 one picker per space, 3 column. `LGTERM_MSG_FOPEN=1|2`
//! opens a menu; `LGTERM_MSG_FA` picks one agent for picker 2.

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px, rgb,
};

use super::mock3::{Where, agents_label, chip, env, unread_mark};
use crate::agents::Message;
use crate::model::TermId;
use crate::theme;

/// What the filter adds around the transcript.
#[derive(Default)]
pub struct Parts {
    pub bar: Option<AnyElement>,
    pub column: Option<AnyElement>,
    pub popover: Option<AnyElement>,
}

pub struct Ctx<'a> {
    pub messages: &'a [Message],
    pub info: &'a dyn Fn(TermId) -> Where,
    pub spaces: (Where, Where),
    pub agents: Vec<(TermId, TermId)>,
    pub filter: Option<usize>,
}

impl Ctx<'_> {
    fn label(&self, t: TermId) -> String {
        self.messages
            .iter()
            .find_map(|m| {
                if m.from == Some(t) {
                    Some(m.from_label.clone())
                } else if m.to == t {
                    Some(m.to_label.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default()
    }

    /// Unread in the mockup: messages after 12:05 matching `keep`.
    fn unread(&self, keep: impl Fn(&Message) -> bool) -> usize {
        self.messages
            .iter()
            .filter(|m| m.time.as_str() > "12:05" && keep(m))
            .count()
    }

    fn pair_unread(&self, j: usize) -> usize {
        if self.filter == Some(j) {
            return 0;
        }
        let (a, b) = self.agents[j];
        self.unread(|m| {
            let f = m.from.unwrap_or(m.to);
            (f == a && m.to == b) || (f == b && m.to == a)
        })
    }

    /// Agents of `space` in this conversation, in first-seen order.
    fn agents_in(&self, space: u64) -> Vec<TermId> {
        let mut out = vec![];
        for &(a, b) in &self.agents {
            for t in [a, b] {
                if (self.info)(t).space == space && !out.contains(&t) {
                    out.push(t);
                }
            }
        }
        out
    }

    /// A chip and the agent's tab name.
    fn agent_line(&self, t: TermId, strong: bool) -> gpui::Div {
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .min_w_0()
            .child(chip(t, &(self.info)(t)))
            .child(
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
                    .child(SharedString::from(self.label(t))),
            )
    }

    /// An agent pair as two lines of chip and tab name.
    fn pair_lines(&self, (a, b): (TermId, TermId), strong: bool) -> gpui::Div {
        let (a, b) = if (self.info)(a).space == self.spaces.0.space {
            (a, b)
        } else {
            (b, a)
        };
        div()
            .flex()
            .flex_col()
            .gap(px(5.))
            .min_w_0()
            .child(self.agent_line(a, strong))
            .child(self.agent_line(b, strong))
    }
}

fn menu_row(
    id: (&'static str, usize),
    on: bool,
    body: AnyElement,
    n: usize,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(10.))
        .px(px(10.))
        .py(px(7.))
        .rounded(px(6.))
        .text_size(px(13.))
        .when(on, |d| d.bg(rgb(theme::ROW_SELECTED)))
        .child(div().flex_1().min_w_0().child(body))
        .children(unread_mark(n, "3"))
}

fn menu(rows: Vec<AnyElement>, left: f32, top: f32, w: f32) -> AnyElement {
    div()
        .absolute()
        .left(px(left))
        .top(px(top))
        .w(px(w))
        .child(
            div()
                .id("filter-menu")
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
                .children(rows),
        )
        .into_any_element()
}

/// A button that opens a menu.
fn button(body: AnyElement, open: bool) -> gpui::Div {
    div()
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
        .child(body)
        .child(
            div()
                .text_size(px(10.))
                .text_color(rgb(theme::ICON))
                .child("▾"),
        )
}

fn bar(children: Vec<AnyElement>) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .px(px(16.))
        .py(px(8.))
        .border_b_1()
        .border_color(rgb(theme::ROW_RULE))
        .children(children)
        .into_any_element()
}

fn all(strong: bool) -> AnyElement {
    div()
        .text_color(rgb(if strong {
            theme::TEXT_STRONG
        } else {
            theme::TEXT
        }))
        .child("All")
        .into_any_element()
}

/// Where the transcript starts: the window's sidebar plus the pair list.
const LEFT: f32 = 240.;

pub fn parts(cx: &Ctx, sel: &str) -> Parts {
    let open = env("LGTERM_MSG_FOPEN", "");
    match sel {
        "1" => {
            let shown = match cx.filter {
                Some(j) => {
                    let (a, b) = cx.agents[j];
                    agents_label(a, b, cx.info).into_any_element()
                }
                None => "All agents".into_any_element(),
            };
            let popover = (open == "1").then(|| {
                let mut rows = vec![
                    menu_row(("f-all", 0), cx.filter.is_none(), all(true), 0).into_any_element(),
                ];
                rows.extend(cx.agents.iter().enumerate().map(|(j, &p)| {
                    let n = cx.pair_unread(j);
                    menu_row(
                        ("f-pair", j),
                        cx.filter == Some(j),
                        cx.pair_lines(p, n > 0).into_any_element(),
                        n,
                    )
                    .into_any_element()
                }));
                if env("LGTERM_MSG_R6", "") == "1" {
                    menu(rows, 0., 0., 340.)
                } else {
                    menu(rows, LEFT + 16., 40., 340.)
                }
            });
            if env("LGTERM_MSG_R6", "") != "1" {
                return Parts {
                    bar: Some(bar(vec![button(shown, open == "1").into_any_element()])),
                    column: None,
                    popover,
                };
            }
            // The two spaces, then the menu, which opens under its button.
            let space = |w: &Where| {
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .text_size(px(13.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT_STRONG))
                    .child(super::super::badge::badge(w.color, &w.name, None, 0))
                    .child(SharedString::from(w.name.clone()))
                    .into_any_element()
            };
            let picker = div()
                .relative()
                .ml(px(10.))
                .child(button(shown, open == "1"))
                .children(
                    popover
                        .map(|p| gpui::deferred(div().absolute().top(px(34.)).left_0().child(p))),
                );
            Parts {
                bar: Some(bar(vec![
                    space(&cx.spaces.0),
                    div()
                        .text_color(rgb(theme::ICON))
                        .text_size(px(12.))
                        .child("↔")
                        .into_any_element(),
                    space(&cx.spaces.1),
                    picker.into_any_element(),
                ])),
                column: None,
                popover: None,
            }
        }
        "2" => {
            let one: Option<TermId> = env("LGTERM_MSG_FA", "").parse().ok().map(TermId);
            let side = |w: &Where, k: usize| {
                let mine = cx.agents_in(w.space);
                let picked = one.filter(|t| mine.contains(t));
                let shown = match picked {
                    Some(t) => cx.agent_line(t, true).into_any_element(),
                    None => div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(super::super::badge::badge(w.color, &w.name, None, 0))
                        .child(SharedString::from(w.name.clone()))
                        .into_any_element(),
                };
                let is_open = open == k.to_string();
                (
                    button(shown, is_open).into_any_element(),
                    mine,
                    picked,
                    is_open,
                )
            };
            let (ba, mine_a, pick_a, open_a) = side(&cx.spaces.0, 1);
            let (bb, mine_b, pick_b, open_b) = side(&cx.spaces.1, 2);
            let arrow = div()
                .text_color(rgb(theme::ICON))
                .text_size(px(12.))
                .child("↔")
                .into_any_element();
            let list = |mine: Vec<TermId>, picked: Option<TermId>, left: f32| {
                let mut rows =
                    vec![menu_row(("f-all", 0), picked.is_none(), all(true), 0).into_any_element()];
                rows.extend(mine.into_iter().enumerate().map(|(j, t)| {
                    let n = cx.unread(|m| m.from == Some(t) || m.to == t);
                    menu_row(
                        ("f-agent", j),
                        picked == Some(t),
                        cx.agent_line(t, n > 0).into_any_element(),
                        n,
                    )
                    .into_any_element()
                }));
                menu(rows, left, 40., 300.)
            };
            let popover = if open_a {
                Some(list(mine_a, pick_a, LEFT + 16.))
            } else if open_b {
                Some(list(mine_b, pick_b, LEFT + 200.))
            } else {
                None
            };
            Parts {
                bar: Some(bar(vec![ba, arrow, bb])),
                column: None,
                popover,
            }
        }
        _ => {
            let mut rows = vec![
                div()
                    .id("f-all")
                    .px(px(14.))
                    .py(px(10.))
                    .text_size(px(13.))
                    .border_b_1()
                    .border_color(rgb(theme::ROW_RULE))
                    .when(cx.filter.is_none(), |d| d.bg(rgb(theme::ROW_SELECTED)))
                    .child(all(cx.filter.is_none()))
                    .into_any_element(),
            ];
            rows.extend(cx.agents.iter().enumerate().map(|(j, &p)| {
                let n = cx.pair_unread(j);
                div()
                    .id(("f-pair", j))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(14.))
                    .py(px(9.))
                    .text_size(px(12.5))
                    .border_b_1()
                    .border_color(rgb(theme::ROW_RULE))
                    .when(cx.filter == Some(j), |d| d.bg(rgb(theme::ROW_SELECTED)))
                    .child(div().flex_1().min_w_0().child(cx.pair_lines(p, n > 0)))
                    .children(unread_mark(n, "3"))
                    .into_any_element()
            }));
            Parts {
                bar: None,
                column: Some(
                    div()
                        .id("filter-column")
                        .w(px(230.))
                        .flex_none()
                        .h_full()
                        .overflow_y_scroll()
                        .border_r_1()
                        .border_color(rgb(theme::ROW_RULE))
                        .children(rows)
                        .into_any_element(),
                ),
                popover: None,
            }
        }
    }
}

/// Whether option 2's single-agent pick keeps `m`.
pub fn keeps(m: &Message) -> bool {
    match env("LGTERM_MSG_FA", "").parse().ok().map(TermId) {
        Some(t) => m.from == Some(t) || m.to == t,
        None => true,
    }
}
