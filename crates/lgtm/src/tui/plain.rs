//! The editor alone, for files outside jj: no home, review or agents. The gutter diffs
//! against git's `HEAD` when the root is a git repo. Leaving the editor quits.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Sender};
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

use super::load::Msg;
use super::paint::Canvas;
use super::term::{self, Term};
use super::{POLL, edit, mention, settings, walk};

/// Opens `file`, a name in `root` and a line from 1, else the list of files.
pub fn run(root: PathBuf, file: Option<(String, u32)>) -> i32 {
    let (tx, rx) = mpsc::channel();
    let mut e = edit::Editor::new(&root);
    let mut settings = settings::load();
    e.files = settings.files;
    e.files_width = settings.editor_files_width;
    e.wrap = settings.editor_wrap;
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
                super::draw_editor(c, &mut e, help.as_mut(), pick.as_ref());
                if let Some(at) = c.bar {
                    f.set_cursor_position(at);
                }
            })?;
            if event::poll(Duration::from_millis(50))? {
                match event::read()? {
                    Event::Key(key) if term::is_shift(&key) => {}
                    Event::Key(key) if key.kind != KeyEventKind::Release => {
                        let key = term::plain(key);
                        if key.code == KeyCode::Char('c')
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
                            {
                                settings.files = e.files;
                                settings.editor_wrap = e.wrap;
                                settings.editor_files_width = e.files_width;
                                settings::save(&settings);
                            }
                            match outcome {
                                edit::Outcome::None => {}
                                edit::Outcome::Quit => return Ok(()),
                                edit::Outcome::Home => {
                                    e.notice = Some("no home screen outside jj".into());
                                }
                                edit::Outcome::Review(..) => {
                                    e.notice = Some("not in a jj repo".into());
                                }
                                edit::Outcome::Mention(text) => match mention::send(text) {
                                    mention::Sent::Done => {}
                                    mention::Sent::Pick(p) => pick = Some(p),
                                    mention::Sent::Notice(n) => e.notice = Some(n),
                                },
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
                    Msg::Files(listing) => e.set_files(listing),
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
