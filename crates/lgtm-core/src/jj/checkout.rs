//! What a workspace directory's `.jj` says about it: its name and its repo's store.

use std::path::{Path, PathBuf};

/// The name of the workspace whose root is `root`, from its checkout file.
pub fn name_at(root: &Path) -> Option<String> {
    checkout_name(&std::fs::read(root.join(".jj/working_copy/checkout")).ok()?)
}

/// The repo store directory a workspace at `root` uses: `.jj/repo` itself in the main
/// workspace, or where the `.jj/repo` file points in the others.
pub fn repo_dir(root: &Path) -> Option<PathBuf> {
    let link = root.join(".jj/repo");
    if link.is_dir() {
        return link.canonicalize().ok();
    }
    let target = std::fs::read_to_string(&link).ok()?;
    root.join(".jj").join(target.trim()).canonicalize().ok()
}

/// The workspace name in a `.jj/working_copy/checkout` file: field 3 of jj's `Checkout`
/// protobuf.
fn checkout_name(bytes: &[u8]) -> Option<String> {
    let mut at = 0;
    while at < bytes.len() {
        let key = varint(bytes, &mut at)?;
        let (field, wire) = (key >> 3, key & 7);
        match wire {
            0 => {
                varint(bytes, &mut at)?;
            }
            1 => at += 8,
            2 => {
                let len = usize::try_from(varint(bytes, &mut at)?).ok()?;
                let value = bytes.get(at..at.checked_add(len)?)?;
                if field == 3 {
                    return String::from_utf8(value.to_vec()).ok();
                }
                at += len;
            }
            5 => at += 4,
            _ => return None,
        }
    }
    None
}

fn varint(bytes: &[u8], at: &mut usize) -> Option<u64> {
    let mut out = 0u64;
    for shift in (0..64).step_by(7) {
        let b = *bytes.get(*at)?;
        *at += 1;
        out |= u64::from(b & 0x7f) << shift;
        if b & 0x80 == 0 {
            return Some(out);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_name_past_other_fields() {
        // Field 2 (a 3-byte operation id), a varint field 5, then field 3.
        let mut bytes = vec![0x12, 3, 1, 2, 3, 0x28, 0x96, 0x01, 0x1a, 4];
        bytes.extend(b"auth");
        assert_eq!(checkout_name(&bytes).as_deref(), Some("auth"));
        assert_eq!(checkout_name(&[0x12, 9, 1]), None);
    }
}
