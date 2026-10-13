//! Parses `jj diff --git` output and joins it with per-file metadata from a `-T` template.

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum FileStatus {
    Modified,
    Added,
    Removed,
    Copied,
    Renamed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum RowKind {
    Context,
    Added,
    Removed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub kind: RowKind,
    /// 1-based line in the old file. Set for context and removed rows.
    pub old_line: Option<u32>,
    /// 1-based line in the new file. Set for context and added rows.
    pub new_line: Option<u32>,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Hunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    /// Text after the second `@@`, usually the enclosing function.
    pub section: String,
    pub rows: Vec<Row>,
}

impl Hunk {
    /// Per row, whether it's a removed row in a change that also adds rows. Only the added
    /// rows of such a change need review.
    pub fn replaced(&self) -> Vec<bool> {
        let mut out = vec![false; self.rows.len()];
        let mut start = 0;
        while start < self.rows.len() {
            let end = self.rows[start..]
                .iter()
                .position(|r| r.kind == RowKind::Context)
                .map_or(self.rows.len(), |n| start + n);
            let change = &self.rows[start..end];
            if change.iter().any(|r| r.kind == RowKind::Added) {
                for (i, r) in change.iter().enumerate() {
                    out[start + i] = r.kind == RowKind::Removed;
                }
            }
            start = end + 1;
        }
        out
    }
}

/// One entry of the `-T` template output.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMeta {
    pub path: String,
    pub status: FileStatus,
    pub source: String,
    pub source_type: String,
    pub target_type: String,
    pub source_exec: bool,
    pub target_exec: bool,
    pub conflict: bool,
}

/// Template for `jj diff -T`, one JSON object per line.
pub const META_TEMPLATE: &str = concat!(
    r#""{\"path\":" ++ json(path)"#,
    r#" ++ ",\"status\":" ++ json(status)"#,
    r#" ++ ",\"source\":" ++ json(source.path())"#,
    r#" ++ ",\"sourceType\":" ++ json(source.file_type())"#,
    r#" ++ ",\"targetType\":" ++ json(target.file_type())"#,
    r#" ++ ",\"sourceExec\":" ++ json(source.executable())"#,
    r#" ++ ",\"targetExec\":" ++ json(target.executable())"#,
    r#" ++ ",\"conflict\":" ++ json(target.conflict())"#,
    r#" ++ "}\n""#,
);

pub fn parse_meta(output: &str) -> anyhow::Result<Vec<FileMeta>> {
    output
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).with_context(|| format!("bad diff metadata: {l}")))
        .collect()
}

/// One `diff --git` section.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct GitFileDiff {
    pub header: String,
    /// Lines between the header and the first hunk: modes, renames, `---`/`+++`.
    pub extended: Vec<String>,
    pub binary: bool,
    pub hunks: Vec<Hunk>,
}

pub fn parse_git(output: &str) -> anyhow::Result<Vec<GitFileDiff>> {
    let mut files: Vec<GitFileDiff> = Vec::new();
    let mut lines = output.lines().peekable();
    while let Some(line) = lines.next() {
        if let Some(header) = line.strip_prefix("diff --git ") {
            files.push(GitFileDiff {
                header: header.to_string(),
                ..Default::default()
            });
            continue;
        }
        let Some(file) = files.last_mut() else {
            bail!("diff output before first header: {line}");
        };
        if line.starts_with("Binary files ") {
            file.binary = true;
        } else if let Some(range) = line.strip_prefix("@@ ") {
            let mut hunk = parse_hunk_header(range).with_context(|| format!("bad hunk: {line}"))?;
            let (mut old, mut new) = (hunk.old_start, hunk.new_start);
            let (mut old_left, mut new_left) = (hunk.old_lines, hunk.new_lines);
            while old_left > 0 || new_left > 0 {
                let Some(row) = lines.next() else {
                    bail!("hunk ends early: {line}")
                };
                if row.starts_with('\\') {
                    continue; // "\ No newline at end of file"
                }
                let (kind, text) = match row.as_bytes().first() {
                    None => (RowKind::Context, ""),
                    Some(b' ') => (RowKind::Context, &row[1..]),
                    Some(b'+') => (RowKind::Added, &row[1..]),
                    Some(b'-') => (RowKind::Removed, &row[1..]),
                    _ => bail!("bad hunk row: {row}"),
                };
                let (old_line, new_line) = match kind {
                    RowKind::Context => (Some(old), Some(new)),
                    RowKind::Added => (None, Some(new)),
                    RowKind::Removed => (Some(old), None),
                };
                if kind != RowKind::Added {
                    old += 1;
                    old_left = old_left
                        .checked_sub(1)
                        .context("hunk has too many old rows")?;
                }
                if kind != RowKind::Removed {
                    new += 1;
                    new_left = new_left
                        .checked_sub(1)
                        .context("hunk has too many new rows")?;
                }
                hunk.rows.push(Row {
                    kind,
                    old_line,
                    new_line,
                    text: text.to_string(),
                });
            }
            while lines.peek().is_some_and(|l| l.starts_with('\\')) {
                lines.next();
            }
            file.hunks.push(hunk);
        } else if file.hunks.is_empty() {
            file.extended.push(line.to_string());
        }
    }
    Ok(files)
}

/// Parses `-a,b +c,d @@ section`. A missing count means 1.
fn parse_hunk_header(s: &str) -> Option<Hunk> {
    let (ranges, section) = s.split_once(" @@").unwrap_or((s, ""));
    let (old, new) = ranges.split_once(' ')?;
    let range = |r: &str| -> Option<(u32, u32)> {
        match r.split_once(',') {
            Some((start, len)) => Some((start.parse().ok()?, len.parse().ok()?)),
            None => Some((r.parse().ok()?, 1)),
        }
    };
    let (old_start, old_lines) = range(old.strip_prefix('-')?)?;
    let (new_start, new_lines) = range(new.strip_prefix('+')?)?;
    Some(Hunk {
        old_start,
        old_lines,
        new_start,
        new_lines,
        section: section.trim_start().to_string(),
        rows: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rename_mode_change_and_hunk_line_numbers() {
        let out = "\
diff --git a/README b/README.md
rename from README
rename to README.md
index ce01362503..2227cddb7f 100644
--- a/README
+++ b/README.md
@@ -1,1 +1,2 @@
 hello
+more
diff --git a/main.rs b/main.rs
old mode 100644
new mode 100755
index 1b7c1e7d73..7527576062
--- a/main.rs
+++ b/main.rs
@@ -5,3 +5,3 @@ fn main() {
 fn main() {
-    println!(\"hi\");
+    println!(\"hello\");
 }
";
        let files = parse_git(out).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].header, "a/README b/README.md");
        let hunk = &files[1].hunks[0];
        assert_eq!(hunk.section, "fn main() {");
        let lines: Vec<_> = hunk
            .rows
            .iter()
            .map(|r| (r.kind, r.old_line, r.new_line))
            .collect();
        assert_eq!(
            lines,
            [
                (RowKind::Context, Some(5), Some(5)),
                (RowKind::Removed, Some(6), None),
                (RowKind::Added, None, Some(6)),
                (RowKind::Context, Some(7), Some(7)),
            ]
        );
        assert_eq!(hunk.rows[1].text, "    println!(\"hi\");");
    }

    #[test]
    fn only_removed_rows_of_changes_that_also_add_are_replaced() {
        let out = "\
diff --git a/a b/a
index 1b7c1e7d73..7527576062 100644
--- a/a
+++ b/a
@@ -1,6 +1,5 @@
-one
+uno
+eins
 two
-three
 four
-five
-six
+seis
";
        let hunk = &parse_git(out).unwrap()[0].hunks[0];
        let replaced: Vec<_> = hunk
            .rows
            .iter()
            .zip(hunk.replaced())
            .filter(|(_, r)| *r)
            .map(|(row, _)| row.text.as_str())
            .collect();
        assert_eq!(replaced, ["one", "five", "six"]);
    }

    #[test]
    fn parses_binary_empty_context_rows_and_no_newline_markers() {
        let out = "\
diff --git a/img.png b/img.png
index 1..2 100644
Binary files a/img.png and b/img.png differ
diff --git a/a.txt b/a.txt
--- a/a.txt
+++ b/a.txt
@@ -1,3 +1,3 @@
 one

-old
\\ No newline at end of file
+new
\\ No newline at end of file
";
        let files = parse_git(out).unwrap();
        assert!(files[0].binary && files[0].hunks.is_empty());
        let rows = &files[1].hunks[0].rows;
        assert_eq!(rows.len(), 4);
        assert_eq!(
            (rows[1].kind, rows[1].text.as_str()),
            (RowKind::Context, "")
        );
        assert_eq!(rows[3].new_line, Some(3));
    }

    #[test]
    fn hunk_header_without_counts_means_one_line() {
        let h = parse_hunk_header("-3 +0,0 @@").unwrap();
        assert_eq!(
            (h.old_start, h.old_lines, h.new_start, h.new_lines),
            (3, 1, 0, 0)
        );
    }

    #[test]
    fn meta_template_output_parses() {
        let out = r#"{"path":"b.rs","status":"renamed","source":"a.rs","sourceType":"file","targetType":"file","sourceExec":false,"targetExec":true,"conflict":false}"#;
        let meta = parse_meta(out).unwrap();
        assert_eq!(meta[0].status, FileStatus::Renamed);
        assert_eq!(meta[0].source, "a.rs");
    }
}
