//! The issues screen on its own, outside a jj repo: `lgtm issues` where lgtm's other screens
//! can't run.

use std::io::stdout;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use anyhow::Result;
use lgtm_tui_kit::paint::Canvas;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyEventKind, MouseEventKind,
};
use ratatui::crossterm::execute;

use super::picker::{self, Picker};
use super::{Issues, Outcome};
use crate::folder::Folder;

enum Up {
    Issues(Box<Issues>),
    Picker(Picker),
}

fn start() -> DefaultTerminal {
    let term = ratatui::init();
    let _ = execute!(stdout(), EnableMouseCapture, EnableBracketedPaste);
    term
}

fn stop() {
    let _ = execute!(stdout(), DisableMouseCapture, DisableBracketedPaste);
    ratatui::restore();
}

/// Runs the screen on the issues folder `dir`, or on a folder picked under it.
pub fn run(dir: &Path, who: String) -> Result<()> {
    let mut up = match Folder::open(dir) {
        Ok(f) => Up::Issues(Box::new(Issues::new(f, who.clone()))),
        Err(_) => Up::Picker(Picker::new(dir)),
    };
    let mut term = start();
    let result = (|| -> Result<()> {
        loop {
            term.draw(|f| {
                let area = f.area();
                let c = &mut Canvas::new(f.buffer_mut(), area);
                match &mut up {
                    Up::Issues(i) => i.draw(c),
                    Up::Picker(p) => p.draw(c),
                }
                if let Some(at) = c.bar {
                    f.set_cursor_position(at);
                }
            })?;
            if let Up::Issues(i) = &mut up {
                i.tick();
            }
            if !event::poll(Duration::from_millis(100))? {
                continue;
            }
            let ev = event::read()?;
            match &mut up {
                Up::Picker(p) => {
                    if let Event::Key(key) = ev
                        && key.kind != KeyEventKind::Release
                    {
                        match p.key(key) {
                            picker::Outcome::Open(f) => {
                                up = Up::Issues(Box::new(Issues::new(f, who.clone())))
                            }
                            picker::Outcome::Cancel => return Ok(()),
                            picker::Outcome::None => {}
                        }
                    }
                }
                Up::Issues(i) => {
                    let outcome = match ev {
                        Event::Key(key) if key.kind != KeyEventKind::Release => i.key(key),
                        Event::Paste(text) => {
                            i.paste(&text);
                            Outcome::None
                        }
                        Event::Mouse(m) => {
                            match m.kind {
                                MouseEventKind::ScrollDown => i.scroll(3),
                                MouseEventKind::ScrollUp => i.scroll(-3),
                                _ => {}
                            }
                            Outcome::None
                        }
                        _ => Outcome::None,
                    };
                    match outcome {
                        Outcome::Quit => return Ok(()),
                        Outcome::Folders => up = Up::Picker(Picker::new(dir)),
                        Outcome::Edit(path) => {
                            stop();
                            let result = edit(&path);
                            term = start();
                            term.clear()?;
                            if let Err(e) = result {
                                i.notice(format!("couldn't open an editor: {e:#}"));
                            }
                            i.reload();
                        }
                        Outcome::Home | Outcome::None => {}
                    }
                }
            }
        }
    })();
    stop();
    result
}

/// Opens `path` in lgtm's editor, or `$VISUAL`/`$EDITOR` where lgtm isn't installed.
fn edit(path: &Path) -> Result<()> {
    match Command::new("lgtm").arg(path).status() {
        Ok(_) => return Ok(()),
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
        Err(_) => {}
    }
    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".into());
    Command::new("sh")
        .arg("-c")
        .arg(format!("{editor} \"$1\""))
        .arg("sh")
        .arg(path)
        .status()?;
    Ok(())
}
