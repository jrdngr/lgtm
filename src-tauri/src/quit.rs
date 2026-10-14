//! Quitting asks each repo window about its unsaved edits in turn, and stops at the first
//! Cancel. Once every window agrees, they all close and the app ends.

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

use objc2::runtime::{AnyClass, AnyObject, Imp, Sel};
use objc2::{class, ffi, msg_send, sel};
use tauri::{AppHandle, Manager};

use crate::agent::Runs;
use crate::repo::{RepoWindows, START_LABEL};

static APP: OnceLock<AppHandle> = OnceLock::new();
static QUIT: Mutex<Option<Quit>> = Mutex::new(None);
const NS_TERMINATE_NOW: usize = 1;
const NS_TERMINATE_LATER: usize = 2;

enum Quit {
    /// Repo windows still to ask. The first is asking now.
    Asking(VecDeque<String>),
    /// Every window agreed. The app ends once stopped agent runs have cleaned up.
    Ending,
}

fn asking(quit: &Option<Quit>, label: &str) -> bool {
    matches!(quit, Some(Quit::Asking(queue)) if queue.front().is_some_and(|l| l == label))
}

/// Whether a quit is under way, which ends the app itself.
pub fn quitting() -> bool {
    QUIT.lock().unwrap().is_some()
}

/// The window confirmed its close. When it's the one asking, it stays open until every
/// window has, and this returns true. Any other window closes as usual.
pub fn confirmed(label: &str) -> bool {
    let mut quit = QUIT.lock().unwrap();
    let Some(Quit::Asking(queue)) = quit.as_mut() else {
        return false;
    };
    if queue.front().is_some_and(|l| l == label) {
        queue.pop_front();
        drop(quit);
        ask_next();
        return true;
    }
    queue.retain(|l| l != label);
    false
}

/// The window kept itself open. When it's the one asking, the quit stops.
pub fn cancelled(label: &str) {
    let mut quit = QUIT.lock().unwrap();
    if asking(&quit, label) {
        *quit = None;
        drop(quit);
        reply(false);
    }
}

/// The window went without asking, like a repo window that hadn't loaded yet.
pub fn closed(label: &str) {
    let mut quit = QUIT.lock().unwrap();
    let was_asking = asking(&quit, label);
    if let Some(Quit::Asking(queue)) = quit.as_mut() {
        queue.retain(|l| l != label);
    }
    drop(quit);
    if was_asking {
        ask_next();
    }
}

/// Sends the close request that makes the next window ask, or ends the app when none is left.
/// Closing can destroy a window at once, which comes back through `closed`, so no lock is held.
fn ask_next() {
    let Some(app) = APP.get() else { return };
    loop {
        let next = match QUIT.lock().unwrap().as_mut() {
            Some(Quit::Asking(queue)) => queue.front().cloned(),
            _ => return,
        };
        let Some(label) = next else { break };
        if let Some(window) = app.get_webview_window(&label) {
            window.close().ok();
            return;
        }
        if let Some(Quit::Asking(queue)) = QUIT.lock().unwrap().as_mut() {
            queue.retain(|l| *l != label);
        }
    }
    *QUIT.lock().unwrap() = Some(Quit::Ending);
    let (windows, runs) = (app.state::<RepoWindows>(), app.state::<Runs>());
    for (label, window) in app.webview_windows() {
        // Destroying finishes later, so the runs stop here for `settle` to wait on.
        windows.remove(&label, &runs);
        window.destroy().ok();
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        app.state::<RepoWindows>().settle().await;
        reply(true);
    });
}

/// Answers the `NSTerminateLater` that `should_terminate` gave.
fn reply(terminate: bool) {
    let Some(app) = APP.get() else { return };
    app.run_on_main_thread(move || {
        // SAFETY: on the main thread, where NSApp exists after launch.
        unsafe {
            let ns_app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
            let _: () = msg_send![ns_app, replyToApplicationShouldTerminate: terminate];
        }
    })
    .ok();
}

/// Routes every macOS quit (⌘Q, the Dock, logging out) through the windows. Quit sends
/// `applicationShouldTerminate:` to the app delegate, which Tao doesn't implement, so the
/// app would end at once. This adds it to Tao's delegate, deferring the answer until each
/// window has asked about unsaved edits. Tao itself exits by stopping its run loop, which
/// doesn't come here.
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
        ffi::class_addMethod(
            cls,
            sel!(applicationShouldTerminate:),
            imp,
            c"Q@:@".as_ptr(),
        );
    }
}

extern "C-unwind" fn should_terminate(_: &AnyObject, _: Sel, _: *mut AnyObject) -> usize {
    let Some(app) = APP.get() else {
        return NS_TERMINATE_NOW;
    };
    let mut quit = QUIT.lock().unwrap();
    if quit.is_some() {
        return NS_TERMINATE_LATER;
    }
    let repos = app
        .webview_windows()
        .into_keys()
        .filter(|l| l != START_LABEL);
    *quit = Some(Quit::Asking(repos.collect()));
    drop(quit);
    // Called from the main thread, `run_on_main_thread` runs the task at once. Sent from
    // another thread, it waits until this has answered, so windows don't close inside
    // AppKit's call.
    let app = app.clone();
    tauri::async_runtime::spawn(async move { app.run_on_main_thread(ask_next).ok() });
    NS_TERMINATE_LATER
}
