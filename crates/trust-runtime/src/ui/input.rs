use super::*;

pub(super) fn handle_key(
    key: KeyEvent,
    client: &mut ControlClient,
    state: &mut UiState,
    no_input: bool,
) -> anyhow::Result<bool> {
    if no_input && key.code == KeyCode::Char('q') {
        return Ok(true);
    }

    if state.prompt.active {
        return handle_prompt_key(key, client, state);
    }

    if no_input {
        if key.code == KeyCode::Char('/') {
            state.prompt.set_output(vec![PromptLine::plain(
                "Read-only mode.",
                Style::default().fg(COLOR_INFO),
            )]);
        }
        return Ok(false);
    }

    if let Some(confirm) = state.pending_confirm.take() {
        return handle_confirm(confirm, key, client);
    }

    if key.code == KeyCode::Char('/') {
        state.prompt.activate_with("/");
        state
            .prompt
            .set_suggestions_list(command_suggestions(state, None));
        return Ok(false);
    }

    if key.code == KeyCode::Tab {
        advance_panel_page(state);
        return Ok(false);
    }

    let action = match key.code {
        KeyCode::Char('p') | KeyCode::Char('P') => Some("pause"),
        KeyCode::Char('r') | KeyCode::Char('R') => Some("resume"),
        KeyCode::Char('s') | KeyCode::Char('S') => Some("step_in"),
        KeyCode::Char('o') | KeyCode::Char('O') => Some("step_over"),
        KeyCode::Char('u') | KeyCode::Char('U') => Some("step_out"),
        KeyCode::Char('w') | KeyCode::Char('W') => {
            state.pending_confirm = Some(ConfirmAction::RestartWarm);
            return Ok(false);
        }
        KeyCode::Char('c') | KeyCode::Char('C') => {
            state.pending_confirm = Some(ConfirmAction::RestartCold);
            return Ok(false);
        }
        KeyCode::Char('x') | KeyCode::Char('X') => {
            state.pending_confirm = Some(ConfirmAction::Shutdown);
            return Ok(false);
        }
        KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(true),
        _ => None,
    };

    if let Some(action) = action {
        if matches!(
            action,
            "pause" | "resume" | "step_in" | "step_over" | "step_out"
        ) && !state.debug_controls
        {
            state.prompt.set_output(vec![PromptLine::plain(
                "Debug controls disabled.",
                Style::default().fg(COLOR_AMBER),
            )]);
            return Ok(false);
        }
        let request = match action {
            "pause" => json!({"id": 1, "type": "pause"}),
            "resume" => json!({"id": 1, "type": "resume"}),
            "step_in" => json!({"id": 1, "type": "step_in"}),
            "step_over" => json!({"id": 1, "type": "step_over"}),
            "step_out" => json!({"id": 1, "type": "step_out"}),
            _ => json!({"id": 1, "type": "status"}),
        };
        let _ = client.request(request);
    }
    Ok(false)
}

pub(super) fn handle_prompt_key(
    key: KeyEvent,
    client: &mut ControlClient,
    state: &mut UiState,
) -> anyhow::Result<bool> {
    match key.code {
        KeyCode::Esc => {
            let mode = state.prompt.mode;
            state.prompt.deactivate();
            state.prompt.clear_suggestions();
            match mode {
                PromptMode::IoSelect(_) => {
                    open_menu(MenuKind::Io, state);
                }
                PromptMode::IoValueSelect => {
                    if let Some(action) = state.io_pending_action {
                        open_io_select(action, state);
                    } else {
                        open_menu(MenuKind::Io, state);
                    }
                }
                PromptMode::Menu(_) | PromptMode::SettingsSelect => {
                    state.prompt.clear_output();
                    state.prompt.mode = PromptMode::Normal;
                }
                _ => {
                    state.prompt.clear_output();
                    state.prompt.mode = PromptMode::Normal;
                }
            }
            return Ok(false);
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.prompt.deactivate();
            state.prompt.mode = PromptMode::Normal;
            state.prompt.clear_suggestions();
            return Ok(false);
        }
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.prompt.mode = PromptMode::ConfirmAction(ConfirmAction::ExitConsole);
            state.prompt.set_output(vec![PromptLine::plain(
                "Exit console? [y/N]",
                Style::default().fg(COLOR_AMBER),
            )]);
            state.prompt.input.clear();
            state.prompt.cursor = 0;
            return Ok(false);
        }
        KeyCode::Enter => {
            if state.prompt.showing_suggestions
                && state.prompt.mode == PromptMode::Normal
                && !state.prompt.input.trim().contains(' ')
            {
                if let Some(selected) = state.prompt.selected_suggestion() {
                    let cmd = format!("/{}", selected.cmd);
                    state.prompt.push_history(cmd.clone());
                    state.prompt.deactivate();
                    state.prompt.clear_suggestions();
                    return execute_command(&cmd, client, state);
                }
            }
            let input = state.prompt.input.trim().to_string();
            state.prompt.push_history(input.clone());
            state.prompt.deactivate();
            state.prompt.clear_suggestions();
            return handle_prompt_submit(&input, client, state);
        }
        KeyCode::Backspace => {
            if state.prompt.cursor > 0 {
                state.prompt.cursor -= 1;
                state.prompt.input.remove(state.prompt.cursor);
            }
        }
        KeyCode::Left => {
            if state.prompt.cursor > 0 {
                state.prompt.cursor -= 1;
            }
        }
        KeyCode::Right => {
            if state.prompt.cursor < state.prompt.input.len() {
                state.prompt.cursor += 1;
            }
        }
        KeyCode::Up => {
            if state.prompt.showing_suggestions {
                state.prompt.move_suggestion(-1);
                return Ok(false);
            }
            if state.prompt.mode == PromptMode::SettingsSelect {
                move_settings_selection(state, -1);
                return Ok(false);
            }
            if let PromptMode::Menu(kind) = state.prompt.mode {
                move_menu_selection(state, kind, -1);
                return Ok(false);
            }
            if let PromptMode::IoSelect(action) = state.prompt.mode {
                move_io_selection(state, action, -1);
                return Ok(false);
            }
            if state.prompt.mode == PromptMode::IoValueSelect {
                move_io_value_selection(state, -1);
                return Ok(false);
            }
            state.prompt.history_prev();
        }
        KeyCode::Down => {
            if state.prompt.showing_suggestions {
                state.prompt.move_suggestion(1);
                return Ok(false);
            }
            if state.prompt.mode == PromptMode::SettingsSelect {
                move_settings_selection(state, 1);
                return Ok(false);
            }
            if let PromptMode::Menu(kind) = state.prompt.mode {
                move_menu_selection(state, kind, 1);
                return Ok(false);
            }
            if let PromptMode::IoSelect(action) = state.prompt.mode {
                move_io_selection(state, action, 1);
                return Ok(false);
            }
            if state.prompt.mode == PromptMode::IoValueSelect {
                move_io_value_selection(state, 1);
                return Ok(false);
            }
            state.prompt.history_next();
        }
        KeyCode::Char(ch) => {
            state.prompt.input.insert(state.prompt.cursor, ch);
            state.prompt.cursor += 1;
        }
        _ => {}
    }
    update_command_suggestions(state);
    Ok(false)
}

pub(super) fn handle_prompt_submit(
    input: &str,
    client: &mut ControlClient,
    state: &mut UiState,
) -> anyhow::Result<bool> {
    let trimmed = input.trim();
    if trimmed.is_empty() && state.prompt.mode == PromptMode::Normal {
        return Ok(false);
    }

    match state.prompt.mode {
        PromptMode::SettingsSelect => return handle_settings_select(trimmed, client, state),
        PromptMode::SettingsValue(key) => {
            return handle_settings_value(trimmed, key, client, state)
        }
        PromptMode::Menu(kind) => return handle_menu_select(trimmed, kind, client, state),
        PromptMode::IoSelect(action) => return handle_io_select(trimmed, action, client, state),
        PromptMode::IoValueSelect => return handle_io_value_select(trimmed, client, state),
        PromptMode::ConfirmAction(action) => {
            return handle_prompt_confirm(trimmed, action, client, state)
        }
        PromptMode::Normal => {}
    }

    execute_command(trimmed, client, state)
}

pub(super) fn handle_prompt_confirm(
    input: &str,
    action: ConfirmAction,
    client: &mut ControlClient,
    state: &mut UiState,
) -> anyhow::Result<bool> {
    match input.trim().to_ascii_lowercase().as_str() {
        "y" | "yes" => match action {
            ConfirmAction::ExitConsole => return Ok(true),
            ConfirmAction::Shutdown => {
                let _ = client.request(json!({"id": 1, "type": "shutdown"}));
                state.prompt.set_output(vec![PromptLine::plain(
                    "Shutdown requested.",
                    Style::default().fg(COLOR_GREEN),
                )]);
            }
            ConfirmAction::RestartCold => {
                let _ = client
                    .request(json!({"id": 1, "type": "restart", "params": { "mode": "cold" }}));
                state.prompt.set_output(vec![PromptLine::plain(
                    "Restarting (cold)...",
                    Style::default().fg(COLOR_GREEN),
                )]);
            }
            ConfirmAction::RestartWarm => {
                let _ = client
                    .request(json!({"id": 1, "type": "restart", "params": { "mode": "warm" }}));
                state.prompt.set_output(vec![PromptLine::plain(
                    "Restarting (warm)...",
                    Style::default().fg(COLOR_GREEN),
                )]);
            }
        },
        _ => {
            state.prompt.set_output(vec![PromptLine::plain(
                "Cancelled.",
                Style::default().fg(COLOR_INFO),
            )]);
        }
    }
    state.prompt.mode = PromptMode::Normal;
    Ok(false)
}
