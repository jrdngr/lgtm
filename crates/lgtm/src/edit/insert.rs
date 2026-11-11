//! Insert mode: starting it, and the keys that type.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::buffer::{Buffer, Sel};
use super::keys::{Keys, Mode};
use super::motion;

impl Keys {
    pub(super) fn enter_insert(&mut self, b: &mut Buffer, at: usize) {
        b.checkpoint();
        b.sel = Sel::point(at);
        self.start_insert(b);
    }

    pub(super) fn start_insert(&mut self, b: &Buffer) {
        self.inserted_at = b.version;
        self.mode = Mode::Insert;
    }

    /// Opens a line below or above the selection's, with its indent, to type into.
    pub(super) fn open_line(&mut self, b: &mut Buffer, below: bool) {
        b.checkpoint();
        let line = if below {
            b.line_of(b.sel.to())
        } else {
            b.line_of(b.sel.from())
        };
        let indent = b.indent(line);
        let at = if below {
            let end = b.line_end(line);
            b.insert(end, &format!("\n{indent}"));
            end + 1 + indent.chars().count()
        } else {
            let start = b.line_start(line);
            b.insert(start, &format!("{indent}\n"));
            start + indent.chars().count()
        };
        b.sel = Sel::point(at);
        self.start_insert(b);
    }

    pub(super) fn insert_key(&mut self, b: &mut Buffer, key: KeyEvent) {
        let at = b.sel.head;
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Esc => {
                super::keys::renumber(b);
                let at = b.sel.head;
                b.drop_empty_checkpoint();
                self.mode = Mode::Normal;
                // Back onto the last char typed, as vim does.
                let typed = b.version != self.inserted_at;
                let back = typed && at > b.line_start(b.line_of(at));
                b.sel = Sel::point(if back { at - 1 } else { at });
                b.clamp();
                return;
            }
            KeyCode::Char('w') if ctrl => {
                let from = motion::word_back_from(b, at);
                b.remove(from, at);
                b.sel = Sel::point(from);
            }
            KeyCode::Char(c) if !ctrl => {
                b.insert(at, &c.to_string());
                b.sel = Sel::point(at + 1);
            }
            KeyCode::Tab => {
                let tab = motion::indent_unit(b);
                b.insert(at, &tab);
                b.sel = Sel::point(at + tab.chars().count());
            }
            KeyCode::Enter if self.markdown(b, super::markdown::enter) => {}
            KeyCode::BackTab => {
                self.markdown(b, super::markdown::smart);
            }
            KeyCode::Enter => {
                let indent = b.indent(b.line_of(at));
                // Not more indent than the cursor is into the line.
                let into = at - b.line_start(b.line_of(at));
                let indent: String = indent.chars().take(into).collect();
                b.insert(at, &format!("\n{indent}"));
                b.sel = Sel::point(at + 1 + indent.chars().count());
            }
            KeyCode::Backspace if at > 0 => {
                b.remove(at - 1, at);
                b.sel = Sel::point(at - 1);
            }
            KeyCode::Delete if at < b.len() => b.remove(at, at + 1),
            KeyCode::Left => b.sel = Sel::point(at.saturating_sub(1)),
            KeyCode::Right => b.sel = Sel::point((at + 1).min(b.len())),
            KeyCode::Home => b.sel = Sel::point(b.line_start(b.line_of(at))),
            KeyCode::End => b.sel = Sel::point(b.line_end(b.line_of(at))),
            KeyCode::Up | KeyCode::Down => {
                let by = if key.code == KeyCode::Up { -1 } else { 1 };
                let (to, column) = motion::vertical(b, at, by, b.column);
                b.column = Some(column);
                b.sel = Sel::point(to);
                return;
            }
            _ => {}
        }
        b.column = None;
    }
}
