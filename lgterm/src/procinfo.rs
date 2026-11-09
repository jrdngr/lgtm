//! What a terminal is running and where: the foreground process's name and working folder.

use std::path::{Path, PathBuf};

/// The folder a space is named after: the jj workspace holding `cwd`, else `cwd` itself, shown
/// as `~` at home.
pub fn space_name(cwd: &Path, home: Option<&Path>) -> String {
    if home == Some(cwd) {
        return "~".into();
    }
    let root = jj_root(cwd).unwrap_or(cwd);
    match root.file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => root.to_string_lossy().into_owned(),
    }
}

fn jj_root(cwd: &Path) -> Option<&Path> {
    cwd.ancestors().find(|dir| dir.join(".jj").is_dir())
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
    fn spaces_are_named_after_the_jj_workspace_holding_the_folder() {
        let tmp = std::env::temp_dir().join(format!("lgterm-name-{}", std::process::id()));
        let deep = tmp.join("repo/src/ui");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::create_dir_all(tmp.join("repo/.jj")).unwrap();
        assert_eq!(space_name(&deep, None), "repo");
        assert_eq!(
            space_name(&tmp, None),
            tmp.file_name().unwrap().to_str().unwrap()
        );
        assert_eq!(space_name(&tmp, Some(&tmp)), "~");
        std::fs::remove_dir_all(&tmp).unwrap();
    }
}
