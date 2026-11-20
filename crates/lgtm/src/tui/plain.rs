//! The editor outside source control, and the issues screen: no revisions, review or
//! agents. Leaving the editor quits.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Sender};
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

use super::load::Msg;
use super::paint::Canvas;
use super::plain_issues::{Asked, Pane};
use super::term::{self, Term};
use super::{POLL, edit, mention, settings, walk};
use lgtm_tui_kit::mode::Mode;

/// Opens `file`, a name in `root` and a line from 1, else the list of files, with the
/// issues screen over it when `issues`.
pub fn run(root: PathBuf, file: Option<(String, u32)>, issues: bool) -> i32 {
    let (tx, rx) = mpsc::channel();
    let mut e = edit::Editor::new(&root);
    e.modes = vec![Mode::Issues];
    let mut pane = issues.then(|| Pane::new(&root));
    let mut on_issues = issues;
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
    match file {
        Some((name, line)) => {
            if let Err(err) = e.open(&name, line) {
                eprintln!("lgtm: couldn't open {name}: {err}");
                return 1;
            }
        }
        None => e.browse(),
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
                match pane.as_mut().filter(|_| on_issues) {
                    Some(p) => p.draw(c),
                    None => super::frame::draw_editor(c, &mut e, help.as_mut(), pick.as_ref()),
                }
                if let Some(at) = c.bar {
                    f.set_cursor_position(at);
                }
            })?;
            if let Some(p) = pane.as_mut().filter(|_| on_issues) {
                p.tick();
            }
            if event::poll(Duration::from_millis(50))? {
                match event::read()? {
                    Event::Key(key) if term::is_shift(&key) => {}
                    Event::Key(key) if key.kind != KeyEventKind::Release && on_issues => {
                        let key = term::plain(key);
                        let p = pane.as_mut().expect("issues");
                        let asked = match key.code == KeyCode::Char('c')
                            && key.modifiers.contains(KeyModifiers::CONTROL)
                        {
                            true => Asked::Quit,
                            false => p.key(key),
                        };
                        match asked {
                            Asked::None => {}
                            Asked::Quit if !e.blocks_quit() => return Ok(()),
                            Asked::Quit | Asked::Editor => on_issues = false,
                            Asked::Edit(name) => match e.open(&name, 1) {
                                Ok(()) => on_issues = false,
                                Err(err) => e.notice = Some(format!("couldn't open {name}: {err}")),
                            },
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
                                edit::Outcome::Issues => {
                                    pane.get_or_insert_with(|| Pane::new(&root));
                                    on_issues = true;
                                }
                                edit::Outcome::Review(..) => {
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
                    Event::Mouse(ev) if on_issues => pane.as_mut().expect("issues").mouse(ev),
                    Event::Paste(text) if on_issues => pane.as_mut().expect("issues").paste(&text),
                    Event::Mouse(ev) if help.is_none() => e.mouse(ev),
                    Event::Paste(text) if help.is_none() => e.paste(&text),
                    _ => {}
                }
            }
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    Msg::Files(l) => e.set_files(l),
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
