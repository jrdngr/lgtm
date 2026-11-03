//! The editor's `:` commands.

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::{Editor, Outcome};

/// The commands, and what they do, for the menu over the command line.
pub const COMMANDS: &[(&str, &str)] = &[
    ("w", "save"),
    ("q", "leave the editor"),
    ("q!", "leave, dropping unsaved edits"),
    ("wq", "save and leave"),
    ("N", "go to line N"),
];

impl Editor {
    pub(super) fn command_key(&mut self, key: KeyEvent) -> Outcome {
        let Some(text) = self.cmd.as_mut() else {
            return Outcome::None;
        };
        match key.code {
            KeyCode::Esc => self.cmd = None,
            KeyCode::Backspace if text.is_empty() => self.cmd = None,
            KeyCode::Backspace => {
                text.pop();
            }
            KeyCode::Char(c) => text.push(c),
            KeyCode::Enter => {
                let line = self.cmd.take().unwrap_or_default();
                return self.run_command(line.trim());
            }
            _ => {}
        }
        Outcome::None
    }

    pub(super) fn run_command(&mut self, line: &str) -> Outcome {
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
            Ok(()) => true,
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

    /// The bottom line while typing a command.
    pub fn prompt(&self) -> Option<String> {
        self.cmd.as_ref().map(|t| format!(":{t}"))
    }

    /// The commands that start with what's typed.
    pub fn command_menu(&self) -> Vec<(String, &'static str)> {
        let Some(typed) = &self.cmd else {
            return vec![];
        };
        COMMANDS
            .iter()
            .filter(|(c, _)| c.starts_with(typed.as_str()))
            .map(|&(c, what)| (c.to_string(), what))
            .collect()
    }
}
