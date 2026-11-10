//! The sidebar: layouts, their spaces, and the agents in each space.

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, ClickEvent, Context, ElementId, FontWeight, InteractiveElement, IntoElement,
    MouseButton, ParentElement, SharedString, StatefulInteractiveElement, Styled, Window,
    WindowControlArea, div, px, rgb,
};

use super::Workspace;
use super::marks;
use super::rename::Target;
use super::theme;
use crate::status::Status;

struct Row {
    id: ElementId,
    depth: u8,
    label: String,
    /// Some when the row folds: whether it's folded now.
    folded: Option<bool>,
    bold: bool,
    bg: Option<u32>,
    rename: Option<Target>,
    status: Option<Status>,
}

impl Workspace {
    pub(super) fn render_sidebar(&mut self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let mut rows = Vec::new();
        for (i, layout) in self.model.layouts.iter().enumerate() {
            let folded = self.collapsed.contains(&layout.id);
            let active = i == self.model.active;
            rows.push(
                self.row(
                    Row {
                        id: ("layout", layout.id).into(),
                        depth: 0,
                        label: self.layout_label(layout, cx),
                        folded: Some(folded),
                        bold: true,
                        bg: active.then_some(theme::ROW_SELECTED),
                        rename: Some(Target::Layout(layout.id)),
                        status: folded
                            .then(|| {
                                self.worst(
                                    layout.spaces.leaves().into_iter().flat_map(|s| s.terms()),
                                )
                            })
                            .flatten(),
                    },
                    i > 0,
                    layout.id,
                    cx,
                    move |w, _, _| w.model.select_layout(i),
                ),
            );
            if folded {
                continue;
            }
            for space in layout.spaces.leaves() {
                let space_folded = self.collapsed.contains(&space.id);
                let space_id = space.id;
                rows.push(self.row(
                    Row {
                        id: ("space", space.id).into(),
                        depth: 1,
                        label: self.space_label(space, cx),
                        folded: Some(space_folded),
                        bold: false,
                        bg: None,
                        rename: Some(Target::Space(space.id)),
                        status: space_folded.then(|| self.worst(space.terms())).flatten(),
                    },
                    false,
                    space.id,
                    cx,
                    move |w, _, _| w.model.focus_space(space_id),
                ));
                if space_folded {
                    continue;
                }
                for term in space.terms().filter(|t| self.is_agent(*t)) {
                    let focused = active && term == self.model.term();
                    rows.push(self.row(
                        Row {
                            id: ("term", term).into(),
                            depth: 2,
                            label: self.term_label(term, cx),
                            folded: None,
                            bold: false,
                            bg: focused.then_some(theme::ROW_HOVER),
                            rename: None,
                            status: self.status(term),
                        },
                        false,
                        term,
                        cx,
                        move |w, _, _| w.model.focus_term(term),
                    ));
                }
            }
        }

        div()
            .w(px(theme::SIDEBAR_WIDTH))
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
                    .justify_between()
                    .pt(px(4.))
                    .pb(px(8.))
                    .pl(px(20.))
                    .pr(px(18.))
                    .text_size(px(11.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child("LAYOUTS")
                    .child(
                        div()
                            .id("new-layout")
                            .text_size(px(16.))
                            .cursor_pointer()
                            .child("+")
                            .on_click(cx.listener(|w, _, window, cx| {
                                w.apply(window, cx, |m| Some(m.new_layout()))
                            })),
                    ),
            )
            .child(
                div()
                    .id("sidebar-rows")
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .px(px(8.))
                    .overflow_y_scroll()
                    .children(rows),
            )
            .into_any_element()
    }

    fn row(
        &self,
        row: Row,
        gap_above: bool,
        fold_id: u64,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut Workspace, &mut Window, &mut Context<Workspace>) + 'static,
    ) -> AnyElement {
        let renaming = self
            .rename
            .as_ref()
            .filter(|r| Some(r.target) == row.rename);
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
        let chevron = row.folded.map(|folded| {
            div()
                .id(("fold", fold_id))
                .w(px(12.))
                .flex_none()
                .text_size(px(10.))
                .text_color(rgb(theme::ICON))
                .child(if folded { "▸" } else { "▾" })
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |w, _, _, cx| {
                    if !w.collapsed.remove(&fold_id) {
                        w.collapsed.insert(fold_id);
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
            .pl(px(10. + [0., 16., 38.][row.depth as usize]))
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
            .children(chevron)
            .child(label)
            .children(row.status.map(marks::mark))
            .on_click(cx.listener(move |w, ev: &ClickEvent, window, cx| {
                if ev.click_count() == 2
                    && let Some(target) = rename
                {
                    let text = w.current_name(target, cx);
                    w.start_rename(target, text, window, cx);
                    return;
                }
                on_click(w, window, cx);
                w.focus_current(window, cx);
            }))
            .into_any_element()
    }

    fn current_name(&self, target: Target, cx: &gpui::App) -> String {
        let model = &self.model;
        match target {
            Target::Layout(id) => model
                .layouts
                .iter()
                .find(|l| l.id == id)
                .map(|l| self.layout_label(l, cx)),
            Target::Space(id) => model
                .layouts
                .iter()
                .find_map(|l| l.spaces.find(id))
                .map(|s| self.space_label(s, cx)),
        }
        .unwrap_or_default()
    }
}
