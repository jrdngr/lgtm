//! Mockup round 6: back-to-back messages from one sender to one recipient share a
//! header. `LGTERM_MSG_GROUP`: 1 times in a gutter with rules between, 2 one card each.

use gpui::prelude::FluentBuilder;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px, rgb};

use super::super::message_body;
use super::mock3::{Where, chip, env};
use crate::agents::Message;
use crate::fonts;
use crate::model::TermId;
use crate::theme;

fn time(t: &str) -> gpui::Div {
    div()
        .flex_none()
        .font_family(fonts::mono())
        .text_size(px(11.5))
        .text_color(rgb(theme::TEXT_MUTED))
        .child(t.to_owned())
}

pub fn transcript(shown: &[&Message], info: &dyn Fn(TermId) -> Where) -> Vec<AnyElement> {
    let style = env("LGTERM_MSG_GROUP", "1");
    let mut groups: Vec<Vec<&Message>> = vec![];
    for &m in shown {
        match groups.last_mut() {
            Some(g) if g[0].from == m.from && g[0].to == m.to => g.push(m),
            _ => groups.push(vec![m]),
        }
    }
    groups
        .into_iter()
        .map(|g| {
            let who = g[0].from.unwrap_or(g[0].to);
            let w = info(who);
            let head = div()
                .flex()
                .items_center()
                .gap(px(8.))
                .text_size(px(13.))
                .child(chip(who, &w))
                .child(div().text_color(rgb(theme::ICON)).child("→"))
                .child(chip(g[0].to, &info(g[0].to)));
            let items = g.iter().enumerate().map(|(i, m)| {
                if style == "2" {
                    div()
                        .flex()
                        .items_start()
                        .gap(px(16.))
                        .px(px(14.))
                        .py(px(10.))
                        .rounded(px(7.))
                        .bg(rgb(0x252525))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(message_body::render(&m.text)),
                        )
                        .child(time(&m.time))
                        .into_any_element()
                } else {
                    div()
                        .flex()
                        .items_start()
                        .gap(px(14.))
                        .when(i > 0, |d| {
                            d.pt(px(12.))
                                .border_t_1()
                                .border_color(rgb(theme::ROW_RULE))
                        })
                        .child(time(&m.time).w(px(40.)).pt(px(1.)))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(message_body::render(&m.text)),
                        )
                        .into_any_element()
                }
            });
            div()
                .flex()
                .gap(px(12.))
                .child(
                    div()
                        .w(px(3.))
                        .flex_none()
                        .rounded(px(2.))
                        .bg(rgb(theme::space_color(w.color))),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .max_w(px(820.))
                        .flex()
                        .flex_col()
                        .gap(px(if style == "2" { 8. } else { 12. }))
                        .child(head)
                        .children(items),
                )
                .into_any_element()
        })
        .collect()
}
