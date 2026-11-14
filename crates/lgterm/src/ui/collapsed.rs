//! A space's collapsed splits: status marks beside its name, and the list they open.

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement, Styled, anchored, deferred, div, px, rgb, rgba,
};

use super::Workspace;
use super::sidebar::agent_icon;
use crate::model::{Space, SpaceId, Tab};
use crate::theme;

/// The most status marks shown beside a space's name.
const MARKS: usize = 8;
const ROW: f32 = 26.;
/// The line between two splits' tabs, with its margin.
const RULE: f32 = 5.;
/// What the list leaves free of the window's height.
const LIST_MARGIN: f32 = 60.;

impl Workspace {
    /// The marks of a space's collapsed tabs, most urgent first, opening the list of them.
    pub(super) fn collapsed_button(
        &self,
        space: &Space,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let collapsed = space.collapsed();
        if collapsed.is_empty() {
            return None;
        }
        let id = space.id;
        let mut tabs: Vec<Tab> = collapsed.iter().flat_map(|s| s.tabs.clone()).collect();
        tabs.sort_by_key(|t| std::cmp::Reverse(self.urgency(*t)));
        let open = self.collapsed_open == Some(id);
        Some(
            div()
                .id(("collapsed", id.0))
                .flex_none()
                .flex()
                .items_center()
                .gap(px(6.))
                .h(px(20.))
                .px(px(7.))
                .rounded(px(6.))
                .bg(rgba(0x0000_0040))
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |w, _, _, cx| {
                        cx.stop_propagation();
                        // A press that just shut the list leaves it shut.
                        if !std::mem::take(&mut w.collapsed_shut) {
                            w.collapsed_open = Some(id);
                        }
                        cx.notify();
                    }),
                )
                .children(tabs.iter().take(MARKS).map(|t| self.tab_mark(*t)))
                .when(open, |d| {
                    d.child(
                        div().absolute().top(px(24.)).left(px(0.)).child(deferred(
                            anchored()
                                .snap_to_window_with_margin(px(8.))
                                .child(self.collapsed_list(space, cx)),
                        )),
                    )
                })
                .relative()
                .into_any_element(),
        )
    }

    /// A space's collapsed tabs, a split's after another's, in as many columns as the window's
    /// height needs.
    fn collapsed_list(&self, space: &Space, cx: &mut Context<Self>) -> AnyElement {
        let room = (self.view_height - LIST_MARGIN).max(ROW);
        // Each row: its tab, and whether a line sets it apart from the split before.
        let mut columns: Vec<Vec<(Tab, bool)>> = vec![vec![]];
        let mut used = 0.;
        for split in space.collapsed() {
            for (i, &tab) in split.tabs.iter().enumerate() {
                let column = columns.last_mut().expect("a column");
                let rule = i == 0 && !column.is_empty();
                let need = ROW + if rule { RULE } else { 0. };
                if used + need > room && !column.is_empty() {
                    columns.push(vec![(tab, false)]);
                    used = ROW;
                } else {
                    column.push((tab, rule));
                    used += need;
                }
            }
        }
        div()
            .id("collapsed-list")
            .p(px(4.))
            .flex()
            .bg(rgb(theme::DIALOG))
            .border_1()
            .border_color(rgb(theme::CARD_BORDER))
            .rounded(px(8.))
            .shadow_lg()
            .font_weight(gpui::FontWeight::NORMAL)
            .text_size(px(13.))
            .text_color(rgb(theme::TEXT))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down_out(cx.listener(|w, _, _, cx| {
                w.collapsed_open = None;
                w.collapsed_shut = true;
                cx.notify();
            }))
            .children(columns.into_iter().enumerate().map(|(c, rows)| {
                div()
                    .w(px(190.))
                    .flex()
                    .flex_col()
                    .when(c > 0, |d| {
                        d.ml(px(4.))
                            .pl(px(4.))
                            .border_l_1()
                            .border_color(rgb(theme::CARD_BORDER))
                    })
                    .children(
                        rows.into_iter()
                            .map(|(tab, rule)| self.collapsed_row(tab, rule, cx)),
                    )
            }))
            .into_any_element()
    }

    fn collapsed_row(&self, tab: Tab, rule: bool, cx: &mut Context<Self>) -> AnyElement {
        let kind = tab.term().and_then(|t| self.agents.get(t)).map(|a| a.kind);
        let key = tab.term().map_or(0, |t| t.0);
        div()
            .when(rule, |d| {
                d.mt(px(2.))
                    .pt(px(2.))
                    .border_t_1()
                    .border_color(rgb(theme::CARD_BORDER))
            })
            .child(
                div()
                    .id(("collapsed-tab", key))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .h(px(ROW))
                    .px(px(8.))
                    .rounded(px(5.))
                    .hover(|s| s.bg(rgb(theme::ROW_HOVER)))
                    .cursor_pointer()
                    .child(
                        div()
                            .w(px(16.))
                            .flex_none()
                            .flex()
                            .justify_center()
                            .children(kind.map(agent_icon)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(self.term_label(tab)),
                    )
                    .child(self.tab_mark(tab))
                    .on_click(cx.listener(move |w, _, window, cx| {
                        w.collapsed_open = None;
                        w.model.focus_tab(tab);
                        w.focus_current(window, cx);
                    })),
            )
            .into_any_element()
    }

    /// How much a tab's status wants attention; tabs without one come last.
    fn urgency(&self, tab: Tab) -> Option<u8> {
        tab.term()
            .and_then(|t| self.agents.shown(t))
            .map(|s| s.urgency())
    }

    /// A tab's status mark, or an empty dot when it has none.
    fn tab_mark(&self, tab: Tab) -> AnyElement {
        match tab.term().and_then(|t| self.agents.shown(t)) {
            Some(s) => self.mark(s),
            None => div()
                .size(px(7.))
                .flex_none()
                .rounded_full()
                .bg(rgb(theme::CARD_BORDER))
                .into_any_element(),
        }
    }

    /// Shuts the collapsed list of a space that has none left.
    pub(super) fn prune_collapsed(&mut self) {
        let gone = |id: SpaceId| {
            self.model
                .find_space(id)
                .is_none_or(|s| s.collapsed().is_empty())
        };
        if self.collapsed_open.is_some_and(gone) {
            self.collapsed_open = None;
        }
    }
}
