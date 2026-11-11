//! Status marks: a dot for blocked and done, a ring for idle, a spinner for working.

use std::f32::consts::TAU;
use std::sync::OnceLock;
use std::time::Instant;

use gpui::{AnyElement, IntoElement, PathBuilder, Styled, canvas, div, point, px, rgb, rgba};

use super::Workspace;
use crate::agents::Status;
use crate::theme;

/// How far round the spinner is, from 0 to 1.
fn phase() -> f32 {
    static START: OnceLock<Instant> = OnceLock::new();
    let ms = START.get_or_init(Instant::now).elapsed().as_millis();
    (ms % 900) as f32 / 900.
}

impl Workspace {
    /// A status mark, noting a spinner so the window keeps turning it.
    pub(super) fn mark(&self, status: Status) -> AnyElement {
        if status == Status::Working {
            self.spinning.set(true);
        }
        mark(status)
    }
}

fn mark(status: Status) -> AnyElement {
    match status {
        Status::Blocked => dot(theme::BLOCKED),
        Status::Done => dot(theme::DONE),
        Status::Idle => div()
            .size(px(7.))
            .flex_none()
            .rounded_full()
            .border(px(1.5))
            .border_color(rgb(theme::IDLE))
            .into_any_element(),
        Status::Working => spinner(),
    }
}

fn dot(color: u32) -> AnyElement {
    div()
        .size(px(7.))
        .flex_none()
        .rounded_full()
        .bg(rgb(color))
        .into_any_element()
}

fn spinner() -> AnyElement {
    let start = phase() * TAU;
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let c = bounds.center();
            let r = px(4.5);
            let at = |a: f32| point(c.x + r * a.sin(), c.y - r * a.cos());
            let mut ring = PathBuilder::stroke(px(2.));
            ring.move_to(at(0.));
            ring.arc_to(point(r, r), px(0.), false, true, at(TAU / 2.));
            ring.arc_to(point(r, r), px(0.), false, true, at(0.));
            if let Ok(path) = ring.build() {
                window.paint_path(path, rgba((theme::WORKING << 8) | 0x40));
            }
            let mut arc = PathBuilder::stroke(px(2.));
            arc.move_to(at(start));
            arc.arc_to(point(r, r), px(0.), false, true, at(start + TAU / 4.));
            if let Ok(path) = arc.build() {
                window.paint_path(path, rgb(theme::WORKING));
            }
        },
    )
    .size(px(11.))
    .flex_none()
    .into_any_element()
}
