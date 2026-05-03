use super::*;

mod catalog;
mod control_actions;
mod control_config;
mod io_nav;
mod menu_nav;
mod settings;

use catalog::{help_lines, info_lines, is_beginner_command, status_lines};
use control_actions::{
    handle_access_command, handle_build_command, handle_control_command, handle_focus_command,
    handle_layout_command, handle_linking_command, handle_log_command, handle_reload_command,
};
use io_nav::handle_io_command;
use settings::settings_menu_lines;

pub(super) fn execute_command(
    input: &str,
    client: &mut ControlClient,
    state: &mut UiState,
) -> anyhow::Result<bool> {
    let raw = input.trim();
    if raw.is_empty() {
        return Ok(false);
    }
    if raw == "/" {
        state
            .prompt
            .set_suggestions_list(command_suggestions(state, None));
        return Ok(false);
    }
    let mut cmd = raw;
    if let Some(stripped) = cmd.strip_prefix('/') {
        cmd = stripped;
    }
    let mut parts = cmd.split_whitespace();
    let head = parts.next().unwrap_or("");
    match head {
        "s" => {
            state.prompt.set_output(status_lines(state));
            return Ok(false);
        }
        "h" => {
            state.prompt.set_output(help_lines(state));
            return Ok(false);
        }
        "q" => return Ok(true),
        "p" => {
            handle_control_command(vec!["pause"], client, state)?;
            return Ok(false);
        }
        "r" => {
            handle_control_command(vec!["resume"], client, state)?;
            return Ok(false);
        }
        _ => {}
    }

    if state.beginner_mode && !is_beginner_command(head) {
        state.prompt.set_output(vec![PromptLine::plain(
            "Beginner mode: use /help, /status, /settings, /io, /control, /info, /exit.",
            Style::default().fg(COLOR_AMBER),
        )]);
        return Ok(false);
    }

    match head {
        "help" => {
            state.prompt.set_output(help_lines(state));
        }
        "status" => {
            state.prompt.set_output(status_lines(state));
        }
        "info" => {
            state.prompt.set_output(info_lines(state));
        }
        "clear" => {
            state.prompt.clear_output();
            state.alerts.clear();
        }
        "exit" => return Ok(true),
        "settings" => {
            state.prompt.mode = PromptMode::SettingsSelect;
            state.settings_index = 0;
            state
                .prompt
                .set_output(settings_menu_lines(state, state.settings_index));
            state.prompt.activate_with("");
        }
        "io" => {
            handle_io_command(parts.collect::<Vec<_>>(), client, state)?;
        }
        "control" => {
            handle_control_command(parts.collect::<Vec<_>>(), client, state)?;
        }
        "access" => {
            handle_access_command(parts.collect::<Vec<_>>(), client, state)?;
        }
        "linking" => {
            handle_linking_command(parts.collect::<Vec<_>>(), client, state)?;
        }
        "build" => {
            handle_build_command(state)?;
        }
        "reload" => {
            handle_reload_command(client, state)?;
        }
        "watch" => {
            if let Some(name) = parts.next() {
                if !state.watch_list.iter().any(|v| v == name) {
                    state.watch_list.push(name.to_string());
                }
                state.prompt.set_output(vec![PromptLine::plain(
                    format!("Watching {name}."),
                    Style::default().fg(COLOR_GREEN),
                )]);
            }
        }
        "unwatch" => match parts.next() {
            Some("all") => {
                state.watch_list.clear();
                state.watch_values.clear();
                state.prompt.set_output(vec![PromptLine::plain(
                    "Watches cleared.",
                    Style::default().fg(COLOR_INFO),
                )]);
            }
            Some(name) => {
                state.watch_list.retain(|v| v != name);
                state.prompt.set_output(vec![PromptLine::plain(
                    format!("Stopped watching {name}."),
                    Style::default().fg(COLOR_INFO),
                )]);
            }
            None => {
                state.prompt.set_output(vec![PromptLine::plain(
                    "Usage: /unwatch <name|all>",
                    Style::default().fg(COLOR_INFO),
                )]);
            }
        },
        "log" => {
            handle_log_command(parts.collect::<Vec<_>>(), client, state)?;
        }
        "layout" => {
            handle_layout_command(parts.collect::<Vec<_>>(), state)?;
        }
        "focus" => {
            handle_focus_command(parts.collect::<Vec<_>>(), state)?;
        }
        "unfocus" => {
            state.focus = None;
            state.prompt.set_output(vec![PromptLine::plain(
                "Returned to grid view.",
                Style::default().fg(COLOR_INFO),
            )]);
        }
        _ => {
            state.prompt.set_output(vec![PromptLine::plain(
                "Unknown command. Type /help.",
                Style::default().fg(COLOR_RED),
            )]);
        }
    }
    Ok(false)
}

pub(super) fn handle_settings_select(
    input: &str,
    client: &mut ControlClient,
    state: &mut UiState,
) -> anyhow::Result<bool> {
    settings::handle_settings_select(input, client, state)
}

pub(super) fn handle_settings_value(
    input: &str,
    key: SettingKey,
    client: &mut ControlClient,
    state: &mut UiState,
) -> anyhow::Result<bool> {
    settings::handle_settings_value(input, key, client, state)
}

pub(super) fn update_command_suggestions(state: &mut UiState) {
    catalog::update_command_suggestions(state);
}

pub(super) fn command_suggestions(state: &UiState, filter: Option<&str>) -> Vec<CommandHelp> {
    catalog::command_suggestions(state, filter)
}

pub(super) fn suggestion_lines(
    suggestions: &[CommandHelp],
    selected: Option<usize>,
) -> Vec<PromptLine> {
    catalog::suggestion_lines(suggestions, selected)
}

pub(super) fn move_settings_selection(state: &mut UiState, delta: i32) {
    settings::move_settings_selection(state, delta);
}

pub(super) fn move_menu_selection(state: &mut UiState, kind: MenuKind, delta: i32) {
    menu_nav::move_menu_selection(state, kind, delta);
}

pub(super) fn move_io_selection(state: &mut UiState, action: IoActionKind, delta: i32) {
    io_nav::move_io_selection(state, action, delta);
}

pub(super) fn move_io_value_selection(state: &mut UiState, delta: i32) {
    io_nav::move_io_value_selection(state, delta);
}

pub(super) fn handle_menu_select(
    input: &str,
    kind: MenuKind,
    client: &mut ControlClient,
    state: &mut UiState,
) -> anyhow::Result<bool> {
    let entries = menu_nav::menu_entries(kind);
    if entries.is_empty() {
        state.prompt.mode = PromptMode::Normal;
        return Ok(false);
    }
    let choice = input.trim();
    let selected = if choice.is_empty() {
        Some(state.menu_index)
    } else if let Ok(num) = choice.parse::<usize>() {
        if num == 0 {
            Some(entries.len().saturating_sub(1))
        } else {
            num.checked_sub(1)
        }
    } else {
        None
    };
    let Some(selected) = selected else {
        state.prompt.set_output(vec![PromptLine::plain(
            "Invalid choice.",
            Style::default().fg(COLOR_RED),
        )]);
        return Ok(false);
    };
    if selected >= entries.len() {
        state.prompt.set_output(vec![PromptLine::plain(
            "Invalid choice.",
            Style::default().fg(COLOR_RED),
        )]);
        return Ok(false);
    }
    let entry = entries[selected];
    if entry.command.is_empty() {
        state.prompt.clear_output();
        state.prompt.mode = PromptMode::Normal;
        return Ok(false);
    }
    state.prompt.mode = PromptMode::Normal;
    state.prompt.clear_output();
    if kind == MenuKind::Io {
        match entry.command {
            "/io read" => {
                io_nav::open_io_select(IoActionKind::Read, state);
                return Ok(false);
            }
            "/io set" => {
                io_nav::open_io_select(IoActionKind::Set, state);
                return Ok(false);
            }
            "/io force" => {
                io_nav::open_io_select(IoActionKind::Force, state);
                return Ok(false);
            }
            "/io unforce" => {
                io_nav::open_io_select(IoActionKind::Unforce, state);
                return Ok(false);
            }
            _ => {}
        }
    }
    if entry.needs_input {
        let mut cmd = entry.command.to_string();
        if !cmd.ends_with(' ') {
            cmd.push(' ');
        }
        state.prompt.activate_with(&cmd);
        return Ok(false);
    }
    execute_command(entry.command, client, state)
}

pub(super) fn handle_io_select(
    input: &str,
    action: IoActionKind,
    client: &mut ControlClient,
    state: &mut UiState,
) -> anyhow::Result<bool> {
    io_nav::handle_io_select(input, action, client, state)
}

pub(super) fn handle_io_value_select(
    input: &str,
    client: &mut ControlClient,
    state: &mut UiState,
) -> anyhow::Result<bool> {
    io_nav::handle_io_value_select(input, client, state)
}

pub(super) fn open_menu(kind: MenuKind, state: &mut UiState) {
    menu_nav::open_menu(kind, state);
}

pub(super) fn open_io_select(action: IoActionKind, state: &mut UiState) {
    io_nav::open_io_select(action, state);
}

pub(super) fn advance_panel_page(state: &mut UiState) {
    menu_nav::advance_panel_page(state);
}
