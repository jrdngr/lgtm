//! Reading files out of commits: whole files, many at once, and their content hashes.

use std::collections::HashMap;

use super::Jj;

impl Jj {
    /// A content hash of each of `paths` at `commit`, a commit id, in at most one read. Paths
    /// not there are left out.
    pub async fn file_hashes(
        &self,
        commit: &str,
        paths: &[String],
    ) -> anyhow::Result<HashMap<String, String>> {
        let key = |p: &str| (commit.to_string(), p.to_string());
        let mut found = HashMap::new();
        let mut unread = Vec::new();
        {
            let hashes = HASHES.lock().unwrap();
            for p in paths {
                match hashes.get(&key(p)) {
                    Some(Some(h)) => {
                        found.insert(p.clone(), h.clone());
                    }
                    Some(None) => {}
                    None => unread.push(p.clone()),
                }
            }
        }
        if unread.is_empty() {
            return Ok(found);
        }
        let mut read = self.files_at(commit, &unread).await?;
        let mut hashes = HASHES.lock().unwrap();
        if hashes.len() >= MAX_HASHES {
            hashes.clear();
        }
        for p in unread {
            let hash = read
                .remove(&p)
                .map(|bytes| blake3::hash(&bytes).to_hex().to_string());
            if let Some(h) = &hash {
                found.insert(p.clone(), h.clone());
            }
            hashes.insert(key(&p), hash);
        }
        Ok(found)
    }

    /// The contents of each of `paths` at `commit`, a few hundred paths per read. Paths not
    /// there are left out.
    pub async fn files_at(
        &self,
        commit: &str,
        paths: &[String],
    ) -> anyhow::Result<HashMap<String, Vec<u8>>> {
        let mut found = HashMap::new();
        for chunk in paths.chunks(FILES_PER_READ) {
            // A union with none() makes a missing path print nothing rather than fail.
            let fileset = chunk
                .iter()
                .map(|p| fileset_literal(p))
                .chain(["none()".to_string()])
                .collect::<Vec<_>>()
                .join(" | ");
            // Each file starts with a marker and its path, so one output splits back into files.
            let out = self
                .exec(
                    &[
                        "file",
                        "show",
                        "-r",
                        commit,
                        "-T",
                        FILE_MARK_TEMPLATE,
                        &fileset,
                    ],
                    true,
                )
                .await?;
            found.extend(
                split_files(&out)
                    .into_iter()
                    .map(|(path, bytes)| (path, bytes.to_vec())),
            );
        }
        Ok(found)
    }

    /// File contents at `commit`, or empty for the root commit.
    pub async fn file(&self, commit: Option<&str>, path: &str) -> anyhow::Result<Vec<u8>> {
        let Some(commit) = commit else {
            return Ok(Vec::new());
        };
        self.exec(
            &["file", "show", "-r", commit, &fileset_literal(path)],
            true,
        )
        .await
    }

    /// File contents at `commit`, or `None` when the file isn't there or `commit` is the root.
    pub async fn file_if_any(
        &self,
        commit: Option<&str>,
        path: &str,
    ) -> anyhow::Result<Option<Vec<u8>>> {
        let Some(commit) = commit else {
            return Ok(None);
        };
        // A union makes a missing path print nothing rather than fail, and the template marks
        // the start of the file when it's there, so an empty file still shows.
        let fileset = format!("{} | none()", fileset_literal(path));
        let out = self
            .exec(
                &["file", "show", "-r", commit, "-T", r#""+""#, &fileset],
                true,
            )
            .await?;
        Ok(out.strip_prefix(b"+").map(<[u8]>::to_vec))
    }
}

const FILE_MARK: &[u8] = b"\0lgtm-file\0";
/// File hashes by commit and path, `None` where the file isn't there.
type Hashes = HashMap<(String, String), Option<String>>;
/// The hashes looked up so far. Commits never change.
static HASHES: std::sync::LazyLock<std::sync::Mutex<Hashes>> =
    std::sync::LazyLock::new(Default::default);

/// Past this many files the hash cache starts over.
const MAX_HASHES: usize = 10_000;

/// Keeps a batched `jj file show`'s arguments well under the OS limit.
const FILES_PER_READ: usize = 300;

const FILE_MARK_TEMPLATE: &str = r#""\0lgtm-file\0" ++ path ++ "\0""#;

/// `jj file show` output under `FILE_MARK_TEMPLATE`, as each file's path and bytes.
fn split_files(out: &[u8]) -> Vec<(String, &[u8])> {
    let mut starts = Vec::new();
    let mut i = 0;
    while let Some(at) = out[i..]
        .windows(FILE_MARK.len())
        .position(|w| w == FILE_MARK)
    {
        starts.push(i + at);
        i += at + FILE_MARK.len();
    }
    starts
        .iter()
        .enumerate()
        .filter_map(|(n, &start)| {
            let body =
                &out[start + FILE_MARK.len()..starts.get(n + 1).copied().unwrap_or(out.len())];
            let end = body.iter().position(|&b| b == 0)?;
            Some((
                String::from_utf8_lossy(&body[..end]).into_owned(),
                &body[end + 1..],
            ))
        })
        .collect()
}

/// A fileset matching exactly `path`, relative to the repo root.
fn fileset_literal(path: &str) -> String {
    format!(
        "root-file:\"{}\"",
        path.replace('\\', "\\\\").replace('"', "\\\"")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jj::test_support;

    #[test]
    fn splits_marked_file_output_into_paths_and_bytes() {
        let out = b"\0lgtm-file\0a.md\0one\n\0lgtm-file\0d/b.md\0\0lgtm-file\0c.md\0x";
        let got: Vec<(String, &[u8])> = split_files(out);
        assert_eq!(
            got,
            [
                ("a.md".to_string(), &b"one\n"[..]),
                ("d/b.md".to_string(), &b""[..]),
                ("c.md".to_string(), &b"x"[..]),
            ]
        );
    }

    #[tokio::test]
    async fn file_if_any_reads_a_file_or_finds_none() {
        let (dir, j) = test_support::repo();
        test_support::write(dir.path(), "a.txt", "a\n");
        test_support::write(dir.path(), "empty.txt", "");
        j.snapshot().await.unwrap();
        let at = j.commit_ids("@").await.unwrap();
        let at = Some(at[0].as_str());
        assert_eq!(
            j.file_if_any(at, "a.txt").await.unwrap(),
            Some(b"a\n".to_vec())
        );
        assert_eq!(j.file_if_any(at, "empty.txt").await.unwrap(), Some(vec![]));
        assert_eq!(j.file_if_any(at, "nope.txt").await.unwrap(), None);
        assert_eq!(j.file_if_any(None, "a.txt").await.unwrap(), None);
        assert!(j.file_if_any(Some("no_such_rev"), "a.txt").await.is_err());
    }

    #[tokio::test]
    async fn files_at_reads_paths_across_batches_and_skips_missing_ones() {
        let (dir, j) = test_support::repo();
        let paths: Vec<String> = (0..FILES_PER_READ + 5)
            .map(|i| format!("f{i}.txt"))
            .collect();
        for (i, p) in paths.iter().enumerate() {
            test_support::write(dir.path(), p, &format!("{i}\n"));
        }
        test_support::write(dir.path(), "empty.txt", "");
        j.snapshot().await.unwrap();
        let at = j.commit_ids("@").await.unwrap().remove(0);
        let asked: Vec<String> = paths
            .iter()
            .cloned()
            .chain(["empty.txt".to_string(), "nope.txt".to_string()])
            .collect();
        let got = j.files_at(&at, &asked).await.unwrap();
        assert_eq!(got.len(), paths.len() + 1);
        assert_eq!(
            got[&paths[FILES_PER_READ + 4]],
            format!("{}\n", FILES_PER_READ + 4).as_bytes()
        );
        assert_eq!(got["empty.txt"], b"");
        assert!(!got.contains_key("nope.txt"));
    }

    #[test]
    fn fileset_literal_escapes_quotes() {
        assert_eq!(fileset_literal(r#"a "b"\c"#), r#"root-file:"a \"b\"\\c""#);
    }
}
