//! Builds a review's per-file state and the UI's view of each file from a diff.

use std::collections::HashMap;

use serde::Serialize;
use specta::Type;

use crate::anchor::{self, Anchor, FileIndex, MatchConfig, Side};
use crate::config::Config;
use crate::diff::{FileStatus, Hunk, RowKind};
use crate::jj::FileDiff;
use crate::store::Store;
use crate::syntax::{self, CodeSymbol, Ref, Segment};

#[derive(Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RowDto {
    pub kind: RowKind,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    /// A removed row in a change that also adds rows. It doesn't need review.
    pub replaced: bool,
    pub text: String,
    /// Highlight segments; absent when the language is unknown or the file has no contents.
    pub hl: Option<Vec<Segment>>,
}

#[derive(Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HunkDto {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub section: String,
    pub rows: Vec<RowDto>,
}

#[derive(Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileDto {
    pub path: String,
    pub old_path: Option<String>,
    pub status: FileStatus,
    pub binary: bool,
    pub conflict: bool,
    pub old_exec: bool,
    pub new_exec: bool,
    /// Matches a `skip` glob from the repo config.
    pub skipped: bool,
    pub hunks: Vec<HunkDto>,
    pub old_symbols: Vec<CodeSymbol>,
    pub new_symbols: Vec<CodeSymbol>,
}

#[derive(Clone)]
pub struct FileSide {
    /// `None` for the root commit and for interdiffs, whose old side isn't a commit.
    pub commit: Option<String>,
    pub path: String,
    pub index: Option<FileIndex>,
    pub symbols: Vec<CodeSymbol>,
    /// Identifiers the side mentions, for the symbol graph.
    pub refs: Vec<Ref>,
}

impl FileSide {
    /// The commit a mark or pin on this side anchors to, after saving its line hashes so it
    /// can be remapped later. `None` for an interdiff's old side, which isn't a commit.
    pub fn anchor_commit(&self, store: &Store) -> anyhow::Result<Option<String>> {
        let Some(commit) = self.commit.clone() else {
            return Ok(None);
        };
        if let Some(index) = &self.index {
            store.save_line_hashes(&commit, &self.path, &index.hashes)?;
        }
        Ok(Some(commit))
    }
}

/// A changed row and the anchor a mark made on it would get.
#[derive(Clone)]
pub struct Keyed {
    pub row: u32,
    pub side: Side,
    pub line: u32,
    pub anchor: Anchor,
}

/// A row's kind and line numbers, enough to find it again after the diff changes.
#[derive(Clone, Copy, PartialEq)]
pub struct RowPos {
    pub kind: RowKind,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    /// See `RowDto::replaced`.
    pub replaced: bool,
}

#[derive(Clone)]
pub struct ReviewFile {
    pub status: FileStatus,
    pub path: String,
    pub old_path: Option<String>,
    pub old: FileSide,
    pub new: FileSide,
    pub rows: Vec<RowPos>,
    /// Each row's code, for text sent to agents.
    pub texts: Vec<String>,
    pub keyed: Vec<Keyed>,
    /// Matches a `skip` glob from the repo config.
    pub skipped: bool,
}

impl ReviewFile {
    pub fn side(&self, side: Side) -> &FileSide {
        match side {
            Side::Old => &self.old,
            Side::New => &self.new,
        }
    }
}

/// The commits whose file contents form the old and new sides.
pub struct Sides {
    pub old: Option<String>,
    pub new: String,
}

/// Builds a file's review state and the UI's view of it. CPU-bound: parsing and highlighting.
pub fn build_file(
    sides: &Sides,
    d: FileDiff,
    old_text: Option<String>,
    new_text: Option<String>,
    config: &Config,
) -> (ReviewFile, FileDto) {
    let old_path = d.old_path.clone().unwrap_or_else(|| d.path.clone());
    let old_norm = anchor::norm_for(&old_path);
    let new_norm = anchor::norm_for(&d.path);
    let old_parsed = old_text
        .as_deref()
        .and_then(|t| syntax::parse(&old_path, t));
    let new_parsed = new_text.as_deref().and_then(|t| syntax::parse(&d.path, t));

    let rows = d.hunks.iter().flat_map(|h| &h.rows);
    let old_lines: Vec<u32> = rows
        .clone()
        .filter(|r| r.kind == RowKind::Removed)
        .filter_map(|r| r.old_line)
        .collect();
    let new_lines: Vec<u32> = rows.clone().filter_map(|r| r.new_line).collect();
    let old_hl = old_parsed
        .as_ref()
        .map(|p| p.highlight(old_text.as_deref().unwrap(), &old_lines));
    let new_hl = new_parsed
        .as_ref()
        .map(|p| p.highlight(new_text.as_deref().unwrap(), &new_lines));

    let side_syms = |p: Option<&syntax::Parsed>, t: Option<&str>| match (p, t) {
        (Some(p), Some(t)) => (p.symbols(t), p.refs(t)),
        _ => (vec![], vec![]),
    };
    let (old_symbols, old_refs) = side_syms(old_parsed.as_ref(), old_text.as_deref());
    let (new_symbols, new_refs) = side_syms(new_parsed.as_ref(), new_text.as_deref());
    let old = FileSide {
        commit: sides.old.clone(),
        path: old_path.clone(),
        index: old_text.as_deref().map(|t| FileIndex::new(t, old_norm)),
        symbols: old_symbols,
        refs: old_refs,
    };
    let new = FileSide {
        commit: Some(sides.new.clone()),
        path: d.path.clone(),
        index: new_text.as_deref().map(|t| FileIndex::new(t, new_norm)),
        symbols: new_symbols,
        refs: new_refs,
    };

    let keyed = rows
        .clone()
        .enumerate()
        .filter_map(|(i, r)| {
            let (side, line, fs, norm) = match r.kind {
                RowKind::Context => return None,
                RowKind::Removed => (Side::Old, r.old_line?, &old, old_norm),
                RowKind::Added => (Side::New, r.new_line?, &new, new_norm),
            };
            Some(Keyed {
                row: i as u32,
                side,
                line,
                anchor: key(fs, side, &r.text, line, norm, config.matching),
            })
        })
        .collect();

    let hunks = d
        .hunks
        .iter()
        .map(|h| hunk_dto(h, old_hl.as_ref(), new_hl.as_ref()))
        .collect();
    let dto = FileDto {
        path: d.path.clone(),
        old_path: d.old_path.clone(),
        status: d.status,
        binary: d.binary,
        conflict: d.conflict,
        old_exec: d.old_exec,
        new_exec: d.new_exec,
        skipped: config.skips(&d.path),
        hunks,
        old_symbols: old.symbols.clone(),
        new_symbols: new.symbols.clone(),
    };
    let file = ReviewFile {
        skipped: dto.skipped,
        texts: rows.clone().map(|r| r.text.clone()).collect(),
        status: d.status,
        path: d.path,
        old_path: d.old_path,
        old,
        new,
        rows: rows
            .zip(d.hunks.iter().flat_map(|h| h.replaced()))
            .map(|(r, replaced)| RowPos {
                kind: r.kind,
                old_line: r.old_line,
                new_line: r.new_line,
                replaced,
            })
            .collect(),
        keyed,
    };
    (file, dto)
}

fn key(
    fs: &FileSide,
    side: Side,
    text: &str,
    line: u32,
    norm: anchor::Norm,
    cfg: MatchConfig,
) -> Anchor {
    let (above, below) = fs
        .index
        .as_ref()
        .map(|ix| ix.context(line, cfg.window))
        .unwrap_or_default();
    Anchor {
        path: fs.path.clone(),
        side,
        line_hash: anchor::hash_line(text, norm),
        above,
        below,
    }
}

fn hunk_dto(
    h: &Hunk,
    old_hl: Option<&HashMap<u32, Vec<Segment>>>,
    new_hl: Option<&HashMap<u32, Vec<Segment>>>,
) -> HunkDto {
    let rows = h
        .rows
        .iter()
        .zip(h.replaced())
        .map(|(r, replaced)| {
            let hl = match r.kind {
                RowKind::Removed => old_hl.zip(r.old_line).and_then(|(m, l)| m.get(&l)),
                _ => new_hl.zip(r.new_line).and_then(|(m, l)| m.get(&l)),
            };
            RowDto {
                kind: r.kind,
                old_line: r.old_line,
                new_line: r.new_line,
                replaced,
                text: r.text.clone(),
                hl: hl.cloned(),
            }
        })
        .collect();
    HunkDto {
        old_start: h.old_start,
        old_lines: h.old_lines,
        new_start: h.new_start,
        new_lines: h.new_lines,
        section: h.section.clone(),
        rows,
    }
}
