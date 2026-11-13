//! Which side the file trees and lists are on: the right unless `:side` moved them.

use std::sync::atomic::{AtomicBool, Ordering};

use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;

static LEFT: AtomicBool = AtomicBool::new(false);

pub fn files_left() -> bool {
    LEFT.load(Ordering::Relaxed)
}

pub fn set_files_left(left: bool) {
    LEFT.store(left, Ordering::Relaxed);
}

/// Moves the files to the other side, and keeps that for the next run.
pub fn toggle() {
    set_files_left(!files_left());
    super::settings::save(&super::settings::load_file());
}

/// `area` split into the files pane `files` wide, the column of the divider between, and
/// the rest, with the files on their side.
pub fn split(area: Rect, files: u16) -> (Rect, u16, Rect) {
    let files = files.min(area.width.saturating_sub(1));
    let rest = area.width - files - 1;
    if files_left() {
        (
            Rect::new(area.x, area.y, files, area.height),
            area.x + files,
            Rect::new(area.x + files + 1, area.y, rest, area.height),
        )
    } else {
        (
            Rect::new(area.x + rest + 1, area.y, files, area.height),
            area.x + rest,
            Rect::new(area.x, area.y, rest, area.height),
        )
    }
}

/// Reads `C-w <` and `C-w >` as how far they widen the files pane (see `width::read`).
pub fn files_step(pending: &mut bool, key: &KeyEvent) -> Option<Option<i32>> {
    Some(super::width::read(pending, key)?.map(toward_files))
}

/// How far a divider step widens the files pane: moving the divider right widens it on the
/// left and narrows it on the right.
pub fn toward_files(step: i32) -> i32 {
    if files_left() { step } else { -step }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_puts_the_files_on_their_side_with_the_divider_between() {
        let area = Rect::new(2, 1, 30, 5);
        set_files_left(true);
        let (files, divider, rest) = split(area, 10);
        assert_eq!(
            (files.x, files.width, divider, rest.x, rest.width),
            (2, 10, 12, 13, 19)
        );
        set_files_left(false);
        let (files, divider, rest) = split(area, 10);
        assert_eq!(
            (rest.x, rest.width, divider, files.x, files.width),
            (2, 19, 21, 22, 10)
        );
    }
}
