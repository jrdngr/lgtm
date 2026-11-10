//! The command palette: every action by name, filtered as you type.

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, FocusHandle, FontWeight, InteractiveElement, IntoElement, KeyDownEvent,
    MouseButton, ParentElement, PathBuilder, StatefulInteractiveElement, Styled, StyledText,
    Subscription, TextRun, Window, canvas, div, font, point, px, rgb,
};

use super::actions::{COMMANDS, Command, display};
use super::field::{self, Key};
use super::{Workspace, theme};

pub struct Palette {
    pub query: String,
    pub selected: usize,
    pub focus: FocusHandle,
    _blur: Subscription,
}

/// Rows shown at once.
const ROWS: usize = 10;

/// Where `query`'s characters fall in `name`, in order and ignoring case, scored so
/// earlier and closer-together matches come first. None if they don't all appear.
pub fn fuzzy(name: &str, query: &str) -> Option<(i32, Vec<usize>)> {
    let lower = name.to_lowercase();
    let q = query.to_lowercase();
    if q.is_empty() {
        return Some((0, vec![]));
    }
    if let Some(at) = lower.find(&q) {
        let word_start = at == 0 || lower[..at].ends_with(' ');
        let score = 1000 - at as i32 + if word_start { 500 } else { 0 };
        return Some((score, (at..at + q.len()).collect()));
    }
    let mut hits = Vec::new();
    let mut from = 0;
    for c in q.chars() {
        let i = lower[from..].find(c)? + from;
        hits.push(i);
        from = i + c.len_utf8();
    }
    let spread = hits.last()? - hits[0];
    Some((-(spread as i32), hits))
}

/// Commands matching `query`, best first.
pub fn matches(query: &str) -> Vec<(&'static Command, Vec<usize>)> {
    let mut found: Vec<(i32, usize, &Command, Vec<usize>)> = COMMANDS
        .iter()
        .enumerate()
        .filter_map(|(i, c)| fuzzy(c.name, query).map(|(score, hits)| (score, i, c, hits)))
        .collect();
    found.sort_by_key(|(score, i, _, _)| (-score, *i));
    found.into_iter().map(|(_, _, c, h)| (c, h)).collect()
}

impl Workspace {
    pub(super) fn toggle_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette.take().is_some() {
            self.focus_current(window, cx);
            return;
        }
        self.settings_open = None;
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let blur = cx.on_focus_out(&focus, window, |w, _, window, cx| {
            if window.is_window_active() {
                w.palette = None;
                cx.notify();
            }
        });
        self.palette = Some(Palette {
            query: String::new(),
            selected: 0,
            focus,
            _blur: blur,
        });
        cx.notify();
    }

    fn run_command(&mut self, command: &Command, window: &mut Window, cx: &mut Context<Self>) {
        self.palette = None;
        self.focus_current(window, cx);
        window.dispatch_action((command.action)(), cx);
    }

    fn palette_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = &mut self.palette else { return };
        let count = matches(&p.query).len().min(ROWS);
        match field::key(&mut p.query, &ev.keystroke) {
            Key::Other => return,
            Key::Edited => p.selected = 0,
            Key::Up => p.selected = p.selected.saturating_sub(1),
            Key::Down => p.selected = (p.selected + 1).min(count.saturating_sub(1)),
            Key::Escape => {
                self.palette = None;
                self.focus_current(window, cx);
            }
            Key::Enter | Key::ShiftEnter => {
                if let Some((c, _)) = matches(&p.query).into_iter().nth(p.selected) {
                    self.run_command(c, window, cx);
                }
            }
        }
        cx.stop_propagation();
        cx.notify();
    }

    pub(super) fn render_palette(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let p = self.palette.as_ref()?;
        let rows = matches(&p.query)
            .into_iter()
            .take(ROWS)
            .enumerate()
            .map(|(i, (c, hits))| {
                let on = i == p.selected;
                let keys = c.keys.first().map(|k| display(k)).unwrap_or_default();
                div()
                    .id(("command", i))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .h(px(34.))
                    .px(px(12.))
                    .rounded(px(7.))
                    .text_size(px(13.))
                    .cursor_pointer()
                    .when(on, |d| d.bg(rgb(theme::TAB_ACTIVE)))
                    .child(name(c.name, &hits, on))
                    .child(
                        div()
                            .ml_auto()
                            .flex()
                            .gap(px(4.))
                            .children(keys.into_iter().map(|k| {
                                div()
                                    .min_w(px(20.))
                                    .h(px(20.))
                                    .px(px(5.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .border_1()
                                    .border_color(rgb(theme::CARD_BORDER))
                                    .rounded(px(5.))
                                    .bg(rgb(theme::CARD))
                                    .text_size(px(11.))
                                    .text_color(rgb(theme::TEXT_SPACE))
                                    .child(k)
                            })),
                    )
                    .on_click(cx.listener(move |w, _, window, cx| {
                        let query = w.palette.as_ref().map(|p| p.query.clone());
                        if let Some((c, _)) = query.and_then(|q| matches(&q).into_iter().nth(i)) {
                            w.run_command(c, window, cx);
                        }
                    }))
            });
        let dialog = div()
            .id("palette")
            .w(px(600.))
            .bg(rgb(theme::DIALOG))
            .border_1()
            .border_color(rgb(theme::CARD_BORDER))
            .rounded(px(12.))
            .shadow_lg()
            .overflow_hidden()
            .track_focus(&p.focus)
            .on_key_down(cx.listener(Self::palette_key))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .h(px(46.))
                    .px(px(16.))
                    .border_b_1()
                    .border_color(rgb(theme::CARD_BORDER))
                    .text_size(px(14.))
                    .text_color(rgb(theme::TEXT_STRONG))
                    .child(magnifier())
                    .child(p.query.clone())
                    .child(div().w(px(1.5)).h(px(17.)).bg(rgb(theme::FOCUS))),
            )
            .child(div().p(px(6.)).flex().flex_col().gap(px(1.)).children(rows));
        Some(overlay("palette-overlay", dialog.into_any_element(), cx))
    }
}

/// A dialog over the window. Clicking outside it closes it.
pub(super) fn overlay(
    id: &'static str,
    dialog: AnyElement,
    cx: &mut Context<Workspace>,
) -> AnyElement {
    div()
        .id(id)
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .flex()
        .justify_center()
        .items_start()
        .pt(px(80.))
        .occlude()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|w, _, window, cx| {
                w.palette = None;
                w.settings_open = None;
                w.focus_current(window, cx);
            }),
        )
        .child(dialog)
        .into_any_element()
}

fn magnifier() -> AnyElement {
    canvas(
        |_, _, _| {},
        |bounds, _, window, _| {
            let o = bounds.origin;
            let at = |x: f32, y: f32| point(o.x + px(x), o.y + px(y));
            let mut path = PathBuilder::stroke(px(1.6));
            path.move_to(at(11.4, 6.5));
            path.arc_to(point(px(4.9), px(4.9)), px(0.), true, true, at(11.39, 6.4));
            path.move_to(at(10., 10.));
            path.line_to(at(13.5, 13.5));
            if let Ok(path) = path.build() {
                window.paint_path(path, rgb(theme::ICON));
            }
        },
    )
    .size(px(15.))
    .flex_none()
    .into_any_element()
}

/// A command name with the matched characters picked out.
fn name(text: &str, hits: &[usize], on: bool) -> StyledText {
    let base = rgb(if on { 0xffffff } else { theme::TEXT });
    let mut runs: Vec<TextRun> = Vec::new();
    for (i, ch) in text.char_indices() {
        let hit = hits.contains(&i);
        let mut f = font(theme::UI_FONT);
        if hit {
            f.weight = FontWeight::SEMIBOLD;
        }
        let color = if hit { rgb(theme::MATCH) } else { base };
        match runs.last_mut() {
            Some(r) if r.font == f && r.color == color.into() => r.len += ch.len_utf8(),
            _ => runs.push(TextRun {
                len: ch.len_utf8(),
                font: f,
                color: color.into(),
                background_color: None,
                underline: None,
                strikethrough: None,
            }),
        }
    }
    StyledText::new(text.to_owned()).with_runs(runs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substrings_at_word_starts_rank_first() {
        let names: Vec<&str> = matches("spl").iter().map(|(c, _)| c.name).collect();
        assert_eq!(
            &names[..4],
            [
                "Split right",
                "Split down",
                "Split space right",
                "Split space down"
            ]
        );
        assert!(names.contains(&"Close split"));
    }

    #[test]
    fn scattered_letters_still_match_in_order() {
        let (_, hits) = fuzzy("Next waiting agent", "nwa").unwrap();
        assert_eq!(hits, [0, 5, 6]);
        assert_eq!(fuzzy("Zoom split", "xyz"), None);
    }
}
