//! The editor outside source control, with a home screen of the file tree alone: no
//! revisions, review or agents. Leaving the editor quits.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Sender};
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

use super::flow::Piece;
use super::home::{self, Home};
use super::load::Msg;
use super::paint::Canvas;
use super::term::{self, Term};
use super::theme::{self, FG, PANEL, RED};
use super::{POLL, edit, keybar, mention, settings, walk};

/// Opens `file`, a name in `root` and a line from 1, else the list of files.
pub fn run(root: PathBuf, file: Option<(String, u32)>) -> i32 {
    let (tx, rx) = mpsc::channel();
    let mut e = edit::Editor::new(&root);
    let mut settings = settings::load();
    e.files = settings.files;
    e.files_width = settings.editor_files_width;
    e.wrap = settings.editor_wrap;
    e.render_markdown = settings.render_markdown;
    {
        let (root, tx) = (root.clone(), tx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Files(walk::list(&root)));
        });
    }
    git_decor(&root, tx.clone());
    // The home screen while it's up, and the files for it.
    let mut home = None;
    let mut listing: Option<walk::Listing> = None;
    match file {
        Some((name, line)) => {
            if let Err(err) = e.open(&name, line) {
                eprintln!("lgtm: couldn't open {name}: {err}");
                return 1;
            }
        }
        None => home = Some(Home::files_only()),
    }
    let git = root.join(".git").exists();
    let mut term = Term::start();
    let mut help: Option<super::help::Help> = None;
    let mut pick: Option<mention::Pick> = None;
    let mut polled = Instant::now();
    let mut refreshed = Instant::now();
    let result = (|| -> anyhow::Result<()> {
        loop {
            term.terminal.draw(|f| {
                let area = f.area();
                let c = &mut Canvas::new(f.buffer_mut(), area);
                match home.as_mut() {
                    Some(h) => draw_home(c, h, &root),
                    None => super::frame::draw_editor(c, &mut e, help.as_mut(), pick.as_ref()),
                }
                if let Some(at) = c.bar {
                    f.set_cursor_position(at);
                }
            })?;
            if event::poll(Duration::from_millis(50))? {
                match event::read()? {
                    Event::Key(key) if term::is_shift(&key) => {}
                    Event::Key(key) if key.kind != KeyEventKind::Release && home.is_some() => {
                        let key = term::plain(key);
                        if key.code == KeyCode::Char('c')
                            && key.modifiers.contains(KeyModifiers::CONTROL)
                        {
                            return Ok(());
                        }
                        let h = home.as_mut().expect("home");
                        match h.key(key) {
                            home::Outcome::Quit => return Ok(()),
                            home::Outcome::Edit(path) => match e.open(&path, 0) {
                                Ok(()) => home = None,
                                Err(err) => h.notice = Some(format!("couldn't open {path}: {err}")),
                            },
                            _ => {}
                        }
                    }
                    Event::Key(key) if key.kind != KeyEventKind::Release => {
                        let key = term::plain(key);
                        if key.code == KeyCode::Char('c')
                            && key.modifiers.contains(KeyModifiers::SUPER)
                            && let Some(text) = e.selection()
                        {
                            e.notice = Some(match lgtm_tui_kit::clipboard::copy(&text) {
                                Ok(()) => "copied".into(),
                                Err(e) => format!("couldn't copy: {e}"),
                            });
                        } else if key.code == KeyCode::Char('c')
                            && key.modifiers.contains(KeyModifiers::CONTROL)
                        {
                            if !e.blocks_quit() {
                                return Ok(());
                            }
                        } else if let Some(h) = help.as_mut() {
                            if !h.key(&key) {
                                help = None;
                            }
                        } else if let Some(p) = pick.take() {
                            e.notice = p.key(key);
                        } else if key.code == KeyCode::Char('?') && !e.typing() {
                            help = Some(super::help::Help::default());
                        } else {
                            let outcome = e.key(key);
                            if e.files != settings.files
                                || e.files_width != settings.editor_files_width
                                || e.wrap != settings.editor_wrap
                                || e.render_markdown != settings.render_markdown
                            {
                                settings.files = e.files;
                                settings.editor_wrap = e.wrap;
                                settings.render_markdown = e.render_markdown;
                                settings.editor_files_width = e.files_width;
                                settings::save(&settings);
                            }
                            match outcome {
                                edit::Outcome::None => {}
                                edit::Outcome::Quit => return Ok(()),
                                edit::Outcome::Home => {
                                    let mut h = Home::files_only();
                                    if let Some(l) = &listing {
                                        h.files.set_listing(l.clone());
                                    }
                                    home = Some(h);
                                }
                                edit::Outcome::Review(..) | edit::Outcome::Issues => {
                                    e.notice = Some("not in a jj or git repo".into());
                                }
                                edit::Outcome::Mention(text) => match mention::send(text) {
                                    mention::Sent::Done => {}
                                    mention::Sent::Pick(p) => pick = Some(p),
                                    mention::Sent::Notice(n) => e.notice = Some(n),
                                },
                            }
                        }
                    }
                    Event::Mouse(ev) if home.is_some() => {
                        let h = home.as_mut().expect("home");
                        if let home::Outcome::Edit(path) = h.mouse(ev) {
                            match e.open(&path, 0) {
                                Ok(()) => home = None,
                                Err(err) => h.notice = Some(format!("couldn't open {path}: {err}")),
                            }
                        }
                    }
                    Event::Mouse(ev) if help.is_none() => e.mouse(ev),
                    Event::Paste(text) if help.is_none() => e.paste(&text),
                    _ => {}
                }
            }
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    Msg::Files(l) => {
                        if let Some(h) = home.as_mut() {
                            h.files.set_listing(l.clone());
                        }
                        e.set_files(l.clone());
                        listing = Some(l);
                    }
                    Msg::Base { name, text } => e.set_base(&name, text),
                    Msg::Decor(decor) => e.set_decor(decor),
                    _ => {}
                }
            }
            if polled.elapsed() >= POLL {
                polled = Instant::now();
                e.check_disk();
            }
            if std::mem::take(&mut e.saved) || refreshed.elapsed() >= super::REFRESH {
                refreshed = Instant::now();
                git_decor(&root, tx.clone());
            }
            // Outside jj the markers' own labels are all there is.
            e.take_resolve_requests();
            for name in e.take_base_requests() {
                if git {
                    base(&root, name, tx.clone());
                }
            }
        }
    })();
    term.stop();
    match result {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("lgtm: {err:#}");
            1
        }
    }
}

/// The home screen: the folder in the top bar, the file tree, and its keys or `:` line.
fn draw_home(c: &mut Canvas, h: &mut Home, root: &Path) {
    let left = vec![Piece::new(2, super::home(root), theme::fg(FG).bg(PANEL))];
    let right: Vec<Piece> = h
        .notice
        .iter()
        .map(|n| Piece::new(2, n.clone(), theme::fg(RED).bg(PANEL)))
        .collect();
    let prompt = h.prompt();
    let hints = h.hints();
    let foot = match prompt {
        Some(_) => 1,
        None => keybar::rows(c.area.width, None, &hints),
    };
    let bottom = c.area.bottom().saturating_sub(1);
    let Some(body) = super::frame::frame(c, left, &right, foot) else {
        return;
    };
    h.draw(c, body);
    match prompt {
        Some(text) => keybar::prompt(c, bottom, &text, &[]),
        None => {
            keybar::draw_badged(c, bottom, None, &hints);
        }
    }
    keybar::menu(c, body, &h.command_menu());
}

/// Finds what git says changed, for the sidebar.
fn git_decor(root: &Path, tx: Sender<Msg>) {
    let root = root.to_path_buf();
    std::thread::spawn(move || {
        let _ = tx.send(Msg::Decor(super::decor::git(&root)));
    });
}

/// Reads the file `name` at git's `HEAD`, empty when it isn't there.
fn base(root: &Path, name: String, tx: Sender<Msg>) {
    let root = root.to_path_buf();
    std::thread::spawn(move || {
        let Ok(out) = Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(["show", &format!("HEAD:{name}")])
            .output()
        else {
            return;
        };
        let text = match out.status.success() {
            true => String::from_utf8_lossy(&out.stdout).into_owned(),
            false => String::new(),
        };
        let _ = tx.send(Msg::Base { name, text });
    });
}
