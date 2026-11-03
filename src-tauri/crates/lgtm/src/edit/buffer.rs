//! An open file: its text, its selection, what's on disk, and its undo history.

use std::path::{Path, PathBuf};

use ropey::Rope;

/// A selection: the chars from `anchor` to `head`, both included. `head` is the cursor. In
/// insert mode `head` is the gap typing goes into, before that char.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sel {
    pub anchor: usize,
    pub head: usize,
}

impl Sel {
    pub fn point(at: usize) -> Self {
        Self {
            anchor: at,
            head: at,
        }
    }

    pub fn new(anchor: usize, head: usize) -> Self {
        Self { anchor, head }
    }

    pub fn from(&self) -> usize {
        self.anchor.min(self.head)
    }

    pub fn to(&self) -> usize {
        self.anchor.max(self.head)
    }
}

#[derive(Clone)]
struct State {
    text: Rope,
    sel: Sel,
}

pub struct Buffer {
    pub text: Rope,
    pub sel: Sel,
    /// Where the file is.
    pub path: PathBuf,
    /// Its path in the workspace.
    pub name: String,
    /// The text as last read from or written to disk.
    saved: Rope,
    /// Bumps on every change to the text, for what's derived from it.
    pub version: u64,
    undo: Vec<State>,
    redo: Vec<State>,
    /// The column `j` and `k` keep to across shorter lines.
    pub column: Option<usize>,
}

impl Buffer {
    /// The file at `path`, `name` in the workspace, or an empty one when there's none yet.
    pub fn open(path: &Path, name: &str) -> std::io::Result<Self> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e),
        };
        Ok(Self::with_text(path, name, &text))
    }

    pub fn with_text(path: &Path, name: &str, text: &str) -> Self {
        let text = Rope::from_str(text);
        Self {
            saved: text.clone(),
            text,
            sel: Sel::default(),
            path: path.to_path_buf(),
            name: name.to_string(),
            version: 0,
            undo: vec![],
            redo: vec![],
            column: None,
        }
    }

    /// Whether the text differs from what's on disk.
    pub fn dirty(&self) -> bool {
        self.text != self.saved
    }

    pub fn save(&mut self) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut file = std::io::BufWriter::new(std::fs::File::create(&self.path)?);
        self.text.write_to(&mut file)?;
        std::io::Write::flush(&mut file)?;
        self.saved = self.text.clone();
        Ok(())
    }

    /// Marks where an undo step starts: undo comes back to the text and selection now.
    pub fn checkpoint(&mut self) {
        self.undo.push(State {
            text: self.text.clone(),
            sel: self.sel,
        });
        self.redo.clear();
    }

    /// Drops the last checkpoint when nothing changed after it.
    pub fn drop_empty_checkpoint(&mut self) {
        if self.undo.last().is_some_and(|s| s.text == self.text) {
            self.undo.pop();
        }
    }

    pub fn undo(&mut self) -> bool {
        self.step(true)
    }

    pub fn redo(&mut self) -> bool {
        self.step(false)
    }

    fn step(&mut self, back: bool) -> bool {
        let (from, to) = if back {
            (&mut self.undo, &mut self.redo)
        } else {
            (&mut self.redo, &mut self.undo)
        };
        let Some(state) = from.pop() else {
            return false;
        };
        to.push(State {
            text: std::mem::replace(&mut self.text, state.text),
            sel: self.sel,
        });
        self.sel = state.sel;
        self.version += 1;
        self.clamp();
        true
    }

    /// Inserts `s` at char `at`.
    pub fn insert(&mut self, at: usize, s: &str) {
        self.text.insert(at, s);
        self.version += 1;
    }

    /// Removes chars `from..to`.
    pub fn remove(&mut self, from: usize, to: usize) {
        if from < to {
            self.text.remove(from..to);
            self.version += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.text.len_chars()
    }

    pub fn char(&self, i: usize) -> Option<char> {
        (i < self.len()).then(|| self.text.char(i))
    }

    pub fn line_of(&self, i: usize) -> usize {
        self.text.char_to_line(i.min(self.len()))
    }

    pub fn line_start(&self, line: usize) -> usize {
        self.text.line_to_char(line)
    }

    /// The char index of `line`'s end: its newline, or the text's end.
    pub fn line_end(&self, line: usize) -> usize {
        let next = self.line_start((line + 1).min(self.text.len_lines()));
        if line + 1 < self.text.len_lines() {
            next - 1
        } else {
            self.len()
        }
    }

    /// The last line that has text or follows a newline the text doesn't end on. A text
    /// ending in a newline has no line after it.
    pub fn last_line(&self) -> usize {
        let lines = self.text.len_lines();
        if lines > 1 && self.line_start(lines - 1) == self.len() {
            lines - 2
        } else {
            lines - 1
        }
    }

    /// The last char a normal-mode cursor can be on.
    pub fn last_char(&self) -> usize {
        self.len().saturating_sub(1)
    }

    /// The selection kept inside the text.
    pub fn clamp(&mut self) {
        let max = self.last_char();
        self.sel.anchor = self.sel.anchor.min(max);
        self.sel.head = self.sel.head.min(max);
    }

    /// `line`'s leading spaces and tabs.
    pub fn indent(&self, line: usize) -> String {
        let start = self.line_start(line);
        let end = self.line_end(line);
        self.text
            .slice(start..end)
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect()
    }
}
