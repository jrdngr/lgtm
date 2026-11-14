//! The sidebar: every space and the agents in it, with the layout box at the bottom; or only
//! space badges and layout maps when narrow.

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, ClickEvent, Context, CursorStyle, FontWeight, InteractiveElement,
    IntoElement, MouseButton, MouseDownEvent, ParentElement, SharedString, Stateful,
    StatefulInteractiveElement, Styled, Window, div, px, rgb, svg,
};

use super::actions::ToggleSidebar;
use super::branch::Nothing;
use super::menu::For;
use super::rename::Target;
use super::space_drag::{self, DragsSpace};
use super::{MovesWindow, Workspace, badge};
use crate::agents::Kind;
use crate::ipc::Place;
use crate::model::{Space, SpaceId, TermId};
use crate::theme;

/// Dragged by the sidebar's right edge.
pub(super) struct SidebarDrag;

/// The sidebar width for a drag to `x`: badges only below halfway to the minimum.
pub(super) fn snap(x: f32) -> f32 {
    if x < (theme::SIDEBAR_ICONS + theme::SIDEBAR_MIN) / 2. {
        theme::SIDEBAR_ICONS
    } else {
        x.clamp(theme::SIDEBAR_MIN, theme::SIDEBAR_MAX)
    }
}

impl Workspace {
    /// Whether the sidebar shows only space badges.
    pub(super) fn sidebar_icons(&self) -> bool {
        self.sidebar_width <= theme::SIDEBAR_ICONS
    }

    pub(super) fn render_sidebar(&mut self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let icons = self.sidebar_icons();
        let rows = if icons {
            self.badge_rows(cx)
        } else {
            self.space_rows(cx)
        };
        let plus = div()
            .id("new-space")
            .text_size(px(16.))
            .cursor_pointer()
            .child("+")
            .on_click(cx.listener(|w, _, window, cx| {
                let _ = w.open(None, Place::Layout, None, &[], true, window, cx);
            }));
        let handle = div()
            .id("sidebar-edge")
            .group("sidebar-edge")
            .absolute()
            .top_0()
            .right(px(-4.))
            .w(px(8.))
            .h_full()
            .flex()
            .justify_center()
            .cursor(CursorStyle::ResizeLeftRight)
            .child(
                div()
                    .w(px(2.))
                    .h_full()
                    .group_hover("sidebar-edge", |s| s.bg(rgb(theme::FOCUS))),
            )
            .on_drag(SidebarDrag, |_, _, _, cx| cx.new(|_| Nothing));

        let side = div()
            .relative()
            .w(px(self.sidebar_width))
            .flex_none()
            .h_full()
            .bg(rgb(theme::SIDEBAR))
            .border_r_1()
            .border_color(rgb(theme::SIDEBAR_BORDER))
            .flex()
            .flex_col()
            .child(
                div()
                    .id("sidebar-titlebar")
                    .h(px(theme::TITLEBAR))
                    .flex_none()
                    .moves_window(),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .pt(px(4.))
                    .pb(px(8.))
                    .text_size(px(11.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT_MUTED))
                    .when(icons, |d| d.justify_center())
                    .when(!icons, |d| {
                        d.justify_between().pl(px(20.)).pr(px(18.)).child("SPACES")
                    })
                    .child(plus),
            )
            .child(
                div()
                    .id("sidebar-rows")
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .when(icons, |d| d.items_center().gap(px(6.)))
                    .when(!icons, |d| d.gap(px(1.)).px(px(8.)))
                    .overflow_y_scroll()
                    .children(rows),
            )
            .child(self.layout_box(icons, cx))
            .child(handle);
        space_drag::track_sidebar(side, cx).into_any_element()
    }

    /// One badge per space, with its worst status on the corner.
    fn badge_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let shown = self.model.layout().spaces();
        self.model
            .spaces
            .iter()
            .map(|space| {
                let id = space.id;
                let on = shown.contains(&id);
                let status = self.agents.worst(space.terms());
                let ring = if on {
                    theme::ROW_SELECTED
                } else {
                    theme::SIDEBAR
                };
                let label = self.space_label(space);
                let el = div()
                    .id(("space-badge", id.0))
                    .relative()
                    .w(px(36.))
                    .h(px(32.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.))
                    .when(on, |d| d.bg(rgb(theme::ROW_SELECTED)))
                    .when(!on, |d| d.hover(|s| s.bg(rgb(theme::ROW_HOVER))))
                    .cursor_pointer()
                    .child(badge::badge(
                        space.color,
                        &label,
                        status.map(|s| self.mark(s)),
                        ring,
                    ))
                    .children(
                        self.space_number(id)
                            .map(|n| badge::chip(n).absolute().top(px(-3.)).right(px(-5.))),
                    )
                    .space_row(id, label, cx);
                space_drag::track_row(el, id, cx).into_any_element()
            })
            .collect()
    }

    /// Each space, then the agents in it unless it's folded.
    fn space_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut rows = Vec::new();
        for (i, space) in self.model.spaces.iter().enumerate() {
            rows.push(self.space_row(space, i > 0, cx));
            if self.collapsed.contains(&space.id.0) {
                continue;
            }
            for term in space.terms().filter(|t| self.is_agent((*t).into())) {
                rows.push(self.agent_row(term, cx));
            }
        }
        rows
    }

    fn space_row(&self, space: &Space, gap_above: bool, cx: &mut Context<Self>) -> AnyElement {
        let id = space.id;
        let layout = self.model.layout();
        let on = layout.slot_of(id).is_some();
        let focused = layout.slot().space == Some(id);
        let agents: Vec<TermId> = space
            .terms()
            .filter(|t| self.is_agent((*t).into()))
            .collect();
        let folded = self.collapsed.contains(&id.0);
        let label = self.space_label(space);
        let group = SharedString::from(format!("space-row-{}", id.0));
        let chevron = div()
            .id(("fold", id.0))
            .w(px(12.))
            .flex_none()
            .text_size(px(10.))
            .text_color(gpui::transparent_black())
            .when(!agents.is_empty(), |d| {
                d.group_hover(group.clone(), |s| s.text_color(rgb(theme::ICON)))
                    .child(if folded { "▸" } else { "▾" })
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |w, _, win, cx| {
                        if !w.collapsed.remove(&id.0) {
                            w.collapsed.insert(id.0);
                        }
                        if w.renaming().is_some() {
                            w.focus_current(win, cx);
                        }
                        cx.stop_propagation();
                        cx.notify();
                    }))
            });
        let status = folded
            .then(|| self.agents.worst(agents.iter().copied()))
            .flatten();
        let el = div()
            .id(("space-row", id.0))
            .group(group)
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(28.))
            .flex_none()
            .pl(px(4.))
            .pr(px(10.))
            .rounded(px(6.))
            .text_size(px(13.))
            .text_color(rgb(theme::TEXT))
            .when(focused, |d| d.font_weight(FontWeight::SEMIBOLD))
            .when(gap_above, |d| d.mt(px(6.)))
            .when(on, |d| d.bg(rgb(theme::ROW_SELECTED)))
            .when(!on, |d| d.hover(|s| s.bg(rgb(theme::ROW_HOVER))))
            .cursor_pointer()
            .when(self.dragged_space(cx) == Some(id), |d| d.opacity(0.35))
            .child(chevron)
            .child(badge::badge(space.color, &label, None, theme::SIDEBAR))
            .child(self.row_label(Target::Space(id), label.clone(), cx))
            .map(|d| match self.space_number(id) {
                Some(n) => d.child(badge::chip(n)),
                None => d.children(status.map(|s| self.mark(s))),
            })
            .space_row(id, label, cx);
        space_drag::track_row(el, id, cx).into_any_element()
    }

    fn agent_row(&self, term: TermId, cx: &mut Context<Self>) -> AnyElement {
        let focused = Some(term) == self.model.term();
        div()
            .id(("term", term.0))
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(28.))
            .flex_none()
            // Lines the icon up under the space row's badge.
            .pl(px(24.))
            .pr(px(10.))
            .rounded(px(6.))
            .text_size(px(13.))
            .text_color(rgb(theme::TEXT))
            .when(focused, |d| d.bg(rgb(theme::ROW_HOVER)))
            .when(!focused, |d| d.hover(|s| s.bg(rgb(theme::ROW_HOVER))))
            .cursor_pointer()
            .child(
                div()
                    .w(px(20.))
                    .flex_none()
                    .flex()
                    .justify_center()
                    .children(self.agents.get(term).map(|a| agent_icon(a.kind))),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .child(self.term_label(term.into())),
            )
            .children(self.waiting_mark(term))
            .children(self.agents.shown(term).map(|s| self.mark(s)))
            .on_click(cx.listener(move |w, _, window, cx| {
                w.model.focus_term(term);
                w.focus_current(window, cx);
            }))
            .into_any_element()
    }

    /// A row's name, or the field renaming it.
    pub(super) fn row_label(
        &self,
        target: Target,
        label: String,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match self.renaming().filter(|r| r.target == target) {
            Some(r) => div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .track_focus(&r.focus)
                .on_key_down(cx.listener(Self::rename_key))
                .child(SharedString::from(r.text.clone()))
                .child(div().w(px(1.5)).h(px(15.)).bg(rgb(theme::FOCUS)))
                .into_any_element(),
            None => div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .child(label)
                .into_any_element(),
        }
    }
}

trait SpaceRow: Sized {
    /// Shows the space on click, renames it on double click, opens its menu on right click,
    /// and drags it.
    fn space_row(self, id: SpaceId, label: String, cx: &mut Context<Workspace>) -> Self;
}

impl SpaceRow for Stateful<gpui::Div> {
    fn space_row(self, id: SpaceId, label: String, cx: &mut Context<Workspace>) -> Self {
        self.on_mouse_down(
            MouseButton::Right,
            cx.listener(move |w, ev: &MouseDownEvent, window, cx| {
                w.open_menu(For::Space(id, None), ev.position, window, cx)
            }),
        )
        .on_click(cx.listener(move |w, ev: &ClickEvent, window, cx| {
            if ev.click_count() == 2 {
                let text = w
                    .model
                    .find_space(id)
                    .map(|s| w.space_label(s))
                    .unwrap_or_default();
                w.start_rename(Target::Space(id), text, window, cx);
                return;
            }
            w.model.show_space(id);
            w.focus_current(window, cx);
        }))
        .drags_space(id, label)
    }
}

impl Workspace {
    /// The button that shows or hides the sidebar: at the sidebar header's right edge, over
    /// the new space button, or just right of the traffic lights when the sidebar is hidden or
    /// shows only badges.
    pub(super) fn toggle_left(&self) -> f32 {
        if self.sidebar && !self.sidebar_icons() {
            self.sidebar_width - 37.
        } else {
            78.
        }
    }

    pub(super) fn render_sidebar_toggle(&self) -> AnyElement {
        let left = self.toggle_left();
        let icon = if self.sidebar {
            "ui/sidebar-open.svg"
        } else {
            "ui/sidebar-closed.svg"
        };
        div()
            .id("sidebar-toggle")
            .absolute()
            .top(px(12.))
            .left(px(left))
            .size(px(22.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(5.))
            .occlude()
            .cursor_pointer()
            .group("sidebar-toggle")
            .hover(|s| s.bg(rgb(theme::ROW_HOVER)))
            .child(
                svg()
                    .path(icon)
                    .size(px(16.))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .group_hover("sidebar-toggle", |s| s.text_color(rgb(theme::TEXT))),
            )
            .on_click(|_, window, cx| window.dispatch_action(Box::new(ToggleSidebar), cx))
            .into_any_element()
    }
}

/// The agent's mark, in its own color.
fn agent_icon(kind: Kind) -> AnyElement {
    let (path, color) = match kind {
        Kind::Claude => ("agents/claude.svg", theme::CLAUDE),
        Kind::Codex => ("agents/codex.svg", theme::TEXT),
        Kind::Pi => ("agents/pi.svg", theme::TEXT),
    };
    svg()
        .path(path)
        .size(px(13.))
        .flex_none()
        .text_color(rgb(color))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::snap;
    use crate::theme::{SIDEBAR_ICONS, SIDEBAR_MAX, SIDEBAR_MIN};

    #[test]
    fn drags_snap_to_badges_or_clamp() {
        assert_eq!(snap(20.), SIDEBAR_ICONS);
        assert_eq!(snap(120.), SIDEBAR_ICONS);
        assert_eq!(snap(140.), SIDEBAR_MIN);
        assert_eq!(snap(300.), 300.);
        assert_eq!(snap(900.), SIDEBAR_MAX);
    }
}
