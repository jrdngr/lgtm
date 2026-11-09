//! Code folding: ranges of lines from the syntax tree, or from indentation where there's no
//! grammar, and which of them are closed.

use std::collections::{BTreeMap, BTreeSet};

use lgtm_core::syntax::nodes::{self, add_span};

use super::buffer::Buffer;

/// A foldable run of lines: `start` stays in view, `start + 1..=end` hide when it's closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Range {
    pub start: usize,
    pub end: usize,
    /// How many ranges hold it.
    pub depth: usize,
}

#[derive(Default)]
pub struct Folds {
    pub ranges: Vec<Range>,
    /// Closed ranges, by start line.
    closed: BTreeSet<usize>,
    /// The buffer version the ranges are for.
    version: Option<u64>,
    /// Ranges this deep or deeper are closed, after `zm` or `zr`.
    level: Option<usize>,
}

/// What `z` keys do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fold {
    Toggle,
    Close,
    Open,
    CloseAll,
    OpenAll,
    CloseLevel,
    OpenLevel,
}

impl Folds {
    /// Finds the ranges again when the text changed, keeping closed what still starts on the
    /// same lines.
    pub fn update(&mut self, b: &Buffer) {
        if self.version == Some(b.version) {
            return;
        }
        self.version = Some(b.version);
        self.ranges = ranges(b);
        let starts: BTreeSet<usize> = self.ranges.iter().map(|r| r.start).collect();
        self.closed.retain(|s| starts.contains(s));
    }

    fn max_depth(&self) -> usize {
        self.ranges.iter().map(|r| r.depth + 1).max().unwrap_or(0)
    }

    pub fn is_closed(&self, start: usize) -> bool {
        self.closed.contains(&start)
    }

    /// The innermost range starting on `line`, else the innermost holding it.
    fn at(&self, line: usize, pick: impl Fn(&Range) -> bool) -> Option<Range> {
        let holding = |r: &&Range| r.start <= line && line <= r.end && pick(r);
        let starting = self
            .ranges
            .iter()
            .filter(|r| r.start == line && pick(r))
            .max_by_key(|r| r.depth);
        starting
            .or_else(|| self.ranges.iter().filter(holding).max_by_key(|r| r.depth))
            .copied()
    }

    pub fn apply(&mut self, fold: Fold, line: usize) {
        match fold {
            Fold::Toggle => match self.at(line, |r| self.is_closed(r.start)) {
                Some(r) => self.open_range(r),
                None => self.close(line),
            },
            Fold::Close => self.close(line),
            Fold::Open => {
                if let Some(r) = self.outermost_closed(line) {
                    self.open_range(r);
                }
            }
            Fold::CloseAll => self.set_level(0),
            Fold::OpenAll => self.set_level(self.max_depth()),
            Fold::CloseLevel => {
                let level = self.level.unwrap_or(self.max_depth());
                self.set_level(level.saturating_sub(1));
            }
            Fold::OpenLevel => {
                let level = self.level.unwrap_or(self.max_depth());
                self.set_level((level + 1).min(self.max_depth()));
            }
        }
    }

    fn close(&mut self, line: usize) {
        if let Some(r) = self.at(line, |r| !self.is_closed(r.start)) {
            self.closed.insert(r.start);
        }
    }

    fn open_range(&mut self, r: Range) {
        self.closed.remove(&r.start);
    }

    fn outermost_closed(&self, line: usize) -> Option<Range> {
        self.ranges
            .iter()
            .filter(|r| r.start <= line && line <= r.end && self.is_closed(r.start))
            .min_by_key(|r| r.depth)
            .copied()
    }

    fn set_level(&mut self, level: usize) {
        self.level = Some(level);
        self.closed = self
            .ranges
            .iter()
            .filter(|r| r.depth >= level)
            .map(|r| r.start)
            .collect();
    }

    /// Opens the closed ranges that hide `line`.
    pub fn reveal(&mut self, line: usize) {
        let hiding: Vec<usize> = self
            .ranges
            .iter()
            .filter(|r| r.start < line && line <= r.end && self.is_closed(r.start))
            .map(|r| r.start)
            .collect();
        for s in hiding {
            self.closed.remove(&s);
        }
    }

    /// The closed ranges that hide lines, outermost only, by start line.
    pub fn hidden(&self) -> Vec<(usize, usize)> {
        let mut out: Vec<(usize, usize)> = vec![];
        for r in &self.ranges {
            if !self.is_closed(r.start) {
                continue;
            }
            if out.last().is_some_and(|&(_, end)| r.start <= end) {
                continue;
            }
            out.push((r.start, r.end));
        }
        out
    }

    /// The lines in view, in order, through `last`.
    pub fn visible(&self, last: usize) -> Vec<usize> {
        let hidden = self.hidden();
        let mut out = Vec::with_capacity(last + 1);
        let mut skip = hidden.iter().peekable();
        let mut line = 0;
        while line <= last {
            out.push(line);
            match skip.peek() {
                Some(&&(start, end)) if start == line => {
                    skip.next();
                    line = end + 1;
                }
                _ => line += 1,
            }
        }
        out
    }
}

/// Every foldable range, sorted by start line, one per start line.
pub fn ranges(b: &Buffer) -> Vec<Range> {
    let mut ends: BTreeMap<usize, usize> = BTreeMap::new();
    match nodes::fold_spans(&b.name, &b.text.to_string()) {
        Some(spans) => ends = spans,
        None => indent_ranges(b, &mut ends),
    }
    let spans: Vec<(usize, usize)> = ends.into_iter().collect();
    spans
        .iter()
        .map(|&(start, end)| Range {
            start,
            end,
            depth: spans
                .iter()
                .filter(|&&(s, e)| s < start && end <= e)
                .count(),
        })
        .collect()
}

/// For a file with no grammar: a line and the more indented lines after it.
fn indent_ranges(b: &Buffer, ends: &mut BTreeMap<usize, usize>) {
    let last = b.last_line();
    let blank = |l: usize| b.line_end(l) == b.line_start(l) + b.indent(l).chars().count();
    let depth = |l: usize| {
        b.indent(l)
            .chars()
            .map(|c| if c == '\t' { 4 } else { 1 })
            .sum::<usize>()
    };
    for line in 0..=last {
        if blank(line) {
            continue;
        }
        let d = depth(line);
        let mut end = line;
        for next in line + 1..=last {
            if blank(next) {
                continue;
            }
            if depth(next) <= d {
                break;
            }
            end = next;
        }
        add_span(ends, line, end);
    }
}
