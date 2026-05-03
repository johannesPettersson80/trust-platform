use super::control_config::{is_bool_value, set_simple_response};
use super::menu_nav::open_menu;
use super::*;
fn io_action_label(action: IoActionKind) -> &'static str {
    match action {
        IoActionKind::Read => "Read I/O",
        IoActionKind::Set => "Set I/O value",
        IoActionKind::Force => "Force I/O value",
        IoActionKind::Unforce => "Release I/O force",
    }
}

fn io_entries_for_action(state: &UiState, action: IoActionKind) -> Vec<usize> {
    let mut indices = Vec::new();
    for (idx, entry) in state.data.io.iter().enumerate() {
        if matches!(
            action,
            IoActionKind::Set | IoActionKind::Force | IoActionKind::Unforce
        ) && !entry.direction.eq_ignore_ascii_case("OUT")
        {
            continue;
        }
        indices.push(idx);
    }
    indices
}

fn io_select_lines(state: &UiState, action: IoActionKind, selected: usize) -> Vec<PromptLine> {
    let indices = io_entries_for_action(state, action);
    let mut lines = Vec::new();
    lines.push(PromptLine::plain(io_action_label(action), header_style()));
    lines.push(PromptLine::from_segments(vec![
        seg("DIR ", header_style()),
        seg("ADDR    ", header_style()),
        seg("NAME       ", header_style()),
        seg("VALUE", header_style()),
    ]));
    if indices.is_empty() {
        lines.push(PromptLine::plain(
            "No matching I/O.",
            Style::default().fg(COLOR_INFO),
        ));
        return lines;
    }
    for (row, idx) in indices.iter().enumerate() {
        let entry = &state.data.io[*idx];
        let forced = if state.forced_io.contains(&entry.address) {
            " *"
        } else {
            ""
        };
        let line_text = format!(
            "{:<3} {:<7} {:<10} {}{forced}",
            entry.direction, entry.address, entry.name, entry.value
        );
        if row == selected {
            lines.push(PromptLine::plain(
                line_text,
                Style::default()
                    .bg(COLOR_TEAL)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            lines.push(PromptLine::from_segments(vec![
                seg(format!("{:<3} ", entry.direction), label_style()),
                seg(format!("{:<7} ", entry.address), value_style()),
                seg(format!("{:<10} ", entry.name), value_style()),
                seg(entry.value.clone(), value_style()),
                seg(forced, Style::default().fg(COLOR_AMBER)),
            ]));
        }
    }
    lines.push(PromptLine::plain(
        "Use ↑/↓ and Enter. Esc to back.",
        Style::default().fg(COLOR_INFO),
    ));
    lines
}

pub(super) fn open_io_select(action: IoActionKind, state: &mut UiState) {
    state.prompt.mode = PromptMode::IoSelect(action);
    state.io_index = 0;
    state
        .prompt
        .set_output(io_select_lines(state, action, state.io_index));
    state.prompt.activate_with("");
}

pub(super) fn move_io_selection(state: &mut UiState, action: IoActionKind, delta: i32) {
    let indices = io_entries_for_action(state, action);
    let len = indices.len();
    if len == 0 {
        return;
    }
    let mut next = state.io_index as i32 + delta;
    if next < 0 {
        next = len as i32 - 1;
    } else if next >= len as i32 {
        next = 0;
    }
    state.io_index = next as usize;
    state
        .prompt
        .set_output(io_select_lines(state, action, state.io_index));
}

fn io_value_lines(state: &UiState, selected: usize) -> Vec<PromptLine> {
    let mut lines = Vec::new();
    let address = state.io_pending_address.as_deref().unwrap_or("<io>");
    let action = state
        .io_pending_action
        .map(io_action_label)
        .unwrap_or("I/O");
    lines.push(PromptLine::plain(
        format!("{action} — {address}"),
        header_style(),
    ));
    lines.push(PromptLine::plain(
        "Select value:",
        Style::default().fg(COLOR_INFO),
    ));
    let options = ["TRUE", "FALSE", "Back"];
    for (idx, option) in options.iter().enumerate() {
        if idx == selected {
            lines.push(PromptLine::plain(
                (*option).to_string(),
                Style::default()
                    .bg(COLOR_TEAL)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            lines.push(PromptLine::from_segments(vec![seg(*option, value_style())]));
        }
    }
    lines
}

fn open_io_value_select(action: IoActionKind, address: String, state: &mut UiState) {
    state.io_pending_action = Some(action);
    state.io_pending_address = Some(address);
    state.io_value_index = 0;
    state.prompt.mode = PromptMode::IoValueSelect;
    state
        .prompt
        .set_output(io_value_lines(state, state.io_value_index));
    state.prompt.activate_with("");
}

pub(super) fn move_io_value_selection(state: &mut UiState, delta: i32) {
    let options_len: i32 = 3;
    let mut next = state.io_value_index as i32 + delta;
    if next < 0 {
        next = options_len - 1;
    } else if next >= options_len {
        next = 0;
    }
    state.io_value_index = next as usize;
    state
        .prompt
        .set_output(io_value_lines(state, state.io_value_index));
}

pub(super) fn handle_io_value_select(
    input: &str,
    client: &mut ControlClient,
    state: &mut UiState,
) -> anyhow::Result<bool> {
    let action = match state.io_pending_action {
        Some(action) => action,
        None => {
            state.prompt.mode = PromptMode::Normal;
            return Ok(false);
        }
    };
    let address = match state.io_pending_address.clone() {
        Some(addr) => addr,
        None => {
            state.prompt.mode = PromptMode::Normal;
            return Ok(false);
        }
    };
    let choice = input.trim();
    let selected = if choice.is_empty() {
        Some(state.io_value_index)
    } else if let Ok(num) = choice.parse::<usize>() {
        if num == 0 {
            Some(2)
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
    match selected {
        0 | 1 => {
            let value = if selected == 0 { "true" } else { "false" };
            state.prompt.mode = PromptMode::Normal;
            state.prompt.clear_output();
            match action {
                IoActionKind::Set => {
                    let response = client.request(json!({
                        "id": 1,
                        "type": "io.write",
                        "params": { "address": address, "value": value }
                    }));
                    set_simple_response(state, response, "I/O set queued.");
                }
                IoActionKind::Force => {
                    let response = client.request(json!({
                        "id": 1,
                        "type": "io.force",
                        "params": { "address": address, "value": value }
                    }));
                    state.forced_io.insert(address);
                    set_simple_response(state, response, "I/O forced.");
                }
                _ => {}
            }
        }
        _ => {
            open_io_select(action, state);
        }
    }
    Ok(false)
}

pub(super) fn handle_io_select(
    input: &str,
    action: IoActionKind,
    client: &mut ControlClient,
    state: &mut UiState,
) -> anyhow::Result<bool> {
    let indices = io_entries_for_action(state, action);
    if indices.is_empty() {
        state.prompt.mode = PromptMode::Normal;
        return Ok(false);
    }
    let choice = input.trim();
    let selected = if choice.is_empty() {
        Some(state.io_index)
    } else if let Ok(num) = choice.parse::<usize>() {
        if num == 0 {
            None
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
    if selected >= indices.len() {
        state.prompt.set_output(vec![PromptLine::plain(
            "Invalid choice.",
            Style::default().fg(COLOR_RED),
        )]);
        return Ok(false);
    }
    let entry = &state.data.io[indices[selected]];
    let address = entry.address.clone();
    state.prompt.mode = PromptMode::Normal;
    state.prompt.clear_output();
    match action {
        IoActionKind::Read => {
            handle_io_command(vec!["read", &address], client, state)?;
        }
        IoActionKind::Set => {
            if is_bool_value(&entry.value) {
                open_io_value_select(action, address, state);
            } else {
                let cmd = format!("/io set {} ", address);
                state.prompt.activate_with(&cmd);
                state.prompt.set_output(vec![PromptLine::plain(
                    "Enter value:",
                    Style::default().fg(COLOR_INFO),
                )]);
            }
        }
        IoActionKind::Force => {
            if is_bool_value(&entry.value) {
                open_io_value_select(action, address, state);
            } else {
                let cmd = format!("/io force {} ", address);
                state.prompt.activate_with(&cmd);
                state.prompt.set_output(vec![PromptLine::plain(
                    "Enter value:",
                    Style::default().fg(COLOR_INFO),
                )]);
            }
        }
        IoActionKind::Unforce => {
            handle_io_command(vec!["unforce", &address], client, state)?;
        }
    }
    Ok(false)
}

pub(super) fn handle_io_command(
    args: Vec<&str>,
    client: &mut ControlClient,
    state: &mut UiState,
) -> anyhow::Result<()> {
    if args.is_empty() {
        open_menu(MenuKind::Io, state);
        return Ok(());
    }
    match args[0] {
        "list" => {
            let mut lines = Vec::new();
            for entry in &state.data.io {
                lines.push(PromptLine::plain(
                    format!(
                        "{} {}{} {}",
                        entry.direction,
                        if entry.name.is_empty() {
                            ""
                        } else {
                            entry.name.as_str()
                        },
                        entry.address,
                        entry.value
                    ),
                    Style::default().fg(COLOR_INFO),
                ));
            }
            state.prompt.set_output(lines);
        }
        "read" => {
            if args.get(1).is_none() {
                open_io_select(IoActionKind::Read, state);
                return Ok(());
            }
            if let Some(addr) = args.get(1) {
                if let Some(entry) = state.data.io.iter().find(|e| &e.address == addr) {
                    state.prompt.set_output(vec![PromptLine::plain(
                        format!("{} = {}", entry.address, entry.value),
                        Style::default().fg(COLOR_INFO),
                    )]);
                } else {
                    state.prompt.set_output(vec![PromptLine::plain(
                        "Address not found.",
                        Style::default().fg(COLOR_RED),
                    )]);
                }
            }
        }
        "set" => {
            if args.len() < 3 {
                open_io_select(IoActionKind::Set, state);
                return Ok(());
            }
            let response = client.request(json!({
                "id": 1,
                "type": "io.write",
                "params": { "address": args[1], "value": args[2] }
            }));
            set_simple_response(state, response, "I/O set queued.");
        }
        "force" => {
            if args.len() < 3 {
                open_io_select(IoActionKind::Force, state);
                return Ok(());
            }
            let response = client.request(json!({
                "id": 1,
                "type": "io.force",
                "params": { "address": args[1], "value": args[2] }
            }));
            state.forced_io.insert(args[1].to_string());
            set_simple_response(state, response, "I/O forced.");
        }
        "unforce" => {
            if args.len() < 2 {
                open_io_select(IoActionKind::Unforce, state);
                return Ok(());
            }
            if args[1] == "all" {
                for addr in state.forced_io.clone() {
                    let _ = client.request(json!({
                        "id": 1,
                        "type": "io.unforce",
                        "params": { "address": addr }
                    }));
                }
                state.forced_io.clear();
                state.prompt.set_output(vec![PromptLine::plain(
                    "All forced I/O released.",
                    Style::default().fg(COLOR_INFO),
                )]);
            } else {
                let response = client.request(json!({
                    "id": 1,
                    "type": "io.unforce",
                    "params": { "address": args[1] }
                }));
                state.forced_io.remove(args[1]);
                set_simple_response(state, response, "I/O released.");
            }
        }
        "forced" => {
            if state.forced_io.is_empty() {
                state.prompt.set_output(vec![PromptLine::plain(
                    "No forced I/O.",
                    Style::default().fg(COLOR_INFO),
                )]);
            } else {
                let lines = state
                    .forced_io
                    .iter()
                    .map(|addr| PromptLine::plain(addr.clone(), Style::default().fg(COLOR_INFO)))
                    .collect::<Vec<_>>();
                state.prompt.set_output(lines);
            }
        }
        _ => {
            state.prompt.set_output(vec![PromptLine::plain(
                "Unknown /io command.",
                Style::default().fg(COLOR_RED),
            )]);
        }
    }
    Ok(())
}
