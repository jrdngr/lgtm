//! Finding text in a terminal's scrollback.

use ghostty_vt::search::Search;
use gpui::{
    AnyElement, Context, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, MouseButton,
    ParentElement, Styled, Window, div, px, rgb,
};

use super::{Paste, TermView};
use crate::ui::field::{self, Key};
use crate::ui::theme;

pub struct Find {
    query: String,
    search: Search,
    focus: FocusHandle,
    total: usize,
    index: Option<usize>,
}

impl TermView {
    pub fn open_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.find.is_none() {
            let Ok(search) = Search::new(&self.vt) else {
                return;
            };
            self.find = Some(Find {
                query: String::new(),
                search,
                focus: cx.focus_handle(),
                total: 0,
                index: None,
            });
        }
        if let Some(f) = &self.find {
            window.focus(&f.focus, cx);
        }
        cx.notify();
    }

    fn close_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.find = None;
        let _ = self.vt.set_selection(None);
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Moves to the next older match, or newer with `back`, and selects it.
    fn find_step(&mut self, back: bool) {
        let Some(f) = &mut self.find else { return };
        let vt = &self.vt;
        if f.search.run(vt).is_err() {
            return;
        }
        let moved = if back {
            f.search.select_prev(vt)
        } else {
            f.search.select_next(vt)
        };
        f.total = f.search.total_matches().unwrap_or(0);
        f.index = f.search.selected_index().ok().flatten();
        let selected = match moved {
            Ok(true) => f.search.selected_match(vt).ok().flatten(),
            _ => None,
        };
        let _ = vt.set_selection(selected.as_ref());
    }

    /// Searches again for the current query, from the newest match.
    fn find_query_changed(&mut self) {
        let Some(f) = &mut self.find else { return };
        let needle = Some(f.query.as_str()).filter(|q| !q.is_empty());
        let _ = f.search.set_needle(&self.vt, needle);
        f.total = 0;
        f.index = None;
        if needle.is_some() {
            self.find_step(false);
        } else {
            let _ = self.vt.set_selection(None);
        }
    }

    fn find_paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) else {
            return;
        };
        let Some(f) = &mut self.find else { return };
        f.query.extend(text.chars().filter(|c| !c.is_control()));
        self.find_query_changed();
        cx.notify();
    }

    fn find_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(f) = &mut self.find else { return };
        match field::key(&mut f.query, &ev.keystroke) {
            Key::Other if ev.keystroke.modifiers.platform => return,
            Key::Other => {}
            Key::Escape => {
                self.close_find(window, cx);
                cx.stop_propagation();
                return;
            }
            Key::Edited => self.find_query_changed(),
            Key::Enter | Key::Up => self.find_step(false),
            Key::ShiftEnter | Key::Down => self.find_step(true),
        }
        cx.stop_propagation();
        cx.notify();
    }

    pub(super) fn render_find(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let f = self.find.as_ref()?;
        let count = match (f.index, f.total) {
            (_, 0) if f.query.is_empty() => String::new(),
            (Some(i), n) => format!("{}/{n}", i + 1),
            (None, n) => format!("0/{n}"),
        };
        Some(
            div()
                .absolute()
                .top(px(8.))
                .right(px(12.))
                .w(px(260.))
                .h(px(32.))
                .flex()
                .items_center()
                .gap(px(8.))
                .px(px(10.))
                .bg(rgb(theme::DIALOG))
                .border_1()
                .border_color(rgb(theme::CARD_BORDER))
                .rounded(px(8.))
                .shadow_md()
                .font_family(theme::UI_FONT)
                .text_size(px(13.))
                .text_color(rgb(theme::TEXT_STRONG))
                .track_focus(&f.focus)
                .on_key_down(cx.listener(Self::find_key))
                .on_action(cx.listener(Self::find_paste))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .overflow_hidden()
                        .child(f.query.clone())
                        .child(div().w(px(1.5)).h(px(15.)).bg(rgb(theme::FOCUS))),
                )
                .child(
                    div()
                        .flex_none()
                        .text_size(px(11.5))
                        .text_color(rgb(theme::TEXT_MUTED))
                        .child(count),
                )
                .into_any_element(),
        )
    }
}
