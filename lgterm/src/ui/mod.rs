//! The window: sidebar of layouts, and the active layout's spaces, splits and tabs.

pub mod actions;
mod agents;
mod badge;
mod control;
mod dispatch;
mod marks;
mod menu;
mod messages;
mod overlay;
mod palette;
mod panes;
mod picker;
mod rename;
mod restore;
mod settings_view;
mod sidebar;
mod space_drag;
mod terms;
mod toasts;

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;

use gpui::{Context, FocusHandle, Focusable, KeyDownEvent, Window};

use crate::agents::Agents;
use crate::fonts::{self, Fonts};
use crate::ipc::{self, Place};
use crate::model::{Model, Tab, TermId};
use crate::procinfo;
use crate::settings::Settings;
use crate::state::State;
use crate::terminal::links::Link;
use crate::terminal::{FontSize, SpawnEnv};
use crate::theme;
use overlay::Overlay;
use terms::Term;

/// How often terminals are checked for what they run.
const POLL: Duration = Duration::from_secs(1);
/// How often a spinner turns.
const SPIN: Duration = Duration::from_millis(80);

pub struct Workspace {
    model: Model,
    terms: HashMap<TermId, Term>,
    agents: Agents,
    sidebar: bool,
    sidebar_width: f32,
    /// Layouts and spaces folded in the sidebar, by id.
    collapsed: HashSet<u64>,
    overlay: Option<Overlay>,
    toasts: Vec<toasts::Toast>,
    toast_id: u64,
    space_drop: space_drag::SpaceDrop,
    home: Option<PathBuf>,
    /// What an unnamed space with no terminal is called.
    home_name: String,
    settings: Settings,
    messages_focus: FocusHandle,
    /// Has the keyboard while an empty slot is focused.
    root_focus: FocusHandle,
    /// The terminal last given focus, told when it loses it.
    focused: Option<TermId>,
    /// Agents on the Dock badge.
    badge: usize,
    spawn_env: SpawnEnv,
    /// The last frame drew a spinner.
    spinning: Cell<bool>,
    /// Something changed since the state was last saved.
    dirty: bool,
    /// The state last written to disk.
    saved: Option<Vec<u8>>,
    _server: Option<ipc::Server>,
}

impl Workspace {
    pub fn new(cwd: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings = Settings::load();
        cx.set_global(FontSize(settings.font_size));
        fonts::set(settings.fonts);
        let (model, first) = Model::new();
        let home = dirs::home_dir();
        let home_name =
            procinfo::space_name(home.as_deref().unwrap_or("/".as_ref()), home.as_deref());
        let mut this = Workspace {
            model,
            terms: HashMap::new(),
            agents: Agents::default(),
            sidebar: true,
            sidebar_width: theme::SIDEBAR_WIDTH,
            collapsed: HashSet::new(),
            overlay: None,
            toasts: Vec::new(),
            toast_id: 0,
            space_drop: Default::default(),
            home,
            home_name,
            settings,
            messages_focus: cx.focus_handle(),
            root_focus: cx.focus_handle(),
            focused: None,
            badge: 0,
            spawn_env: SpawnEnv {
                socket: None,
                bin: std::env::current_exe().ok(),
            },
            spinning: Cell::new(false),
            dirty: false,
            saved: None,
            _server: None,
        };
        this.listen(window, cx);
        match State::load().filter(|_| this.settings.restore) {
            Some(state) => this.restore(state, window, cx),
            None => {
                let _ = this.spawn(first, &cwd, &[], window, cx);
            }
        }
        this.focus_current(window, cx);
        this.start_timers(window, cx);
        this.start_saving(window, cx);
        this.keep_running_on_close(window, cx);
        this
    }

    /// Closing the window hides the app on macOS, so terminals keep running until Quit. The
    /// window's place is saved on close, on quit, and as it moves.
    fn keep_running_on_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.observe_window_bounds(window, |_, window, _| {
            crate::window_place::save(window.window_bounds())
        })
        .detach();
        window.on_window_should_close(cx, |window, cx| {
            crate::window_place::save(window.window_bounds());
            if cfg!(target_os = "macos") {
                cx.hide();
                false
            } else {
                true
            }
        });
    }

    /// Serves the control socket and answers notification clicks.
    fn listen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let path = ipc::socket_path();
        match ipc::serve(&path) {
            Ok((server, requests)) => {
                self.spawn_env.socket = Some(path);
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
            let Some(term) = TermId::parse(&response.tag) else {
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

    /// Polls terminals every second, and turns spinners while one shows.
    fn start_timers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL).await;
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
                cx.background_executor().timer(SPIN).await;
                let alive = this.update(cx, |w, cx| {
                    if w.spinning.get() {
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

    fn focus_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_rename(true);
        let now = self.model.term();
        if now != self.focused {
            // Terminal views draw again only when told, and both show focus in their cursor.
            for t in [self.focused, now].into_iter().flatten() {
                if let Some(t) = self.terms.get(&t) {
                    t.view.update(cx, |_, cx| cx.notify());
                }
            }
            self.focused = now;
        }
        match now.and_then(|t| self.terms.get(&t)) {
            Some(t) => {
                let handle = t.view.read(cx).focus_handle(cx);
                window.focus(&handle, cx);
            }
            None if self.model.tab().is_some() => window.focus(&self.messages_focus, cx),
            None => window.focus(&self.root_focus, cx),
        }
        self.with_agents(window, cx, |a, env| a.settle(env));
        cx.notify();
    }

    /// Runs a model change, then focuses whatever is focused now.
    fn apply(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut Model),
    ) {
        change(&mut self.model);
        self.focus_current(window, cx);
    }

    fn close(&mut self, tab: Tab, window: &mut Window, cx: &mut Context<Self>) {
        if let Tab::Term(term) = tab {
            self.terms.remove(&term);
            self.agents.closed(term);
            self.run_effects(cx);
        }
        let was_focused = self.model.tab() == Some(tab);
        if !self.model.remove_tab(tab) {
            cx.quit();
            return;
        }
        if was_focused && !self.overlay_has_keyboard() {
            self.focus_current(window, cx);
        } else {
            cx.notify();
        }
    }

    /// Closes the focused split's tabs, or the focused slot when it's empty.
    fn close_split(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.model.tab().is_none() {
            self.apply(window, cx, |m| {
                m.close_slot();
            });
            return;
        }
        for tab in self.model.split_tabs() {
            self.close(tab, window, cx);
        }
    }

    /// Opens a URL in the browser, or a file at its line in `lgtm` in a split beside `term`.
    fn open_link(
        &mut self,
        term: TermId,
        link: &Link,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match link {
            Link::Url(url) => cx.open_url(url),
            Link::File(path, line) => {
                let cwd = self.cwd_of(Tab::Term(term));
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
                let from = Some(Tab::Term(term));
                let _ = self.open(from, Place::Right, Some(cwd), &argv, true, window, cx);
            }
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

    fn set_fonts(&mut self, fonts: Fonts, cx: &mut Context<Self>) {
        fonts::set(fonts);
        self.settings.fonts = fonts;
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
            self.apply(window, cx, |m| m.select_tab(d - 1));
            cx.stop_propagation();
        }
    }
}

/// Empty titlebar space: dragging it moves the window, and a double click does what the
/// system's titlebar double click setting says.
trait MovesWindow {
    fn moves_window(self) -> Self;
}

impl MovesWindow for gpui::Stateful<gpui::Div> {
    fn moves_window(self) -> Self {
        use gpui::InteractiveElement;
        self.on_mouse_down(gpui::MouseButton::Left, |ev, window, _| {
            if ev.click_count == 2 {
                window.titlebar_double_click();
            } else {
                window.start_window_move();
            }
        })
    }
}
