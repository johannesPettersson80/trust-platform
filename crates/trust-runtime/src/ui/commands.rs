use super::*;

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
