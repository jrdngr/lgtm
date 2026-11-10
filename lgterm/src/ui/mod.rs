//! The window: sidebar of layouts, and the active layout's spaces, splits and tabs.

pub mod actions;
mod agents;
mod control;
mod dispatch;
pub mod field;
mod labels;
mod marks;
mod messages;
mod palette;
mod panes;
mod rename;
mod restore;
mod settings_view;
mod sidebar;
pub mod theme;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui::{App, Context, Entity, FocusHandle, Focusable, KeyDownEvent, Subscription, Window};

use crate::ipc::{self, Place, Reply};
use crate::model::Model;
use crate::procinfo;
use crate::settings::Settings;
use crate::state::State;
use crate::terminal::links::Link;
use crate::terminal::{FontSize, TermEvent, TermView};
use agents::Agent;
pub use messages::Message;
use rename::Rename;

/// What polling found a terminal running.
#[derive(Clone, Debug, Default, PartialEq)]
struct Info {
    process: Option<String>,
    cwd: Option<PathBuf>,
}

struct Term {
    view: Entity<TermView>,
    info: Info,
    agent: Option<Agent>,
    _sub: Subscription,
}

pub struct Workspace {
    model: Model,
    terms: HashMap<u64, Term>,
    sidebar: bool,
    /// Layouts and spaces folded in the sidebar.
    collapsed: HashSet<u64>,
    rename: Option<Rename>,
    home: Option<PathBuf>,
    settings: Settings,
    messages: Vec<Message>,
    /// The Messages tab, if one is open.
    messages_tab: Option<u64>,
    messages_focus: FocusHandle,
    /// `lgterm wait` calls, by terminal.
    waiters: Vec<(u64, async_channel::Sender<Reply>)>,
    /// Agents on the Dock badge.
    badge: usize,
    palette: Option<palette::Palette>,
    /// The Settings dialog's focus, while it's open.
    settings_open: Option<FocusHandle>,
    /// The state last written to disk.
    saved: Option<Vec<u8>>,
    _server: Option<ipc::Server>,
}

impl Workspace {
    pub fn new(cwd: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings = Settings::load();
        cx.set_global(FontSize(settings.font_size));
        let (model, first) = Model::new();
        let mut this = Workspace {
            model,
            terms: HashMap::new(),
            sidebar: true,
            collapsed: HashSet::new(),
            rename: None,
            home: dirs::home_dir(),
            settings,
            messages: Vec::new(),
            messages_tab: None,
            messages_focus: cx.focus_handle(),
            waiters: Vec::new(),
            badge: 0,
            palette: None,
            settings_open: None,
            saved: None,
            _server: None,
        };
        this.listen(window, cx);
        match State::load().filter(|_| this.settings.restore) {
            Some(state) => this.restore(state, window, cx),
            None => this.spawn(first, &cwd, &[], window, cx),
        }
        this.focus_current(window, cx);
        this.start_timers(window, cx);
        this.start_saving(window, cx);
        this
    }

    /// Serves the control socket and answers notification clicks.
    fn listen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let path = ipc::socket_path();
        match ipc::serve(&path) {
            Ok((server, requests)) => {
                let _ = crate::terminal::SOCKET.set(path);
                self._server = Some(server);
                cx.spawn_in(window, async move |this, cx| {
                    while let Ok(inc) = requests.recv().await {
                        if this
                            .update_in(cx, |w, window, cx| w.handle(inc, window, cx))
                            .is_err()
                        {
                            return;
                        }
                    }
                })
                .detach();
            }
            Err(err) => eprintln!("lgterm: no control socket: {err:#}"),
        }
        let this = cx.entity().downgrade();
        let handle = window.window_handle();
        cx.on_system_notification_response(move |response, cx| {
            let Some(term) = ipc::parse_term(&response.tag) else {
                return;
            };
            let this = this.clone();
            let _ = handle.update(cx, |_, window, cx| {
                window.activate_window();
                let _ = this.update(cx, |w, cx| {
                    w.model.focus_term(term);
                    w.focus_current(window, cx);
                });
            });
            cx.activate(true);
        });
    }

    /// Polls terminals every second, and spins working marks.
    fn start_timers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                if this
                    .update_in(cx, |w, window, cx| w.poll(window, cx))
                    .is_err()
                {
                    return;
                }
            }
        })
        .detach();
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                let alive = this.update(cx, |w, cx| {
                    if w.any_working() {
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    return;
                }
            }
        })
        .detach();
    }

    fn spawn(
        &mut self,
        id: u64,
        cwd: &Path,
        argv: &[String],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let view = match TermView::spawn(id, cwd, argv, cx) {
            Ok(view) => view,
            Err(err) => {
                eprintln!("lgterm: can't start a shell: {err:#}");
                self.close(id, window, cx);
                return;
            }
        };
        let sub = cx.subscribe_in(&view, window, move |this, _, ev, window, cx| match ev {
            TermEvent::Exited => this.close(id, window, cx),
            TermEvent::Changed => cx.notify(),
            TermEvent::Link(link) => this.open_link(id, link, window, cx),
        });
        self.terms.insert(
            id,
            Term {
                view,
                info: Info::default(),
                agent: None,
                _sub: sub,
            },
        );
    }

    /// The folder new terminals beside `term` start in.
    fn cwd_of(&self, term: u64, cx: &App) -> PathBuf {
        let Some(t) = self.terms.get(&term) else {
            return self.home.clone().unwrap_or_else(|| PathBuf::from("/"));
        };
        t.info
            .cwd
            .clone()
            .unwrap_or_else(|| t.view.read(cx).known_cwd().to_path_buf())
    }

    fn poll(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut changed = false;
        for term in self.terms.values_mut() {
            let pid = term.view.read(cx).foreground_pid();
            let info = Info {
                process: pid.and_then(procinfo::name),
                cwd: pid.and_then(procinfo::cwd),
            };
            if info != term.info {
                term.info = info;
                changed = true;
            }
        }
        if changed {
            cx.notify();
        }
        self.poll_agents(window, cx);
    }

    fn focus_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_rename(true);
        let term = self.model.term();
        if self.messages_tab == Some(term) {
            window.focus(&self.messages_focus, cx);
        } else if let Some(t) = self.terms.get(&term) {
            let handle = t.view.read(cx).focus_handle(cx);
            window.focus(&handle, cx);
        }
        self.clear_viewed(window, cx);
        cx.notify();
    }

    /// Runs a model change that may open a terminal, then focuses whatever is focused now.
    fn apply(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut Model) -> Option<u64>,
    ) {
        let cwd = self.cwd_of(self.model.term(), cx);
        if let Some(new) = change(&mut self.model) {
            self.spawn(new, &cwd, &[], window, cx);
        }
        self.focus_current(window, cx);
    }

    fn close(&mut self, term: u64, window: &mut Window, cx: &mut Context<Self>) {
        if self.terms.remove(&term).is_some() {
            self.finish_waits(term, "exited");
            self.update_badge();
        }
        if self.messages_tab == Some(term) {
            self.messages_tab = None;
        }
        if !self.model.remove_term(term) {
            cx.quit();
            return;
        }
        self.focus_current(window, cx);
    }

    fn close_split(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for term in self.model.split_terms() {
            self.close(term, window, cx);
        }
    }

    fn term_label(&self, term: u64, cx: &App) -> String {
        if self.messages_tab == Some(term) {
            return "Messages".into();
        }
        let Some(t) = self.terms.get(&term) else {
            return String::new();
        };
        let title = t.view.read(cx).title.as_str();
        if t.agent.is_some() {
            let name = t.info.process.as_deref().unwrap_or("agent");
            return labels::clean_title(title).unwrap_or(name).to_owned();
        }
        match t.info.process.as_deref() {
            Some(p) => p.to_owned(),
            None => labels::clean_title(title).unwrap_or("shell").to_owned(),
        }
    }

    fn is_agent(&self, term: u64) -> bool {
        self.terms.get(&term).is_some_and(|t| t.agent.is_some())
    }

    /// Opens a URL in the browser, or a file at its line in `lgtm` in a split beside `term`.
    fn open_link(&mut self, term: u64, link: &Link, window: &mut Window, cx: &mut Context<Self>) {
        match link {
            Link::Url(url) => cx.open_url(url),
            Link::File(path, line) => {
                let cwd = self.cwd_of(term, cx);
                let path = match (path.strip_prefix("~/"), &self.home) {
                    (Some(rest), Some(home)) => home.join(rest),
                    _ => cwd.join(path),
                };
                if !path.is_file() {
                    return;
                }
                let argv = [
                    "lgtm".to_owned(),
                    "--split".to_owned(),
                    format!("{}:{}", path.display(), line.unwrap_or(1)),
                ];
                self.open(Some(term), Place::Right, Some(cwd), &argv, true, window, cx);
            }
        }
    }

    /// Opens the Messages tab beside the focused terminal, or shows it if it's open.
    fn show_messages(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.messages_tab {
            Some(tab) => self.model.focus_term(tab),
            None => self.messages_tab = Some(self.model.new_tab()),
        }
        self.focus_current(window, cx);
    }

    fn space_label(&self, space: &crate::model::Space, cx: &App) -> String {
        if let Some(name) = &space.name {
            return name.clone();
        }
        let cwd = self.cwd_of(space.first_term(), cx);
        procinfo::space_name(&cwd, self.home.as_deref())
    }

    fn layout_label(&self, layout: &crate::model::Layout, cx: &App) -> String {
        match &layout.name {
            Some(name) => name.clone(),
            None => self.space_label(layout.spaces.first(), cx),
        }
    }

    fn set_font_size(&mut self, size: f32, cx: &mut Context<Self>) {
        let size = size.clamp(8., 32.);
        cx.set_global(FontSize(size));
        self.settings.font_size = size;
        self.settings.save();
        for t in self.terms.values() {
            t.view.update(cx, |_, cx| cx.notify());
        }
        cx.notify();
    }

    fn key_down(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = &ev.keystroke;
        let m = k.modifiers;
        if m.platform
            && !m.alt
            && !m.control
            && !m.shift
            && let Some(d) = k.key.parse::<usize>().ok().filter(|d| (1..=9).contains(d))
        {
            self.apply(window, cx, |m| {
                m.select_tab(d - 1);
                None
            });
            cx.stop_propagation();
        }
    }
}
