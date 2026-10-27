pub mod anchor;
mod bots;
mod canvas;
mod commands;
mod config;
mod ctx;
pub mod diff;
mod dragged;
mod drawings;
mod events;
mod files;
mod frame;
mod github;
mod gitmeta;
mod html;
mod jj;
mod layout;
mod logging;
mod lsp;
mod mcp;
mod menu;
mod notes;
mod par;
mod paths;
mod placement;
mod pr;
mod project;
mod prompts;
mod pty;
mod qa;
#[cfg(target_os = "macos")]
mod quit;
mod recent;
mod repo;
mod review;
mod revisions;
mod search;
mod settings;
mod store;
mod symbol_info;
mod syntax;
mod tools;
mod turns;
mod watch;
mod workspace;
mod workspaces;
mod zoom;

use std::path::Path;

use tauri::{Manager, WindowEvent};
use tauri_specta::{collect_commands, collect_events};

use html::HtmlRoots;
use placement::Placements;
use pty::Ptys;
use repo::Repos;

fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::new()
        .constant("HIGHLIGHT_NAMES", syntax::HIGHLIGHT_NAMES)
        .commands(collect_commands![
            commands::repo::current_repo,
            commands::repo::check_tools,
            commands::repo::open_repo,
            commands::repo::init_root,
            commands::repo::init_jj,
            commands::repo::recent_repos,
            commands::project::current_project,
            commands::project::add_repo,
            commands::project::remove_repo,
            commands::project::sleep_repo,
            commands::project::wake_repo,
            commands::project::set_active_repo,
            commands::project::rename_project,
            commands::project::save_project_ui,
            commands::project::closed_projects,
            commands::project::open_project,
            commands::project::delete_project,
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
            commands::workspaces::locate_workspace,
            commands::workspaces::open_workspace,
            commands::workspaces::sleep_workspace,
            commands::workspaces::create_workspace,
            commands::workspaces::approve_setup,
            commands::workspaces::delete_workspace,
            commands::review::open_review,
            commands::review::close_review,
            commands::review::context_lines,
            commands::review::refresh_review,
            commands::review::log_view,
            commands::review::operation,
            commands::review::set_log_revset,
            commands::review::trunk_bookmark,
            commands::review::revset_commits,
            commands::review::working_copy,
            commands::review::jj_act,
            commands::review::nearest_bookmarks,
            commands::review::push_plan,
            commands::log::log,
            commands::log::log_level,
            commands::search::search_text,
            commands::search::replace_text,
            commands::search::find_references,
            commands::search::workspace_symbols,
            commands::search::workspace_files,
            commands::search::request_symbol_info,
            commands::search::symbol_infos,
            commands::search::dismiss_symbol_info,
            commands::search::mark_symbol_asked,
            commands::review::mark_rows,
            commands::review::clear_file_marks,
            commands::review::set_files_checked,
            commands::review::undo_marks,
            commands::review::redo_marks,
            commands::review::reset_review_marks,
            commands::arch::review_arch,
            commands::arch::arch_history,
            commands::arch::check_look,
            commands::arch::assess_prompt,
            commands::guided::guided,
            commands::guided::guided_prompt,
            commands::guided::set_guided_verdicts,
            commands::guided::promote_excluded,
            commands::guided::chunk_prompts,
            commands::arch::system_map,
            commands::arch::arch_view,
            commands::arch::set_arch_view,
            commands::design::designs,
            commands::design::save_design,
            commands::design::delete_design,
            commands::design::save_design_canvas,
            commands::design::undo_design_agent,
            commands::design::design_prompt,
            commands::design::revision_files,
            commands::design::design_bookmarks,
            commands::drawing::drawings,
            commands::drawing::drawing,
            commands::drawing::create_drawing,
            commands::drawing::rename_drawing,
            commands::drawing::delete_drawing,
            commands::drawing::save_drawing_canvas,
            commands::drawing::undo_drawing_agent,
            commands::drawing::set_canvas_focus,
            commands::drawing::write_export,
            commands::notes::notes,
            commands::notes::note,
            commands::notes::create_note,
            commands::notes::save_note,
            commands::notes::rename_note,
            commands::notes::move_note,
            commands::notes::delete_note,
            commands::notes::create_note_folder,
            commands::notes::rename_note_folder,
            commands::notes::delete_note_folder,
            commands::arch::describe_prompt,
            commands::arch::system_most_used,
            commands::arch::system_find,
            commands::arch::tour_prompt,
            commands::arch::system_symbols,
            commands::arch::system_edge,
            commands::review::focus_review,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::settings_load_error,
            commands::settings::claude_command,
            commands::settings::tool_status,
            commands::settings::prompt_files,
            commands::settings::edit_prompt,
            commands::settings::prompt_text,
            commands::settings::remove_prompt,
            commands::repo::mcp_status,
            commands::repo::register_mcp,
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
            commands::comments::dismiss_suggestion,
            commands::comments::set_agent_link,
            commands::bots::delete_bots,
            commands::bots::bot_runs,
            commands::bots::bot_run,
            commands::bots::rename_report,
            commands::bots::dismiss_findings,
            commands::files::list_files,
            commands::files::read_file,
            commands::lsp::lsp_definition,
            commands::lsp::lsp_hover,
            commands::files::read_image,
            commands::files::save_file,
            commands::files::files_at,
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
            commands::github::review_stack_pr,
            commands::github::pin_stack_pr,
            commands::github::follow_review,
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
            commands::qa::add_qa_turn,
            commands::qa::defer_preview,
            commands::turns::agent_turns,
            commands::turns::set_checkpoint,
            commands::turns::undo_turn,
            commands::turns::remove_turn,
        ])
        .events(collect_events![
            events::RepoChanged,
            events::ReviewOutdated,
            events::WorkingCopyMoved,
            events::AddRepo,
            events::ShowRepo,
            events::WordWrap,
            events::SettingsChanged,
            events::OpenSettings,
            events::OpenShortcuts,
            events::WorkingCopyStale,
            events::JjBlocked,
            events::JjPaused,
            events::BotRunChanged,
            events::AgentShow,
            events::AgentRevset,
            events::LogRevsetChanged,
            events::ReviewStateChanged,
            events::QaChanged,
            events::ArchChanged,
            events::GuidedChanged,
            events::TurnsChanged,
            events::SymbolInfoChanged,
            events::SystemChanged,
            events::DesignChanged,
            events::DrawingChanged,
            events::NotesChanged,
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

/// lgtm's own store, for global notes. Without one, global notes last until lgtm quits.
fn global_store(app: &tauri::App) -> store::Store {
    let opened = paths::data_dir(app)
        .map_err(anyhow::Error::msg)
        .and_then(|dir| store::Store::open(&dir.join("global.db")));
    opened.unwrap_or_else(|e| {
        tracing::error!("open the global store: {e:#}");
        store::Store::in_memory()
    })
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
    logging::init();
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
            repo::open_from_args(
                app,
                argv.get(1..).unwrap_or_default(),
                Path::new(&cwd),
                false,
            );
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .menu(menu::build)
        .on_menu_event(menu::handle)
        .manage(Ptys::default())
        .manage(Repos::default())
        .manage(project::Projects::default())
        .manage(HtmlRoots::default())
        .register_asynchronous_uri_scheme_protocol(html::SCHEME, html::handle)
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            app.manage(Placements::load(app.handle()));
            #[cfg(target_os = "macos")]
            quit::intercept(app.handle());
            match paths::data_dir(app) {
                Ok(dir) => mcp::listen(app.handle().clone(), mcp::socket_path(&dir)),
                Err(e) => tracing::error!("no data directory for the MCP socket: {e}"),
            }
            app.manage(notes::GlobalStore(std::sync::Mutex::new(global_store(app))));
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
                #[cfg(target_os = "macos")]
                let quitting = quit::quitting();
                #[cfg(not(target_os = "macos"))]
                let quitting = false;
                project::closed(window.app_handle(), window.label(), quitting);
                #[cfg(target_os = "macos")]
                quit::closed(window.label());
            }
            _ => {}
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_, event| {
            // jj processes the app started don't end with it on their own.
            if let tauri::RunEvent::Exit = event {
                jj::end_all();
            }
            // The last window closed. During a quit, the quit ends the app itself.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::ExitRequested {
                code: None, api, ..
            } = &event
                && quit::quitting()
            {
                api.prevent_exit();
            }
            #[cfg(not(target_os = "macos"))]
            let _ = event;
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
