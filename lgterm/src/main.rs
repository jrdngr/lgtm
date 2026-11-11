mod agents;
mod assets;
mod cli;
mod dock;
mod field;
mod fonts;
mod guide;
mod input;
mod ipc;
mod model;
mod procinfo;
mod settings;
mod state;
mod terminal;
mod theme;
mod ui;
mod window_place;

use std::path::PathBuf;

use gpui::{App, AppContext, Menu, MenuItem, TitlebarOptions, WindowOptions, point, px};

use terminal::FontSize;
use ui::Workspace;
use ui::actions::{self, OpenSettings, Quit};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if cli::is_command(&args) {
        std::process::exit(cli::run(&args));
    }
    let cwd = std::env::current_dir()
        .ok()
        .filter(|d| d.parent().is_some())
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("/"));
    if let Some(home) = dirs::home_dir() {
        guide::install_skill(&home);
    }
    let app = gpui_platform::application().with_assets(assets::Assets);
    app.on_reopen(|cx| cx.activate(true));
    app.run(move |cx: &mut App| {
        cx.set_global(FontSize(theme::DEFAULT_FONT_SIZE));
        cx.set_app_identity("dev.lgtm.lgterm", "lgterm");
        actions::bind(cx);
        input::bind(cx);
        cx.on_action(|_: &Quit, cx| cx.quit());
        fonts::load(cx);
        cx.set_menus([Menu::new("lgterm").items([
            MenuItem::action("Settings…", OpenSettings),
            MenuItem::separator(),
            MenuItem::action("Quit lgterm", Quit),
        ])]);
        let (bounds, display_id) = window_place::load(cx);
        let opened = cx.open_window(
            WindowOptions {
                window_bounds: Some(bounds),
                display_id,
                titlebar: Some(TitlebarOptions {
                    title: Some("lgterm".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(16.), px(17.))),
                }),
                app_owns_titlebar_drag: true,
                ..Default::default()
            },
            |window, cx| cx.new(|cx| Workspace::new(cwd, window, cx)),
        );
        if let Err(err) = opened {
            eprintln!("lgterm: can't open a window: {err:#}");
            cx.quit();
        }
        cx.on_window_closed(|cx, _| cx.quit()).detach();
        cx.activate(true);
    });
}
