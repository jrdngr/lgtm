//! A one-line text box: click to place the cursor, drag or shift-click to select, double-click
//! for a word, and the usual Mac editing keys.

mod edit;
mod element;
mod ime;

use std::ops::Range;

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, EntityInputHandler, EventEmitter,
    FocusHandle, Focusable, InteractiveElement, IntoElement, KeyBinding, MouseButton,
    MouseDownEvent, MouseMoveEvent, ParentElement, Pixels, Point, Render, ShapedLine, Styled,
    Window, actions, div, px,
};

use element::TextElement;

actions!(
    text_input,
    [
        Backspace,
        Delete,
        DeleteWordLeft,
        DeleteToStart,
        Left,
        Right,
        WordLeft,
        WordRight,
        SelectLeft,
        SelectRight,
        SelectWordLeft,
        SelectWordRight,
        SelectToStart,
        SelectToEnd,
        SelectAll,
        Home,
        End,
        Paste,
        Cut,
        Copy,
        Submit,
    ]
);

const CONTEXT: &str = "TextInput";

pub fn bind(cx: &mut App) {
    let c = Some(CONTEXT);
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, c),
        KeyBinding::new("delete", Delete, c),
        KeyBinding::new("alt-backspace", DeleteWordLeft, c),
        KeyBinding::new("cmd-backspace", DeleteToStart, c),
        KeyBinding::new("left", Left, c),
        KeyBinding::new("right", Right, c),
        KeyBinding::new("alt-left", WordLeft, c),
        KeyBinding::new("alt-right", WordRight, c),
        KeyBinding::new("shift-left", SelectLeft, c),
        KeyBinding::new("shift-right", SelectRight, c),
        KeyBinding::new("alt-shift-left", SelectWordLeft, c),
        KeyBinding::new("alt-shift-right", SelectWordRight, c),
        KeyBinding::new("cmd-shift-left", SelectToStart, c),
        KeyBinding::new("cmd-shift-right", SelectToEnd, c),
        KeyBinding::new("shift-home", SelectToStart, c),
        KeyBinding::new("shift-end", SelectToEnd, c),
        KeyBinding::new("cmd-a", SelectAll, c),
        KeyBinding::new("cmd-left", Home, c),
        KeyBinding::new("cmd-right", End, c),
        KeyBinding::new("home", Home, c),
        KeyBinding::new("end", End, c),
        KeyBinding::new("ctrl-a", Home, c),
        KeyBinding::new("ctrl-e", End, c),
        KeyBinding::new("cmd-v", Paste, c),
        KeyBinding::new("cmd-c", Copy, c),
        KeyBinding::new("cmd-x", Cut, c),
        KeyBinding::new("enter", Submit, c),
    ]);
}

pub enum InputEvent {
    Changed,
    /// Enter.
    Submit,
}

pub struct TextInput {
    focus: FocusHandle,
    text: String,
    selected: Range<usize>,
    /// The cursor is at the selection's start.
    reversed: bool,
    /// Text the input method is still composing.
    marked: Option<Range<usize>>,
    line: Option<ShapedLine>,
    bounds: Option<Bounds<Pixels>>,
    /// How far the text is scrolled left to keep the cursor in view.
    scroll: Pixels,
    dragging: bool,
}

impl EventEmitter<InputEvent> for TextInput {}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl TextInput {
    pub fn new(text: impl Into<String>, cx: &mut Context<Self>) -> Self {
        let text = text.into();
        let end = text.len();
        TextInput {
            focus: cx.focus_handle(),
            text,
            selected: end..end,
            reversed: false,
            marked: None,
            line: None,
            bounds: None,
            scroll: px(0.),
            dragging: false,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn set_text(&mut self, text: String, cx: &mut Context<Self>) {
        if text != self.text {
            let end = text.len();
            self.text = text;
            self.selected = end.min(self.selected.start)..end.min(self.selected.end);
            self.marked = None;
            cx.notify();
        }
    }

    fn cursor(&self) -> usize {
        if self.reversed {
            self.selected.start
        } else {
            self.selected.end
        }
    }

    fn move_to(&mut self, at: usize, window: &Window, cx: &mut Context<Self>) {
        self.selected = at..at;
        self.reversed = false;
        window.notify_text_selection_changed(&self.focus);
        cx.notify();
    }

    fn select_to(&mut self, at: usize, window: &Window, cx: &mut Context<Self>) {
        if self.reversed {
            self.selected.start = at;
        } else {
            self.selected.end = at;
        }
        if self.selected.end < self.selected.start {
            self.reversed = !self.reversed;
            self.selected = self.selected.end..self.selected.start;
        }
        window.notify_text_selection_changed(&self.focus);
        cx.notify();
    }

    fn select(&mut self, range: Range<usize>, window: &Window, cx: &mut Context<Self>) {
        self.selected = range;
        self.reversed = false;
        window.notify_text_selection_changed(&self.focus);
        cx.notify();
    }

    /// Moves to `at`, or to the selection's `edge` when there is a selection.
    fn step(&mut self, at: usize, edge: usize, window: &Window, cx: &mut Context<Self>) {
        let to = if self.selected.is_empty() { at } else { edge };
        self.move_to(to, window, cx);
    }

    /// Deletes the selection, or from the cursor to `to` when there isn't one.
    fn delete_to(&mut self, to: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected.is_empty() {
            self.select_to(to, window, cx);
        }
        self.replace(self.selected.clone(), "", window, cx);
    }

    fn replace(&mut self, range: Range<usize>, new: &str, window: &Window, cx: &mut Context<Self>) {
        self.text.replace_range(range.clone(), new);
        let at = range.start + new.len();
        self.selected = at..at;
        self.reversed = false;
        self.marked = None;
        window.notify_text_input_changed(&self.focus);
        cx.emit(InputEvent::Changed);
        cx.notify();
    }

    fn index_at(&self, p: Point<Pixels>) -> usize {
        let (Some(bounds), Some(line)) = (self.bounds, &self.line) else {
            return 0;
        };
        line.closest_index_for_x(p.x - bounds.left() + self.scroll)
    }

    fn mouse_down(&mut self, ev: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        window.focus(&self.focus, cx);
        let at = self.index_at(ev.position);
        match ev.click_count {
            2 => self.select(edit::word_at(&self.text, at), window, cx),
            n if n >= 3 => self.select(0..self.text.len(), window, cx),
            _ if ev.modifiers.shift => self.select_to(at, window, cx),
            _ => self.move_to(at, window, cx),
        }
        self.dragging = ev.click_count == 1;
    }

    fn mouse_move(&mut self, ev: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.dragging && ev.dragging() {
            self.select_to(self.index_at(ev.position), window, cx);
        }
    }

    fn selected_text(&self) -> Option<ClipboardItem> {
        (!self.selected.is_empty())
            .then(|| ClipboardItem::new_string(self.text[self.selected.clone()].to_owned()))
    }
}

impl Render for TextInput {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .items_center()
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(|t, _: &Backspace, w, cx| {
                t.delete_to(edit::prev_char(&t.text, t.cursor()), w, cx)
            }))
            .on_action(cx.listener(|t, _: &Delete, w, cx| {
                t.delete_to(edit::next_char(&t.text, t.cursor()), w, cx)
            }))
            .on_action(cx.listener(|t, _: &DeleteWordLeft, w, cx| {
                t.delete_to(edit::prev_word(&t.text, t.cursor()), w, cx)
            }))
            .on_action(cx.listener(|t, _: &DeleteToStart, w, cx| t.delete_to(0, w, cx)))
            .on_action(cx.listener(|t, _: &Left, w, cx| {
                t.step(
                    edit::prev_char(&t.text, t.cursor()),
                    t.selected.start,
                    w,
                    cx,
                )
            }))
            .on_action(cx.listener(|t, _: &Right, w, cx| {
                t.step(edit::next_char(&t.text, t.cursor()), t.selected.end, w, cx)
            }))
            .on_action(cx.listener(|t, _: &WordLeft, w, cx| {
                t.move_to(edit::prev_word(&t.text, t.cursor()), w, cx)
            }))
            .on_action(cx.listener(|t, _: &WordRight, w, cx| {
                t.move_to(edit::next_word(&t.text, t.cursor()), w, cx)
            }))
            .on_action(cx.listener(|t, _: &SelectLeft, w, cx| {
                t.select_to(edit::prev_char(&t.text, t.cursor()), w, cx)
            }))
            .on_action(cx.listener(|t, _: &SelectRight, w, cx| {
                t.select_to(edit::next_char(&t.text, t.cursor()), w, cx)
            }))
            .on_action(cx.listener(|t, _: &SelectWordLeft, w, cx| {
                t.select_to(edit::prev_word(&t.text, t.cursor()), w, cx)
            }))
            .on_action(cx.listener(|t, _: &SelectWordRight, w, cx| {
                t.select_to(edit::next_word(&t.text, t.cursor()), w, cx)
            }))
            .on_action(cx.listener(|t, _: &SelectToStart, w, cx| t.select_to(0, w, cx)))
            .on_action(cx.listener(|t, _: &SelectToEnd, w, cx| t.select_to(t.text.len(), w, cx)))
            .on_action(cx.listener(|t, _: &SelectAll, w, cx| t.select(0..t.text.len(), w, cx)))
            .on_action(cx.listener(|t, _: &Home, w, cx| t.move_to(0, w, cx)))
            .on_action(cx.listener(|t, _: &End, w, cx| t.move_to(t.text.len(), w, cx)))
            .on_action(cx.listener(|t, _: &Copy, _, cx| {
                if let Some(item) = t.selected_text() {
                    cx.write_to_clipboard(item);
                }
            }))
            .on_action(cx.listener(|t, _: &Cut, w, cx| {
                if let Some(item) = t.selected_text() {
                    cx.write_to_clipboard(item);
                    t.replace(t.selected.clone(), "", w, cx);
                }
            }))
            .on_action(cx.listener(|t, _: &Paste, w, cx| {
                if let Some(item) = cx.read_from_clipboard() {
                    t.paste(item, w, cx);
                }
            }))
            .on_action(cx.listener(|_, _: &Submit, _, cx| cx.emit(InputEvent::Submit)))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|t, _, _, _| t.dragging = false),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|t, _, _, _| t.dragging = false),
            )
            .on_mouse_move(cx.listener(Self::mouse_move))
            .child(TextElement { input: cx.entity() })
    }
}
