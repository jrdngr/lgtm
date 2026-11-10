//! Window-wide actions, their keys, and their names in the command palette.

use gpui::{Action, App, KeyBinding, KeyBindingContextPredicate, actions};

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
        MoveTabToNewSplit,
        FocusLeft,
        FocusRight,
        FocusUp,
        FocusDown,
        NewLayout,
        NextLayout,
        PrevLayout,
        NextWaiting,
        ShowMessages,
        ToggleSidebar,
        TogglePalette,
        OpenSettings,
        Find,
        FontBigger,
        FontSmaller,
        FontReset,
        Quit,
    ]
);

/// A command: its palette name, its keys (the first is shown), and its action.
pub struct Command {
    pub name: &'static str,
    pub keys: &'static [&'static str],
    pub action: fn() -> Box<dyn Action>,
}

macro_rules! commands {
    ($($name:literal, [$($key:literal),*], $action:ident;)*) => {
        pub const COMMANDS: &[Command] = &[
            $(Command { name: $name, keys: &[$($key),*], action: || Box::new($action) },)*
        ];
    };
}

commands! {
    "New tab", ["cmd-t"], NewTab;
    "Close tab", ["cmd-w"], CloseTab;
    "Next tab", ["cmd-shift-]", "cmd-}"], NextTab;
    "Previous tab", ["cmd-shift-[", "cmd-{"], PrevTab;
    "Split right", ["cmd-d"], SplitRight;
    "Split down", ["cmd-shift-d"], SplitDown;
    "Split space right", ["cmd-alt-d"], SplitSpaceRight;
    "Split space down", ["cmd-alt-shift-d"], SplitSpaceDown;
    "Close split", ["cmd-shift-w"], CloseSplit;
    "Zoom split", ["cmd-shift-enter"], ZoomSplit;
    "Next split", ["cmd-]"], NextSplit;
    "Previous split", ["cmd-["], PrevSplit;
    "Move tab to new split", [], MoveTabToNewSplit;
    "Focus left", ["cmd-alt-left"], FocusLeft;
    "Focus right", ["cmd-alt-right"], FocusRight;
    "Focus up", ["cmd-alt-up"], FocusUp;
    "Focus down", ["cmd-alt-down"], FocusDown;
    "New layout", ["cmd-shift-n"], NewLayout;
    "Next layout", ["ctrl-cmd-down"], NextLayout;
    "Previous layout", ["ctrl-cmd-up"], PrevLayout;
    "Next waiting agent", ["cmd-j"], NextWaiting;
    "Messages", ["cmd-shift-m"], ShowMessages;
    "Toggle sidebar", ["cmd-b"], ToggleSidebar;
    "Command palette", ["cmd-shift-p", "cmd-k"], TogglePalette;
    "Settings", ["cmd-,"], OpenSettings;
    "Find", ["cmd-f"], Find;
    "Bigger text", ["cmd-=", "cmd-+"], FontBigger;
    "Smaller text", ["cmd--"], FontSmaller;
    "Reset text size", ["cmd-0"], FontReset;
    "Quit", ["cmd-q"], Quit;
}

pub fn bind(cx: &mut App) {
    let context: std::rc::Rc<KeyBindingContextPredicate> =
        KeyBindingContextPredicate::parse("Workspace")
            .expect("context")
            .into();
    let mut bindings = Vec::new();
    for c in COMMANDS {
        for key in c.keys {
            match KeyBinding::load(key, (c.action)(), Some(context.clone()), false, None, cx) {
                Ok(b) => bindings.push(b),
                Err(e) => eprintln!("lgterm: bad key {key}: {e:?}"),
            }
        }
    }
    bindings.push(KeyBinding::new("cmd-c", Copy, Some("Terminal")));
    bindings.push(KeyBinding::new("cmd-v", Paste, Some("Terminal")));
    cx.bind_keys(bindings);
}

/// A key as the palette shows it, like ⇧⌘D.
pub fn display(key: &str) -> Vec<String> {
    let mut parts: Vec<&str> = key.split('-').collect();
    let last = match parts.pop() {
        Some("") => "-",
        Some(k) => k,
        None => return vec![],
    };
    if key.ends_with("--") {
        parts.pop();
    }
    let mut out = Vec::new();
    for (m, sym) in [("ctrl", "⌃"), ("alt", "⌥"), ("shift", "⇧"), ("cmd", "⌘")] {
        if parts.contains(&m) {
            out.push(sym.to_owned());
        }
    }
    out.push(match last {
        "enter" => "↩".to_owned(),
        "up" => "↑".to_owned(),
        "down" => "↓".to_owned(),
        "left" => "←".to_owned(),
        "right" => "→".to_owned(),
        k => k.to_uppercase(),
    });
    out
}

#[cfg(test)]
mod tests {
    use super::display;

    #[test]
    fn keys_show_as_mac_symbols_in_mac_order() {
        assert_eq!(display("cmd-alt-shift-d"), ["⌥", "⇧", "⌘", "D"]);
        assert_eq!(display("cmd-shift-enter"), ["⇧", "⌘", "↩"]);
        assert_eq!(display("cmd--"), ["⌘", "-"]);
        assert_eq!(display("ctrl-cmd-up"), ["⌃", "⌘", "↑"]);
    }
}
