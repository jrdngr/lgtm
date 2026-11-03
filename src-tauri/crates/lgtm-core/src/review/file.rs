//! A file under review: its sides, its rows, and the anchor each changed row would get.

use crate::anchor::{self, Anchor, FileIndex, MatchConfig, Side};
use crate::diff::{FileStatus, RowKind};
use crate::jj::FileDiff;
use crate::store::Store;
use crate::syntax::{CodeSymbol, Parsed, Ref};

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
    /// A removed row in a change that also adds rows. It doesn't need review.
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
    /// See `fingerprint`.
    pub fingerprint: i64,
    /// Matches a `skip` glob from the repo config.
    pub skipped: bool,
}

/// The commits whose file contents form the old and new sides.
pub struct Sides {
    pub old: Option<String>,
    pub new: String,
}

/// A side's text and its parse, when it has a grammar.
#[derive(Clone, Copy, Default)]
pub struct Text<'a> {
    pub text: Option<&'a str>,
    pub parsed: Option<&'a Parsed>,
}

impl ReviewFile {
    pub fn side(&self, side: Side) -> &FileSide {
        match side {
            Side::Old => &self.old,
            Side::New => &self.new,
        }
    }

    /// The file's review state from its diff and each side's text. Symbols come from the
    /// parses, so a side without one has none.
    pub fn build(
        sides: &Sides,
        d: &FileDiff,
        old: Text,
        new: Text,
        cfg: MatchConfig,
        skipped: bool,
    ) -> Self {
        let old_path = d.old_path.clone().unwrap_or_else(|| d.path.clone());
        let old_norm = anchor::norm_for(&old_path);
        let new_norm = anchor::norm_for(&d.path);
        let side_syms = |t: Text| match (t.parsed, t.text) {
            (Some(p), Some(text)) => (p.symbols(text), p.refs(text)),
            _ => (vec![], vec![]),
        };
        let (old_symbols, old_refs) = side_syms(old);
        let (new_symbols, new_refs) = side_syms(new);
        let old_side = FileSide {
            commit: sides.old.clone(),
            path: old_path,
            index: old.text.map(|t| FileIndex::new(t, old_norm)),
            symbols: old_symbols,
            refs: old_refs,
        };
        let new_side = FileSide {
            commit: Some(sides.new.clone()),
            path: d.path.clone(),
            index: new.text.map(|t| FileIndex::new(t, new_norm)),
            symbols: new_symbols,
            refs: new_refs,
        };
        let rows = d.hunks.iter().flat_map(|h| &h.rows);
        let keyed = rows
            .clone()
            .enumerate()
            .filter_map(|(i, r)| {
                let (side, line, fs, norm) = match r.kind {
                    RowKind::Context => return None,
                    RowKind::Removed => (Side::Old, r.old_line?, &old_side, old_norm),
                    RowKind::Added => (Side::New, r.new_line?, &new_side, new_norm),
                };
                Some(Keyed {
                    row: i as u32,
                    side,
                    line,
                    anchor: key(fs, side, &r.text, line, norm, cfg),
                })
            })
            .collect();
        ReviewFile {
            skipped,
            fingerprint: fingerprint(d),
            texts: rows.clone().map(|r| r.text.clone()).collect(),
            status: d.status,
            path: d.path.clone(),
            old_path: d.old_path.clone(),
            old: old_side,
            new: new_side,
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
        }
    }
}

/// A hash of the parts of a file's diff that have no rows: its status, rename source, modes,
/// and contents when binary. A check stays current only while this is unchanged.
fn fingerprint(d: &FileDiff) -> i64 {
    let blobs = d.binary.then_some(d.blobs.as_deref()).flatten();
    let meta = format!(
        "{:?} {:?} {} {} {:?}",
        d.status, d.old_path, d.old_exec, d.new_exec, blobs
    );
    let digest = blake3::hash(meta.as_bytes());
    i64::from_le_bytes(digest.as_bytes()[..8].try_into().unwrap())
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
