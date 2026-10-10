//! File metadata from a `diff --git` section's extended header, for commands like
//! `jj interdiff` that don't take a `-T` template.

use crate::diff::{FileMeta, FileStatus, GitFileDiff};

/// Undoes git's C-quoting of a path, if it's quoted.
fn unquote(s: &str) -> String {
    let Some(inner) = s.strip_prefix('"').and_then(|s| s.strip_suffix('"')) else {
        return s.to_string();
    };
    let mut out = Vec::new();
    let mut bytes = inner.bytes();
    while let Some(b) = bytes.next() {
        if b != b'\\' {
            out.push(b);
            continue;
        }
        match bytes.next() {
            Some(b't') => out.push(b'\t'),
            Some(b'n') => out.push(b'\n'),
            Some(d @ b'0'..=b'7') => {
                let digits = [
                    d,
                    bytes.next().unwrap_or(b'0'),
                    bytes.next().unwrap_or(b'0'),
                ];
                let n = digits.iter().fold(0u32, |n, d| n * 8 + (d - b'0') as u32);
                out.push(n as u8);
            }
            Some(other) => out.push(other),
            None => {}
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A `---`/`+++` path without its `a/`/`b/` prefix, or `None` for `/dev/null`.
fn side_path(s: &str) -> Option<String> {
    let path = unquote(s.trim_end());
    if path == "/dev/null" {
        return None;
    }
    Some(path.get(2..).unwrap_or_default().to_string())
}

pub fn meta_from_git(g: &GitFileDiff) -> FileMeta {
    let mut status = FileStatus::Modified;
    let (mut source, mut target) = (None, None);
    let (mut source_exec, mut target_exec) = (false, false);
    let exec = |mode: &str| mode.trim() == "100755";
    for line in &g.extended {
        if let Some(mode) = line.strip_prefix("new file mode ") {
            status = FileStatus::Added;
            target_exec = exec(mode);
        } else if let Some(mode) = line.strip_prefix("deleted file mode ") {
            status = FileStatus::Removed;
            source_exec = exec(mode);
        } else if let Some(mode) = line.strip_prefix("old mode ") {
            source_exec = exec(mode);
        } else if let Some(mode) = line.strip_prefix("new mode ") {
            target_exec = exec(mode);
        } else if let Some(modes) = line.strip_prefix("index ") {
            if let Some((_, mode)) = modes.split_once(' ') {
                (source_exec, target_exec) = (exec(mode), exec(mode));
            }
        } else if let Some(p) = line.strip_prefix("rename from ") {
            status = FileStatus::Renamed;
            source = Some(unquote(p));
        } else if let Some(p) = line.strip_prefix("rename to ") {
            target = Some(unquote(p));
        } else if let Some(p) = line.strip_prefix("copy from ") {
            status = FileStatus::Copied;
            source = Some(unquote(p));
        } else if let Some(p) = line.strip_prefix("copy to ") {
            target = Some(unquote(p));
        } else if let Some(p) = line.strip_prefix("--- ") {
            source = source.or_else(|| side_path(p));
        } else if let Some(p) = line.strip_prefix("+++ ") {
            target = target.or_else(|| side_path(p));
        }
    }
    // A mode-only change has no `---`/`+++` lines: split the header itself.
    let fallback = || {
        let h = &g.header;
        h.find(" b/")
            .map_or_else(|| h.clone(), |i| h[i + 3..].to_string())
    };
    let path = target.or_else(|| source.clone()).unwrap_or_else(fallback);
    FileMeta {
        source: source.unwrap_or_else(|| path.clone()),
        path,
        status,
        source_type: "file".into(),
        target_type: "file".into(),
        source_exec,
        target_exec,
        conflict: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::parse_git;

    #[test]
    fn reads_status_paths_and_modes_from_extended_headers() {
        let out = "\
diff --git a/old.rs b/new.rs
rename from old.rs
rename to new.rs
diff --git a/gone.txt b/gone.txt
deleted file mode 100644
index 1..0
--- a/gone.txt
+++ /dev/null
@@ -1,1 +0,0 @@
-bye
diff --git a/run.sh b/run.sh
old mode 100644
new mode 100755
diff --git \"a/\\303\\251.txt\" \"b/\\303\\251.txt\"
new file mode 100644
index 0..1
--- /dev/null
+++ \"b/\\303\\251.txt\"
@@ -0,0 +1,1 @@
+hi
";
        let metas: Vec<_> = parse_git(out).unwrap().iter().map(meta_from_git).collect();
        let summary: Vec<_> = metas
            .iter()
            .map(|m| (m.status, m.source.as_str(), m.path.as_str(), m.target_exec))
            .collect();
        assert_eq!(
            summary,
            [
                (FileStatus::Renamed, "old.rs", "new.rs", false),
                (FileStatus::Removed, "gone.txt", "gone.txt", false),
                (FileStatus::Modified, "run.sh", "run.sh", true),
                (FileStatus::Added, "é.txt", "é.txt", false),
            ]
        );
    }
}
