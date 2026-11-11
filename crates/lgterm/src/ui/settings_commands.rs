//! The Settings rows where the user sets the command that starts each agent.

use gpui::{
    AnyElement, AppContext, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement,
    Styled, Subscription, Window, div, px, rgb,
};

use super::Workspace;
use super::overlay::Overlay;
use crate::agents::Kind;
use crate::fonts;
use crate::input::{InputEvent, TextInput};
use crate::theme;

const AGENTS: &[(Kind, &str)] = &[
    (Kind::Claude, "Claude command"),
    (Kind::Codex, "Codex command"),
];

/// A command's text box.
pub struct CommandBox {
    kind: Kind,
    input: Entity<TextInput>,
    _changes: Subscription,
}

impl Workspace {
    /// Text boxes holding the agents' commands; each edit is saved as it's typed.
    pub(super) fn command_boxes(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<CommandBox> {
        AGENTS
            .iter()
            .map(|&(kind, _)| {
                let text = self.settings.agent_command(kind).to_owned();
                let input = cx.new(|cx| TextInput::new(text, cx));
                let changes =
                    cx.subscribe_in(&input, window, move |w, input, ev, window, cx| match ev {
                        InputEvent::Changed => {
                            let text = input.read(cx).text().to_owned();
                            if let Some(command) = w.settings.agent_command_mut(kind) {
                                *command = text;
                            }
                            w.settings.save();
                        }
                        InputEvent::Submit => {
                            w.save_commands(cx);
                            if let Some(Overlay::Settings { focus, .. }) = &w.overlay {
                                window.focus(focus, cx);
                            }
                        }
                    });
                CommandBox {
                    kind,
                    input,
                    _changes: changes,
                }
            })
            .collect()
    }

    /// Trims the commands and saves Settings.
    pub(super) fn save_commands(&mut self, cx: &mut Context<Self>) {
        if let Some(Overlay::Settings { commands, .. }) = &self.overlay {
            for b in commands {
                let text = b.input.read(cx).text().trim().to_owned();
                b.input.update(cx, |i, cx| i.set_text(text.clone(), cx));
                if let Some(command) = self.settings.agent_command_mut(b.kind) {
                    *command = text;
                }
            }
        }
        self.settings.save();
    }

    pub(super) fn command_rows(&self, window: &Window, cx: &Context<Self>) -> Vec<AnyElement> {
        let Some(Overlay::Settings { commands, .. }) = &self.overlay else {
            return Vec::new();
        };
        commands
            .iter()
            .filter_map(|b| {
                let label = AGENTS.iter().find(|(k, _)| *k == b.kind)?.1;
                let focus: FocusHandle = b.input.focus_handle(cx);
                let on = focus.is_focused(window);
                let row = div()
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
                            .w(px(260.))
                            .h(px(24.))
                            .px(px(8.))
                            .rounded(px(6.))
                            .border_1()
                            .border_color(rgb(if on { theme::FOCUS } else { theme::CARD_BORDER }))
                            .bg(rgb(theme::CARD))
                            .font_family(fonts::mono())
                            .text_size(px(12.))
                            .line_height(px(16.))
                            .child(b.input.clone()),
                    );
                Some(row.into_any_element())
            })
            .collect()
    }
}
