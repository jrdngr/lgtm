//! The resolver's rows: the file bar, the text between conflicts, and each conflict's head,
//! side labels and lines.

use lgtm_core::resolve::{PartView, ResolveView, SideView, Tint};

/// Where a side sits in its conflict: one it keeps, by place, or one it doesn't.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Pick(usize),
    Rest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    /// What to do with a file a side deleted or moved.
    FileBar,
    Blank,
    /// Line `n` of part `part`, between conflicts, numbered `line` from 1.
    Text {
        part: usize,
        n: usize,
        line: u32,
        text: String,
    },
    Head {
        conflict: usize,
    },
    /// A side's label. `open` when its lines show under it.
    Side {
        conflict: usize,
        key: u8,
        slot: Slot,
        open: bool,
    },
    /// Line `n` of a side, with its file line once saved when it's kept.
    Code {
        conflict: usize,
        key: u8,
        slot: Slot,
        n: usize,
        line: Option<u32>,
        text: String,
        tints: Vec<(u32, u32)>,
    },
}

impl Row {
    pub fn conflict(&self) -> Option<usize> {
        match *self {
            Row::Head { conflict } | Row::Side { conflict, .. } | Row::Code { conflict, .. } => {
                Some(conflict)
            }
            _ => None,
        }
    }

    /// The side the row is part of, and where it sits.
    pub fn side(&self) -> Option<(usize, u8, Slot)> {
        match *self {
            Row::Side {
                conflict,
                key,
                slot,
                ..
            }
            | Row::Code {
                conflict,
                key,
                slot,
                ..
            } => Some((conflict, key, slot)),
            _ => None,
        }
    }
}

/// A side's label, then its lines when `open`, numbered from `first` when given.
fn side_rows(
    out: &mut Vec<Row>,
    (conflict, s, slot): (usize, &SideView, Slot),
    open: bool,
    first: Option<u32>,
) {
    out.push(Row::Side {
        conflict,
        key: s.key,
        slot,
        open,
    });
    if !open {
        return;
    }
    for (n, text) in s.text.lines().enumerate() {
        let tints = s
            .tints
            .iter()
            .filter(|t: &&Tint| t.line as usize == n)
            .map(|t| (t.from, t.to))
            .collect();
        out.push(Row::Code {
            conflict,
            key: s.key,
            slot,
            n,
            line: first.map(|f| f + n as u32),
            text: text.to_string(),
            tints,
        });
    }
}

/// `rows` with the lines of pick `pick` of `conflict` replaced by those of `text`, as it's
/// being edited.
pub fn editing(rows: &mut Vec<Row>, conflict: usize, pick: usize, text: &str) {
    let mine = |r: &Row| matches!(r, Row::Code { conflict: c, slot: Slot::Pick(p), .. } if *c == conflict && *p == pick);
    let Some(label) = rows.iter().position(|r| matches!(r, Row::Side { conflict: c, slot: Slot::Pick(p), .. } if *c == conflict && *p == pick)) else {
        return;
    };
    let (key, first) = match (&rows[label], rows.get(label + 1)) {
        (Row::Side { key, .. }, Some(Row::Code { line, .. })) if mine(&rows[label + 1]) => {
            (*key, *line)
        }
        (Row::Side { key, .. }, _) => (*key, None),
        _ => return,
    };
    let end = label + 1 + rows[label + 1..].iter().take_while(|r| mine(r)).count();
    let mut lines: Vec<&str> = text.split('\n').collect();
    if lines.len() > 1 && lines.last() == Some(&"") {
        lines.pop();
    }
    let new = lines.iter().enumerate().map(|(n, l)| Row::Code {
        conflict,
        key,
        slot: Slot::Pick(pick),
        n,
        line: first.map(|f| f + n as u32),
        text: l.to_string(),
        tints: vec![],
    });
    rows.splice(label + 1..end, new);
}

/// The rows for `view`. A conflict that keeps nothing shows its sides' lines, but the base's;
/// one that keeps something shows only what it keeps, the rest as labels.
pub fn rows(view: &ResolveView) -> Vec<Row> {
    let mut out = vec![];
    if view.fate != lgtm_core::resolve::Fate::Here {
        out.push(Row::FileBar);
        out.push(Row::Blank);
    }
    for (i, part) in view.parts.iter().enumerate() {
        match part {
            PartView::Text { line, text } => {
                for (n, l) in text.lines().enumerate() {
                    out.push(Row::Text {
                        part: i,
                        n,
                        line: line + n as u32,
                        text: l.to_string(),
                    });
                }
            }
            PartView::Conflict(c) => {
                let i = c.index as usize;
                out.push(Row::Head { conflict: i });
                let mut line = c.line;
                for (p, s) in c.picks.iter().enumerate() {
                    side_rows(&mut out, (i, s, Slot::Pick(p)), true, Some(line));
                    line += s.text.lines().count() as u32;
                }
                for s in &c.rest {
                    let open = c.picks.is_empty() && s.key != 0;
                    side_rows(&mut out, (i, s, Slot::Rest), open, None);
                }
            }
        }
    }
    out
}
