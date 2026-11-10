//! Requests from the `lgterm` command in the app's terminals.

use std::path::PathBuf;

use gpui::{Context, Window};
use serde_json::json;

use super::Workspace;
use crate::ipc::{Incoming, Place, Reply, Request};
use crate::model::{Axis, Model, Tab, TermId};

impl Workspace {
    pub(super) fn handle(&mut self, inc: Incoming, window: &mut Window, cx: &mut Context<Self>) {
        let reply = match inc.request {
            Request::Wait { term } => match self.term_arg(&term) {
                Ok(id) => {
                    self.agents.wait(id, inc.reply);
                    return;
                }
                Err(e) => e,
            },
            request => self.reply(request, window, cx),
        };
        let _ = inc.reply.try_send(reply);
    }

    fn term_arg(&self, term: &str) -> Result<TermId, Reply> {
        TermId::parse(term)
            .filter(|id| self.terms.contains_key(id))
            .ok_or_else(|| Reply::Err(format!("no terminal {term}")))
    }

    fn reply(&mut self, request: Request, window: &mut Window, cx: &mut Context<Self>) -> Reply {
        let result = (|| -> Result<Reply, Reply> {
            Ok(match request {
                Request::Hook { term, event } => {
                    let id = self.term_arg(&term)?;
                    self.with_agents(window, cx, |a, env| a.hook(id, &event, env));
                    Reply::Ok(String::new())
                }
                Request::Ls { json } => Reply::Ok(self.list(json)),
                Request::Open {
                    from,
                    place,
                    cwd,
                    argv,
                    focus,
                } => {
                    let from = match from {
                        Some(f) => Some(Tab::Term(self.term_arg(&f)?)),
                        None => None,
                    };
                    match self.open(from, place, cwd, &argv, focus, window, cx) {
                        Ok(id) => Reply::Ok(id.to_string()),
                        Err(err) => Reply::Err(err),
                    }
                }
                Request::Send { from, to, text } => {
                    let to = self.term_arg(&to)?;
                    let from = from.and_then(|f| TermId::parse(&f));
                    self.send_message(from, to, text, window, cx)
                }
                Request::Type { to, text, enter } => {
                    let to = self.term_arg(&to)?;
                    let mut bytes = text.into_bytes();
                    if enter {
                        bytes.push(b'\r');
                    }
                    if let Some(t) = self.terms.get(&to) {
                        t.view.update(cx, |v, cx| v.input(&bytes, cx));
                    }
                    Reply::Ok(String::new())
                }
                Request::Read { term, lines } => {
                    let id = self.term_arg(&term)?;
                    let text = self
                        .terms
                        .get(&id)
                        .map(|t| t.view.read(cx).text(lines))
                        .unwrap_or_default();
                    Reply::Ok(text)
                }
                Request::Focus { term } => {
                    let id = self.term_arg(&term)?;
                    self.model.focus_term(id);
                    self.focus_current(window, cx);
                    Reply::Ok(String::new())
                }
                Request::Agents { term } => {
                    let id = self.term_arg(&term)?;
                    Reply::Ok(self.agents_near(id))
                }
                Request::Paste { to, text, focus } => {
                    let to = self.term_arg(&to)?;
                    if let Some(t) = self.terms.get(&to) {
                        t.view.update(cx, |v, cx| v.paste_text(text, cx));
                    }
                    if focus {
                        self.model.focus_term(to);
                        self.focus_current(window, cx);
                    }
                    Reply::Ok(String::new())
                }
                Request::Close { term } => {
                    let id = self.term_arg(&term)?;
                    self.close(Tab::Term(id), window, cx);
                    Reply::Ok(String::new())
                }
                Request::Wait { .. } => unreachable!("waits are handled before"),
            })
        })();
        result.unwrap_or_else(|e| e)
    }

    /// Opens a terminal beside `from`, or the focused tab, and returns its id. A new space from
    /// the sidebar, or one in an empty slot, starts in the home folder. Without `focus`, focus
    /// and zoom stay as they are.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn open(
        &mut self,
        from: Option<Tab>,
        place: Place,
        cwd: Option<PathBuf>,
        argv: &[String],
        focus: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<TermId, String> {
        let from = from.or_else(|| self.model.tab());
        let cwd = cwd.unwrap_or_else(|| match (place, from) {
            (Place::Layout, _) | (_, None) => self.home_dir(),
            (_, Some(from)) => self.cwd_of(from),
        });
        let m = &mut self.model;
        let id = match from {
            Some(from) if !focus => open_at(m, from, place),
            Some(from) => {
                m.focus_tab(from);
                Some(open_focused(m, place))
            }
            None => Some(open_focused(m, place)),
        };
        let id = id.ok_or("no such terminal")?;
        if let Err(err) = self.spawn(id, &cwd, argv, window, cx) {
            return Err(format!("can't start terminal: {err:#}"));
        }
        if focus {
            self.focus_current(window, cx);
        } else {
            cx.notify();
        }
        Ok(id)
    }

    /// The agents shown with `term`, in reading order, as JSON: those in the shown layout when
    /// `term` is there, else those in the first layout showing it, else those in its space.
    fn agents_near(&self, term: TermId) -> String {
        let m = &self.model;
        let Some(own) = m.space_of(Tab::Term(term)).map(|s| s.id) else {
            return "[]".to_owned();
        };
        let spaces = std::iter::once(m.layout())
            .chain(&m.layouts)
            .map(|l| l.spaces())
            .find(|s| s.contains(&own))
            .unwrap_or_else(|| vec![own]);
        let agents: Vec<_> = spaces
            .into_iter()
            .filter_map(|s| m.find_space(s))
            .flat_map(|s| s.terms())
            .filter(|&t| t != term && self.is_agent(Tab::Term(t)))
            .map(|t| json!({ "id": t.to_string(), "label": self.term_label(Tab::Term(t)) }))
            .collect();
        serde_json::to_string(&agents).unwrap_or_default()
    }

    fn list(&self, as_json: bool) -> String {
        let focused = self.model.term();
        let mut rows = Vec::new();
        for space in &self.model.spaces {
            let space_label = self.space_label(space);
            for term in space.terms() {
                rows.push(json!({
                    "id": term.to_string(),
                    "label": self.term_label(Tab::Term(term)),
                    "status": self.agents.status(term).map(|s| s.name()),
                    "space": space_label,
                    "cwd": self.cwd_of(Tab::Term(term)),
                    "focused": Some(term) == focused,
                }));
            }
        }
        if as_json {
            return serde_json::to_string_pretty(&rows).unwrap_or_default();
        }
        rows.iter()
            .map(|r| {
                format!(
                    "{}{:<5} {:<8} {} · {}",
                    if r["focused"] == true { "*" } else { " " },
                    r["id"].as_str().unwrap_or_default(),
                    r["status"].as_str().unwrap_or("-"),
                    r["space"].as_str().unwrap_or_default(),
                    r["label"].as_str().unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Opens a terminal beside the focused tab, focusing it.
fn open_focused(m: &mut Model, place: Place) -> TermId {
    match place {
        Place::Tab => m.new_tab(),
        Place::Right => m.split_split(Axis::Row),
        Place::Down => m.split_split(Axis::Column),
        Place::SpaceRight => m.split_slot_with_space(Axis::Row),
        Place::SpaceDown => m.split_slot_with_space(Axis::Column),
        Place::Layout => m.add_space(),
    }
}

/// Opens a terminal beside `at`, leaving focus where it is.
fn open_at(m: &mut Model, at: Tab, place: Place) -> Option<TermId> {
    match place {
        Place::Tab => m.new_tab_at(at),
        Place::Right => m.split_split_at(at, Axis::Row),
        Place::Down => m.split_split_at(at, Axis::Column),
        Place::SpaceRight => m.split_slot_at(at, Axis::Row),
        Place::SpaceDown => m.split_slot_at(at, Axis::Column),
        Place::Layout => Some(m.add_space_behind()),
    }
}
