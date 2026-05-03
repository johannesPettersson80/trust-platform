use super::menu_nav::open_menu;
use super::*;

pub(super) struct ConfigSetResult {
    pub(super) ok: bool,
    pub(super) restart_required: bool,
    pub(super) error: Option<String>,
}
pub(super) fn config_set(client: &mut ControlClient, params: serde_json::Value) -> ConfigSetResult {
    let response = client.request(json!({"id": 1, "type": "config.set", "params": params}));
    if let Ok(value) = response {
        if let Some(err) = value.get("error").and_then(|v| v.as_str()) {
            return ConfigSetResult {
                ok: false,
                restart_required: false,
                error: Some(err.to_string()),
            };
        }
        let restart_required = value
            .get("result")
            .and_then(|r| r.get("restart_required"))
            .and_then(|v| v.as_array())
            .map(|arr| !arr.is_empty())
            .unwrap_or(false);
        return ConfigSetResult {
            ok: true,
            restart_required,
            error: None,
        };
    }
    ConfigSetResult {
        ok: false,
        restart_required: false,
        error: Some("request failed".to_string()),
    }
}

pub(super) fn set_config_response(state: &mut UiState, result: ConfigSetResult, success: &str) {
    if !result.ok {
        state.prompt.set_output(vec![PromptLine::plain(
            result.error.unwrap_or_else(|| "error".into()),
            Style::default().fg(COLOR_RED),
        )]);
        return;
    }
    if result.restart_required {
        open_menu(MenuKind::Restart, state);
    } else {
        state.prompt.set_output(vec![PromptLine::plain(
            success,
            Style::default().fg(COLOR_GREEN),
        )]);
    }
}

pub(super) fn set_simple_response(
    state: &mut UiState,
    response: anyhow::Result<serde_json::Value>,
    success: &str,
) {
    match response {
        Ok(value) => {
            if let Some(err) = value.get("error").and_then(|v| v.as_str()) {
                state.prompt.set_output(vec![PromptLine::plain(
                    err.to_string(),
                    Style::default().fg(COLOR_RED),
                )]);
            } else {
                state.prompt.set_output(vec![PromptLine::plain(
                    success.to_string(),
                    Style::default().fg(COLOR_GREEN),
                )]);
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

pub(super) fn update_runtime_toml(root: &Path, key: &str, value: &str) -> anyhow::Result<()> {
    let path = root.join("runtime.toml");
    let text = fs::read_to_string(&path)?;
    let mut doc: toml::Value = text.parse()?;
    set_toml_value(&mut doc, key, value)?;
    let output = toml::to_string_pretty(&doc)?;
    crate::config::validate_runtime_toml_text(&output)
        .map_err(|err| anyhow::anyhow!(err.to_string()))?;
    fs::write(&path, output)?;
    Ok(())
}

pub(super) fn set_toml_value(doc: &mut toml::Value, key: &str, value: &str) -> anyhow::Result<()> {
    let mut parts = key.split('.').peekable();
    let mut current = doc;
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            *current
                .as_table_mut()
                .ok_or_else(|| anyhow::anyhow!("invalid toml path"))?
                .entry(part)
                .or_insert(toml::Value::String(value.to_string())) = parse_toml_value(value);
            return Ok(());
        }
        current = current
            .as_table_mut()
            .ok_or_else(|| anyhow::anyhow!("invalid toml path"))?
            .entry(part)
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
    }
    Ok(())
}

pub(super) fn parse_toml_value(value: &str) -> toml::Value {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("true") {
        return toml::Value::Boolean(true);
    }
    if trimmed.eq_ignore_ascii_case("false") {
        return toml::Value::Boolean(false);
    }
    if let Ok(number) = trimmed.parse::<i64>() {
        return toml::Value::Integer(number);
    }
    toml::Value::String(trimmed.to_string())
}

pub(super) fn parse_bool_value(value: &str) -> Option<bool> {
    let trimmed = value.trim().to_ascii_lowercase();
    match trimmed.as_str() {
        "true" | "1" | "yes" | "on" | "enable" | "enabled" => Some(true),
        "false" | "0" | "no" | "off" | "disable" | "disabled" => Some(false),
        _ => None,
    }
}

pub(super) fn is_bool_value(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("true") || trimmed.eq_ignore_ascii_case("false") {
        return true;
    }
    trimmed.starts_with("Bool(") || trimmed.contains("Bool(")
}
