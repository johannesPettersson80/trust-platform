use super::*;

pub(super) fn push_alert(state: &mut UiState, text: &str, style: Style) {
    if state.alerts.len() > 4 {
        state.alerts.pop_front();
    }
    state
        .alerts
        .push_back(PromptLine::plain(text.to_string(), style));
}

pub(super) fn update_cycle_history(state: &mut UiState) {
    let status = match state.data.status.as_ref() {
        Some(status) => status,
        None => return,
    };
    let value = (status.cycle_last * 10.0).max(0.0).round() as u64;
    if state.cycle_history.len() >= 120 {
        state.cycle_history.pop_front();
    }
    state.cycle_history.push_back(value.max(1));
}

pub(super) fn update_watch_values(client: &mut ControlClient, state: &mut UiState) {
    if state.watch_list.is_empty() {
        state.watch_values.clear();
        return;
    }
    let mut out = Vec::new();
    for name in state.watch_list.iter() {
        let response = client.request(json!({
            "id": 1,
            "type": "eval",
            "params": { "expr": name }
        }));
        match response {
            Ok(value) => {
                if let Some(result) = value.get("result").and_then(|r| r.get("value")) {
                    out.push((name.clone(), result.to_string()));
                } else if let Some(err) = value.get("error").and_then(|e| e.as_str()) {
                    out.push((name.clone(), format!("error: {err}")));
                } else {
                    out.push((name.clone(), "unknown".to_string()));
                }
            }
            Err(_) => out.push((name.clone(), "unavailable".to_string())),
        }
    }
    state.watch_values = out;
}

pub(super) fn update_event_alerts(state: &mut UiState) {
    let events = state.data.events.clone();
    for event in events {
        if state.seen_events.contains(&event.label) {
            continue;
        }
        state.seen_events.insert(event.label.clone());
        match event.kind {
            EventKind::Fault => push_alert(
                state,
                &format!("[FAULT] {}", event.message),
                Style::default().fg(COLOR_RED),
            ),
            EventKind::Warn => push_alert(
                state,
                &format!("[WARN] {}", event.message),
                Style::default().fg(COLOR_AMBER),
            ),
            EventKind::Info => {}
        }
        if state.seen_events.len() > 400 {
            state.seen_events.clear();
        }
    }
}
