//! What a terminal is running and where: the foreground process's name, command and working
//! folder.

use std::path::{Path, PathBuf};

/// The folder a space is named after: the jj workspace or git working tree holding `cwd`,
/// else `cwd` itself, shown as `~` at home.
pub fn space_name(cwd: &Path, home: Option<&Path>) -> String {
    if home == Some(cwd) {
        return "~".into();
    }
    let root = repo_root(cwd).unwrap_or(cwd);
    match root.file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => root.to_string_lossy().into_owned(),
    }
}

fn repo_root(cwd: &Path) -> Option<&Path> {
    cwd.ancestors()
        .find(|dir| dir.join(".jj").is_dir() || dir.join(".git").exists())
}

/// The name of process `pid`, without its folder.
#[cfg(target_os = "macos")]
pub fn name(pid: i32) -> Option<String> {
    let mut buf = [0u8; 256];
    let len = unsafe { libc::proc_name(pid, buf.as_mut_ptr().cast(), buf.len() as u32) };
    (len > 0).then(|| String::from_utf8_lossy(&buf[..len as usize]).into_owned())
}

/// The working folder of process `pid`.
#[cfg(target_os = "macos")]
pub fn cwd(pid: i32) -> Option<PathBuf> {
    use std::ffi::CStr;
    let mut info: libc::proc_vnodepathinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_vnodepathinfo>() as i32;
    let got = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDVNODEPATHINFO,
            0,
            (&raw mut info).cast(),
            size,
        )
    };
    if got != size {
        return None;
    }
    let path = unsafe { CStr::from_ptr(info.pvi_cdir.vip_path.as_ptr().cast()) };
    Some(PathBuf::from(path.to_str().ok()?))
}

/// The executable path and arguments of process `pid`.
#[cfg(target_os = "macos")]
pub fn command(pid: i32) -> Option<(String, Vec<String>)> {
    let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid];
    let mut size: libc::size_t = 0;
    let sysctl = |buf: *mut libc::c_void, size: &mut libc::size_t, mib: &mut [i32; 3]| unsafe {
        libc::sysctl(mib.as_mut_ptr(), 3, buf, size, std::ptr::null_mut(), 0)
    };
    if sysctl(std::ptr::null_mut(), &mut size, &mut mib) != 0 || size == 0 {
        return None;
    }
    let mut buf = vec![0u8; size];
    if sysctl(buf.as_mut_ptr().cast(), &mut size, &mut mib) != 0 {
        return None;
    }
    buf.truncate(size);
    parse_procargs(&buf)
}

/// The executable path and arguments in a `KERN_PROCARGS2` buffer: `argc`, the path, padding,
/// then the arguments and environment, each ending in a nul.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn parse_procargs(buf: &[u8]) -> Option<(String, Vec<String>)> {
    let argc = i32::from_ne_bytes(buf.get(..4)?.try_into().ok()?);
    let mut parts = buf[4..].split(|b| *b == 0);
    let exe = String::from_utf8_lossy(parts.next()?).into_owned();
    let argv = parts
        .filter(|p| !p.is_empty())
        .take(argc.max(0) as usize)
        .map(|p| String::from_utf8_lossy(p).into_owned())
        .collect();
    Some((exe, argv))
}

#[cfg(not(target_os = "macos"))]
pub fn command(_pid: i32) -> Option<(String, Vec<String>)> {
    None
}

#[cfg(not(target_os = "macos"))]
pub fn name(_pid: i32) -> Option<String> {
    None
}

#[cfg(not(target_os = "macos"))]
pub fn cwd(_pid: i32) -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn procargs_give_the_path_and_only_the_arguments() {
        let mut buf = 2i32.to_ne_bytes().to_vec();
        buf.extend_from_slice(b"/usr/local/bin/node\0\0\0\0node\0/x/bin/claude\0HOME=/u\0");
        let (exe, argv) = parse_procargs(&buf).unwrap();
        assert_eq!(exe, "/usr/local/bin/node");
        assert_eq!(argv, ["node", "/x/bin/claude"]);
    }

    #[test]
    fn spaces_are_named_after_the_jj_workspace_or_git_tree_holding_the_folder() {
        let tmp = std::env::temp_dir().join(format!("lgterm-name-{}", std::process::id()));
        let deep = tmp.join("repo/src/ui");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::create_dir_all(tmp.join("repo/.jj")).unwrap();
        assert_eq!(space_name(&deep, None), "repo");
        let tree = tmp.join("tree/lib");
        std::fs::create_dir_all(&tree).unwrap();
        std::fs::write(tmp.join("tree/.git"), "gitdir: elsewhere\n").unwrap();
        assert_eq!(space_name(&tree, None), "tree");
        assert_eq!(
            space_name(&tmp, None),
            tmp.file_name().unwrap().to_str().unwrap()
        );
        assert_eq!(space_name(&tmp, Some(&tmp)), "~");
        std::fs::remove_dir_all(&tmp).unwrap();
    }
}
