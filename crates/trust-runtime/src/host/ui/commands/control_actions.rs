use super::control_config::{config_set, set_config_response, set_simple_response};
use super::menu_nav::open_menu;
use super::*;
pub(super) fn handle_control_command(
    args: Vec<&str>,
    client: &mut ControlClient,
    state: &mut UiState,
) -> anyhow::Result<()> {
    if args.is_empty() {
        open_menu(MenuKind::Control, state);
        return Ok(());
    }
    match args[0] {
        "pause" => {
            let response = client.request(json!({"id": 1, "type": "pause"}));
            set_simple_response(state, response, "Paused.");
        }
        "resume" => {
            let response = client.request(json!({"id": 1, "type": "resume"}));
            set_simple_response(state, response, "Resumed.");
        }
        "step" => {
            let response = client.request(json!({"id": 1, "type": "step_in"}));
            set_simple_response(state, response, "Step.");
        }
        "step-over" => {
            let response = client.request(json!({"id": 1, "type": "step_over"}));
            set_simple_response(state, response, "Step over.");
        }
        "step-out" => {
            let response = client.request(json!({"id": 1, "type": "step_out"}));
            set_simple_response(state, response, "Step out.");
        }
        "restart" => {
            if args.len() < 2 {
                open_menu(MenuKind::Restart, state);
                return Ok(());
            }
            let mode = args.get(1).copied().unwrap_or("warm");
            let response =
                client.request(json!({"id": 1, "type": "restart", "params": { "mode": mode }}));
            set_simple_response(state, response, "Restarting...");
        }
        "shutdown" => {
            state.prompt.mode = PromptMode::ConfirmAction(ConfirmAction::Shutdown);
            state.prompt.set_output(vec![PromptLine::plain(
                "This will stop the PLC. Are you sure? [y/N]",
                Style::default().fg(COLOR_AMBER),
            )]);
            state.prompt.activate_with("");
        }
        "break" => {
            if let Some(loc) = args.get(1) {
                if let Some((file, line)) = loc.split_once(':') {
                    let line_num = line.parse::<u32>().unwrap_or(1);
                    let response = client.request(json!({
                        "id": 1,
                        "type": "breakpoints.set",
                        "params": { "source": file, "lines": [line_num] }
                    }));
                    set_simple_response(state, response, "Breakpoint set.");
                }
            }
        }
        "breaks" => {
            let response = client.request(json!({"id": 1, "type": "breakpoints.list"}));
            match response {
                Ok(value) => {
                    if let Some(err) = value.get("error").and_then(|v| v.as_str()) {
                        state.prompt.set_output(vec![PromptLine::plain(
                            err.to_string(),
                            Style::default().fg(COLOR_RED),
                        )]);
                    } else if let Some(list) = value
                        .get("result")
                        .and_then(|r| r.get("breakpoints"))
                        .and_then(|v| v.as_array())
                    {
                        if list.is_empty() {
                            state.prompt.set_output(vec![PromptLine::plain(
                                "No breakpoints.",
                                Style::default().fg(COLOR_INFO),
                            )]);
                        } else {
                            let mut lines = Vec::new();
                            for bp in list {
                                let file_id =
                                    bp.get("file_id").and_then(|v| v.as_u64()).unwrap_or(0);
                                let start = bp.get("start").and_then(|v| v.as_u64()).unwrap_or(0);
                                lines.push(PromptLine::plain(
                                    format!("file {file_id} @ {start}"),
                                    Style::default().fg(COLOR_INFO),
                                ));
                            }
                            state.prompt.set_output(lines);
                        }
                    }
                }
                Err(err) => {
                    state.prompt.set_output(vec![PromptLine::plain(
                        format!("Error: {err}"),
                        Style::default().fg(COLOR_RED),
                    )]);
                }
            }
        }
        "delete" => {
            if let Some(target) = args.get(1) {
                if *target == "all" {
                    let response =
                        client.request(json!({"id": 1, "type": "breakpoints.clear_all"}));
                    set_simple_response(state, response, "Breakpoints cleared.");
                } else if let Ok(id) = target.parse::<u32>() {
                    let response = client.request(json!({
                        "id": 1,
                        "type": "breakpoints.clear_id",
                        "params": { "file_id": id }
                    }));
                    set_simple_response(state, response, "Breakpoint cleared.");
                }
            }
        }
        _ => {
            state.prompt.set_output(vec![PromptLine::plain(
                "Unknown /control command.",
                Style::default().fg(COLOR_RED),
            )]);
        }
    }
    Ok(())
}

pub(super) fn handle_access_command(
    args: Vec<&str>,
    client: &mut ControlClient,
    state: &mut UiState,
) -> anyhow::Result<()> {
    if args.is_empty() {
        open_menu(MenuKind::Access, state);
        return Ok(());
    }
    match args[0] {
        "start" => {
            let response = client.request(json!({"id": 1, "type": "pair.start"}));
            if let Ok(value) = response {
                if let Some(code) = value
                    .get("result")
                    .and_then(|r| r.get("code"))
                    .and_then(|v| v.as_str())
                {
                    state.prompt.set_output(vec![PromptLine::plain(
                        format!("Access code: {code} (valid 5 min)"),
                        Style::default().fg(COLOR_GREEN),
                    )]);
                } else {
                    set_simple_response(state, Ok(value), "Access code generated.");
                }
            }
        }
        "claim" => {
            if let Some(code) = args.get(1) {
                let response = client.request(json!({
                    "id": 1,
                    "type": "pair.claim",
                    "params": { "code": code }
                }));
                if let Ok(value) = response {
                    if let Some(token) = value
                        .get("result")
                        .and_then(|r| r.get("token"))
                        .and_then(|v| v.as_str())
                    {
                        state.prompt.set_output(vec![PromptLine::plain(
                            format!("Token: {token}"),
                            Style::default().fg(COLOR_GREEN),
                        )]);
                    } else {
                        set_simple_response(state, Ok(value), "Claimed.");
                    }
                }
            }
        }
        "list" => {
            let response = client.request(json!({"id": 1, "type": "pair.list"}));
            set_simple_response(state, response, "Tokens:");
        }
        "revoke" => {
            if let Some(id) = args.get(1) {
                let response = client.request(json!({
                    "id": 1,
                    "type": "pair.revoke",
                    "params": { "id": id }
                }));
                set_simple_response(state, response, "Revoked.");
            } else {
                state.prompt.set_output(vec![PromptLine::plain(
                    "Usage: /access revoke <id|all>",
                    Style::default().fg(COLOR_INFO),
                )]);
            }
        }
        _ => {
            state.prompt.set_output(vec![PromptLine::plain(
                "Unknown /access command.",
                Style::default().fg(COLOR_RED),
            )]);
        }
    }
    Ok(())
}

pub(super) fn handle_linking_command(
    args: Vec<&str>,
    client: &mut ControlClient,
    state: &mut UiState,
) -> anyhow::Result<()> {
    if args.is_empty() {
        open_menu(MenuKind::Linking, state);
        return Ok(());
    }
    let settings = state.data.settings.clone().unwrap_or_default();
    match args[0] {
        "enable" | "disable" => {
            let enabled = args[0] == "enable";
            let response = config_set(client, json!({ "mesh.enabled": enabled }));
            set_config_response(state, response, "Saved.");
        }
        "publish" => {
            if let Some(var) = args.get(1) {
                let mut publish = settings.mesh_publish.clone();
                if !publish.iter().any(|v| v == var) {
                    publish.push(var.to_string());
                }
                let response = config_set(client, json!({ "mesh.publish": publish }));
                set_config_response(state, response, "Saved.");
            }
        }
        "subscribe" => {
            if args.len() >= 3 {
                let mut subscribe = settings
                    .mesh_subscribe
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect::<std::collections::BTreeMap<_, _>>();
                subscribe.insert(args[1].to_string(), args[2].to_string());
                let map = subscribe
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                    .collect::<serde_json::Map<_, _>>();
                let response = config_set(client, json!({ "mesh.subscribe": map }));
                set_config_response(state, response, "Saved.");
            }
        }
        _ => {
            state.prompt.set_output(vec![PromptLine::plain(
                "Unknown /linking command.",
                Style::default().fg(COLOR_RED),
            )]);
        }
    }
    Ok(())
}

pub(super) fn handle_log_command(
    args: Vec<&str>,
    client: &mut ControlClient,
    state: &mut UiState,
) -> anyhow::Result<()> {
    if args.is_empty() {
        open_menu(MenuKind::Log, state);
        return Ok(());
    }
    if args[0] == "tail" {
        let limit = args
            .get(1)
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(10);
        let response =
            client.request(json!({"id": 1, "type": "events.tail", "params": { "limit": limit }}));
        match response {
            Ok(value) => {
                if let Some(err) = value.get("error").and_then(|v| v.as_str()) {
                    state.prompt.set_output(vec![PromptLine::plain(
                        err.to_string(),
                        Style::default().fg(COLOR_RED),
                    )]);
                } else {
                    let events = parse_events(&value);
                    if events.is_empty() {
                        state.prompt.set_output(vec![PromptLine::plain(
                            "No events.",
                            Style::default().fg(COLOR_INFO),
                        )]);
                    } else {
                        let lines = events
                            .into_iter()
                            .map(|event| {
                                PromptLine::plain(event.label, Style::default().fg(COLOR_INFO))
                            })
                            .collect();
                        state.prompt.set_output(lines);
                    }
                }
            }
            Err(err) => {
                state.prompt.set_output(vec![PromptLine::plain(
                    format!("Error: {err}"),
                    Style::default().fg(COLOR_RED),
                )]);
            }
        }
        return Ok(());
    }
    let response = config_set(client, json!({ "log.level": args[0] }));
    set_config_response(state, response, "Saved.");
    Ok(())
}

pub(super) fn handle_layout_command(args: Vec<&str>, state: &mut UiState) -> anyhow::Result<()> {
    if args.is_empty() {
        let names = state
            .layout
            .iter()
            .map(|p| format!("{:?}", p).to_ascii_lowercase())
            .collect::<Vec<_>>()
            .join(" ");
        state.prompt.set_output(vec![PromptLine::plain(
            format!("Current: {names}"),
            Style::default().fg(COLOR_INFO),
        )]);
        return Ok(());
    }
    let mut panels = Vec::new();
    for arg in args.iter().take(4) {
        if let Some(panel) = PanelKind::parse(arg) {
            if !panels.contains(&panel) {
                panels.push(panel);
            }
        }
    }
    if !panels.is_empty() {
        while panels.len() < 4 {
            panels.push(PanelKind::Status);
        }
        state.layout = panels;
        state.panel_page = 0;
        state.prompt.set_output(vec![PromptLine::plain(
            "Layout updated.",
            Style::default().fg(COLOR_GREEN),
        )]);
    }
    Ok(())
}

pub(super) fn handle_focus_command(args: Vec<&str>, state: &mut UiState) -> anyhow::Result<()> {
    if let Some(name) = args.first() {
        if let Some(panel) = PanelKind::parse(name) {
            state.focus = Some(panel);
            state.prompt.set_output(vec![PromptLine::plain(
                format!("Focused {name}."),
                Style::default().fg(COLOR_INFO),
            )]);
        }
    }
    Ok(())
}

pub(super) fn handle_build_command(state: &mut UiState) -> anyhow::Result<()> {
    let Some(root) = state.bundle_root.as_ref() else {
        state.prompt.set_output(vec![PromptLine::plain(
            "Project path required.",
            Style::default().fg(COLOR_RED),
        )]);
        return Ok(());
    };
    match build_program_stbc(root, None) {
        Ok(report) => {
            state.prompt.set_output(vec![PromptLine::plain(
                format!("Built program.stbc ({} sources).", report.sources.len()),
                Style::default().fg(COLOR_GREEN),
            )]);
        }
        Err(err) => {
            state.prompt.set_output(vec![PromptLine::plain(
                format!("Build failed: {err}"),
                Style::default().fg(COLOR_RED),
            )]);
        }
    }
    Ok(())
}

pub(super) fn handle_reload_command(
    client: &mut ControlClient,
    state: &mut UiState,
) -> anyhow::Result<()> {
    let Some(root) = state.bundle_root.as_ref() else {
        state.prompt.set_output(vec![PromptLine::plain(
            "Project path required.",
            Style::default().fg(COLOR_RED),
        )]);
        return Ok(());
    };
    let path = root.join("program.stbc");
    let bytes = fs::read(&path)?;
    let encoded = BASE64_STANDARD.encode(bytes);
    let response = client.request(json!({
        "id": 1,
        "type": "bytecode.reload",
        "params": { "bytes": encoded }
    }));
    set_simple_response(state, response, "Reloaded.");
    Ok(())
}
