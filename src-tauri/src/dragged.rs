//! The files of the drag that last dropped on the app. WebKit hands a page dropped files'
//! contents but not their paths, so they come from macOS's drag pasteboard.

/// Paths of the files on the drag pasteboard, as Finder or the app's own drag-out left them.
#[cfg(target_os = "macos")]
pub fn paths() -> Vec<String> {
    use std::ffi::{CStr, c_char};

    use objc2::rc::autoreleasepool;
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};

    unsafe fn ns_string(s: &CStr) -> *mut AnyObject {
        unsafe { msg_send![class!(NSString), stringWithUTF8String: s.as_ptr()] }
    }
    unsafe fn rust_string(s: *mut AnyObject) -> Option<String> {
        if s.is_null() {
            return None;
        }
        let chars: *const c_char = unsafe { msg_send![s, UTF8String] };
        (!chars.is_null()).then(|| {
            unsafe { CStr::from_ptr(chars) }
                .to_string_lossy()
                .into_owned()
        })
    }

    autoreleasepool(|_| unsafe {
        // NSPasteboardNameDrag.
        let board: *mut AnyObject = msg_send![class!(NSPasteboard), pasteboardWithName: ns_string(c"Apple CFPasteboard drag")];
        if board.is_null() {
            return Vec::new();
        }
        let items: *mut AnyObject = msg_send![board, pasteboardItems];
        if items.is_null() {
            return Vec::new();
        }
        let file_url = ns_string(c"public.file-url");
        let count: usize = msg_send![items, count];
        let mut out = Vec::new();
        for i in 0..count {
            let item: *mut AnyObject = msg_send![items, objectAtIndex: i];
            let text: *mut AnyObject = msg_send![item, stringForType: file_url];
            if text.is_null() {
                continue;
            }
            // Finder's are file reference URLs (file:///.file/id=…), so resolve them to paths.
            let url: *mut AnyObject = msg_send![class!(NSURL), URLWithString: text];
            if url.is_null() {
                continue;
            }
            let path_url: *mut AnyObject = msg_send![url, filePathURL];
            if path_url.is_null() {
                continue;
            }
            let path: *mut AnyObject = msg_send![path_url, path];
            out.extend(rust_string(path));
        }
        out
    })
}

#[cfg(not(target_os = "macos"))]
pub fn paths() -> Vec<String> {
    Vec::new()
}
