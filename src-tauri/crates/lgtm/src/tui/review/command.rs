//! The `:` command line: typing a command, Tab completing its name or path, and running it.

use std::collections::BTreeSet;

use lgtm_core::store::{MarkColor, Store};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::undo::Step;
use super::{Outcome, Review};

/// Commands, with whether each takes a path, and what they do.
const COMMANDS: &[(&str, bool, &str)] = &[
    (
        "review",
        true,
        "review a file or folder, or the current file",
    ),
    ("flag", true, "flag a file or folder, or the current file"),
    (
        "clear",
        true,
        "clear marks on a file or folder, or the current file",
    ),
    ("clear-all", false, "clear every mark in the review"),
    ("resolved", false, "show or hide resolved comments"),
    ("pull", false, "pull PR comments"),
    ("split", false, "side by side"),
    ("edit", false, "open the editor"),
    ("h", false, "back to the home screen"),
    ("q", false, "quit"),
];

#[derive(Default)]
pub struct CmdLine {
    pub text: String,
    /// What Tab offers for the word being typed, and which one it's on when cycling.
    pub offers: Vec<String>,
    pub(super) cycle: Option<usize>,
    /// Whether it's the `/` search line.
    pub search: bool,
}

/// What's left to answer before a command runs.
pub enum Confirm {
    ClearAll,
}

/// The longest start every one of `words` shares.
fn common_start(words: &[String]) -> String {
    let Some(first) = words.first() else {
        return String::new();
    };
    let mut end = first.len();
    for w in &words[1..] {
        end = first
            .char_indices()
            .zip(w.chars())
            .take_while(|((_, a), b)| a == b)
            .map(|((i, a), _)| i + a.len_utf8())
            .last()
            .unwrap_or(0)
            .min(end);
    }
    first[..end].to_string()
}

/// `text` after Tab: its last word completed from `options`, or the next of `offers` when
/// Tab pressed again has nothing more in common to add.
fn complete(line: &mut CmdLine, options: &[String]) {
    let start = line.text.rfind(' ').map_or(0, |i| i + 1);
    if let Some(i) = line.cycle {
        if line.offers.is_empty() {
            return;
        }
        let i = (i + 1) % line.offers.len();
        line.text.replace_range(start.., &line.offers[i]);
        line.cycle = Some(i);
        return;
    }
    let word = &line.text[start..];
    let offers: Vec<String> = options
        .iter()
        .filter(|o| o.starts_with(word))
        .cloned()
        .collect();
    let common = common_start(&offers);
    match offers.len() {
        0 => {}
        1 => line.text.replace_range(start.., &offers[0]),
        _ if common.len() > word.len() => line.text.replace_range(start.., &common),
        _ => {
            // Tab again goes through the offers, from the one after what's typed.
            let i = usize::from(offers[0] == word) % offers.len();
            line.text.replace_range(start.., &offers[i]);
            line.cycle = Some(i);
        }
    }
    line.offers = if offers.len() > 1 { offers } else { vec![] };
}

impl Review {
    pub(super) fn start_command(&mut self) {
        self.cmd = Some(CmdLine::default());
    }

    /// The bottom line while typing a command or answering a question.
    pub fn prompt(&self) -> Option<(String, &[String])> {
        if self.confirm.is_some() {
            return Some(("Clear every mark in the review? y/n".into(), &[]));
        }
        let cmd = self.cmd.as_ref()?;
        let lead = if cmd.search { '/' } else { ':' };
        Some((format!("{lead}{}", cmd.text), &cmd.offers))
    }

    /// The commands whose names start with what's typed, each with a word on what it takes
    /// and what it does: all of them for `?`. None once a path is being typed.
    pub fn command_menu(&self) -> Vec<(String, &'static str)> {
        let Some(cmd) = self.cmd.as_ref().filter(|c| !c.search) else {
            return vec![];
        };
        let typed = cmd.text.as_str();
        if typed.contains(' ') {
            return vec![];
        }
        COMMANDS
            .iter()
            .filter(|(c, ..)| typed == "?" || c.starts_with(typed))
            .map(|&(c, path, what)| {
                let name = if path {
                    format!("{c} [path]")
                } else {
                    c.to_string()
                };
                (name, what)
            })
            .collect()
    }

    /// Every file path in the review, and every folder with a `/` after it.
    fn paths(&self) -> Vec<String> {
        let mut out = BTreeSet::new();
        for f in &self.files {
            out.insert(f.path.clone());
            let mut at = 0;
            while let Some(i) = f.path[at..].find('/') {
                out.insert(f.path[..at + i + 1].to_string());
                at += i + 1;
            }
        }
        out.into_iter().collect()
    }

    pub(super) fn command_key(&mut self, key: KeyEvent, store: Option<&mut Store>) -> Outcome {
        if self.confirm.take().is_some() {
            if key.code == KeyCode::Char('y') {
                let all: Vec<usize> = (0..self.files.len()).collect();
                self.clear(&all, store);
            }
            return Outcome::None;
        }
        let Some(line) = self.cmd.as_mut() else {
            return Outcome::None;
        };
        if key.code != KeyCode::Tab {
            line.cycle = None;
        }
        match key.code {
            KeyCode::Esc => self.cmd = None,
            KeyCode::Enter if line.search => {
                let text = self.cmd.take().map(|c| c.text).unwrap_or_default();
                self.search(&text);
            }
            KeyCode::Enter => {
                let text = self.cmd.take().map(|c| c.text).unwrap_or_default();
                return self.run(&text, store);
            }
            KeyCode::Tab if line.search => {}
            KeyCode::Tab => {
                let options = if line.text.contains(' ') {
                    self.paths()
                } else {
                    COMMANDS.iter().map(|(c, ..)| c.to_string()).collect()
                };
                if let Some(line) = self.cmd.as_mut() {
                    complete(line, &options);
                }
            }
            KeyCode::Backspace => {
                if line.text.pop().is_none() {
                    self.cmd = None;
                } else {
                    line.offers.clear();
                }
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                line.text.push(c);
                line.offers.clear();
            }
            _ => {}
        }
        Outcome::None
    }

    /// Runs a command line.
    fn run(&mut self, text: &str, store: Option<&mut Store>) -> Outcome {
        let mut words = text.split_whitespace();
        let Some(name) = words.next() else {
            return Outcome::None;
        };
        let path = words.next();
        if name == "?" {
            return Outcome::None;
        }
        let takes_path = COMMANDS
            .iter()
            .find(|(c, ..)| *c == name)
            .map(|(_, p, _)| *p);
        match takes_path {
            None => {
                self.notice = Some(format!("no command `{name}`"));
                return Outcome::None;
            }
            Some(false) if path.is_some() => {
                self.notice = Some(format!("`{name}` takes no path"));
                return Outcome::None;
            }
            _ => {}
        }
        let files = match path {
            Some(p) => self.files_at(p),
            None => self.list_files(),
        };
        if let (Some(p), true) = (path, files.is_empty()) {
            self.notice = Some(format!("`{p}` isn't in the review"));
            return Outcome::None;
        }
        match name {
            "review" | "flag" => {
                let (Some(marks), Some(store)) = (self.marks.as_mut(), store) else {
                    return Outcome::None;
                };
                let result = match name {
                    "review" => marks.set_checks(store, &files, true),
                    _ => marks.mark_files(store, &files, MarkColor::Red),
                };
                match result {
                    Ok(edit) => {
                        self.history.did(Step::Marks(edit));
                        self.refold(&files);
                    }
                    Err(e) => self.notice = Some(format!("{e:#}")),
                }
            }
            "clear" => self.clear(&files, store),
            "clear-all" => self.confirm = Some(Confirm::ClearAll),
            "pull" => return Outcome::Pull,
            "split" => {
                self.split = !self.split;
                self.relayout();
            }
            "resolved" => {
                self.show_resolved = !self.show_resolved;
                if let Some(store) = store {
                    self.load_comments(store);
                }
            }
            "edit" => {
                if let Some((path, line)) = self.edit_target() {
                    return Outcome::Editor(Some((path, line)));
                }
            }
            "h" => return Outcome::Back,
            _ => return Outcome::Quit,
        }
        Outcome::None
    }

    /// Clears the files' marks and checks, the agent's too.
    fn clear(&mut self, files: &[usize], store: Option<&mut Store>) {
        let (Some(marks), Some(store)) = (self.marks.as_mut(), store) else {
            return;
        };
        match marks.clear(store, files) {
            Ok(edit) => {
                self.history.did(Step::Marks(edit));
                self.refold(files);
            }
            Err(e) => self.notice = Some(format!("{e:#}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tab(text: &str, options: &[&str]) -> CmdLine {
        let mut line = CmdLine {
            text: text.into(),
            ..Default::default()
        };
        let options: Vec<String> = options.iter().map(|o| o.to_string()).collect();
        complete(&mut line, &options);
        line
    }

    #[test]
    fn tab_completes_what_the_offers_share_then_cycles_through_them() {
        let paths = ["src/", "src/a.rs", "src/ab.rs", "README.md"];
        assert_eq!(tab("review R", &paths).text, "review README.md");
        assert_eq!(tab("review src/a", &paths).text, "review src/a.rs");
        let mut line = tab("clear s", &paths);
        assert_eq!(line.text, "clear src/");
        assert_eq!(line.offers, ["src/", "src/a.rs", "src/ab.rs"]);
        complete(&mut line, &paths.map(String::from));
        assert_eq!(line.text, "clear src/a.rs");
        complete(&mut line, &paths.map(String::from));
        assert_eq!(line.text, "clear src/ab.rs");
        assert_eq!(tab("cl", &["clear", "clear-all"]).text, "clear");
    }
}
