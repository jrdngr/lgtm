//! Right-click menus: a tab bar's, a space's, an empty slot's, and a layout's; and the layout
//! picker's list.

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, Pixels, Point, StatefulInteractiveElement, Styled, Window, anchored, deferred,
    div, px, rgb,
};

use super::Workspace;
use super::actions::{self, COMMANDS};
use super::overlay::Overlay;
use super::rename::Target;
use crate::agents::Kind;
use crate::ipc::Place;
use crate::model::{COLORS, LayoutId, Model, SlotId, SpaceId, SplitId};
use crate::theme;

/// What a menu is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum For {
    Tabs(SplitId),
    /// A space, and the slot whose title it was opened on.
    Space(SpaceId, Option<SlotId>),
    /// An empty slot.
    Slot(SlotId),
    Layout(LayoutId),
    /// The layout picker's list.
    Layouts,
}

pub struct Menu {
    pub at: Point<Pixels>,
    pub target: For,
}

#[derive(Clone, Copy)]
enum Item {
    NewTerminal,
    NewAgent(Kind),
    NewSpace,
    Messages,
    Rename,
    RemoveFromWindow,
    RemoveSplit,
    Delete,
    PickLayout(usize),
}

/// The first key of the palette command named `command`, as the palette shows it.
fn key(command: &str) -> String {
    COMMANDS
        .iter()
        .find(|c| c.name == command)
        .and_then(|c| c.keys.first())
        .map(|k| actions::display(k).concat())
        .unwrap_or_default()
}

impl Workspace {
    pub(super) fn open_menu(
        &mut self,
        target: For,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.overlay_has_keyboard() {
            self.close_overlay(window, cx);
        }
        self.overlay = Some(Overlay::Menu(Menu { at, target }));
        cx.notify();
    }

    fn close_menu(&mut self, cx: &mut Context<Self>) {
        if matches!(self.overlay, Some(Overlay::Menu(_))) {
            self.overlay = None;
            cx.notify();
        }
    }

    fn run_menu(&mut self, item: Item, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Overlay::Menu(menu)) = self.overlay.take() else {
            return;
        };
        match (menu.target, item) {
            (For::Tabs(split), _) => {
                self.model.focus_split(split);
                let place = match item {
                    Item::NewSpace => Place::SpaceRight,
                    Item::Messages => return self.apply(window, cx, Model::show_messages),
                    Item::NewAgent(kind) => return self.open_agent(kind, window, cx),
                    _ => Place::Tab,
                };
                let _ = self.open(None, place, None, &[], true, window, cx);
            }
            (For::Space(id, _), Item::Rename) => {
                let text = self
                    .model
                    .find_space(id)
                    .map(|s| self.space_label(s))
                    .unwrap_or_default();
                self.start_rename(Target::Space(id), text, window, cx);
            }
            (For::Space(id, _), Item::RemoveFromWindow) => {
                self.apply(window, cx, |m| m.unassign(id))
            }
            (For::Space(_, Some(slot)) | For::Slot(slot), Item::RemoveSplit) => {
                self.remove_slot(slot, window, cx)
            }
            (For::Layout(id), Item::Rename) => {
                if let Some(i) = self.model.layouts.iter().position(|l| l.id == id) {
                    self.model.select_layout(i);
                    let text = self.layout_label(i);
                    self.start_rename(Target::Layout(id), text, window, cx);
                }
            }
            (For::Layout(id), Item::Delete) => self.apply(window, cx, |m| m.delete_layout(id)),
            (_, Item::PickLayout(i)) => self.apply(window, cx, |m| m.select_layout(i)),
            _ => cx.notify(),
        }
    }

    /// Whether a slot can come out of the layout: it isn't the only one, and the terminals that
    /// close with it aren't the last ones.
    fn removable(&self, slot: SlotId) -> bool {
        let slots = &self.model.layout().slots;
        let space = slots.find(slot).and_then(|s| s.space);
        slots.leaves().len() > 1 && (space.is_none() || self.model.spaces.len() > 1)
    }

    /// Closes the terminals of the space in a slot, then takes the slot out of the layout.
    fn remove_slot(&mut self, slot: SlotId, window: &mut Window, cx: &mut Context<Self>) {
        let space = self.model.layout().slots.find(slot).and_then(|s| s.space);
        let tabs: Vec<_> = space
            .and_then(|id| self.model.find_space(id))
            .map(|s| s.tabs().collect())
            .unwrap_or_default();
        for tab in tabs {
            self.close(tab, window, cx);
        }
        self.apply(window, cx, |m| {
            m.remove_slot(slot);
        });
    }

    fn set_color(&mut self, space: SpaceId, color: usize, cx: &mut Context<Self>) {
        self.overlay = None;
        self.model.set_color(space, color);
        cx.notify();
    }

    fn items(&self, target: For) -> Vec<(Item, String, String)> {
        let named = |item, name: &str, command: &str| (item, name.to_owned(), key(command));
        match target {
            For::Tabs(_) => vec![
                named(Item::NewTerminal, "New terminal", "New tab"),
                named(Item::NewAgent(Kind::Claude), "New claude", ""),
                named(Item::NewAgent(Kind::Codex), "New codex", ""),
                named(Item::NewSpace, "New space", ""),
                named(Item::Messages, "Messages", "Messages"),
            ],
            For::Space(id, slot) => {
                let mut items = vec![named(Item::Rename, "Rename", "")];
                if self.model.layout().slot_of(id).is_some() {
                    items.push(named(Item::RemoveFromWindow, "Remove from window", ""));
                }
                if slot.is_some_and(|s| self.removable(s)) {
                    items.push(named(Item::RemoveSplit, "Remove split", ""));
                }
                items
            }
            For::Slot(slot) => [named(Item::RemoveSplit, "Remove split", "")]
                .into_iter()
                .filter(|_| self.removable(slot))
                .collect(),
            For::Layout(_) => {
                let mut items = vec![named(Item::Rename, "Rename", "")];
                if self.model.layouts.len() > 1 {
                    items.push(named(Item::Delete, "Delete", ""));
                }
                items
            }
            For::Layouts => (0..self.model.layouts.len())
                .map(|i| (Item::PickLayout(i), self.layout_label(i), String::new()))
                .collect(),
        }
    }

    pub(super) fn render_menu(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let Some(Overlay::Menu(menu)) = &self.overlay else {
            return None;
        };
        let target = menu.target;
        let rows = self
            .items(target)
            .into_iter()
            .enumerate()
            .map(|(i, (item, name, key))| {
                let group = format!("menu-item-{i}");
                let on = matches!(item, Item::PickLayout(l) if l == self.model.active);
                div()
                    .id(("menu-item", i))
                    .group(group.clone())
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(24.))
                    .h(px(26.))
                    .px(px(10.))
                    .rounded(px(5.))
                    .text_color(rgb(theme::TEXT))
                    .when(on, |d| d.bg(rgb(theme::TAB_ACTIVE)))
                    .hover(|s| s.bg(rgb(theme::FOCUS)).text_color(rgb(theme::BADGE_TEXT)))
                    .cursor_pointer()
                    .child(name)
                    .child(
                        div()
                            .text_color(rgb(theme::TEXT_MUTED))
                            .group_hover(group, |s| s.text_color(rgb(theme::BADGE_TEXT)))
                            .child(key),
                    )
                    .on_click(cx.listener(move |w, _, window, cx| w.run_menu(item, window, cx)))
                    .when_some(
                        match item {
                            Item::PickLayout(l) => self.model.layouts.get(l).map(|l| l.id),
                            _ => None,
                        },
                        |d, id| {
                            d.on_mouse_down(
                                MouseButton::Right,
                                cx.listener(move |w, ev: &MouseDownEvent, window, cx| {
                                    cx.stop_propagation();
                                    w.open_menu(For::Layout(id), ev.position, window, cx)
                                }),
                            )
                        },
                    )
            })
            .collect::<Vec<_>>();
        let swatches = match target {
            For::Space(id, _) => Some(self.swatches(id, cx)),
            _ => None,
        };
        let list = div()
            .id("menu")
            .min_w(px(200.))
            .p(px(4.))
            .flex()
            .flex_col()
            .bg(rgb(theme::DIALOG))
            .border_1()
            .border_color(rgb(theme::CARD_BORDER))
            .rounded(px(8.))
            .shadow_lg()
            .text_size(px(13.))
            .on_mouse_down_out(cx.listener(|w, _, _, cx| w.close_menu(cx)))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .children(swatches)
            .children(rows);
        Some(deferred(anchored().position(menu.at).snap_to_window().child(list)).into_any_element())
    }

    /// A row of the space colors, the space's own ringed.
    fn swatches(&self, space: SpaceId, cx: &mut Context<Self>) -> AnyElement {
        let current = self.model.find_space(space).map(|s| s.color);
        div()
            .flex()
            .gap(px(6.))
            .px(px(10.))
            .py(px(7.))
            .children((0..COLORS).map(|c| {
                div()
                    .id(("swatch", c))
                    .size(px(16.))
                    .rounded(px(4.))
                    .bg(rgb(theme::space_color(c)))
                    .border_2()
                    .border_color(rgb(if current == Some(c) {
                        theme::TEXT_STRONG
                    } else {
                        theme::space_color(c)
                    }))
                    .cursor_pointer()
                    .on_click(cx.listener(move |w, _, _, cx| w.set_color(space, c, cx)))
            }))
            .into_any_element()
    }
}
