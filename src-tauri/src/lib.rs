mod agent;
pub mod anchor;
mod bots;
mod commands;
mod config;
pub mod diff;
mod dragged;
mod events;
mod files;
mod github;
mod html;
mod gitmeta;
mod jj;
mod layout;
mod mcp;
mod menu;
mod placement;
mod par;
mod pr;
mod prompts;
mod pty;
mod qa;
#[cfg(target_os = "macos")]
mod quit;
mod recent;
mod repo;
mod review;
mod settings;
mod store;
mod syntax;
mod tools;
mod watch;
mod workspace;
mod workspaces;
mod zoom;

use std::path::Path;

use tauri::{Manager, RunEvent, WindowEvent};
use tauri_specta::{collect_commands, collect_events};

use agent::Runs;
use placement::Placements;
use html::HtmlRoots;
use pty::Ptys;
use repo::RepoWindows;

fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::new()
        .constant("HIGHLIGHT_NAMES", syntax::HIGHLIGHT_NAMES)
        .commands(collect_commands![
            commands::repo::current_repo,
            commands::repo::check_tools,
            commands::repo::open_repo,
            commands::repo::git_root,
            commands::repo::init_jj,
            commands::repo::recent_repos,
            commands::repo::switch_repo,
            commands::repo::close_window,
            commands::repo::keep_window,
            commands::repo::set_word_wrap,
            commands::repo::load_layout,
            commands::html::html_root,
            commands::html::html_changed,
            commands::repo::save_layout,
            commands::pty::pty_spawn,
            commands::pty::pty_write,
            commands::pty::pty_resize,
            commands::pty::pty_kill,
            commands::pty::pty_running,
            commands::workspaces::list_workspaces,
            commands::workspaces::create_workspace,
            commands::workspaces::delete_workspace,
            commands::review::open_review,
            commands::review::close_review,
            commands::review::context_lines,
            commands::review::refresh_review,
            commands::review::log_graph,
            commands::review::trunk_bookmark,
            commands::review::revset_commits,
            commands::review::jj_act,
            commands::review::mark_rows,
            commands::review::clear_file_marks,
            commands::review::set_files_checked,
            commands::review::undo_marks,
            commands::review::redo_marks,
            commands::review::reset_review_marks,
            commands::review::review_graph,
            commands::arch::review_arch,
            commands::arch::arch_history,
            commands::arch::arch_lens,
            commands::arch::add_intent,
            commands::arch::remove_intent,
            commands::arch::check_look,
            commands::arch::intent_prompt,
            commands::arch::run_assess,
            commands::arch::stop_assess,
            commands::arch::fix_intents,
            commands::arch::stop_intent_fix,
            commands::review::focus_review,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::settings_load_error,
            commands::settings::claude_command,
            commands::settings::tool_status,
            commands::settings::prompt_files,
            commands::settings::edit_prompt,
            commands::settings::remove_prompt,
            commands::repo::mcp_status,
            commands::repo::register_mcp,
            commands::notes::walkthrough_prompt,
            commands::comments::add_comment,
            commands::comments::set_thread_stage,
            commands::comments::reset_thread,
            commands::comments::set_note_done,
            commands::comments::draft_from_note,
            commands::comments::edit_comment,
            commands::comments::add_quote,
            commands::comments::delete_comment,
            commands::comments::add_chat_message,
            commands::comments::take_suggestion,
            commands::comments::triage_threads,
            commands::comments::stop_thread_run,
            commands::bots::delete_bots,
            commands::bots::bot_runs,
            commands::bots::bot_run,
            commands::bots::rename_report,
            commands::bots::dismiss_findings,
            commands::files::list_files,
            commands::files::read_file,
            commands::files::read_image,
            commands::files::save_file,
            commands::files::create_path,
            commands::files::add_files,
            commands::files::drag_out,
            commands::files::rename_path,
            commands::files::trash_path,
            commands::files::reveal_path,
            commands::files::dropped_paths,
            commands::files::highlight_text,
            commands::github::open_pr,
            commands::github::pr_info,
            commands::github::open_pr_page,
            commands::github::open_url,
            commands::github::nearby_prs,
            commands::github::sync_stack_pr,
            commands::github::edit_back_to_stack_pr,
            commands::github::select_stack_pr,
            commands::github::clear_stack_pr,
            commands::github::pr_preview,
            commands::github::pr_head,
            commands::github::refetch_pr,
            commands::github::pull_pr_comments,
            commands::github::send_threads,
            commands::github::send_thread,
            commands::github::pr_discussion,
            commands::github::send_combined,
            commands::qa::list_qa,
            commands::qa::create_qa,
            commands::qa::edit_qa,
            commands::qa::delete_qa,
            commands::qa::act_on_question,
            commands::qa::apply_answer,
            commands::qa::discuss_question,
            commands::qa::reask_question,
            commands::qa::defer_preview,
            commands::qa::stop_qa,
        ])
        .events(collect_events![
            events::RepoChanged,
            events::WorkingCopyMoved,
            events::SwitchRepo,
            events::WordWrap,
            events::SettingsChanged,
            events::OpenSettings,
            events::OpenShortcuts,
            events::WorkingCopyStale,
            events::BotRunChanged,
            events::AgentShow,
            events::AgentRevset,
            events::ReviewStateChanged,
            events::QaChanged,
            events::TerminalAgent
        ])
}

#[cfg(any(debug_assertions, test))]
const BINDINGS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/bindings.ts");

#[cfg(any(debug_assertions, test))]
fn export_bindings(builder: &tauri_specta::Builder<tauri::Wry>, path: &str) {
    builder
        .export(specta_typescript::Typescript::default(), path)
        .expect("export TypeScript bindings");
}

/// Apps opened from Finder get a bare PATH without Homebrew or ~/.cargo/bin, so jj and the
/// user's tools wouldn't resolve. Adopt the PATH their login shell sets up.
fn adopt_login_shell_path() {
    let Ok(out) = std::process::Command::new(pty::default_shell())
        .args(["-l", "-i", "-c", "printf '\\n__LGTM_PATH__%s' \"$PATH\""])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
    else {
        return;
    };
    let out = String::from_utf8_lossy(&out.stdout);
    if let Some(path) = out.rsplit_once("__LGTM_PATH__").map(|(_, p)| p.trim())
        && !path.is_empty()
    {
        // SAFETY: runs during startup before any other thread reads the environment.
        unsafe { std::env::set_var("PATH", path) };
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Claude Code starts this, inside a terminal that already has the user's PATH.
    if args.first().is_some_and(|a| a == "mcp") {
        std::process::exit(mcp::bridge_main());
    }
    if args.first().is_some_and(|a| a == "hook") {
        std::process::exit(mcp::hook::main());
    }
    adopt_login_shell_path();
    settings::load();
    if args.first().is_some_and(|a| a == "gc") {
        std::process::exit(workspaces::gc::main(&args[1..]));
    }
    let builder = specta_builder();
    #[cfg(debug_assertions)]
    export_bindings(&builder, BINDINGS_PATH);

    tauri::Builder::default()
        // Must be registered first. Later launches hand their arguments to this process.
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            repo::open_from_args(app, argv.get(1..).unwrap_or_default(), Path::new(&cwd), false);
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .menu(menu::build)
        .on_menu_event(menu::handle)
        .manage(Ptys::default())
        .manage(Runs::default())
        .manage(RepoWindows::default())
        .manage(HtmlRoots::default())
        .register_asynchronous_uri_scheme_protocol(html::SCHEME, html::handle)
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            app.manage(Placements::load(app.handle()));
            #[cfg(target_os = "macos")]
            quit::intercept(app.handle());
            match repo::data_dir(app) {
                Ok(dir) => mcp::listen(app.handle().clone(), mcp::socket_path(&dir)),
                Err(e) => eprintln!("lgtm: no data directory for the MCP socket: {e}"),
            }
            let cwd = std::env::current_dir()?;
            repo::open_from_args(app.handle(), &args, &cwd, true);
            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::Moved(_) | WindowEvent::Resized(_) => {
                window.state::<Placements>().record(window);
            }
            WindowEvent::Destroyed => {
                window.state::<Placements>().save();
                window.state::<Ptys>().kill_window(window.label());
                window.state::<HtmlRoots>().close_window(window.label());
                let runs = window.state::<Runs>();
                window.state::<RepoWindows>().remove(window.label(), &runs);
                #[cfg(target_os = "macos")]
                quit::closed(window.label());
            }
            _ => {}
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // The last window closed. Agent runs it stopped get to clean up first.
            if let RunEvent::ExitRequested {
                code: None, api, ..
            } = &event
            {
                #[cfg(target_os = "macos")]
                if quit::quitting() {
                    // The quit ends the app itself.
                    return api.prevent_exit();
                }
                if !app.state::<RepoWindows>().settled() {
                    api.prevent_exit();
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        app.state::<RepoWindows>().settle().await;
                        app.exit(0);
                    });
                }
            }
        });
}

#[cfg(test)]
mod tests {
    #[test]
    fn bindings_match_commands() {
        let dir = tempfile::tempdir().unwrap();
        let fresh = dir.path().join("bindings.ts");
        super::export_bindings(&super::specta_builder(), fresh.to_str().unwrap());
        let fresh = std::fs::read_to_string(fresh).unwrap();
        if std::env::var_os("UPDATE_BINDINGS").is_some() {
            std::fs::write(super::BINDINGS_PATH, &fresh).unwrap();
        }
        let committed = std::fs::read_to_string(super::BINDINGS_PATH).unwrap_or_default();
        assert!(
            committed == fresh,
            "src/bindings.ts is stale. Rerun with UPDATE_BINDINGS=1."
        );
    }
}
