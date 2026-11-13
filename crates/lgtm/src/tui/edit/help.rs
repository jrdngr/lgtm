//! The editor's keys for `?`, in groups.

use crate::tui::help::Group;

pub const MOVE: &[(&str, &str)] = &[
    ("h j k l", "move"),
    ("w b e", "word, back, end"),
    ("W B E", "WORD, back, end"),
    ("f t", "find, till char"),
    ("F T", "find, till char back"),
    ("C-d C-u", "half page down, up"),
    ("g g  g e", "file start, end"),
    ("g h  g l", "line start, end"),
    ("g s", "first non-blank"),
    ("C-o", "back"),
];

pub const SELECT: &[(&str, &str)] = &[
    ("v", "select mode"),
    ("x", "line"),
    ("%", "all"),
    (";", "collapse"),
    ("m i  m a", "inside, around object"),
    ("A-o A-i", "grow, shrink syntax node"),
];

pub const SEARCH: &[(&str, &str)] = &[
    ("/", "search"),
    ("n N", "next, previous match"),
    ("*", "search the selection"),
];

const EDIT: &[(&str, &str)] = &[
    ("i a", "insert, append"),
    ("I A", "at line start, end"),
    ("o O", "line below, above"),
    ("d c", "delete, change"),
    ("y", "yank"),
    ("p P", "paste after, before"),
    ("r", "replace with a char"),
    ("> <", "indent, outdent"),
    ("J", "join lines"),
    ("~", "toggle case"),
    ("u U", "undo, redo"),
    ("m s", "add surround"),
    ("m d  m r", "delete, replace surround"),
];

const FILES: &[(&str, &str)] = &[
    ("space f", "files"),
    ("space o", "open files"),
    ("g n  g p", "next, previous open file"),
    ("space /", "search files"),
    ("space e", "file tree"),
    ("space b", "toggle file tree"),
    ("C-w v  C-w s", "split right, below"),
    (":", "command"),
];

const FOLDS: &[(&str, &str)] = &[
    ("z z", "toggle"),
    ("z c  z o", "close, open"),
    ("z M  z R", "close, open all"),
    ("z m  z r", "close, open a level"),
];

const MORE: &[(&str, &str)] = &[
    ("space y  space p", "clipboard yank, paste"),
    ("space w", "word wrap"),
    ("space r", "review"),
    ("space i", "issues"),
    ("space a", "send to agent"),
    ("space m", "markdown preview"),
    ("space x", "toggle checkbox"),
    ("S-tab", "markdown table cell, footnote"),
];

/// The keys while a markdown file shows rendered.
pub fn preview_groups() -> Vec<Group> {
    vec![Group::new(
        "Preview",
        &[
            ("j k", "scroll"),
            ("C-d C-u", "half page down, up"),
            ("g g  g e", "top, bottom"),
            ("space m  Esc", "back to the text"),
            ("click", "open a link"),
            (":", "command"),
        ],
    )]
}

pub fn groups() -> Vec<Group> {
    vec![
        Group::new("Move", MOVE),
        Group::new("Select", SELECT),
        Group::new("Search", SEARCH),
        Group::new("Edit", EDIT),
        Group::new("Files", FILES),
        Group::new("Folds", FOLDS),
        Group::new("More", MORE),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::keymap::BINDS;

    /// Each entry's keys, with two spaces between bindings of more than one key.
    fn bindings(keys: &str) -> Vec<String> {
        if keys.contains("  ") {
            return keys.split("  ").map(str::to_string).collect();
        }
        match keys.split_once(' ') {
            Some(("g" | "z" | "m" | "space" | "C-w", _)) => {
                vec![keys.to_string()]
            }
            _ => keys.split(' ').map(str::to_string).collect(),
        }
    }

    #[test]
    fn every_binding_shows_once_and_every_key_shown_is_bound() {
        let shown: Vec<String> = groups()
            .iter()
            .flat_map(|g| g.keys.iter().flat_map(|(k, _)| bindings(k)))
            .collect();
        for b in BINDS {
            if matches!(
                b.keys,
                "left" | "right" | "up" | "down" | "home" | "end" | "esc"
            ) {
                continue;
            }
            let n = shown.iter().filter(|s| *s == b.keys).count();
            assert_eq!(n, 1, "{} shows {n} times", b.keys);
        }
        for s in shown.iter().filter(|s| *s != "C-o") {
            assert!(BINDS.iter().any(|b| b.keys == s), "{s} isn't bound");
        }
    }
}
