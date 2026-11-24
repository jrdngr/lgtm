//! Folded splits: a split folded beside others in a column shrinks to its tab bar, and one
//! folded beside others in a row to a narrow strip of its tabs' marks. Picking a tab opens
//! its split again.

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, Pixels, StatefulInteractiveElement, Styled, Transformation, div, px, radians,
    rgb, svg,
};

use super::Workspace;
use super::menu::For;
use super::sidebar::agent_icon;
use super::tab_drag::TAB_STRIP;
use crate::model::{Axis, Split, Spot, Tab};
use crate::theme;

/// How wide a strip of folded splits is.
const STRIP: f32 = 34.;

impl Workspace {
    /// Splits folded together as one bar or strip, by the axis of the branch they're in, with
    /// the length it takes along that axis.
    pub(super) fn folded(
        &self,
        splits: &[&Split],
        (axis, end): (Axis, Spot),
        cx: &mut Context<Self>,
    ) -> (Pixels, AnyElement) {
        let (icon, turn) = icon((axis, end), true);
        let ids: Vec<_> = splits.iter().map(|s| s.id).collect();
        let key = ids.first().map_or(0, |s| s.0);
        let row = axis == Axis::Row;
        let open = div()
            .id(("unfold", key))
            .flex_none()
            .size(px(22.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(5.))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(theme::ROW_HOVER)))
            .child(
                svg()
                    .path(icon)
                    .size(px(14.))
                    .text_color(rgb(theme::ICON))
                    .with_transformation(Transformation::rotate(radians(turn))),
            )
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |w, _, window, cx| {
                cx.stop_propagation();
                w.apply(window, cx, |m| m.unfold(&ids))
            }));
        let mut items = Vec::new();
        for (i, split) in splits.iter().enumerate() {
            if row || i > 0 {
                items.push(rule(row));
            }
            for (i, &tab) in split.tabs.iter().enumerate() {
                let active = i == split.active;
                items.push(if row {
                    self.strip_tab(tab, active, cx)
                } else {
                    self.bar_tab(tab, active, cx)
                });
            }
        }
        let card = div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .gap(px(3.))
            .bg(rgb(theme::CARD))
            .border_1()
            .border_color(rgb(theme::CARD_BORDER))
            .rounded(px(10.))
            .overflow_hidden();
        // A strip's open button heads it; a bar's sits at its right end.
        let card = if row {
            card.flex_col()
                .items_center()
                .py(px(6.))
                .child(open)
                .children(items)
        } else {
            card.items_center()
                .px(px(6.))
                .children(items)
                .child(div().flex_1())
                .child(open)
        };
        let len = if row {
            theme::ui(STRIP)
        } else {
            theme::ui(TAB_STRIP) + px(2.)
        };
        (len, card.into_any_element())
    }

    /// A tab in a folded split's bar: its name and mark.
    fn bar_tab(&self, tab: Tab, active: bool, cx: &mut Context<Self>) -> AnyElement {
        self.folded_tab(tab, cx)
            .gap(px(7.))
            .h(theme::ui(26.))
            .px(px(10.))
            .text_size(theme::ui(12.5))
            .whitespace_nowrap()
            .text_color(rgb(theme::TEXT_MUTED))
            .when(active, |d| {
                d.bg(rgb(theme::TAB_ACTIVE))
                    .text_color(rgb(theme::TEXT_STRONG))
            })
            .child(self.term_label(tab))
            .children(
                tab.term()
                    .and_then(|t| self.agents.shown(t))
                    .map(|s| self.mark(s)),
            )
            .into_any_element()
    }

    /// A tab in a folded split's strip: its agent's icon, or a terminal's, with its mark on
    /// the corner.
    fn strip_tab(&self, tab: Tab, active: bool, cx: &mut Context<Self>) -> AnyElement {
        let kind = tab.term().and_then(|t| self.agents.get(t)).map(|a| a.kind);
        let icon = match tab {
            Tab::Messages => div()
                .text_size(theme::ui(12.))
                .child("✉")
                .into_any_element(),
            Tab::Term(_) => kind.map_or_else(terminal_icon, agent_icon),
        };
        self.folded_tab(tab, cx)
            .relative()
            .size(theme::ui(24.))
            .justify_center()
            .when(active, |d| d.bg(rgb(theme::TAB_ACTIVE)))
            .child(icon)
            .children(tab.term().and_then(|t| self.agents.shown(t)).map(|s| {
                div()
                    .absolute()
                    .right(px(1.))
                    .bottom(px(1.))
                    .child(self.mark(s))
            }))
            .into_any_element()
    }

    /// A folded tab that opens its split on click and has the tab menu on right click.
    fn folded_tab(&self, tab: Tab, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let key = match tab {
            Tab::Term(t) => t.0,
            Tab::Messages => 0,
        };
        div()
            .id(("folded-tab", key))
            .flex_none()
            .flex()
            .items_center()
            .rounded(px(7.))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(theme::ROW_HOVER)))
            .when_some(tab.term(), |d, t| {
                d.on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |w, ev: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        w.open_menu(For::Tab(t), ev.position, window, cx)
                    }),
                )
            })
            .on_click(cx.listener(move |w, _, window, cx| {
                w.picked.clear();
                w.model.focus_tab(tab);
                w.focus_current(window, cx);
            }))
    }
}

/// The icon for folding toward `to`, or for opening back from it, and the turn that points it
/// that way: its arrow points where the split goes.
pub(super) fn icon((axis, end): (Axis, Spot), open: bool) -> (&'static str, f32) {
    use std::f32::consts::{FRAC_PI_2, PI};
    let path = match (end, open) {
        (Spot::Middle, false) => "ui/fold-middle.svg",
        (Spot::Middle, true) => "ui/unfold-middle.svg",
        (_, false) => "ui/fold.svg",
        (_, true) => "ui/unfold.svg",
    };
    // Unturned, the bar is on top: a top split folds up and opens down.
    let turn = match (axis, end) {
        (Axis::Column, Spot::Start | Spot::Middle) => 0.,
        (Axis::Column, Spot::End) => PI,
        (Axis::Row, Spot::Start) => -FRAC_PI_2,
        (Axis::Row, Spot::Middle | Spot::End) => FRAC_PI_2,
    };
    (path, turn)
}

/// The line before each folded split's tabs, across the bar or strip.
fn rule(row: bool) -> AnyElement {
    div()
        .flex_none()
        .bg(rgb(theme::CARD_BORDER))
        .map(|d| {
            if row {
                d.w(px(16.)).h(px(1.)).my(px(2.))
            } else {
                d.w(px(1.)).h(px(16.)).mx(px(2.))
            }
        })
        .into_any_element()
}

/// The mark of plain terminals, without an agent.
fn terminal_icon() -> AnyElement {
    svg()
        .path("ui/terminal.svg")
        .w(px(12.))
        .h(px(10.))
        .flex_none()
        .text_color(rgb(theme::TEXT))
        .into_any_element()
}
