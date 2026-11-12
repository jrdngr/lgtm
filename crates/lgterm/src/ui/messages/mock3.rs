//! Mockup round 3: conversations between spaces, agents in their space's color.

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px, rgb,
};

use super::super::{badge, message_body};
use crate::agents::Message;
use crate::fonts;
use crate::model::TermId;
use crate::theme;

/// A terminal's space: id, name and color index.
#[derive(Clone)]
pub struct Where {
    pub space: u64,
    pub name: String,
    pub color: usize,
}

type Info<'a> = &'a dyn Fn(TermId) -> Where;

pub(super) fn env(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_owned())
}

/// `t17` in its space's color, on a tint of it.
pub(super) fn chip(id: TermId, color: usize) -> gpui::Div {
    let c = theme::space_color(color);
    div()
        .flex_none()
        .px(px(5.))
        .rounded(px(4.))
        .bg(rgb(c))
        .font_family(fonts::mono())
        .text_size(px(11.5))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(theme::BADGE_TEXT))
        .child(id.to_string())
}

pub(super) fn agent_pair(m: &Message) -> (TermId, TermId) {
    let f = m.from.unwrap_or(m.to);
    if f.0 < m.to.0 { (f, m.to) } else { (m.to, f) }
}

fn space_pair(m: &Message, info: Info) -> (u64, u64) {
    let (a, b) = agent_pair(m);
    let (a, b) = (info(a).space, info(b).space);
    if a <= b { (a, b) } else { (b, a) }
}

/// How a row shows it has unread messages.
pub(super) fn unread_mark(n: usize, style: &str) -> Option<AnyElement> {
    if n == 0 {
        return None;
    }
    Some(match style {
        // A dot.
        "1" => div()
            .flex_none()
            .size(px(7.))
            .rounded(px(4.))
            .bg(rgb(theme::TEXT_STRONG))
            .into_any_element(),
        // A plain number.
        "2" => div()
            .flex_none()
            .text_size(px(12.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(theme::TEXT_STRONG))
            .child(n.to_string())
            .into_any_element(),
        // A number on a faint chip.
        "3" => div()
            .flex_none()
            .px(px(6.))
            .rounded(px(4.))
            .bg(rgb(theme::CHIP))
            .text_size(px(11.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(theme::TEXT))
            .child(n.to_string())
            .into_any_element(),
        // Nothing on the right: the row's bar and bold names say it.
        _ => div().into_any_element(),
    })
}

/// A space line: badge and name, bold when unread.
fn space_line(w: &Where, bold: bool) -> impl IntoElement {
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

pub(super) fn agents_label(a: TermId, b: TermId, info: Info) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .child(chip(a, info(a).color))
        .child(
            div()
                .text_color(rgb(theme::ICON))
                .text_size(px(12.))
                .child("↔"),
        )
        .child(chip(b, info(b).color))
}

/// A space pair's row: its spaces, an arrow when it nests, and its unread mark.
#[allow(clippy::too_many_arguments)]
fn pair_row(
    i: usize,
    spaces: (&Where, &Where),
    picked: bool,
    unread: usize,
    style: &str,
    arrow: Option<bool>,
    extra: Option<AnyElement>,
) -> impl IntoElement {
    let bold = unread > 0 && style != "0";
    let same = spaces.0.space == spaces.1.space;
    div()
        .id(("space-pair", i))
        .relative()
        .flex()
        .flex_col()
        .gap(px(8.))
        .pl(px(if arrow.is_some() { 8. } else { 14. }))
        .pr(px(14.))
        .py(px(10.))
        .border_b_1()
        .border_color(rgb(theme::ROW_RULE))
        .when(picked, |d| d.bg(rgb(theme::ROW_SELECTED)))
        .when(style == "4" && unread > 0, |d| {
            d.child(
                div()
                    .absolute()
                    .left_0()
                    .top(px(10.))
                    .bottom(px(10.))
                    .w(px(3.))
                    .rounded(px(2.))
                    .bg(rgb(theme::TEXT_STRONG)),
            )
        })
        .text_size(px(13.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .when_some(arrow, |d, open| {
                    d.child(
                        div()
                            .w(px(12.))
                            .flex_none()
                            .text_size(px(10.))
                            .text_color(rgb(theme::ICON))
                            .child(if open { "▾" } else { "▸" }),
                    )
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(space_line(spaces.0, bold))
                        .when(!same, |d| d.child(space_line(spaces.1, bold))),
                )
                .children(unread_mark(unread, style)),
        )
        .children(extra)
}

pub fn conversations(messages: &[Message], info: Info) -> AnyElement {
    let side = env("LGTERM_MSG_SIDE", "a");
    let style = env("LGTERM_MSG_UNREAD", "2");
    // Space pairs, newest first; each with its agent pairs, newest first.
    let mut pairs: Vec<((u64, u64), Vec<(TermId, TermId)>)> = vec![];
    for m in messages.iter().rev() {
        let sp = space_pair(m, info);
        let ap = agent_pair(m);
        match pairs.iter_mut().find(|(s, _)| *s == sp) {
            Some((_, agents)) => {
                if !agents.contains(&ap) {
                    agents.push(ap);
                }
            }
            None => pairs.push((sp, vec![ap])),
        }
    }
    let picked = env("LGTERM_MSG_PICK", "0").parse::<usize>().unwrap_or(0);
    let filter: Option<usize> = env("LGTERM_MSG_FILTER", "").parse().ok();
    let where_of = |space: u64| {
        messages
            .iter()
            .flat_map(|m| [Some(m.to), m.from])
            .flatten()
            .map(info)
            .find(|w| w.space == space)
            .unwrap()
    };
    // Unread in the mockup: messages after 12:05 outside the picked pair.
    let unread = |i: usize, ap: Option<(TermId, TermId)>| -> usize {
        if i == picked {
            return 0;
        }
        messages
            .iter()
            .filter(|m| space_pair(m, info) == pairs[i].0)
            .filter(|m| ap.is_none_or(|ap| agent_pair(m) == ap))
            .filter(|m| m.time.as_str() > "12:05")
            .count()
    };
    let mut rows: Vec<AnyElement> = vec![];
    for (i, ((sa, sb), agents)) in pairs.iter().enumerate() {
        let (wa, wb) = (where_of(*sa), where_of(*sb));
        let is_picked = i == picked;
        match side.as_str() {
            "a" => {
                // Rows open only by their arrow, picked or not.
                let open = env("LGTERM_MSG_OPEN", "")
                    .split(',')
                    .any(|n| n.parse() == Ok(i));
                rows.push(
                    pair_row(
                        i,
                        (&wa, &wb),
                        is_picked && filter.is_none(),
                        unread(i, None),
                        &style,
                        Some(open),
                        None,
                    )
                    .into_any_element(),
                );
                if open {
                    for (j, &(a, b)) in agents.iter().enumerate() {
                        let n = unread(i, Some((a, b)));
                        rows.push(
                            div()
                                .id(("agent-pair", j))
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .pl(px(34.))
                                .pr(px(14.))
                                .py(px(7.))
                                .border_b_1()
                                .border_color(rgb(theme::ROW_RULE))
                                .when(filter == Some(j), |d| d.bg(rgb(theme::ROW_SELECTED)))
                                .child(agents_label(a, b, info))
                                .child(div().flex_1())
                                .children(unread_mark(n, &style))
                                .into_any_element(),
                        );
                    }
                }
            }
            "b" => {
                let pills = is_picked.then(|| {
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(6.))
                        .children(agents.iter().enumerate().map(|(j, &(a, b))| {
                            agents_label(a, b, info)
                                .px(px(6.))
                                .py(px(3.))
                                .rounded(px(6.))
                                .border_1()
                                .border_color(rgb(if filter == Some(j) {
                                    theme::CARD_BORDER
                                } else {
                                    theme::ROW_RULE
                                }))
                                .when(filter == Some(j), |d| d.bg(rgb(theme::TAB_ACTIVE)))
                        }))
                        .into_any_element()
                });
                rows.push(
                    pair_row(
                        i,
                        (&wa, &wb),
                        is_picked,
                        unread(i, None),
                        &style,
                        None,
                        pills,
                    )
                    .into_any_element(),
                );
            }
            _ => rows.push(
                pair_row(
                    i,
                    (&wa, &wb),
                    is_picked,
                    unread(i, None),
                    &style,
                    None,
                    None,
                )
                .into_any_element(),
            ),
        }
    }
    let (picked_spaces, picked_agents) = pairs[picked].clone();
    let shown: Vec<&Message> = messages
        .iter()
        .filter(|m| space_pair(m, info) == picked_spaces)
        .filter(|m| filter.is_none_or(|j| agent_pair(m) == picked_agents[j]))
        .filter(|m| super::mock4::keeps(m))
        .collect();
    let fsel = env("LGTERM_MSG_FSEL", "");
    let parts = if side == "c" && !fsel.is_empty() {
        super::mock4::parts(
            &super::mock4::Ctx {
                messages,
                info,
                spaces: (where_of(picked_spaces.0), where_of(picked_spaces.1)),
                agents: picked_agents.clone(),
                filter,
            },
            &fsel,
        )
    } else {
        super::mock4::Parts::default()
    };
    let bar = (side == "c" && fsel.is_empty()).then(|| {
        let tab = |label: AnyElement, on: bool| {
            div()
                .flex()
                .items_center()
                .h(px(26.))
                .px(px(10.))
                .rounded(px(7.))
                .border_1()
                .border_color(gpui::transparent_black())
                .text_size(px(12.5))
                .text_color(rgb(theme::TEXT_MUTED))
                .when(on, |d| {
                    d.bg(rgb(theme::TAB_ACTIVE))
                        .border_color(rgb(theme::CARD_BORDER))
                        .text_color(rgb(theme::TEXT_STRONG))
                })
                .child(label)
        };
        div()
            .flex()
            .items_center()
            .gap(px(4.))
            .px(px(16.))
            .py(px(8.))
            .border_b_1()
            .border_color(rgb(theme::ROW_RULE))
            .child(tab("All".into_any_element(), filter.is_none()))
            .children(picked_agents.iter().enumerate().map(|(j, &(a, b))| {
                tab(
                    agents_label(a, b, info).into_any_element(),
                    filter == Some(j),
                )
            }))
    });
    let transcript = shown.into_iter().map(|m| {
        let who = m.from.unwrap_or(m.to);
        let w = info(who);
        let color = theme::space_color(w.color);
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
                            .child(chip(who, w.color))
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(theme::TEXT_STRONG))
                                    .child(SharedString::from(w.name.clone())),
                            )
                            .child(div().text_color(rgb(theme::ICON)).child("→"))
                            .child(chip(m.to, info(m.to).color))
                            .child(
                                div()
                                    .font_family(fonts::mono())
                                    .text_size(px(11.5))
                                    .text_color(rgb(theme::TEXT_MUTED))
                                    .child(m.time.clone()),
                            ),
                    )
                    .child(message_body::render(&m.text)),
            )
    });
    div()
        .relative()
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
        .children(parts.column)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .h_full()
                .flex()
                .flex_col()
                .children(bar)
                .children(parts.bar)
                .child(
                    div()
                        .id("transcript")
                        .flex_1()
                        .min_h_0()
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
                ),
        )
        .children(parts.popover)
        .into_any_element()
}
