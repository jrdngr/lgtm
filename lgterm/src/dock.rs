//! The Dock badge and alert sound.

/// Shows `count` on the app's Dock icon, or nothing for zero.
#[cfg(target_os = "macos")]
pub fn set_badge(count: usize) {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;
    use objc2_foundation::NSString;

    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let label = (count > 0).then(|| NSString::from_str(&count.to_string()));
    NSApplication::sharedApplication(mtm)
        .dockTile()
        .setBadgeLabel(label.as_deref());
}

#[cfg(not(target_os = "macos"))]
pub fn set_badge(_: usize) {}

#[cfg(target_os = "macos")]
pub fn play_sound() {
    let child = std::process::Command::new("afplay")
        .arg("/System/Library/Sounds/Glass.aiff")
        .stdin(std::process::Stdio::null())
        .spawn();
    if let Ok(mut child) = child {
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
}

#[cfg(not(target_os = "macos"))]
pub fn play_sound() {}
