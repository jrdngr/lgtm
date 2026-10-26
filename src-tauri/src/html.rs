//! The `lgtmhtml:` scheme: a workspace's files for HTML tabs, which show pages in a sandboxed
//! frame. A URL is `lgtmhtml://localhost/<root id>/<path>`. Each root id belongs to one
//! repo, served only to the window showing it, and records the files it served, so the tab
//! can ask whether any changed.

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use percent_encoding::percent_decode_str;
use tauri::http::{Request, Response, StatusCode, header};
use tauri::{Manager, Runtime, UriSchemeContext, UriSchemeResponder};

use crate::files;

pub const SCHEME: &str = "lgtmhtml";
const MAX_BYTES: u64 = 64 * 1024 * 1024;
const FRAME_SCRIPT: &str = include_str!("html_frame.js");
/// Pages run scripts and styles from anywhere, so CDN libraries work, and load everything
/// else from their own files. Fetches, XHR, websockets, beacons and forms can't leave the
/// scheme, but that's all: a script or style URL can still carry data out, and the frame can
/// navigate itself off it. The frame's origin is opaque, so `'self'` alone may match nothing.
const CSP: &str = "default-src 'self' lgtmhtml: http://lgtmhtml.localhost; \
    script-src 'self' lgtmhtml: http://lgtmhtml.localhost 'unsafe-inline' 'unsafe-eval' https:; \
    style-src 'self' lgtmhtml: http://lgtmhtml.localhost 'unsafe-inline' https:; \
    img-src 'self' lgtmhtml: http://lgtmhtml.localhost data: blob:; \
    font-src 'self' lgtmhtml: http://lgtmhtml.localhost data:; \
    media-src 'self' lgtmhtml: http://lgtmhtml.localhost data: blob:; \
    connect-src 'self' lgtmhtml: http://lgtmhtml.localhost; \
    frame-src 'self' lgtmhtml: http://lgtmhtml.localhost; \
    child-src 'self' lgtmhtml: http://lgtmhtml.localhost; \
    form-action 'none'";
/// Other files run no scripts, since only HTML gets the frame script, which must run first so
/// a page's scripts can't take the tab's key port. An SVG shown as a page could otherwise.
const FILE_CSP: &str = "default-src 'self' lgtmhtml: http://lgtmhtml.localhost data:; \
    style-src 'self' lgtmhtml: http://lgtmhtml.localhost 'unsafe-inline'; \
    script-src 'none'; form-action 'none'";

struct Root {
    dir: PathBuf,
    window: String,
    /// Each file served since the last change check, with its modified time then.
    served: HashMap<PathBuf, Option<SystemTime>>,
}

/// The roots HTML tabs serve from.
#[derive(Default)]
pub struct HtmlRoots {
    roots: Mutex<HashMap<String, Root>>,
}

impl HtmlRoots {
    /// A new root id serving `dir` to `window`. Random, so a page can't guess another's.
    pub fn add(&self, window: &str, dir: PathBuf) -> String {
        let id = random_id();
        let root = Root {
            dir,
            window: window.into(),
            served: HashMap::new(),
        };
        self.roots.lock().unwrap().insert(id.clone(), root);
        id
    }

    /// Forgets the window's roots.
    pub fn close_window(&self, window: &str) {
        self.roots.lock().unwrap().retain(|_, r| r.window != window);
    }

    /// Whether a file `id` served changed or went away since it was served. Starts over, so
    /// the reload that follows records afresh.
    pub fn changed(&self, window: &str, id: &str) -> bool {
        let mut roots = self.roots.lock().unwrap();
        let Some(root) = roots.get_mut(id).filter(|r| r.window == window) else {
            return false;
        };
        let changed = root.served.iter().any(|(path, at)| modified(path) != *at);
        if changed {
            root.served.clear();
        }
        changed
    }

    /// The file at a `lgtmhtml:` URL path, for a window showing the repos labelled `owners`,
    /// recorded as served.
    fn locate(&self, owners: &[String], url_path: &str) -> Result<PathBuf, StatusCode> {
        let decoded = percent_decode_str(url_path.trim_start_matches('/'))
            .decode_utf8()
            .map_err(|_| StatusCode::BAD_REQUEST)?;
        let (id, rel) = decoded.split_once('/').ok_or(StatusCode::NOT_FOUND)?;
        let mut roots = self.roots.lock().unwrap();
        let root = roots
            .get_mut(id)
            .filter(|r| owners.contains(&r.window))
            .ok_or(StatusCode::NOT_FOUND)?;
        let mut path = files::resolve(&root.dir, rel).map_err(|_| StatusCode::FORBIDDEN)?;
        if path.is_dir() {
            path.push("index.html");
        }
        files::contain(&root.dir, &path).map_err(|_| StatusCode::FORBIDDEN)?;
        root.served.insert(path.clone(), modified(&path));
        Ok(path)
    }
}

/// 128 random bits in hex.
fn random_id() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("OS randomness");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Answers a `lgtmhtml:` request off the main thread.
pub fn handle<R: Runtime>(
    ctx: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let app = ctx.app_handle().clone();
    let owners = app
        .state::<crate::repo::Repos>()
        .in_window(ctx.webview_label());
    let url_path = request.uri().path().to_string();
    tauri::async_runtime::spawn_blocking(move || {
        responder.respond(serve(app.state::<HtmlRoots>().inner(), &owners, &url_path));
    });
}

fn serve(roots: &HtmlRoots, owners: &[String], url_path: &str) -> Response<Cow<'static, [u8]>> {
    let found = roots.locate(owners, url_path).and_then(|path| {
        let bytes =
            files::read_bytes(&path, url_path, MAX_BYTES).map_err(|_| StatusCode::NOT_FOUND)?;
        Ok((content_type(&path), bytes))
    });
    let (kind, bytes) = match found {
        Ok(found) => found,
        Err(status) => {
            return Response::builder()
                .status(status)
                .header(header::CONTENT_SECURITY_POLICY, CSP)
                .body(Cow::Borrowed(&[][..]))
                .unwrap();
        }
    };
    let (body, csp) = if kind.starts_with("text/html") {
        let page = with_frame_script(&String::from_utf8_lossy(&bytes));
        (page.into_bytes(), CSP)
    } else {
        (bytes, FILE_CSP)
    };
    Response::builder()
        .header(header::CONTENT_TYPE, kind)
        .header(header::CONTENT_SECURITY_POLICY, csp)
        // The frame's origin is opaque, so module scripts and fetches of its own files need
        // this. Only the app's webviews can reach the scheme; the CSP stops sending out.
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::CACHE_CONTROL, "no-store")
        .body(Cow::Owned(body))
        .unwrap()
}

/// `html` with the frame script first in it: after the doctype and any `<html>` and `<head>`
/// tags, so the page keeps standards mode and its `<head>` attributes.
fn with_frame_script(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let mut at = 0;
    for tag in ["<!doctype", "<html", "<head"] {
        if let Some(start) = lower[at..].find(tag).map(|i| i + at)
            && lower[at..start].trim().is_empty()
            && lower[start + tag.len()..].starts_with(|c: char| c == '>' || c.is_whitespace())
            && let Some(end) = tag_end(&lower, start)
        {
            at = end;
        }
    }
    format!(
        "{}<script>{FRAME_SCRIPT}</script>{}",
        &html[..at],
        &html[at..]
    )
}

/// Just past the `>` closing the tag at `start`. A `>` in a quoted attribute value doesn't
/// close it; a quote opens a value only right after `=`, as the HTML parser reads it.
fn tag_end(html: &str, start: usize) -> Option<usize> {
    let mut quote = None;
    let mut last = b'<';
    for (i, &b) in html.as_bytes()[start..].iter().enumerate() {
        match quote {
            Some(q) if b == q => quote = None,
            Some(_) => continue,
            None if b == b'>' => return Some(start + i + 1),
            None if (b == b'"' || b == b'\'') && last == b'=' => {
                quote = Some(b);
                continue;
            }
            None => {}
        }
        if !b.is_ascii_whitespace() {
            last = b;
        }
    }
    None
}

fn content_type(path: &Path) -> &'static str {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext.to_ascii_lowercase().as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "wasm" => "application/wasm",
        "txt" | "md" | "csv" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCRIPT: &str = "<script>";

    #[test]
    fn frame_script_goes_after_doctype_html_and_head() {
        let page =
            "<!DOCTYPE html>\n<html lang=en>\n<head><title>x</title></head><body></body></html>";
        let out = with_frame_script(page);
        assert!(out.starts_with("<!DOCTYPE html>\n<html lang=en>\n<head><script>"));
        assert!(out.ends_with("</script><title>x</title></head><body></body></html>"));
    }

    #[test]
    fn frame_script_goes_first_in_a_fragment_and_skips_tags_later_in_the_page() {
        let out = with_frame_script("<svg></svg><p>&lt;head></p>");
        assert!(out.starts_with(SCRIPT));
        assert!(out.ends_with("</script><svg></svg><p>&lt;head></p>"));
        let out = with_frame_script("<html><header>h</header>");
        assert!(out.starts_with("<html><script>"));
    }

    #[test]
    fn frame_script_goes_after_tags_with_quoted_attributes() {
        let out = with_frame_script(r#"<html lang="en"><head data-theme="dark"><title>"#);
        assert!(out.starts_with(r#"<html lang="en"><head data-theme="dark"><script>"#));
        assert!(out.ends_with("</script><title>"));
        let out = with_frame_script(r#"<html lang=en><head data-x='>' b = "a>b"><p>"#);
        assert!(out.starts_with(r#"<html lang=en><head data-x='>' b = "a>b"><script>"#));
        assert!(out.ends_with("</script><p>"));
        let out = with_frame_script(r#"<!doctype html><html data-x="a>b" class=x><body>"#);
        assert!(out.starts_with(r#"<!doctype html><html data-x="a>b" class=x><script>"#));
        assert!(out.ends_with("</script><body>"));
        // A quote opens a value only after `=`.
        let out = with_frame_script(r#"<html a"b><head>"#);
        assert!(out.starts_with(r#"<html a"b><head><script>"#));
        // A quote left open never ends the tag, so the script goes before it.
        let out = with_frame_script(r#"<html lang="en><head>"#);
        assert!(out.starts_with(SCRIPT));
    }

    fn served(roots: &HtmlRoots, url: &str) -> (StatusCode, String) {
        let r = serve(roots, &["repo-1".into()], url);
        (r.status(), String::from_utf8_lossy(r.body()).into_owned())
    }

    #[test]
    fn serves_only_the_roots_window_and_files_inside_it() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("ws/docs")).unwrap();
        std::fs::write(dir.path().join("ws/docs/a b.css"), "p{}").unwrap();
        std::fs::write(dir.path().join("secret"), "s").unwrap();
        let roots = HtmlRoots::default();
        let id = roots.add("repo-1", dir.path().join("ws"));

        assert_eq!(
            served(&roots, &format!("/{id}/docs/a%20b.css")),
            (StatusCode::OK, "p{}".into())
        );
        assert_eq!(
            serve(&roots, &["repo-2".into()], &format!("/{id}/docs/a%20b.css")).status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            served(&roots, &format!("/{id}/../secret")).0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            served(&roots, &format!("/{id}/%2E%2E/secret")).0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            served(&roots, "/r0/docs/a%20b.css").0,
            StatusCode::NOT_FOUND
        );
    }

    #[test]
    fn root_ids_are_random_128_bit_hex() {
        let roots = HtmlRoots::default();
        let (a, b) = (roots.add("w", "/a".into()), roots.add("w", "/a".into()));
        assert_ne!(a, b);
        for id in [a, b] {
            assert_eq!(id.len(), 32);
            assert!(
                id.bytes()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            );
        }
    }

    #[test]
    fn every_response_carries_the_csp() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("p.html"), "<p>hi</p>").unwrap();
        let roots = HtmlRoots::default();
        let id = roots.add("repo-1", dir.path().into());
        for url in [format!("/{id}/p.html"), format!("/{id}/gone.html")] {
            let r = serve(&roots, &["repo-1".into()], &url);
            let csp = r.headers()[header::CONTENT_SECURITY_POLICY]
                .to_str()
                .unwrap();
            assert!(csp.contains("connect-src 'self' lgtmhtml: http://lgtmhtml.localhost;"));
            assert!(csp.contains("form-action 'none'"));
            assert!(csp.contains("script-src 'self' lgtmhtml: http://lgtmhtml.localhost 'unsafe-inline' 'unsafe-eval' https:;"));
            assert!(
                csp.contains("img-src 'self' lgtmhtml: http://lgtmhtml.localhost data: blob:;")
            );
        }
    }

    #[test]
    fn files_other_than_html_run_no_scripts() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.svg"), "<svg><script>1</script></svg>").unwrap();
        let roots = HtmlRoots::default();
        let id = roots.add("repo-1", dir.path().into());
        let r = serve(&roots, &["repo-1".into()], &format!("/{id}/a.svg"));
        let csp = r.headers()[header::CONTENT_SECURITY_POLICY]
            .to_str()
            .unwrap();
        assert!(csp.contains("script-src 'none'"));
        assert!(!r.body().starts_with(SCRIPT.as_bytes()));
    }

    #[cfg(unix)]
    #[test]
    fn refuses_a_symlink_out_of_the_root() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("ws")).unwrap();
        std::fs::write(dir.path().join("secret.html"), "s").unwrap();
        std::os::unix::fs::symlink(dir.path().join("secret.html"), dir.path().join("ws/l.html"))
            .unwrap();
        let roots = HtmlRoots::default();
        let id = roots.add("repo-1", dir.path().join("ws"));
        assert_eq!(
            served(&roots, &format!("/{id}/l.html")).0,
            StatusCode::FORBIDDEN
        );
    }

    #[test]
    fn a_folder_serves_its_index_with_the_frame_script() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("d")).unwrap();
        std::fs::write(dir.path().join("d/index.html"), "<p>hi</p>").unwrap();
        let roots = HtmlRoots::default();
        let id = roots.add("repo-1", dir.path().into());
        let (status, body) = served(&roots, &format!("/{id}/d"));
        assert_eq!(status, StatusCode::OK);
        assert!(body.starts_with(SCRIPT) && body.ends_with("</script><p>hi</p>"));
    }

    #[test]
    fn changed_reports_edits_and_deletions_of_served_files_once() {
        let dir = tempfile::tempdir().unwrap();
        let page = dir.path().join("p.html");
        std::fs::write(&page, "1").unwrap();
        let roots = HtmlRoots::default();
        let id = roots.add("repo-1", dir.path().into());
        served(&roots, &format!("/{id}/p.html"));
        assert!(!roots.changed("repo-1", &id));

        let later = SystemTime::now() + std::time::Duration::from_secs(5);
        std::fs::File::options()
            .write(true)
            .open(&page)
            .unwrap()
            .set_modified(later)
            .unwrap();
        assert!(!roots.changed("repo-2", &id));
        assert!(roots.changed("repo-1", &id));
        assert!(!roots.changed("repo-1", &id));

        served(&roots, &format!("/{id}/p.html"));
        std::fs::remove_file(&page).unwrap();
        assert!(roots.changed("repo-1", &id));
    }
}
