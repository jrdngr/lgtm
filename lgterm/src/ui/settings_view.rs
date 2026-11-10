//! The Settings dialog.

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, KeyDownEvent, MouseButton,
    ParentElement, StatefulInteractiveElement, Styled, Window, div, px, rgb,
};

use super::palette::overlay;
use super::{Workspace, theme};
use crate::settings::Settings;
use crate::terminal::FontSize;

type Toggle = (&'static str, fn(&mut Settings) -> &mut bool);

const ATTENTION: &[Toggle] = &[
    ("Dock badge", |s| &mut s.dock_badge),
    ("Notify when blocked", |s| &mut s.notify_blocked),
    ("Notify when done", |s| &mut s.notify_done),
    ("Sound", |s| &mut s.sound),
];

const SESSIONS: &[Toggle] = &[
    ("Restore layouts on launch", |s| &mut s.restore),
    ("Resume agent sessions", |s| &mut s.resume),
];

impl Workspace {
    pub(super) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_open.take().is_some() {
            self.focus_current(window, cx);
            return;
        }
        self.palette = None;
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        self.settings_open = Some(focus);
        cx.notify();
    }

    fn settings_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if ev.keystroke.key == "escape" {
            self.settings_open = None;
            self.focus_current(window, cx);
            cx.stop_propagation();
        }
    }

    fn toggle(&mut self, get: fn(&mut Settings) -> &mut bool, cx: &mut Context<Self>) {
        let v = get(&mut self.settings);
        *v = !*v;
        self.settings.save();
        self.update_badge();
        cx.notify();
    }

    pub(super) fn render_settings(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let focus = self.settings_open.as_ref()?;
        let mut settings = self.settings.clone();
        let section = |title: &'static str| {
            div()
                .mt(px(18.))
                .mb(px(6.))
                .text_size(px(11.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(theme::TEXT_MUTED))
                .child(title)
        };
        let mut toggles = |list: &'static [Toggle], cx: &mut Context<Self>| {
            list.iter()
                .map(|(label, get)| {
                    let get = *get;
                    let on = *get(&mut settings);
                    div()
                        .id(*label)
                        .flex()
                        .items_center()
                        .justify_between()
                        .h(px(36.))
                        .text_size(px(13.))
                        .text_color(rgb(theme::TEXT))
                        .border_b_1()
                        .border_color(rgb(0x333333))
                        .cursor_pointer()
                        .child(*label)
                        .child(switch(on))
                        .on_click(cx.listener(move |w, _, _, cx| w.toggle(get, cx)))
                        .into_any_element()
                })
                .collect::<Vec<_>>()
        };
        let attention = toggles(ATTENTION, cx);
        let sessions = toggles(SESSIONS, cx);
        let size = cx.global::<FontSize>().0;
        let step = |id: &'static str, label: &'static str, by: f32, cx: &mut Context<Self>| {
            div()
                .id(id)
                .w(px(26.))
                .h(px(24.))
                .flex()
                .items_center()
                .justify_center()
                .bg(rgb(theme::CARD))
                .text_color(rgb(0xcfcfcf))
                .cursor_pointer()
                .child(label)
                .on_click(cx.listener(move |w, _, _, cx| {
                    let size = cx.global::<FontSize>().0 + by;
                    w.set_font_size(size, cx);
                }))
        };
        let dialog = div()
            .id("settings")
            .w(px(520.))
            .bg(rgb(theme::DIALOG))
            .border_1()
            .border_color(rgb(theme::CARD_BORDER))
            .rounded(px(12.))
            .pt(px(18.))
            .px(px(24.))
            .pb(px(22.))
            .track_focus(focus)
            .on_key_down(cx.listener(Self::settings_key))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT_STRONG))
                            .child("Settings"),
                    )
                    .child(
                        div()
                            .id("close-settings")
                            .text_size(px(18.))
                            .text_color(rgb(theme::TEXT_MUTED))
                            .cursor_pointer()
                            .child("×")
                            .on_click(cx.listener(|w, _, window, cx| {
                                w.settings_open = None;
                                w.focus_current(window, cx);
                            })),
                    ),
            )
            .child(section("ATTENTION"))
            .children(attention)
            .child(section("SESSIONS"))
            .children(sessions)
            .child(section("TERMINAL"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .h(px(36.))
                    .text_size(px(13.))
                    .text_color(rgb(theme::TEXT))
                    .child("Font size")
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .border_1()
                            .border_color(rgb(theme::CARD_BORDER))
                            .rounded(px(6.))
                            .overflow_hidden()
                            .child(step("font-smaller", "−", -1., cx))
                            .child(
                                div()
                                    .w(px(34.))
                                    .flex()
                                    .justify_center()
                                    .text_size(px(12.))
                                    .child(format!("{size}")),
                            )
                            .child(step("font-bigger", "+", 1., cx)),
                    ),
            );
        Some(overlay("settings-overlay", dialog.into_any_element(), cx))
    }
}

fn switch(on: bool) -> AnyElement {
    div()
        .w(px(30.))
        .h(px(18.))
        .flex_none()
        .rounded(px(9.))
        .p(px(2.))
        .flex()
        .when(on, |d| d.justify_end())
        .bg(rgb(if on { theme::FOCUS } else { 0x4a4a4a }))
        .child(div().size(px(14.)).rounded_full().bg(rgb(0xffffff)))
        .into_any_element()
}
