//! Quitting closes windows one by one, so each can ask about unsaved edits. The app ends when
//! the last one goes.

use std::sync::OnceLock;

use objc2::runtime::{AnyClass, AnyObject, Imp, Sel};
use objc2::{class, ffi, msg_send, sel};
use tauri::{AppHandle, Manager};

static APP: OnceLock<AppHandle> = OnceLock::new();
const NS_TERMINATE_CANCEL: usize = 0;

fn close_all(app: &AppHandle) {
    for window in app.webview_windows().into_values() {
        window.close().ok();
    }
}

/// Routes every macOS quit (⌘Q, the Dock, logging out) through `close_all`. Quit sends
/// `applicationShouldTerminate:` to the app delegate, which Tao doesn't implement, so the
/// app would end at once. This adds it to Tao's delegate, cancelling the quit and closing
/// the windows instead. Tao itself exits by stopping its run loop, which doesn't come here.
pub fn intercept(app: &AppHandle) {
    APP.set(app.clone()).ok();
    // SAFETY: runs on the main thread after launch, when NSApp and Tao's delegate exist. The
    // function matches the selector's type encoding (NSUInteger return, self, _cmd, sender).
    unsafe {
        let ns_app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        let delegate: *mut AnyObject = msg_send![ns_app, delegate];
        let Some(delegate) = delegate.as_ref() else {
            return eprintln!("lgtm: no app delegate, so quitting won't ask about unsaved edits");
        };
        let imp: Imp = std::mem::transmute(
            should_terminate as extern "C-unwind" fn(&AnyObject, Sel, *mut AnyObject) -> usize,
        );
        let cls = delegate.class() as *const AnyClass as *mut AnyClass;
        ffi::class_addMethod(cls, sel!(applicationShouldTerminate:), imp, c"Q@:@".as_ptr());
    }
}

extern "C-unwind" fn should_terminate(_: &AnyObject, _: Sel, _: *mut AnyObject) -> usize {
    if let Some(app) = APP.get() {
        let handle = app.clone();
        app.run_on_main_thread(move || close_all(&handle)).ok();
    }
    NS_TERMINATE_CANCEL
}
