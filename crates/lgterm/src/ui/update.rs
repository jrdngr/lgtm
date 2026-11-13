//! Noticing that lgterm's own app was replaced on disk, as `just go` does, and restarting into
//! the new copy.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, Context, IntoElement, ParentElement, Styled, div, prelude::*, px, rgb, svg,
};

use super::Workspace;
use crate::theme;

const EVERY: Duration = Duration::from_secs(60);

/// The running binary and what it looked like on disk at launch.
pub(super) struct Update {
    exe: Option<PathBuf>,
    stamp: Option<Stamp>,
    checked: Instant,
    /// The binary on disk is a different one now.
    pub(super) ready: bool,
}

#[derive(PartialEq)]
struct Stamp {
    inode: u64,
    len: u64,
    modified: Option<std::time::SystemTime>,
}

fn stamp(path: &Path) -> Option<Stamp> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::metadata(path).ok()?;
    Some(Stamp {
        inode: meta.ino(),
        len: meta.len(),
        modified: meta.modified().ok(),
    })
}

impl Update {
    pub(super) fn new() -> Self {
        let exe = std::env::current_exe().ok();
        let stamp = exe.as_deref().and_then(stamp);
        Update {
            exe,
            stamp,
            checked: Instant::now(),
            ready: false,
        }
    }

    /// Whether the binary changed since the last check said it hadn't.
    pub(super) fn check(&mut self) -> bool {
        if self.ready || self.stamp.is_none() || self.checked.elapsed() < EVERY {
            return false;
        }
        self.checked = Instant::now();
        let Some(now) = self.exe.as_deref().and_then(stamp) else {
            return false;
        };
        self.ready = Some(now) != self.stamp;
        self.ready
    }

    /// Opens the new copy once this process has exited.
    fn relaunch(&self) {
        let Some(exe) = &self.exe else { return };
        let target = exe
            .ancestors()
            .find(|p| p.extension().is_some_and(|e| e == "app"))
            .unwrap_or(exe);
        let open = if target == exe.as_path() {
            "\"$0\" &"
        } else {
            "open \"$0\""
        };
        let script = format!("while kill -0 $1 2>/dev/null; do sleep 0.1; done; {open}");
        let _ = Command::new("sh")
            .arg("-c")
            .arg(script)
            .arg(target)
            .arg(std::process::id().to_string())
            .spawn();
    }
}

impl Workspace {
    pub(super) fn check_update(&mut self, cx: &mut Context<Self>) {
        if self.update.check() {
            cx.notify();
        }
    }

    pub(super) fn restart(&mut self, cx: &mut App) {
        self.update.relaunch();
        cx.quit();
    }

    /// Beside the sidebar toggle, once a new copy is on disk.
    pub(super) fn render_update(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.update.ready {
            return None;
        }
        let toggle = self.toggle_left();
        // Beside the traffic lights there's only room to the toggle's right.
        let left = if toggle < 100. {
            toggle + 26.
        } else {
            toggle - 26.
        };
        let el = div()
            .id("update")
            .absolute()
            .top(px(12.))
            .left(px(left))
            .size(px(22.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(5.))
            .occlude()
            .cursor_pointer()
            .hover(|s| s.bg(rgb(theme::ROW_HOVER)))
            .child(
                svg()
                    .path("ui/update.svg")
                    .size(px(16.))
                    .text_color(rgb(theme::DONE)),
            )
            .on_click(cx.listener(|w, _, window, cx| w.confirm_restart(window, cx)));
        Some(el.into_any_element())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn watching(path: &Path, checked: Instant) -> Update {
        Update {
            exe: Some(path.to_owned()),
            stamp: stamp(path),
            checked,
            ready: false,
        }
    }

    #[test]
    fn a_replaced_binary_is_noticed_at_most_once_a_minute() {
        let dir = std::env::temp_dir().join(format!("lgterm-update-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("lgterm");
        std::fs::write(&exe, "old").unwrap();
        let long_ago = Instant::now() - EVERY;
        let mut same = watching(&exe, long_ago);
        let mut fresh = watching(&exe, Instant::now());
        let mut due = watching(&exe, long_ago);
        assert!(!same.check());

        std::fs::remove_file(&exe).unwrap();
        std::fs::write(&exe, "new build").unwrap();
        assert!(!fresh.check());
        assert!(due.check());
        assert!(due.ready);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
