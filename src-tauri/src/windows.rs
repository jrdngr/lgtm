//! The app's windows: a repo's, and the start window shown when none is open.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub const START_LABEL: &str = "start";

/// Builds a window showing the app at `label`.
pub fn window(app: &AppHandle, label: &str, title: &str) -> Result<(), String> {
    let builder = WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
        .title(title)
        // The app is dark only, so its title bar is too, whatever macOS's appearance.
        .theme(Some(tauri::Theme::Dark))
        .inner_size(1400.0, 900.0)
        // Tauri's file-drop handler swallows HTML5 drags, which dockview uses to move panes.
        .disable_drag_drop_handler()
        .on_navigation(stays_in_app);
    // No title bar: the panels' top row is it, with the traffic lights over its left end.
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        .traffic_light_position(tauri::LogicalPosition::new(14.0, 10.0));
    crate::zoom::on_load(crate::placement::place(app, label, builder))
        .build()
        .map_err(|e| e.to_string())?;
    close_start(app);
    Ok(())
}

pub fn close_start(app: &AppHandle) {
    if let Some(start) = app.get_webview_window(START_LABEL) {
        start.close().ok();
    }
}

/// Whether the webview or a frame in it may go to `url`: only the app's own pages and HTML
/// tabs' files, so a dropped file or a link never replaces the app and an HTML tab's page
/// never leaves for the web.
fn stays_in_app(url: &tauri::Url) -> bool {
    match url.scheme() {
        "tauri" | crate::html::SCHEME => true,
        "http" | "https" => matches!(
            url.host_str(),
            Some("localhost" | "tauri.localhost" | "127.0.0.1")
        ),
        _ => false,
    }
}

pub fn open_start(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(START_LABEL) {
        return window.set_focus();
    }
    let builder = WebviewWindowBuilder::new(app, START_LABEL, WebviewUrl::App("index.html".into()))
        .title("lgtm")
        .theme(Some(tauri::Theme::Dark))
        .inner_size(480.0, 420.0)
        .on_navigation(stays_in_app);
    crate::zoom::on_load(crate::placement::place(app, START_LABEL, builder))
        .build()
        .map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_webview_stays_on_the_apps_own_pages() {
        let ok = |u: &str| stays_in_app(&tauri::Url::parse(u).unwrap());
        assert!(ok("tauri://localhost/index.html"));
        assert!(ok("http://localhost:1420/"));
        assert!(ok("lgtmhtml://localhost/r0/docs/map.html"));
        assert!(!ok("file:///Users/j/diagram.png"));
        assert!(!ok("https://github.com/o/r"));
    }
}
