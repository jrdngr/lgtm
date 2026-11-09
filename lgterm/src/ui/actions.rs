//! Window-wide actions and their keys.

use gpui::{App, KeyBinding, actions};

use crate::terminal::{Copy, Paste};

actions!(
    lgterm,
    [
        NewTab,
        CloseTab,
        NextTab,
        PrevTab,
        SplitRight,
        SplitDown,
        SplitSpaceRight,
        SplitSpaceDown,
        CloseSplit,
        ZoomSplit,
        NextSplit,
        PrevSplit,
        FocusLeft,
        FocusRight,
        FocusUp,
        FocusDown,
        NewLayout,
        NextLayout,
        PrevLayout,
        ToggleSidebar,
        FontBigger,
        FontSmaller,
        FontReset,
        Quit,
    ]
);

pub fn bind(cx: &mut App) {
    let w = Some("Workspace");
    cx.bind_keys([
        KeyBinding::new("cmd-t", NewTab, w),
        KeyBinding::new("cmd-w", CloseTab, w),
        KeyBinding::new("cmd-}", NextTab, w),
        KeyBinding::new("cmd-shift-]", NextTab, w),
        KeyBinding::new("cmd-{", PrevTab, w),
        KeyBinding::new("cmd-shift-[", PrevTab, w),
        KeyBinding::new("cmd-d", SplitRight, w),
        KeyBinding::new("cmd-shift-d", SplitDown, w),
        KeyBinding::new("cmd-alt-d", SplitSpaceRight, w),
        KeyBinding::new("cmd-alt-shift-d", SplitSpaceDown, w),
        KeyBinding::new("cmd-shift-w", CloseSplit, w),
        KeyBinding::new("cmd-shift-enter", ZoomSplit, w),
        KeyBinding::new("cmd-]", NextSplit, w),
        KeyBinding::new("cmd-[", PrevSplit, w),
        KeyBinding::new("cmd-alt-left", FocusLeft, w),
        KeyBinding::new("cmd-alt-right", FocusRight, w),
        KeyBinding::new("cmd-alt-up", FocusUp, w),
        KeyBinding::new("cmd-alt-down", FocusDown, w),
        KeyBinding::new("cmd-shift-n", NewLayout, w),
        KeyBinding::new("ctrl-cmd-down", NextLayout, w),
        KeyBinding::new("ctrl-cmd-up", PrevLayout, w),
        KeyBinding::new("cmd-b", ToggleSidebar, w),
        KeyBinding::new("cmd-=", FontBigger, w),
        KeyBinding::new("cmd-+", FontBigger, w),
        KeyBinding::new("cmd--", FontSmaller, w),
        KeyBinding::new("cmd-0", FontReset, w),
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-c", Copy, Some("Terminal")),
        KeyBinding::new("cmd-v", Paste, Some("Terminal")),
    ]);
}
