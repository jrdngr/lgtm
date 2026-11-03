//! Builds a review's per-file state and the UI's view of each file from a diff.

use std::collections::HashMap;

use serde::Serialize;
use specta::Type;

use lgtm_core::review::file::Text;
pub use lgtm_core::review::file::{FileSide, Keyed, ReviewFile, RowPos, Sides};

use crate::config::Config;
use crate::diff::{FileStatus, Hunk, RowKind};
use crate::jj::FileDiff;
use crate::syntax::{self, CodeSymbol, Segment};

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
    /// How many lines the new side has, so the code after the last hunk can be shown.
    pub line_count: u32,
    pub old_symbols: Vec<CodeSymbol>,
    pub new_symbols: Vec<CodeSymbol>,
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

    let file = ReviewFile::build(
        sides,
        &d,
        Text {
            text: old_text.as_deref(),
            parsed: old_parsed.as_ref(),
        },
        Text {
            text: new_text.as_deref(),
            parsed: new_parsed.as_ref(),
        },
        config.matching,
        config.skips(&d.path),
    );
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
        skipped: file.skipped,
        hunks,
        line_count: new_text.as_deref().map_or(0, |t| t.lines().count() as u32),
        old_symbols: file.old.symbols.clone(),
        new_symbols: file.new.symbols.clone(),
    };
    (file, dto)
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
