//! Requests from the `lgterm` command in the app's terminals.

use std::path::PathBuf;

use gpui::{Context, Window};
use serde_json::json;

use super::Workspace;
use crate::ipc::{Incoming, Place, Reply, Request, parse_term};
use crate::model::Axis;

impl Workspace {
    pub(super) fn handle(&mut self, inc: Incoming, window: &mut Window, cx: &mut Context<Self>) {
        let reply = match inc.request {
            Request::Wait { term } => match self.term_arg(&term) {
                Ok(id) if !self.terms.contains_key(&id) => {
                    Reply::Err(format!("no terminal {term}"))
                }
                Ok(id) => match self.status(id) {
                    Some(s @ (crate::status::Status::Done | crate::status::Status::Blocked))
                        if !self.messages.iter().any(|m| m.queued && m.to == id) =>
                    {
                        Reply::Ok(s.name().into())
                    }
                    _ => {
                        self.waiters.retain(|(_, r)| !r.is_closed());
                        self.waiters.push((id, inc.reply));
                        return;
                    }
                },
                Err(e) => e,
            },
            request => self.reply(request, window, cx),
        };
        let _ = inc.reply.try_send(reply);
    }

    fn term_arg(&self, term: &str) -> Result<u64, Reply> {
        parse_term(term)
            .filter(|id| self.terms.contains_key(id) || self.messages_tab == Some(*id))
            .ok_or_else(|| Reply::Err(format!("no terminal {term}")))
    }

    fn reply(&mut self, request: Request, window: &mut Window, cx: &mut Context<Self>) -> Reply {
        let result = (|| -> Result<Reply, Reply> {
            Ok(match request {
                Request::Hook { term, event } => {
                    let id = self.term_arg(&term)?;
                    self.hook(id, &event, window, cx)
                }
                Request::Ls { json } => Reply::Ok(self.list(json, cx)),
                Request::Open {
                    from,
                    place,
                    cwd,
                    argv,
                    focus,
                } => {
                    let from = match from {
                        Some(f) => Some(self.term_arg(&f)?),
                        None => None,
                    };
                    match self.open(from, place, cwd, &argv, focus, window, cx) {
                        Ok(id) => Reply::Ok(format!("t{id}")),
                        Err(err) => Reply::Err(err),
                    }
                }
                Request::Send { from, to, text } => {
                    let to = self.term_arg(&to)?;
                    let from = from.and_then(|f| parse_term(&f));
                    self.send_message(from, to, text, cx)
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
                Request::Close { term } => {
                    let id = self.term_arg(&term)?;
                    self.close(id, window, cx);
                    Reply::Ok(String::new())
                }
                Request::Wait { .. } => unreachable!("waits are handled before"),
            })
        })();
        result.unwrap_or_else(|e| e)
    }

    /// Opens a terminal beside `from`, or the focused terminal, and returns its id. Without
    /// `focus`, focus and zoom stay as they are.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn open(
        &mut self,
        from: Option<u64>,
        place: Place,
        cwd: Option<PathBuf>,
        argv: &[String],
        focus: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<u64, String> {
        let from = from.unwrap_or_else(|| self.model.term());
        let cwd = cwd.unwrap_or_else(|| self.cwd_of(from, cx));
        let m = &mut self.model;
        let id = if focus {
            m.focus_term(from);
            Some(match place {
                Place::Tab => m.new_tab(),
                Place::Right => m.split_split(Axis::Row),
                Place::Down => m.split_split(Axis::Column),
                Place::SpaceRight => m.split_space(Axis::Row),
                Place::SpaceDown => m.split_space(Axis::Column),
                Place::Layout => m.new_layout(),
            })
        } else {
            match place {
                Place::Tab => m.new_tab_at(from),
                Place::Right => m.split_split_at(from, Axis::Row),
                Place::Down => m.split_split_at(from, Axis::Column),
                Place::SpaceRight => m.split_space_at(from, Axis::Row),
                Place::SpaceDown => m.split_space_at(from, Axis::Column),
                Place::Layout => {
                    let active = m.active;
                    let id = m.new_layout();
                    m.active = active;
                    Some(id)
                }
            }
        };
        let id = id.ok_or_else(|| format!("no terminal t{from}"))?;
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

    fn list(&self, as_json: bool, cx: &mut Context<Self>) -> String {
        let focused = self.model.term();
        let mut rows = Vec::new();
        for layout in &self.model.layouts {
            let layout_label = self.layout_label(layout, cx);
            for space in layout.spaces.leaves() {
                let space_label = self.space_label(space, cx);
                for term in space.terms() {
                    if self.messages_tab == Some(term) {
                        continue;
                    }
                    rows.push(json!({
                        "id": format!("t{term}"),
                        "label": self.term_label(term, cx),
                        "status": self.status(term).map(|s| s.name()),
                        "layout": layout_label,
                        "space": space_label,
                        "cwd": self.cwd_of(term, cx),
                        "focused": term == focused,
                    }));
                }
            }
        }
        if as_json {
            return serde_json::to_string_pretty(&rows).unwrap_or_default();
        }
        rows.iter()
            .map(|r| {
                format!(
                    "{}{:<5} {:<8} {} › {} · {}",
                    if r["focused"] == true { "*" } else { " " },
                    r["id"].as_str().unwrap_or_default(),
                    r["status"].as_str().unwrap_or("-"),
                    r["layout"].as_str().unwrap_or_default(),
                    r["space"].as_str().unwrap_or_default(),
                    r["label"].as_str().unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
