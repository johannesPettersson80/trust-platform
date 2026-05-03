use super::control_config::{parse_bool_value, update_runtime_toml};
use super::menu_nav::open_menu;
use super::*;

pub(super) struct SettingApplyResult {
    pub(super) ok: bool,
    pub(super) restart_required: bool,
    pub(super) message: String,
}

struct SettingsMenuEntry {
    key: Option<SettingKey>,
    label: &'static str,
    value: String,
}
pub(super) fn handle_settings_select(
    input: &str,
    _client: &mut ControlClient,
    state: &mut UiState,
) -> anyhow::Result<bool> {
    let choice = input.trim();
    let entries = settings_menu_entries(state);
    let selected = if choice.is_empty() {
        Some(state.settings_index)
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
    let entry = &entries[selected];
    if entry.key.is_none() {
        state.prompt.clear_output();
        state.prompt.mode = PromptMode::Normal;
        return Ok(false);
    }
    let key = entry.key.expect("settings entry key present");
    let current_value = normalize_setting_input(key, &entry.value);
    state.prompt.mode = PromptMode::SettingsValue(key);
    state.prompt.set_output(vec![
        PromptLine::plain(format_setting_key(key), header_style()),
        PromptLine::from_segments(vec![
            seg("Current: ", label_style()),
            seg(entry.value.clone(), value_style()),
        ]),
        PromptLine::plain(
            "Enter new value (Esc to cancel).",
            Style::default().fg(COLOR_INFO),
        ),
    ]);
    state.prompt.activate_with(&current_value);
    Ok(false)
}

pub(super) fn handle_settings_value(
    input: &str,
    key: SettingKey,
    client: &mut ControlClient,
    state: &mut UiState,
) -> anyhow::Result<bool> {
    let value = input.trim();
    if value.is_empty() {
        state.prompt.set_output(vec![PromptLine::plain(
            "Value required.",
            Style::default().fg(COLOR_RED),
        )]);
        return Ok(false);
    }
    let result = apply_setting(key, value, client, state)?;
    state.prompt.set_output(vec![PromptLine::plain(
        result.message,
        Style::default().fg(if result.ok { COLOR_GREEN } else { COLOR_RED }),
    )]);
    if result.restart_required {
        open_menu(MenuKind::Restart, state);
        return Ok(false);
    }
    state.prompt.mode = PromptMode::Normal;
    Ok(false)
}

pub(super) fn apply_setting(
    key: SettingKey,
    value: &str,
    client: &mut ControlClient,
    state: &mut UiState,
) -> anyhow::Result<SettingApplyResult> {
    let mut restart_required = false;
    let mut ok = true;
    let mut message = "Saved.".to_string();

    match key {
        SettingKey::PlcName => {
            if let Some(root) = state.bundle_root.as_ref() {
                if let Err(err) = update_runtime_toml(root, "resource.name", value) {
                    ok = false;
                    message = format!("Failed: {err}");
                } else {
                    restart_required = true;
                    message = "Saved. Restart required.".to_string();
                }
            } else {
                ok = false;
                message = "Project path required.".to_string();
            }
        }
        SettingKey::CycleInterval => {
            if let Ok(ms) = value.trim().parse::<u64>() {
                if let Some(root) = state.bundle_root.as_ref() {
                    if let Err(err) =
                        update_runtime_toml(root, "resource.cycle_interval_ms", &ms.to_string())
                    {
                        ok = false;
                        message = format!("Failed: {err}");
                    } else {
                        restart_required = true;
                        message = "Saved. Restart required.".to_string();
                    }
                } else {
                    ok = false;
                    message = "Project path required.".to_string();
                }
            } else {
                ok = false;
                message = "Invalid number.".to_string();
            }
        }
        SettingKey::LogLevel => {
            let _ = client
                .request(json!({"id": 1, "type": "config.set", "params": { "log.level": value }}));
            if let Some(root) = state.bundle_root.as_ref() {
                let _ = update_runtime_toml(root, "runtime.log.level", value);
            }
        }
        SettingKey::ControlMode => {
            let _ = client.request(
                json!({"id": 1, "type": "config.set", "params": { "control.mode": value }}),
            );
            if let Some(root) = state.bundle_root.as_ref() {
                let _ = update_runtime_toml(root, "runtime.control.mode", value);
            }
            restart_required = true;
            message = "Saved. Restart required.".to_string();
        }
        SettingKey::WebListen => {
            let _ = client
                .request(json!({"id": 1, "type": "config.set", "params": { "web.listen": value }}));
            if let Some(root) = state.bundle_root.as_ref() {
                let _ = update_runtime_toml(root, "runtime.web.listen", value);
            }
            restart_required = true;
            message = "Saved. Restart required.".to_string();
        }
        SettingKey::WebAuth => {
            let _ = client
                .request(json!({"id": 1, "type": "config.set", "params": { "web.auth": value }}));
            if let Some(root) = state.bundle_root.as_ref() {
                let _ = update_runtime_toml(root, "runtime.web.auth", value);
            }
            restart_required = true;
            message = "Saved. Restart required.".to_string();
        }
        SettingKey::DiscoveryEnabled => {
            if let Some(enabled) = parse_bool_value(value) {
                let _ = client.request(json!({
                    "id": 1,
                    "type": "config.set",
                    "params": { "discovery.enabled": enabled }
                }));
                if let Some(root) = state.bundle_root.as_ref() {
                    let _ = update_runtime_toml(
                        root,
                        "runtime.discovery.enabled",
                        &enabled.to_string(),
                    );
                }
                restart_required = true;
                message = "Saved. Restart required.".to_string();
            } else {
                ok = false;
                message = "Use true/false.".to_string();
            }
        }
        SettingKey::MeshEnabled => {
            if let Some(enabled) = parse_bool_value(value) {
                let _ = client.request(
                    json!({"id": 1, "type": "config.set", "params": { "mesh.enabled": enabled }}),
                );
                if let Some(root) = state.bundle_root.as_ref() {
                    let _ = update_runtime_toml(root, "runtime.mesh.enabled", &enabled.to_string());
                }
                restart_required = true;
                message = "Saved. Restart required.".to_string();
            } else {
                ok = false;
                message = "Use true/false.".to_string();
            }
        }
    }

    Ok(SettingApplyResult {
        ok,
        restart_required,
        message,
    })
}

pub(super) fn format_setting_key(key: SettingKey) -> &'static str {
    match key {
        SettingKey::PlcName => "PLC name",
        SettingKey::CycleInterval => "Cycle interval (ms)",
        SettingKey::LogLevel => "Log level",
        SettingKey::ControlMode => "Control mode",
        SettingKey::WebListen => "Web listen",
        SettingKey::WebAuth => "Web auth",
        SettingKey::DiscoveryEnabled => "Discovery enabled",
        SettingKey::MeshEnabled => "PLC Linking enabled",
    }
}
fn settings_menu_entries(state: &UiState) -> Vec<SettingsMenuEntry> {
    let settings = state.data.settings.clone().unwrap_or_default();
    let name = state
        .data
        .status
        .as_ref()
        .map(|s| s.resource.as_str())
        .unwrap_or("plc")
        .to_string();
    vec![
        SettingsMenuEntry {
            key: Some(SettingKey::PlcName),
            label: "PLC name",
            value: name,
        },
        SettingsMenuEntry {
            key: Some(SettingKey::CycleInterval),
            label: "Cycle interval",
            value: settings
                .cycle_interval_ms
                .or_else(|| read_cycle_interval_ms(state))
                .map(|ms| format!("{ms} ms"))
                .unwrap_or_else(|| "--".to_string()),
        },
        SettingsMenuEntry {
            key: Some(SettingKey::LogLevel),
            label: "Log level",
            value: settings.log_level,
        },
        SettingsMenuEntry {
            key: Some(SettingKey::ControlMode),
            label: "Control mode",
            value: settings.control_mode,
        },
        SettingsMenuEntry {
            key: Some(SettingKey::WebListen),
            label: "Web listen",
            value: settings.web_listen,
        },
        SettingsMenuEntry {
            key: Some(SettingKey::WebAuth),
            label: "Web auth",
            value: settings.web_auth,
        },
        SettingsMenuEntry {
            key: Some(SettingKey::DiscoveryEnabled),
            label: "Discovery",
            value: if settings.discovery_enabled {
                "enabled".to_string()
            } else {
                "disabled".to_string()
            },
        },
        SettingsMenuEntry {
            key: Some(SettingKey::MeshEnabled),
            label: "PLC Linking",
            value: if settings.mesh_enabled {
                "enabled".to_string()
            } else {
                "disabled".to_string()
            },
        },
        SettingsMenuEntry {
            key: None,
            label: "Back",
            value: String::new(),
        },
    ]
}

fn read_cycle_interval_ms(state: &UiState) -> Option<u64> {
    let root = state.bundle_root.as_ref()?;
    let path = root.join("runtime.toml");
    let text = fs::read_to_string(path).ok()?;
    let doc: toml::Value = text.parse().ok()?;
    doc.get("resource")?
        .get("cycle_interval_ms")?
        .as_integer()
        .map(|value| value as u64)
}

fn normalize_setting_input(key: SettingKey, value: &str) -> String {
    match key {
        SettingKey::CycleInterval => value
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_string(),
        SettingKey::DiscoveryEnabled | SettingKey::MeshEnabled => {
            if value.eq_ignore_ascii_case("enabled") {
                "true".to_string()
            } else if value.eq_ignore_ascii_case("disabled") {
                "false".to_string()
            } else {
                value.to_string()
            }
        }
        _ => value.to_string(),
    }
}

pub(super) fn settings_menu_lines(state: &UiState, selected: usize) -> Vec<PromptLine> {
    let entries = settings_menu_entries(state);
    let mut lines = Vec::new();
    lines.push(PromptLine::plain("Settings", header_style()));
    for (idx, entry) in entries.iter().enumerate() {
        let highlight = idx == selected;
        if highlight {
            let line = if entry.key.is_some() {
                format!("{:<16} {}", entry.label, entry.value)
            } else {
                entry.label.to_string()
            };
            lines.push(PromptLine::plain(
                line,
                Style::default()
                    .bg(COLOR_TEAL)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            ));
            continue;
        }
        if entry.key.is_some() {
            lines.push(PromptLine::from_segments(vec![
                seg(format!("{:<16} ", entry.label), label_style()),
                seg(entry.value.clone(), value_style()),
            ]));
        } else {
            lines.push(PromptLine::from_segments(vec![seg(
                entry.label,
                label_style(),
            )]));
        }
    }
    lines.push(PromptLine::plain(
        "Use ↑/↓ and Enter. Esc to go back.",
        Style::default().fg(COLOR_INFO),
    ));
    lines
}

pub(super) fn move_settings_selection(state: &mut UiState, delta: i32) {
    let entries = settings_menu_entries(state);
    let len = entries.len();
    if len == 0 {
        return;
    }
    let mut next = state.settings_index as i32 + delta;
    if next < 0 {
        next = len as i32 - 1;
    } else if next >= len as i32 {
        next = 0;
    }
    state.settings_index = next as usize;
    state
        .prompt
        .set_output(settings_menu_lines(state, state.settings_index));
}
