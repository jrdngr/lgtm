//! The editor's `:` commands.

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use std::path::Path;

use super::{Editor, Outcome};

/// The commands, and what they do, for the menu over the command line.
pub const COMMANDS: &[(&str, &str)] = &[
    ("e", "open a file"),
    ("w", "save, or save as a path"),
    ("wa", "save all"),
    ("h", "home"),
    ("heat", "heat on the files"),
    ("q", "leave the editor"),
    ("q!", "leave, dropping unsaved edits"),
    ("wq", "save and leave"),
    ("wqa", "save all and leave"),
    ("close", "close the file in view"),
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
            "e" | "open" | "edit" => {
                match arg {
                    Some(path) => self.open_path(path),
                    None => self.pick(crate::edit::keymap::Pick::Files),
                }
                return Outcome::None;
            }
            "w" if arg.is_some() => {
                self.save_as(arg.unwrap_or_default());
                return Outcome::None;
            }
            _ => {}
        }
        match line {
            "w" => {
                self.save();
            }
            "wa" => {
                self.save_all();
            }
            "wq" | "x" => {
                if self.save() {
                    return self.leave(false);
                }
            }
            "wqa" | "xa" => {
                if self.save_all() {
                    return self.leave(false);
                }
            }
            "h" | "home" => return Outcome::Home,
            "heat" => self.sidebar.heat_on = !self.sidebar.heat_on,
            "q" | "qa" => return self.leave(false),
            "q!" | "qa!" => return self.leave(true),
            "close" | "c" => return self.close(),
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
        self.save_at(self.cur)
    }

    fn save_at(&mut self, i: usize) -> bool {
        let Some(o) = self.open.get_mut(i) else {
            return false;
        };
        match o.buf.save() {
            Ok(()) => {
                o.conflict = false;
                let name = o.buf.name.clone();
                self.sidebar.add(&name);
                self.saved = true;
                true
            }
            Err(e) => {
                self.notice = Some(format!("couldn't save {}: {e}", o.buf.name));
                false
            }
        }
    }

    /// Saves every buffer with unsaved edits. Returns whether all saved.
    fn save_all(&mut self) -> bool {
        let dirty: Vec<usize> = (0..self.open.len())
            .filter(|&i| self.open[i].buf.dirty())
            .collect();
        let mut ok = true;
        for i in dirty {
            ok &= self.save_at(i);
        }
        ok
    }

    /// Opens `path`, from the current directory or whole.
    fn open_path(&mut self, path: &str) {
        let name = self.name_of(path);
        if let Err(e) = self.open(&name, 0) {
            self.notice = Some(format!("couldn't open {name}: {e}"));
        }
    }

    /// Saves the buffer in view to `path`, from the current directory or whole, which it's
    /// then the file of.
    fn save_as(&mut self, path: &str) {
        let name = self.name_of(path);
        let full = self.root.join(&name);
        let Some(o) = self.open.get_mut(self.cur) else {
            return;
        };
        o.buf.path = full;
        o.buf.name = name;
        o.base = None;
        o.base_asked = false;
        o.marks = None;
        o.colors = None;
        self.save();
    }

    /// `path`'s name in the root: from it when it's inside, else the whole path.
    fn name_of(&self, path: &str) -> String {
        let cwd = std::env::current_dir().unwrap_or_else(|_| self.root.clone());
        crate::tui::workspace_name(&self.root, &cwd.join(path))
    }

    /// Closes the buffer in view unless it has unsaved edits. The last one closed opens the
    /// file list.
    fn close(&mut self) -> Outcome {
        let Some(o) = self.open.get(self.cur) else {
            return Outcome::None;
        };
        if o.buf.dirty() {
            self.notice = Some(format!("unsaved: {}", o.buf.name));
            return Outcome::None;
        }
        self.open.remove(self.cur);
        self.cur = self
            .cur
            .saturating_sub(1)
            .min(self.open.len().saturating_sub(1));
        match self.current().map(|b| b.name.clone()) {
            Some(name) if !Path::new(&name).is_absolute() => self.sidebar.reveal(&name),
            Some(_) => {}
            None => self.pick(crate::edit::keymap::Pick::Files),
        }
        Outcome::None
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
