//! Keyboard, mouse and scroll input for a terminal.

use ghostty_vt::key::{self, Action};
use ghostty_vt::mouse::{self, EncoderSize};
use ghostty_vt::paste;
use ghostty_vt::screen::Screen;
use ghostty_vt::selection::{FormatOptions, SelectLineOptions, SelectWordOptions, Selection};
use ghostty_vt::terminal::{Mode, Point as VtPoint, PointCoordinate, ScrollViewport};
use ghostty_vt::{Terminal, fmt};
use gpui::{
    Context, KeyDownEvent, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    Pixels, Point, ScrollEvent, Window,
};

use super::{TermView, keys};

pub fn selection_text(vt: &Terminal) -> Option<String> {
    let opts = FormatOptions::new()
        .with_emit_format(fmt::Format::Plain)
        .with_unwrap(true)
        .with_trim(true);
    let bytes = vt.format_selection_alloc(opts).ok()??;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// `text` as the shell should receive it, bracketed if the program asked for that.
pub fn paste(vt: &Terminal, text: String) -> Vec<u8> {
    let bracketed = vt.mode(Mode::BRACKETED_PASTE).unwrap_or(false);
    let mut data = text.into_bytes();
    let mut buf = vec![0u8; data.len() + 16];
    match paste::encode(&mut data, bracketed, &mut buf) {
        Ok(n) => {
            buf.truncate(n);
            buf
        }
        Err(_) => data,
    }
}

fn mouse_button(b: MouseButton) -> Option<mouse::Button> {
    match b {
        MouseButton::Left => Some(mouse::Button::Left),
        MouseButton::Right => Some(mouse::Button::Right),
        MouseButton::Middle => Some(mouse::Button::Middle),
        _ => None,
    }
}

impl TermView {
    pub(super) fn key_down(&mut self, ev: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let k = &ev.keystroke;
        if k.modifiers.platform {
            return;
        }
        let e = &mut self.key_event;
        e.set_action(if ev.is_held {
            Action::Repeat
        } else {
            Action::Press
        })
        .set_key(keys::key(&k.key))
        .set_mods(keys::mods(&k.modifiers))
        .set_consumed_mods(keys::consumed(k))
        .set_utf8(keys::text(k).map(str::to_owned));
        if let Some(c) = keys::unshifted(&k.key) {
            e.set_unshifted_codepoint(c);
        }
        self.keys.set_options_from_terminal(&self.vt);
        let mut out = Vec::new();
        if self.keys.encode_to_vec(&self.key_event, &mut out).is_ok() && !out.is_empty() {
            self.write(&out);
            self.scroll_to_bottom(cx);
        }
        cx.stop_propagation();
    }

    /// The cell under `pos`, clamped to the screen.
    fn cell_at(&self, pos: Point<Pixels>) -> Option<(u16, u16)> {
        let (origin, cell) = self.grid?;
        let x = ((pos.x - origin.x) / cell.width).floor().max(0.) as u16;
        let y = ((pos.y - origin.y) / cell.height).floor().max(0.) as u16;
        Some((x.min(self.cols - 1), y.min(self.rows - 1)))
    }

    /// Whether mouse events go to the program instead of selecting.
    fn reports_mouse(&self, mods: &Modifiers) -> bool {
        !mods.shift && self.vt.is_mouse_tracking().unwrap_or(false)
    }

    fn send_mouse(
        &mut self,
        action: mouse::Action,
        button: Option<mouse::Button>,
        pos: Point<Pixels>,
        mods: &Modifiers,
    ) {
        let Some((origin, cell)) = self.grid else {
            return;
        };
        let size = EncoderSize {
            screen_width: (cell.width * self.cols as f32).as_f32() as u32,
            screen_height: (cell.height * self.rows as f32).as_f32() as u32,
            cell_width: cell.width.as_f32() as u32,
            cell_height: cell.height.as_f32() as u32,
            padding_top: 0,
            padding_bottom: 0,
            padding_right: 0,
            padding_left: 0,
        };
        let Ok(mut event) = mouse::Event::new() else {
            return;
        };
        event
            .set_action(action)
            .set_button(button)
            .set_mods(keys::mods(mods))
            .set_position(mouse::Position {
                x: (pos.x - origin.x).as_f32(),
                y: (pos.y - origin.y).as_f32(),
            });
        self.mouse
            .set_options_from_terminal(&self.vt)
            .set_size(size);
        let mut out = Vec::new();
        if self.mouse.encode_to_vec(&event, &mut out).is_ok() && !out.is_empty() {
            self.write(&out);
        }
    }

    fn screen_point(&self, (x, y): (u16, u16)) -> (u16, u32) {
        let top = self.vt.scrollbar().map_or(0, |s| s.offset);
        (x, top as u32 + y as u32)
    }

    pub(super) fn mouse_down(
        &mut self,
        ev: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        if self.reports_mouse(&ev.modifiers) {
            self.send_mouse(
                mouse::Action::Press,
                mouse_button(ev.button),
                ev.position,
                &ev.modifiers,
            );
            return;
        }
        if ev.button != MouseButton::Left {
            return;
        }
        let Some(cell) = self.cell_at(ev.position) else {
            return;
        };
        let at = self.screen_point(cell);
        self.mouse_down = true;
        self.drag_from = Some(at);
        let point = VtPoint::Screen(PointCoordinate { x: at.0, y: at.1 });
        let vt = &self.vt;
        let selection = match ev.click_count {
            1 => None,
            2 => vt
                .grid_ref(point)
                .ok()
                .and_then(|r| vt.select_word(SelectWordOptions::new(r)).ok().flatten()),
            _ => vt
                .grid_ref(point)
                .ok()
                .and_then(|r| vt.select_line(SelectLineOptions::new(r)).ok().flatten()),
        };
        let _ = vt.set_selection(selection.as_ref());
        if ev.click_count > 1 {
            self.drag_from = None;
        }
        cx.notify();
    }

    pub(super) fn mouse_move(
        &mut self,
        ev: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.reports_mouse(&ev.modifiers) {
            let button = ev.pressed_button.and_then(mouse_button);
            self.send_mouse(mouse::Action::Motion, button, ev.position, &ev.modifiers);
            return;
        }
        let (Some(from), Some(cell), true) =
            (self.drag_from, self.cell_at(ev.position), self.mouse_down)
        else {
            return;
        };
        let to = self.screen_point(cell);
        let vt = &self.vt;
        let point = |(x, y): (u16, u32)| vt.grid_ref(VtPoint::Screen(PointCoordinate { x, y }));
        if let (Ok(a), Ok(b)) = (point(from), point(to)) {
            let _ = vt.set_selection(Some(&Selection::new(a, b, ev.modifiers.alt)));
        }
        cx.notify();
    }

    pub(super) fn mouse_up(&mut self, ev: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.reports_mouse(&ev.modifiers) {
            self.send_mouse(
                mouse::Action::Release,
                mouse_button(ev.button),
                ev.position,
                &ev.modifiers,
            );
            return;
        }
        self.mouse_down = false;
        cx.notify();
    }

    pub(super) fn scroll(&mut self, ev: &ScrollEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some((_, cell)) = self.grid else { return };
        self.scroll_rest += ev.delta.pixel_delta(cell.height).y.as_f32();
        let lines = (self.scroll_rest / cell.height.as_f32()).trunc();
        if lines == 0. {
            return;
        }
        self.scroll_rest -= lines * cell.height.as_f32();
        let up = lines > 0.;
        let count = lines.abs() as usize;
        if self.reports_mouse(&ev.modifiers) {
            let button = if up {
                mouse::Button::Four
            } else {
                mouse::Button::Five
            };
            for _ in 0..count {
                self.send_mouse(
                    mouse::Action::Press,
                    Some(button),
                    ev.position,
                    &ev.modifiers,
                );
            }
        } else if self.vt.active_screen().ok() == Some(Screen::Alternate)
            && self.vt.mode(Mode::ALT_SCROLL).unwrap_or(false)
        {
            let arrow = if up {
                key::Key::ArrowUp
            } else {
                key::Key::ArrowDown
            };
            self.key_event
                .set_action(Action::Press)
                .set_key(arrow)
                .set_mods(key::Mods::empty())
                .set_consumed_mods(key::Mods::empty())
                .set_utf8(None::<String>);
            self.keys.set_options_from_terminal(&self.vt);
            let mut out = Vec::new();
            if self.keys.encode_to_vec(&self.key_event, &mut out).is_ok() {
                self.write(&out.repeat(count));
            }
        } else {
            let delta = if up {
                -(count as isize)
            } else {
                count as isize
            };
            self.vt.scroll_viewport(ScrollViewport::Delta(delta));
            cx.notify();
        }
    }
}
