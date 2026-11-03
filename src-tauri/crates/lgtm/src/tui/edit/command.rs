//! The editor's `:` commands.

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::{Editor, Outcome};

/// The commands, and what they do, for the menu over the command line.
pub const COMMANDS: &[(&str, &str)] = &[
    ("w", "save"),
    ("q", "leave the editor"),
    ("q!", "leave, dropping unsaved edits"),
    ("wq", "save and leave"),
    ("reload", "read the file again, over edits"),
    ("vsplit", "open in a pane to the right"),
    ("split", "open in a pane below"),
    ("N", "go to line N"),
];

/// Which line is being typed.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Line {
    Command,
    Search,
}

pub struct Prompt {
    pub line: Line,
    pub text: String,
}

impl Prompt {
    pub fn new(line: Line) -> Self {
        Self {
            line,
            text: String::new(),
        }
    }
}

impl Editor {
    pub(super) fn prompt_key(&mut self, key: KeyEvent) -> Outcome {
        let Some(p) = self.prompt.as_mut() else {
            return Outcome::None;
        };
        match key.code {
            KeyCode::Esc => self.prompt = None,
            KeyCode::Backspace if p.text.is_empty() => self.prompt = None,
            KeyCode::Backspace => {
                p.text.pop();
            }
            KeyCode::Char(c) => p.text.push(c),
            KeyCode::Enter => {
                let Some(p) = self.prompt.take() else {
                    return Outcome::None;
                };
                match p.line {
                    Line::Command => return self.run_command(p.text.trim()),
                    Line::Search if !p.text.is_empty() => {
                        if let Some(open) = self.open.get_mut(self.cur) {
                            let action = self.keys.search_for(&mut open.buf, &p.text);
                            self.act(action);
                        }
                    }
                    Line::Search => {}
                }
            }
            _ => {}
        }
        Outcome::None
    }

    pub(super) fn run_command(&mut self, line: &str) -> Outcome {
        let (name, arg) = line.split_once(' ').unwrap_or((line, ""));
        let arg = Some(arg.trim()).filter(|a| !a.is_empty());
        match name {
            "vsplit" | "vs" => {
                self.split(arg, true);
                return Outcome::None;
            }
            "split" | "sp" => {
                self.split(arg, false);
                return Outcome::None;
            }
            _ => {}
        }
        match line {
            "w" => {
                self.save();
            }
            "wq" | "x" => {
                if self.save() {
                    return self.leave(false);
                }
            }
            "q" => return self.leave(false),
            "q!" => return self.leave(true),
            "reload" => {
                if let Some(o) = self.open.get_mut(self.cur) {
                    match o.buf.reload() {
                        Ok(()) => o.conflict = false,
                        Err(e) => self.notice = Some(format!("couldn't read {}: {e}", o.buf.name)),
                    }
                }
            }
            "" => {}
            n if n.parse::<usize>().is_ok() => {
                if let Some(o) = self.open.get_mut(self.cur) {
                    super::go_line(&mut o.buf, n.parse().unwrap_or(1));
                }
            }
            other => self.notice = Some(format!("no command {other}")),
        }
        Outcome::None
    }

    /// Saves the buffer in view. Returns whether it saved.
    fn save(&mut self) -> bool {
        let Some(o) = self.open.get_mut(self.cur) else {
            return false;
        };
        match o.buf.save() {
            Ok(()) => {
                o.conflict = false;
                true
            }
            Err(e) => {
                self.notice = Some(format!("couldn't save {}: {e}", o.buf.name));
                false
            }
        }
    }

    /// Leaves the editor unless a buffer has unsaved edits. `force` drops them.
    pub fn leave(&mut self, force: bool) -> Outcome {
        if force {
            self.open.retain(|o| !o.buf.dirty());
            self.cur = 0;
            return Outcome::Back;
        }
        let unsaved = self.unsaved();
        if unsaved.is_empty() {
            Outcome::Back
        } else {
            self.notice = Some(format!("unsaved: {}", unsaved.join(", ")));
            Outcome::None
        }
    }

    /// The bottom line while typing a command or a search.
    pub fn prompt(&self) -> Option<String> {
        let p = self.prompt.as_ref()?;
        let start = match p.line {
            Line::Command => ':',
            Line::Search => '/',
        };
        Some(format!("{start}{}", p.text))
    }

    /// The commands that start with what's typed.
    pub fn command_menu(&self) -> Vec<(String, &'static str)> {
        let Some(p) = self.prompt.as_ref().filter(|p| p.line == Line::Command) else {
            return vec![];
        };
        COMMANDS
            .iter()
            .filter(|(c, _)| c.starts_with(p.text.as_str()))
            .map(|&(c, what)| (c.to_string(), what))
            .collect()
    }
}
