//! Copying a file or folder within the working copy.

use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

/// `name` with " copy" before its extension, then " copy 2", " copy 3"…, as Finder names
/// copies. A folder's name has no extension.
fn copy_name(name: &str, dir: bool, n: u32) -> String {
    let (stem, ext) = match name.rfind('.') {
        Some(i) if !dir && i > 0 => name.split_at(i),
        _ => (name, ""),
    };
    let tag = if n == 1 {
        " copy".to_string()
    } else {
        format!(" copy {n}")
    };
    format!("{stem}{tag}{ext}")
}

/// Copies `from` into folder `dir` under its own name, or the first free copy name when
/// that's taken. Returns where it went.
pub fn copy_into(from: &Path, dir: &Path) -> anyhow::Result<PathBuf> {
    let name = from
        .file_name()
        .context("can't copy the workspace")?
        .to_string_lossy()
        .into_owned();
    if dir.starts_with(from) {
        bail!("can't copy {name} into itself");
    }
    let is_dir = from.is_dir();
    let mut to = dir.join(&name);
    let mut n = 1;
    while to.symlink_metadata().is_ok() {
        to = dir.join(copy_name(&name, is_dir, n));
        n += 1;
    }
    std::fs::create_dir_all(dir).with_context(|| format!("can't copy {name}"))?;
    copy_tree(from, &to).with_context(|| format!("can't copy {name}"))?;
    Ok(to)
}

/// Copies a file, a symlink as a symlink, or a folder and everything in it.
fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    let kind = from.symlink_metadata()?.file_type();
    if kind.is_symlink() {
        #[cfg(unix)]
        return std::os::unix::fs::symlink(std::fs::read_link(from)?, to);
    }
    if !kind.is_dir() {
        return std::fs::copy(from, to).map(drop);
    }
    std::fs::create_dir(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        copy_tree(&entry.path(), &to.join(entry.file_name()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_names_go_before_a_files_extension_and_count_up() {
        assert_eq!(copy_name("a.test.ts", false, 1), "a.test copy.ts");
        assert_eq!(copy_name("a.ts", false, 3), "a copy 3.ts");
        assert_eq!(copy_name(".env", false, 1), ".env copy");
        assert_eq!(copy_name("v1.2", true, 2), "v1.2 copy 2");
    }

    #[test]
    fn copies_a_folder_into_another_and_takes_a_free_name_beside_itself() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("src/inner")).unwrap();
        std::fs::create_dir_all(root.join("lib")).unwrap();
        std::fs::write(root.join("src/inner/a.rs"), "a").unwrap();

        let moved = copy_into(&root.join("src"), &root.join("lib")).unwrap();
        assert_eq!(moved, root.join("lib/src"));
        assert_eq!(
            std::fs::read(root.join("lib/src/inner/a.rs")).unwrap(),
            b"a"
        );
        assert!(root.join("src/inner/a.rs").exists());

        assert_eq!(
            copy_into(&root.join("src"), root).unwrap(),
            root.join("src copy")
        );
        assert_eq!(
            copy_into(&root.join("src"), root).unwrap(),
            root.join("src copy 2")
        );
        assert!(root.join("src copy 2/inner/a.rs").exists());
    }

    #[test]
    fn refuses_to_copy_a_folder_into_itself() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(src.join("inner")).unwrap();
        assert!(copy_into(&src, &src.join("inner")).is_err());
        assert!(!src.join("inner/src").exists());
    }
}
