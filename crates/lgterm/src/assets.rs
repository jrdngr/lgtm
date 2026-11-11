//! Files built into the binary for `svg()` paths.

use std::borrow::Cow;

use gpui::{AssetSource, SharedString};

pub struct Assets;

const FILES: &[(&str, &[u8])] = &[
    (
        "agents/claude.svg",
        include_bytes!("../icons/agents/claude.svg"),
    ),
    (
        "agents/codex.svg",
        include_bytes!("../icons/agents/codex.svg"),
    ),
    ("agents/pi.svg", include_bytes!("../icons/agents/pi.svg")),
    (
        "ui/sidebar-open.svg",
        include_bytes!("../icons/ui/sidebar-open.svg"),
    ),
    (
        "ui/sidebar-closed.svg",
        include_bytes!("../icons/ui/sidebar-closed.svg"),
    ),
    ("ui/expand.svg", include_bytes!("../icons/ui/expand.svg")),
    (
        "ui/collapse.svg",
        include_bytes!("../icons/ui/collapse.svg"),
    ),
    ("ui/lock.svg", include_bytes!("../icons/ui/lock.svg")),
    ("ui/unlock.svg", include_bytes!("../icons/ui/unlock.svg")),
    ("ui/plus.svg", include_bytes!("../icons/ui/plus.svg")),
];

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        Ok(FILES
            .iter()
            .find(|(p, _)| *p == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(FILES
            .iter()
            .filter(|(p, _)| p.starts_with(path))
            .map(|(p, _)| SharedString::from(*p))
            .collect())
    }
}
