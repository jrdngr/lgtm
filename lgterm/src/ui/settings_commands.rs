//! The Settings rows where the user sets the command that starts each agent.

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    StatefulInteractiveElement, Styled, div, px, rgb,
};

use super::Workspace;
use super::overlay::Overlay;
use crate::agents::Kind;
use crate::field::{self, Key};
use crate::fonts;
use crate::theme;

const AGENTS: &[(Kind, &str)] = &[
    (Kind::Claude, "Claude command"),
    (Kind::Codex, "Codex command"),
];

impl Workspace {
    fn editing_command(&self) -> Option<Kind> {
        match &self.overlay {
            Some(Overlay::Settings { editing, .. }) => *editing,
            _ => None,
        }
    }

    fn edit_command(&mut self, kind: Option<Kind>, cx: &mut Context<Self>) {
        if let Some(Overlay::Settings { editing, .. }) = &mut self.overlay {
            *editing = kind;
        }
        cx.notify();
    }

    /// Types into the command being edited. Returns whether the key was taken.
    pub(super) fn command_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let Some(kind) = self.editing_command() else {
            return false;
        };
        let Some(text) = self.settings.agent_command_mut(kind) else {
            return false;
        };
        match field::key(text, &ev.keystroke) {
            Key::Other => return false,
            Key::Edited => self.settings.save(),
            Key::Enter | Key::Escape => {
                if let Some(text) = self.settings.agent_command_mut(kind) {
                    *text = text.trim().to_owned();
                }
                self.settings.save();
                self.edit_command(None, cx);
            }
            _ => {}
        }
        cx.stop_propagation();
        cx.notify();
        true
    }

    pub(super) fn command_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let editing = self.editing_command();
        AGENTS
            .iter()
            .map(|&(kind, label)| {
                let on = editing == Some(kind);
                let shown = if on {
                    let mut settings = self.settings.clone();
                    let typed = settings.agent_command_mut(kind).map_or("", |t| t.as_str());
                    format!("{typed}▏")
                } else {
                    self.settings.agent_command(kind).to_owned()
                };
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .h(px(36.))
                    .text_size(px(13.))
                    .text_color(rgb(theme::TEXT))
                    .border_b_1()
                    .border_color(rgb(0x333333))
                    .child(label)
                    .child(
                        div()
                            .id(label)
                            .w(px(200.))
                            .h(px(24.))
                            .px(px(8.))
                            .flex()
                            .items_center()
                            .rounded(px(6.))
                            .border_1()
                            .bg(rgb(theme::CARD))
                            .font_family(fonts::mono())
                            .text_size(px(12.))
                            .cursor_text()
                            .when(on, |d| d.border_color(rgb(theme::FOCUS)))
                            .when(!on, |d| d.border_color(rgb(theme::CARD_BORDER)))
                            .child(shown)
                            .on_click(cx.listener(move |w, _, _, cx| {
                                let next = (w.editing_command() != Some(kind)).then_some(kind);
                                w.edit_command(next, cx);
                            })),
                    )
                    .into_any_element()
            })
            .collect()
    }
}
