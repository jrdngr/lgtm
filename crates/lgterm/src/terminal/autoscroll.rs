//! Scrolling while a selection drag sits past the terminal's top or bottom edge, so it reaches
//! text off screen. The farther past the edge, the faster it goes. A terminal against the
//! window's top or bottom, which can't be dragged past, scrolls from its edge row instead.

use std::time::Duration;

use ghostty_vt::terminal::ScrollViewport;
use gpui::{
    App, Context, DispatchPhase, Entity, MouseButton, MouseMoveEvent, Pixels, WeakEntity, Window,
    point,
};

use super::TermView;

const TICK: Duration = Duration::from_millis(50);
/// Most lines a tick scrolls.
const FASTEST: i32 = 8;

/// A drag past an edge: lines each tick scrolls (negative is up), the pointer's x, and
/// whether the selection is a rectangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edge {
    lines: i32,
    x: Pixels,
    rect: bool,
}

/// Lines per tick for a pointer at `y`, given the grid's top and bottom, a row's height and
/// the window's height: 0 inside the grid, except on an edge row that meets the window's edge.
fn speed(y: f32, (top, bottom): (f32, f32), row: f32, window: f32) -> i32 {
    let past = |d: f32| (1 + (d / row) as i32).min(FASTEST);
    if y < top {
        -past(top - y)
    } else if y >= bottom {
        past(y - bottom)
    } else if top < row * 2. && y < top + row {
        -1
    } else if window - bottom < row * 2. && y >= bottom - row {
        1
    } else {
        0
    }
}

/// Follows the pointer anywhere in the window while a drag in `view` selects.
pub fn listen(view: Entity<TermView>, window: &mut Window) {
    window.on_mouse_event(move |ev: &MouseMoveEvent, phase, window, cx: &mut App| {
        if phase == DispatchPhase::Bubble {
            let height = window.viewport_size().height.as_f32();
            view.update(cx, |t, cx| t.drag_edge(ev, height, cx));
        }
    });
}

impl TermView {
    fn drag_edge(&mut self, ev: &MouseMoveEvent, window: f32, cx: &mut Context<Self>) {
        let selecting = self.mouse_down
            && self.drag_from.is_some()
            && self.press_at.is_none()
            && ev.pressed_button == Some(MouseButton::Left);
        let Some((origin, cell)) = self.grid.filter(|_| selecting) else {
            self.autoscroll = None;
            return;
        };
        let top = origin.y.as_f32();
        let bottom = top + cell.height.as_f32() * self.rows as f32;
        let lines = speed(
            ev.position.y.as_f32(),
            (top, bottom),
            cell.height.as_f32(),
            window,
        );
        self.autoscroll = (lines != 0).then_some(Edge {
            lines,
            x: ev.position.x,
            rect: ev.modifiers.alt,
        });
        if self.autoscroll.is_some() && !self.autoscrolling {
            self.autoscrolling = true;
            cx.spawn(async move |this: WeakEntity<TermView>, cx| {
                loop {
                    cx.background_executor().timer(TICK).await;
                    if !this
                        .update(cx, |t, cx| t.autoscroll_tick(cx))
                        .unwrap_or(false)
                    {
                        break;
                    }
                }
            })
            .detach();
        }
    }

    /// Scrolls one tick and stretches the selection to the edge row. Says whether to go on.
    fn autoscroll_tick(&mut self, cx: &mut Context<Self>) -> bool {
        let (Some(edge), Some((origin, _)), true) = (self.autoscroll, self.grid, self.mouse_down)
        else {
            self.autoscroll = None;
            self.autoscrolling = false;
            return false;
        };
        self.vt
            .scroll_viewport(ScrollViewport::Delta(edge.lines as isize));
        if let Some((x, _)) = self.cell_at(point(edge.x, origin.y)) {
            let row = if edge.lines < 0 { 0 } else { self.rows - 1 };
            let to = self.screen_point((x, row));
            self.select_to(to, edge.rect);
        }
        cx.notify();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn past_an_edge_scrolls_faster_the_farther_out_and_inside_only_at_the_window_edge() {
        // Rows of 10 from 100 to 300 in a window 500 high.
        let at = |y| speed(y, (100., 300.), 10., 500.);
        assert_eq!([at(150.), at(105.), at(295.)], [0, 0, 0]);
        assert_eq!([at(99.), at(85.), at(0.)], [-1, -2, -FASTEST]);
        assert_eq!([at(300.), at(325.)], [1, 3]);
        // A grid that starts at the window's top and ends at its bottom.
        let flush = |y| speed(y, (5., 495.), 10., 500.);
        assert_eq!(
            [flush(8.), flush(20.), flush(490.), flush(480.)],
            [-1, 0, 1, 0]
        );
    }
}
