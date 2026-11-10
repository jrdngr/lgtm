//! The sidebar: layouts, their spaces, and the agents in each space; or only layout badges when narrow.

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, ClickEvent, Context, CursorStyle, ElementId, FontWeight,
    InteractiveElement, IntoElement, MouseButton, ParentElement, Render, SharedString, Stateful,
    StatefulInteractiveElement, Styled, Window, WindowControlArea, div, px, rgb,
};

use super::panes::Nothing;
use super::rename::Target;
use super::{Workspace, badge};
use crate::agents::Status;
use crate::ipc::Place;
use crate::model::Tab;
use crate::theme;

struct Row {
    id: ElementId,
    depth: u8,
    badge: Option<AnyElement>,
    label: String,
    /// Some when the row folds: whether it's folded now.
    folded: Option<bool>,
    bold: bool,
    bg: Option<u32>,
    rename: Option<Target>,
    status: Option<Status>,
    /// Some for a layout row: its index, so it drags to reorder.
    layout: Option<usize>,
}

/// A layout being dragged to a new place in the sidebar.
pub(super) struct LayoutDrag {
    index: usize,
    label: String,
}

impl Render for LayoutDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui::IntoElement {
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(28.))
            .px(px(10.))
            .rounded(px(6.))
            .bg(rgb(theme::ROW_SELECTED))
            .border_1()
            .border_color(rgb(theme::CARD_BORDER))
            .text_size(px(13.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(theme::TEXT))
            .child(badge::badge(
                self.index,
                &self.label,
                None,
                theme::ROW_SELECTED,
            ))
            .child(self.label.clone())
    }
}

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
    /// Whether the sidebar shows only layout badges.
    pub(super) fn sidebar_icons(&self) -> bool {
        self.sidebar_width <= theme::SIDEBAR_ICONS
    }

    pub(super) fn render_sidebar(&mut self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let icons = self.sidebar_icons();
        let rows = if icons {
            self.badge_rows(cx)
        } else {
            self.tree_rows(cx)
        };
        let plus = div()
            .id("new-layout")
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

        div()
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
                    .h(px(theme::TITLEBAR))
                    .flex_none()
                    .window_control_area(WindowControlArea::Drag),
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
                        d.justify_between().pl(px(20.)).pr(px(18.)).child("LAYOUTS")
                    })
                    .child(plus),
            )
            .child(
                div()
                    .id("sidebar-rows")
                    .flex()
                    .flex_col()
                    .when(icons, |d| d.items_center().gap(px(6.)))
                    .when(!icons, |d| d.gap(px(1.)).px(px(8.)))
                    .overflow_y_scroll()
                    .children(rows),
            )
            .child(handle)
            .into_any_element()
    }

    /// One badge per layout, with its worst status on the corner.
    fn badge_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut rows = Vec::new();
        for (i, layout) in self.model.layouts.iter().enumerate() {
            let active = i == self.model.active;
            let status = self.agents.worst(layout.terms());
            let ring = if active {
                theme::ROW_SELECTED
            } else {
                theme::SIDEBAR
            };
            let label = self.layout_label(layout);
            rows.push(
                div()
                    .id(("layout", layout.id.0))
                    .w(px(36.))
                    .h(px(32.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.))
                    .when(active, |d| d.bg(rgb(theme::ROW_SELECTED)))
                    .when(!active, |d| d.hover(|s| s.bg(rgb(theme::ROW_HOVER))))
                    .cursor_pointer()
                    .child(badge::badge(i, &label, status.map(|s| self.mark(s)), ring))
                    .reorders(i, &label, cx)
                    .on_click(cx.listener(move |w, _, window, cx| {
                        w.model.select_layout(i);
                        w.focus_current(window, cx);
                    }))
                    .into_any_element(),
            );
        }
        rows
    }

    /// Layouts, their spaces, and the agents in each. A layout with one space has no space row.
    fn tree_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut rows = Vec::new();
        for (i, layout) in self.model.layouts.iter().enumerate() {
            let folded = self.collapsed.contains(&layout.id.0);
            let active = i == self.model.active;
            let spaces = layout.spaces.leaves();
            let single = spaces.len() == 1;
            let has_agents = spaces
                .iter()
                .any(|s| s.terms().any(|t| self.is_agent(Tab::Term(t))));
            let label = self.layout_label(layout);
            rows.push(self.row(
                Row {
                    id: ("layout", layout.id.0).into(),
                    depth: 0,
                    badge: Some(badge::badge(i, &label, None, theme::SIDEBAR)),
                    label,
                    folded: (!single || has_agents).then_some(folded),
                    bold: true,
                    bg: active.then_some(theme::ROW_SELECTED),
                    rename: Some(Target::Layout(layout.id)),
                    status: folded.then(|| self.agents.worst(layout.terms())).flatten(),
                    layout: Some(i),
                },
                i > 0,
                layout.id.0,
                cx,
                move |w, _, _| w.model.select_layout(i),
            ));
            if folded {
                continue;
            }
            for space in spaces {
                let space_folded = !single && self.collapsed.contains(&space.id.0);
                let space_id = space.id;
                if !single {
                    rows.push(
                        self.row(
                            Row {
                                id: ("space", space.id.0).into(),
                                depth: 1,
                                badge: None,
                                label: self.space_label(space),
                                folded: Some(space_folded),
                                bold: false,
                                bg: None,
                                rename: Some(Target::Space(space.id)),
                                status: space_folded
                                    .then(|| self.agents.worst(space.terms()))
                                    .flatten(),
                                layout: None,
                            },
                            false,
                            space.id.0,
                            cx,
                            move |w, _, _| w.model.focus_space(space_id),
                        ),
                    );
                }
                if space_folded {
                    continue;
                }
                for term in space.terms().filter(|t| self.is_agent(Tab::Term(*t))) {
                    let focused = active && Some(term) == self.model.term();
                    rows.push(self.row(
                        Row {
                            id: ("term", term.0).into(),
                            depth: if single { 3 } else { 2 },
                            badge: None,
                            label: self.term_label(Tab::Term(term)),
                            folded: None,
                            bold: false,
                            bg: focused.then_some(theme::ROW_HOVER),
                            rename: None,
                            status: self.agents.status(term),
                            layout: None,
                        },
                        false,
                        term.0,
                        cx,
                        move |w, _, _| w.model.focus_term(term),
                    ));
                }
            }
        }
        rows
    }

    fn row(
        &self,
        row: Row,
        gap_above: bool,
        fold_id: u64,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut Workspace, &mut Window, &mut Context<Workspace>) + 'static,
    ) -> AnyElement {
        let layout = row.layout.map(|i| (i, row.label.clone()));
        let renaming = self.renaming().filter(|r| Some(r.target) == row.rename);
        let label: AnyElement = match renaming {
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
                .child(row.label)
                .into_any_element(),
        };
        let rename = row.rename;
        let slot =
            (row.folded.is_none() && row.badge.is_some()).then(|| div().w(px(12.)).flex_none());
        let chevron = row.folded.map(|folded| {
            div()
                .id(("fold", fold_id))
                .w(px(12.))
                .flex_none()
                .text_size(px(10.))
                .text_color(rgb(theme::ICON))
                .child(if folded { "▸" } else { "▾" })
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |w, _, win, cx| {
                    if !w.collapsed.remove(&fold_id) {
                        w.collapsed.insert(fold_id);
                    }
                    if w.renaming().is_some() {
                        w.focus_current(win, cx);
                    }
                    cx.stop_propagation();
                    cx.notify();
                }))
        });
        div()
            .id(row.id)
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(28.))
            .flex_none()
            .pr(px(10.))
            .pl(px(10. + [0., 16., 38., 56.][row.depth as usize]))
            .rounded(px(6.))
            .text_size(px(13.))
            .text_color(rgb(theme::TEXT))
            .when(row.bold, |d| d.font_weight(FontWeight::SEMIBOLD))
            .when(gap_above, |d| d.mt(px(6.)))
            .when_some(row.bg, |d, bg| d.bg(rgb(bg)))
            .when(row.bg.is_none(), |d| {
                d.hover(|s| s.bg(rgb(theme::ROW_HOVER)))
            })
            .cursor_pointer()
            .children(slot)
            .children(chevron)
            .children(row.badge)
            .child(label)
            .children(row.status.map(|s| self.mark(s)))
            .when_some(layout, |d, (i, label)| d.reorders(i, &label, cx))
            .on_click(cx.listener(move |w, ev: &ClickEvent, window, cx| {
                if ev.click_count() == 2
                    && let Some(target) = rename
                {
                    let text = w.current_name(target);
                    w.start_rename(target, text, window, cx);
                    return;
                }
                on_click(w, window, cx);
                w.focus_current(window, cx);
            }))
            .into_any_element()
    }

    fn current_name(&self, target: Target) -> String {
        match target {
            Target::Layout(id) => self
                .model
                .layouts
                .iter()
                .find(|l| l.id == id)
                .map(|l| self.layout_label(l)),
            Target::Space(id) => self.model.find_space(id).map(|s| self.space_label(s)),
        }
        .unwrap_or_default()
    }
}

trait Reorders {
    /// Drags the layout at `index` and takes dropped layouts into its place.
    fn reorders(self, index: usize, label: &str, cx: &mut Context<Workspace>) -> Self;
}

impl Reorders for Stateful<gpui::Div> {
    fn reorders(self, index: usize, label: &str, cx: &mut Context<Workspace>) -> Self {
        let drag = LayoutDrag {
            index,
            label: label.to_owned(),
        };
        self.on_drag(drag, |d, _, _, cx| {
            cx.new(|_| LayoutDrag {
                index: d.index,
                label: d.label.clone(),
            })
        })
        .drag_over::<LayoutDrag>(move |s, d, _, _| match d.index {
            from if from > index => s.border_t_2().border_color(rgb(theme::FOCUS)),
            from if from < index => s.border_b_2().border_color(rgb(theme::FOCUS)),
            _ => s,
        })
        .on_drop(cx.listener(move |w, d: &LayoutDrag, window, cx| {
            w.model.move_layout(d.index, index);
            w.focus_current(window, cx);
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::snap;
    use crate::theme::{SIDEBAR_ICONS, SIDEBAR_MAX, SIDEBAR_MIN};

    #[test]
    fn drags_snap_to_badges_or_clamp() {
        assert_eq!(snap(20.), SIDEBAR_ICONS);
        assert_eq!(snap(110.), SIDEBAR_ICONS);
        assert_eq!(snap(120.), SIDEBAR_MIN);
        assert_eq!(snap(300.), 300.);
        assert_eq!(snap(900.), SIDEBAR_MAX);
    }
}
