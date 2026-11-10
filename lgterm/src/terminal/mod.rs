//! One terminal: a shell on a pty, parsed by Ghostty, painted by GPUI.

mod boxes;
mod find;
mod frame;
mod input;
mod keys;
pub mod links;
mod paint;
mod pty;

pub use pty::SpawnEnv;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::Result;
use ghostty_vt::selection::Selection;
use ghostty_vt::style::RgbColor;
use ghostty_vt::terminal::{Point as VtPoint, PointCoordinate};
use ghostty_vt::{Event, Terminal, TerminalOptions, fmt, key, mouse};
use gpui::{
    App, AppContext, Bounds, ClipboardItem, Context, Entity, EventEmitter, FocusHandle, Focusable,
    Font, Global, InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels, Point,
    Render, Size, Styled, Window, actions, canvas, div, font, px,
};

use frame::{Frame, Snapshotter};
use paint::{Grid, LineCache};
use pty::Pty;

use crate::fonts;
use crate::model::TermId;
use crate::theme;

actions!(terminal, [Copy, Paste]);

/// How long after the last keypress the user counts as still typing.
const TYPING_PAUSE: Duration = Duration::from_secs(2);

/// Output parsed at once before the window gets a chance to draw.
const FEED_LIMIT: usize = 256 * 1024;

/// The terminal font size, shared by every terminal.
pub struct FontSize(pub f32);

impl Global for FontSize {}

pub enum TermEvent {
    /// The title or working folder changed.
    Changed,
    /// The shell exited.
    Exited,
    /// A link was ⌘-clicked.
    Link(links::Link),
}

pub struct TermView {
    pub id: TermId,
    focus: FocusHandle,
    vt: Terminal,
    snap: Snapshotter,
    keys: key::Encoder,
    key_event: key::Event,
    mouse: mouse::Encoder,
    pty: Pty,
    /// Folder the shell started in.
    start_cwd: PathBuf,
    /// Folder the shell last reported with OSC 7.
    osc_cwd: Option<PathBuf>,
    pub title: String,
    cols: u16,
    rows: u16,
    grid: Option<(Point<Pixels>, Size<Pixels>)>,
    /// Scroll distance not yet turned into whole lines.
    scroll_rest: f32,
    /// Where a selection drag started, in screen rows.
    drag_from: Option<(u16, u32)>,
    /// Where the button went down, until the drag has gone far enough to select.
    press_at: Option<gpui::Point<gpui::Pixels>>,
    mouse_down: bool,
    /// The button held down over the terminal, and whether its press went to the program.
    pressed: Option<(MouseButton, bool)>,
    find: Option<find::Find>,
    /// The user typed a prompt and hasn't sent or dropped it yet.
    draft: bool,
    last_key: Option<Instant>,
    lines: LineCache,
}

fn rgb(c: u32) -> RgbColor {
    RgbColor {
        r: (c >> 16) as u8,
        g: (c >> 8) as u8,
        b: c as u8,
    }
}

impl TermView {
    /// Starts a shell in `cwd`, running `argv` if it's not empty.
    pub fn spawn(
        id: TermId,
        cwd: &Path,
        argv: &[String],
        app: &SpawnEnv,
        cx: &mut App,
    ) -> Result<Entity<Self>> {
        let (cols, rows) = (80, 24);
        let mut vt = Terminal::new(TerminalOptions {
            cols,
            rows,
            ..Default::default()
        })?;
        vt.set_default_fg_color(Some(rgb(theme::TERM_FG)))?
            .set_default_bg_color(Some(rgb(theme::TERM_BG)))?
            .set_default_cursor_color(Some(rgb(theme::TERM_FG)))?;
        let snap = Snapshotter::new()?;
        let keys = key::Encoder::new()?;
        let key_event = key::Event::new()?;
        let mouse = mouse::Encoder::new()?;
        let (pty, output) = Pty::spawn(id, cwd, argv, app, cols, rows)?;
        Ok(cx.new(|cx| {
            cx.spawn(async move |this: gpui::WeakEntity<TermView>, cx| {
                while let Ok(mut bytes) = output.recv().await {
                    while bytes.len() < FEED_LIMIT
                        && let Ok(more) = output.try_recv()
                    {
                        bytes.extend(more);
                    }
                    if this.update(cx, |t, cx| t.feed(&bytes, cx)).is_err() {
                        return;
                    }
                }
                let _ = this.update(cx, |_, cx| cx.emit(TermEvent::Exited));
            })
            .detach();
            TermView {
                id,
                focus: cx.focus_handle(),
                vt,
                snap,
                keys,
                key_event,
                mouse,
                pty,
                start_cwd: cwd.to_path_buf(),
                osc_cwd: None,
                title: String::new(),
                cols,
                rows,
                grid: None,
                scroll_rest: 0.,
                drag_from: None,
                press_at: None,
                mouse_down: false,
                pressed: None,
                find: None,
                draft: false,
                last_key: None,
                lines: LineCache::default(),
            }
        }))
    }

    pub fn foreground_pid(&self) -> Option<i32> {
        self.pty.foreground_pid()
    }

    pub fn shell_pid(&self) -> Option<i32> {
        self.pty.shell_pid()
    }

    /// Whether the user is in the middle of typing something.
    pub fn typing(&self) -> bool {
        self.draft || self.last_key.is_some_and(|t| t.elapsed() < TYPING_PAUSE)
    }

    /// Forgets a typed prompt, once the program has taken it.
    pub fn clear_draft(&mut self) {
        self.draft = false;
    }

    /// Where the shell is, as best we know without asking the OS.
    pub fn known_cwd(&self) -> &Path {
        self.osc_cwd.as_deref().unwrap_or(&self.start_cwd)
    }

    fn feed(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        self.vt.vt_write(bytes);
        let mut changed = false;
        let events: Vec<Event> = self.vt.events().collect();
        for event in events {
            match event {
                Event::PtyWrite(reply) => self.pty.write(&reply),
                Event::TitleChanged(title) => {
                    self.title = title;
                    changed = true;
                }
                Event::PwdChanged(pwd) => {
                    self.osc_cwd = parse_pwd(&pwd);
                    changed = true;
                }
                Event::ClipboardWrite(w) => {
                    let text = w
                        .contents
                        .iter()
                        .find(|c| c.mime.is_empty() || c.mime.starts_with("text/"));
                    if let Some(text) = text.and_then(|c| String::from_utf8(c.data.clone()).ok()) {
                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                    }
                }
                _ => {}
            }
        }
        if changed {
            cx.emit(TermEvent::Changed);
        }
        cx.notify();
    }

    fn write(&mut self, bytes: &[u8]) {
        self.pty.write(bytes);
    }

    /// Types `bytes` into the terminal.
    pub fn input(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        self.write(bytes);
        self.scroll_to_bottom(cx);
    }

    /// Pastes `text`, bracketed if the program asked for that.
    pub fn paste_text(&mut self, text: String, cx: &mut Context<Self>) {
        let bytes = input::paste(&self.vt, text);
        self.input(&bytes, cx);
    }

    /// The bottom `rows` rows of the live screen, wherever the user has scrolled, one line
    /// per row.
    pub fn screen_bottom(&self, rows: u16) -> String {
        let top = self.rows.saturating_sub(rows) as u32;
        let bottom = self.rows.saturating_sub(1) as u32;
        let at = |x, y| VtPoint::Active(PointCoordinate { x, y });
        self.format(at(0, top), at(self.cols.saturating_sub(1), bottom), false)
    }

    /// The last `lines` lines of scrollback and screen, or a screenful, with wrapped lines
    /// joined. Formats only as far back as it needs to.
    pub fn text(&self, lines: Option<usize>) -> String {
        let want = lines.unwrap_or(self.rows as usize);
        let total = self.vt.total_rows().unwrap_or(0);
        let mut span = want.max(1) + self.rows as usize;
        loop {
            let start = total.saturating_sub(span);
            let at = |x, y: usize| VtPoint::Screen(PointCoordinate { x, y: y as u32 });
            let end = at(self.cols.saturating_sub(1), total.saturating_sub(1));
            let text = self.format(at(0, start), end, true);
            let all: Vec<&str> = text.trim_end().lines().collect();
            if all.len() >= want || start == 0 {
                let n = want.min(all.len());
                return all[all.len() - n..].join("\n");
            }
            span *= 2;
        }
    }

    /// Plain text from `start` to `end`.
    fn format(&self, start: VtPoint, end: VtPoint, unwrap: bool) -> String {
        let (Ok(start), Ok(end)) = (self.vt.grid_ref(start), self.vt.grid_ref(end)) else {
            return String::new();
        };
        let sel = Selection::new(start, end, false);
        let opts = fmt::FormatterOptions::new()
            .with_format(fmt::Format::Plain)
            .with_unwrap(unwrap)
            .with_trim(true)
            .with_selection(&sel);
        fmt::Formatter::new(&self.vt, opts)
            .and_then(|mut f| f.format_alloc())
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default()
    }

    fn font(&self, cx: &App) -> (Font, Pixels) {
        (font(fonts::mono()), px(cx.global::<FontSize>().0))
    }

    /// Fits the terminal to `bounds` and snapshots the screen.
    fn layout(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<(Frame, Grid)> {
        let (font, font_size) = self.font(cx);
        let cell = paint::cell_size(&font, font_size, window);
        let inner_w = bounds.size.width - px(2. * theme::TERM_PAD_X);
        let inner_h = bounds.size.height - px(2. * theme::TERM_PAD_Y);
        let cols = ((inner_w / cell.width).floor() as u16).max(2);
        let rows = ((inner_h / cell.height).floor() as u16).max(1);
        if (cols, rows) != (self.cols, self.rows) {
            let (cw, ch) = (cell.width.as_f32() as u32, cell.height.as_f32() as u32);
            if self.vt.resize(cols, rows, cw, ch).is_ok() {
                self.pty.resize(
                    cols,
                    rows,
                    (cols as u32 * cw) as u16,
                    (rows as u32 * ch) as u16,
                );
                (self.cols, self.rows) = (cols, rows);
            }
        }
        let origin = bounds.origin + gpui::point(px(theme::TERM_PAD_X), px(theme::TERM_PAD_Y));
        self.grid = Some((origin, cell));
        let frame = self.snap.frame(&self.vt).ok()?;
        let grid = Grid {
            origin,
            cell,
            font,
            font_size,
            focused: self.focus.is_focused(window),
            lines: self.lines.clone(),
        };
        Some((frame, grid))
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = input::selection_text(&self.vt) {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) else {
            return;
        };
        self.draft = true;
        self.last_key = Some(Instant::now());
        self.paste_text(text, cx);
    }

    fn scroll_to_bottom(&mut self, cx: &mut Context<Self>) {
        self.vt
            .scroll_viewport(ghostty_vt::terminal::ScrollViewport::Bottom);
        let _ = self.vt.set_selection(None);
        cx.notify();
    }
}

/// The folder in an OSC 7 `file://host/path` report, or a bare path.
fn parse_pwd(pwd: &str) -> Option<PathBuf> {
    let path = match pwd.strip_prefix("file://") {
        Some(rest) => &rest[rest.find('/')?..],
        None => pwd,
    };
    let mut out = Vec::with_capacity(path.len());
    let bytes = path.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(hi), Some(lo)) = (hex_digit(bytes[i + 1]), hex_digit(bytes[i + 2]))
        {
            out.push(hi << 4 | lo);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    let path = PathBuf::from(String::from_utf8(out).ok()?);
    path.is_absolute().then_some(path)
}

fn hex_digit(b: u8) -> Option<u8> {
    (b as char).to_digit(16).map(|d| d as u8)
}

impl EventEmitter<TermEvent> for TermView {}

impl Focusable for TermView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for TermView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        div()
            .id(("term", self.id.0))
            .size_full()
            .relative()
            .track_focus(&self.focus)
            .key_context("Terminal")
            .on_key_down(cx.listener(Self::key_down))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::paste))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::mouse_down))
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::mouse_up))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::mouse_up))
            .on_mouse_up(MouseButton::Right, cx.listener(Self::mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::mouse_up_out))
            .on_mouse_up_out(MouseButton::Middle, cx.listener(Self::mouse_up_out))
            .on_mouse_up_out(MouseButton::Right, cx.listener(Self::mouse_up_out))
            .on_scroll(cx.listener(Self::scroll))
            .child(
                canvas(
                    move |bounds, window, cx| this.update(cx, |t, cx| t.layout(bounds, window, cx)),
                    |_, prepared, window, cx| {
                        if let Some((frame, grid)) = prepared {
                            grid.paint(&frame, window, cx);
                        }
                    },
                )
                .size_full(),
            )
            .children(self.render_find(cx))
    }
}

#[cfg(test)]
mod tests {
    use super::parse_pwd;
    use std::path::PathBuf;

    #[test]
    fn osc7_reports_decode_to_paths() {
        assert_eq!(
            parse_pwd("file://mac.local/Users/j/my%20repo"),
            Some(PathBuf::from("/Users/j/my repo"))
        );
        assert_eq!(parse_pwd("/tmp"), Some(PathBuf::from("/tmp")));
        assert_eq!(parse_pwd("relative"), None);
    }

    #[test]
    fn stray_percents_stay_literal() {
        assert_eq!(
            parse_pwd("file://h/tmp/50%aé"),
            Some(PathBuf::from("/tmp/50%aé"))
        );
        assert_eq!(parse_pwd("/tmp/%+a"), Some(PathBuf::from("/tmp/%+a")));
    }
}
