//! The window: sidebar of layouts, and the active layout's spaces, splits and tabs.

pub mod actions;
mod labels;
mod panes;
mod rename;
mod sidebar;
pub mod theme;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui::{
    App, Context, Entity, Focusable, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    Render, Styled, Subscription, Window, div, rgb,
};

use crate::model::{Axis, Dir, Model};
use crate::procinfo;
use crate::terminal::{FontSize, TermEvent, TermView};
use actions::*;
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
}

impl Workspace {
    pub fn new(cwd: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (model, first) = Model::new();
        let mut this = Workspace {
            model,
            terms: HashMap::new(),
            sidebar: true,
            collapsed: HashSet::new(),
            rename: None,
            home: dirs::home_dir(),
        };
        this.spawn(first, &cwd, window, cx);
        this.focus_current(window, cx);
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                if this.update(cx, |w, cx| w.poll(cx)).is_err() {
                    return;
                }
            }
        })
        .detach();
        this
    }

    fn spawn(&mut self, id: u64, cwd: &Path, window: &mut Window, cx: &mut Context<Self>) {
        let view = match TermView::spawn(id, cwd, cx) {
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
        });
        self.terms.insert(
            id,
            Term {
                view,
                info: Info::default(),
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

    fn poll(&mut self, cx: &mut Context<Self>) {
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
    }

    fn focus_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_rename(true);
        if let Some(t) = self.terms.get(&self.model.term()) {
            let handle = t.view.read(cx).focus_handle(cx);
            window.focus(&handle, cx);
        }
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
            self.spawn(new, &cwd, window, cx);
        }
        self.focus_current(window, cx);
    }

    fn close(&mut self, term: u64, window: &mut Window, cx: &mut Context<Self>) {
        self.terms.remove(&term);
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
        let Some(t) = self.terms.get(&term) else {
            return String::new();
        };
        let title = t.view.read(cx).title.as_str();
        match t.info.process.as_deref() {
            Some(p) if labels::is_agent(p) => labels::clean_title(title).unwrap_or(p).to_owned(),
            Some(p) => p.to_owned(),
            None => labels::clean_title(title).unwrap_or("shell").to_owned(),
        }
    }

    fn is_agent(&self, term: u64) -> bool {
        self.terms
            .get(&term)
            .and_then(|t| t.info.process.as_deref())
            .is_some_and(labels::is_agent)
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
        cx.set_global(FontSize(size.clamp(8., 32.)));
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

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("workspace")
            .key_context("Workspace")
            .size_full()
            .flex()
            .flex_row()
            .bg(rgb(theme::WINDOW))
            .font_family(theme::UI_FONT)
            .on_key_down(cx.listener(Self::key_down))
            .on_action(
                cx.listener(|w, _: &NewTab, win, cx| w.apply(win, cx, |m| Some(m.new_tab()))),
            )
            .on_action(cx.listener(|w, _: &CloseTab, win, cx| w.close(w.model.term(), win, cx)))
            .on_action(cx.listener(|w, _: &NextTab, win, cx| {
                w.apply(win, cx, |m| {
                    m.cycle_tab(1);
                    None
                })
            }))
            .on_action(cx.listener(|w, _: &PrevTab, win, cx| {
                w.apply(win, cx, |m| {
                    m.cycle_tab(-1);
                    None
                })
            }))
            .on_action(cx.listener(|w, _: &SplitRight, win, cx| {
                w.apply(win, cx, |m| Some(m.split_split(Axis::Row)))
            }))
            .on_action(cx.listener(|w, _: &SplitDown, win, cx| {
                w.apply(win, cx, |m| Some(m.split_split(Axis::Column)))
            }))
            .on_action(cx.listener(|w, _: &SplitSpaceRight, win, cx| {
                w.apply(win, cx, |m| Some(m.split_space(Axis::Row)))
            }))
            .on_action(cx.listener(|w, _: &SplitSpaceDown, win, cx| {
                w.apply(win, cx, |m| Some(m.split_space(Axis::Column)))
            }))
            .on_action(cx.listener(|w, _: &CloseSplit, win, cx| w.close_split(win, cx)))
            .on_action(cx.listener(|w, _: &ZoomSplit, win, cx| {
                w.apply(win, cx, |m| {
                    m.toggle_zoom();
                    None
                })
            }))
            .on_action(cx.listener(|w, _: &NextSplit, win, cx| {
                w.apply(win, cx, |m| {
                    m.cycle_split(1);
                    None
                })
            }))
            .on_action(cx.listener(|w, _: &PrevSplit, win, cx| {
                w.apply(win, cx, |m| {
                    m.cycle_split(-1);
                    None
                })
            }))
            .on_action(cx.listener(|w, _: &FocusLeft, win, cx| {
                w.apply(win, cx, |m| {
                    m.focus_dir(Dir::Left);
                    None
                })
            }))
            .on_action(cx.listener(|w, _: &FocusRight, win, cx| {
                w.apply(win, cx, |m| {
                    m.focus_dir(Dir::Right);
                    None
                })
            }))
            .on_action(cx.listener(|w, _: &FocusUp, win, cx| {
                w.apply(win, cx, |m| {
                    m.focus_dir(Dir::Up);
                    None
                })
            }))
            .on_action(cx.listener(|w, _: &FocusDown, win, cx| {
                w.apply(win, cx, |m| {
                    m.focus_dir(Dir::Down);
                    None
                })
            }))
            .on_action(
                cx.listener(|w, _: &NewLayout, win, cx| w.apply(win, cx, |m| Some(m.new_layout()))),
            )
            .on_action(cx.listener(|w, _: &NextLayout, win, cx| {
                w.apply(win, cx, |m| {
                    m.cycle_layout(1);
                    None
                })
            }))
            .on_action(cx.listener(|w, _: &PrevLayout, win, cx| {
                w.apply(win, cx, |m| {
                    m.cycle_layout(-1);
                    None
                })
            }))
            .on_action(cx.listener(|w, _: &ToggleSidebar, _, cx| {
                w.sidebar = !w.sidebar;
                cx.notify();
            }))
            .on_action(cx.listener(|w, _: &FontBigger, _, cx| {
                w.set_font_size(cx.global::<FontSize>().0 + 1., cx)
            }))
            .on_action(cx.listener(|w, _: &FontSmaller, _, cx| {
                w.set_font_size(cx.global::<FontSize>().0 - 1., cx)
            }))
            .on_action(
                cx.listener(|w, _: &FontReset, _, cx| {
                    w.set_font_size(theme::DEFAULT_FONT_SIZE, cx)
                }),
            )
            .when(self.sidebar, |d| d.child(self.render_sidebar(window, cx)))
            .child(self.render_main(window, cx))
    }
}

use gpui::prelude::FluentBuilder;
