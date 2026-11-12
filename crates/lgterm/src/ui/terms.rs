//! The window's terminals: starting them, what polling finds in them, and the names shown for
//! terminals, spaces and layouts.

use std::path::{Path, PathBuf};

use gpui::{App, Context, Entity, Subscription, Window};

use super::Workspace;
use crate::agents::{Env, Kind, Seen};
use crate::model::{Space, Tab, TermId};
use crate::procinfo;
use crate::terminal::{TermEvent, TermView};

/// What polling found a terminal running.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Info {
    pub process: Option<String>,
    /// The agent the foreground process runs.
    pub kind: Option<Kind>,
    pub cwd: Option<PathBuf>,
    /// The shell itself is in the foreground, running nothing.
    pub shell_foreground: bool,
}

pub struct Term {
    pub view: Entity<TermView>,
    pub info: Info,
    /// The agent it was started for, which counts when what runs can't be told apart, like a
    /// wrapper the user's command runs it through.
    pub hint: Option<Kind>,
    /// The program's title.
    pub title: String,
    /// The folder the view knows the shell is in.
    known_cwd: PathBuf,
    /// The name a space takes from this terminal's folder.
    pub space_name: String,
    _sub: Subscription,
}

/// Rows from the bottom of the screen that show what an agent is doing.
const STATUS_ROWS: u16 = 16;

impl Term {
    pub fn cwd(&self) -> &Path {
        self.info.cwd.as_deref().unwrap_or(&self.known_cwd)
    }
}

/// A program's title without the spinner or status glyph agents put in front.
pub fn clean_title(title: &str) -> Option<&str> {
    let t = title
        .trim_start_matches(|c: char| !c.is_alphanumeric())
        .trim();
    (!t.is_empty()).then_some(t)
}

impl Workspace {
    pub(super) fn spawn(
        &mut self,
        id: TermId,
        cwd: &Path,
        argv: &[String],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let view = match TermView::spawn(id, cwd, argv, &self.spawn_env, cx) {
            Ok(view) => view,
            Err(err) => {
                eprintln!("lgterm: can't start a shell: {err:#}");
                self.close(Tab::Term(id), window, cx);
                return Err(err);
            }
        };
        let sub = cx.subscribe_in(&view, window, move |this, _, ev, window, cx| match ev {
            TermEvent::Exited => this.close(Tab::Term(id), window, cx),
            TermEvent::Changed => this.term_changed(id, cx),
            TermEvent::Link(link) => this.open_link(id, link, window, cx),
        });
        let space_name = procinfo::space_name(cwd, self.home.as_deref());
        self.terms.insert(
            id,
            Term {
                view,
                info: Info::default(),
                hint: None,
                title: String::new(),
                known_cwd: cwd.to_path_buf(),
                space_name,
                _sub: sub,
            },
        );
        Ok(())
    }

    /// Copies a view's title and folder, so drawing the window doesn't read every view.
    fn term_changed(&mut self, id: TermId, cx: &mut Context<Self>) {
        let home = self.home.clone();
        let Some(t) = self.terms.get_mut(&id) else {
            return;
        };
        let view = t.view.read(cx);
        t.title.clone_from(&view.title);
        if view.known_cwd() != t.known_cwd {
            t.known_cwd = view.known_cwd().to_path_buf();
            t.space_name = procinfo::space_name(t.cwd(), home.as_deref());
        }
        cx.notify();
    }

    /// The folder new terminals beside `tab` start in.
    pub(super) fn cwd_of(&self, tab: Tab) -> PathBuf {
        match tab.term().and_then(|t| self.terms.get(&t)) {
            Some(t) => t.cwd().to_path_buf(),
            None => self.home_dir(),
        }
    }

    pub(super) fn home_dir(&self) -> PathBuf {
        self.home.clone().unwrap_or_else(|| PathBuf::from("/"))
    }

    /// Checks what each terminal runs and where, and what its agent is doing.
    pub(super) fn poll(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut changed = false;
        let home = self.home.clone();
        for term in self.terms.values_mut() {
            let view = term.view.read(cx);
            let pid = view.foreground_pid();
            let process = pid.and_then(procinfo::name);
            let shell_foreground = pid.is_some() && pid == view.shell_pid();
            let kind = process
                .as_deref()
                .and_then(Kind::from_process)
                .or_else(|| {
                    let (exe, argv) = procinfo::command(pid.filter(|_| !shell_foreground)?)?;
                    Kind::from_command(&exe, &argv)
                })
                .or(term.hint.filter(|_| pid.is_some() && !shell_foreground));
            let info = Info {
                process,
                kind,
                cwd: pid.and_then(procinfo::cwd),
                shell_foreground,
            };
            if info != term.info {
                let moved = info.cwd != term.info.cwd;
                term.info = info;
                if moved {
                    term.space_name = procinfo::space_name(term.cwd(), home.as_deref());
                }
                changed = true;
            }
        }
        if changed {
            cx.notify();
        }
        let terms = &self.terms;
        let app: &App = cx;
        let typing = |t: TermId| terms.get(&t).is_some_and(|t| t.view.read(app).typing());
        let env = Env {
            viewed: self.viewed(window),
            typing: &typing,
        };
        for (id, t) in terms {
            let view = t.view.read(app);
            let seen = Seen {
                kind: t.info.kind,
                process_known: t.info.process.is_some(),
                shell_foreground: t.info.shell_foreground,
                screen: &|| view.screen_bottom(STATUS_ROWS),
            };
            self.agents.poll(*id, seen, &env);
        }
        self.agents.settle(&env);
        self.run_effects(cx);
    }

    pub(super) fn term_label(&self, tab: Tab) -> String {
        let Tab::Term(term) = tab else {
            return "Messages".into();
        };
        let Some(t) = self.terms.get(&term) else {
            return String::new();
        };
        if let Some(name) = self.model.name(term) {
            return name.to_owned();
        }
        if super::messages::mock::on()
            && let Some(m) = self
                .agents
                .messages
                .iter()
                .find(|m| m.to == term || m.from == Some(term))
        {
            return if m.to == term {
                m.to_label.clone()
            } else {
                m.from_label.clone()
            };
        }
        if let Some(agent) = self.agents.get(term) {
            return clean_title(&t.title)
                .unwrap_or(agent.kind.name())
                .to_owned();
        }
        match t.info.process.as_deref() {
            Some(p) => p.to_owned(),
            None => clean_title(&t.title).unwrap_or("shell").to_owned(),
        }
    }

    pub(super) fn space_label(&self, space: &Space) -> String {
        if let Some(name) = &space.name {
            return name.clone();
        }
        space
            .terms()
            .find_map(|t| self.terms.get(&t))
            .map_or_else(|| self.home_name.clone(), |t| t.space_name.clone())
    }

    /// A layout's name, or its place in the list.
    pub(super) fn layout_label(&self, index: usize) -> String {
        match self.model.layouts.get(index).and_then(|l| l.name.clone()) {
            Some(name) => name,
            None => format!("Layout {}", index + 1),
        }
    }

    pub(super) fn is_agent(&self, tab: Tab) -> bool {
        tab.term().is_some_and(|t| self.agents.get(t).is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_lose_leading_glyphs() {
        assert_eq!(clean_title("✳ Review keys"), Some("Review keys"));
        assert_eq!(clean_title("⠂ "), None);
        assert_eq!(clean_title("vim"), Some("vim"));
    }
}
