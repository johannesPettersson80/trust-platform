use super::input::{handle_prompt_confirm, handle_prompt_key};
use super::*;

use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{SystemTime, UNIX_EPOCH};

fn output_text(state: &UiState) -> String {
    state
        .prompt
        .output
        .iter()
        .flat_map(|line| line.segments.iter().map(|(text, _)| text.as_str()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn state() -> UiState {
    UiState::new(
        vec![
            PanelKind::Cycle,
            PanelKind::Io,
            PanelKind::Status,
            PanelKind::Events,
        ],
        false,
        None,
    )
}

fn scripted_client(
    responses: Vec<serde_json::Value>,
) -> (
    ControlClient,
    Arc<Mutex<Vec<serde_json::Value>>>,
    JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind scripted control server");
    let address = listener.local_addr().expect("read scripted address");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&requests);
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept console client");
        let reader_stream = stream.try_clone().expect("clone request stream");
        let mut reader = io::BufReader::new(reader_stream);
        let mut writer = stream;
        let mut response_index = 0usize;
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            let request =
                serde_json::from_str::<serde_json::Value>(&line).expect("decode console request");
            captured.lock().expect("lock request log").push(request);
            let response = responses
                .get(response_index)
                .cloned()
                .unwrap_or_else(|| json!({"ok": true, "result": {}}));
            response_index += 1;
            writer
                .write_all(response.to_string().as_bytes())
                .and_then(|_| writer.write_all(b"\n"))
                .and_then(|_| writer.flush())
                .expect("write scripted response");
        }
    });
    let client = ControlClient::connect(ControlEndpoint::Tcp(address), None)
        .expect("connect scripted client");
    (client, requests, server)
}

fn finish_scripted_client(client: ControlClient, server: JoinHandle<()>) {
    drop(client);
    server.join().expect("join scripted control server");
}

fn unique_console_root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-runtime-console-contract-{label}-{}-{nonce}",
        std::process::id()
    ))
}

#[test]
fn panel_names_are_trimmed_case_insensitive_and_closed() {
    let cases = [
        (" cycle ", Some(PanelKind::Cycle)),
        ("IO", Some(PanelKind::Io)),
        ("Status", Some(PanelKind::Status)),
        ("EVENTS", Some(PanelKind::Events)),
        ("tasks", Some(PanelKind::Tasks)),
        ("Watch", Some(PanelKind::Watch)),
        ("driver", None),
        ("", None),
    ];
    for (source, expected) in cases {
        assert_eq!(PanelKind::parse(source), expected, "source={source:?}");
    }
}

#[test]
fn prompt_activation_and_deactivation_reset_transient_navigation() {
    let mut prompt = PromptState::new();
    prompt.history = vec!["/status".to_string()];
    prompt.history_index = Some(0);
    prompt.activate_with("/io ");
    assert!(prompt.active);
    assert_eq!(prompt.input, "/io ");
    assert_eq!(prompt.cursor, "/io ".len());
    assert_eq!(prompt.history_index, None);

    prompt.history_index = Some(0);
    prompt.deactivate();
    assert!(!prompt.active);
    assert_eq!(prompt.cursor, 0);
    assert_eq!(prompt.history_index, None);
}

#[test]
fn prompt_history_ignores_blank_entries_and_saturates_at_oldest() {
    let mut prompt = PromptState::new();
    prompt.push_history(" ".to_string());
    prompt.push_history("/status".to_string());
    prompt.push_history("/io list".to_string());
    assert_eq!(prompt.history, ["/status", "/io list"]);

    prompt.history_prev();
    assert_eq!(prompt.input, "/io list");
    prompt.history_prev();
    assert_eq!(prompt.input, "/status");
    prompt.history_prev();
    assert_eq!(prompt.input, "/status");
    prompt.history_next();
    assert_eq!(prompt.input, "/io list");
    prompt.history_next();
    assert_eq!(prompt.input, "");
    assert_eq!(prompt.cursor, 0);
}

#[test]
fn prompt_suggestion_navigation_wraps_both_directions() {
    let mut prompt = PromptState::new();
    prompt.set_suggestions_list(vec![
        CommandHelp {
            cmd: "status",
            desc: "status",
            beginner: true,
        },
        CommandHelp {
            cmd: "info",
            desc: "info",
            beginner: true,
        },
    ]);
    assert_eq!(
        prompt.selected_suggestion().map(|entry| entry.cmd),
        Some("status")
    );
    prompt.move_suggestion(-1);
    assert_eq!(
        prompt.selected_suggestion().map(|entry| entry.cmd),
        Some("info")
    );
    prompt.move_suggestion(1);
    assert_eq!(
        prompt.selected_suggestion().map(|entry| entry.cmd),
        Some("status")
    );
}

#[test]
fn prompt_editing_preserves_utf8_character_boundaries() {
    let (mut client, _, server) = scripted_client(Vec::new());
    let mut state = state();
    state.prompt.activate_with("éa");

    handle_prompt_key(KeyEvent::from(KeyCode::Left), &mut client, &mut state)
        .expect("move left over ASCII character");
    handle_prompt_key(KeyEvent::from(KeyCode::Backspace), &mut client, &mut state)
        .expect("remove one complete Unicode scalar");

    assert_eq!(state.prompt.input, "a");
    assert_eq!(state.prompt.cursor, 0);
    finish_scripted_client(client, server);
}

#[test]
fn status_projection_requires_string_state_and_uses_closed_defaults() {
    assert!(parse_status(&json!({})).is_none());
    assert!(parse_status(&json!({"result": {"state": 7}})).is_none());

    let status =
        parse_status(&json!({"result": {"state": "running"}})).expect("minimal status response");
    assert_eq!(status.state, "running");
    assert_eq!(status.fault, "none");
    assert_eq!(status.resource, "resource");
    assert_eq!(status.uptime_ms, 0);
    assert_eq!(status.cycle_last, 0.0);
    assert_eq!(status.control_mode, "unknown");
    assert_eq!(status.simulation_mode, "production");
    assert_eq!(status.simulation_time_scale, 1);
}

#[test]
fn task_projection_preserves_order_and_defaults_missing_fields() {
    let tasks = parse_tasks(&json!({
        "result": [
            {"name": "slow", "last_ms": 3.0, "overruns": 2},
            {"name": "fast", "avg_ms": 0.25}
        ]
    }));
    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0].name, "slow");
    assert_eq!(tasks[0].last_ms, 3.0);
    assert_eq!(tasks[0].avg_ms, 0.0);
    assert_eq!(tasks[0].overruns, 2);
    assert_eq!(tasks[1].name, "fast");
    assert_eq!(tasks[1].avg_ms, 0.25);
    assert!(parse_tasks(&json!({"result": {}})).is_empty());
}

#[test]
fn io_projection_preserves_strings_and_json_scalar_spelling() {
    let io = parse_io(&json!({
        "result": [
            {"name": "Text", "address": "MW0", "value": "ready", "direction": "OUT"},
            {"name": "Flag", "address": "QX0.0", "value": true, "direction": "OUT"},
            {"name": "Count", "address": "MD2", "value": 17, "direction": "IN"}
        ]
    }));
    assert_eq!(
        io.iter()
            .map(|entry| entry.value.as_str())
            .collect::<Vec<_>>(),
        ["ready", "true", "17"]
    );
    assert_eq!(
        io.iter()
            .map(|entry| entry.address.as_str())
            .collect::<Vec<_>>(),
        ["MW0", "QX0.0", "MD2"]
    );
}

#[test]
fn event_projection_has_closed_level_mapping_and_preserves_order() {
    let events = parse_events(&json!({
        "result": [
            {"code": "A", "level": "fault"},
            {"code": "B", "level": "ERROR"},
            {"code": "C", "level": "warn"},
            {"code": "D", "level": "WARNING"},
            {"code": "E", "level": "debug"}
        ]
    }));
    assert_eq!(
        events
            .iter()
            .map(|event| event.label.as_str())
            .collect::<Vec<_>>(),
        ["A", "B", "C", "D", "E"]
    );
    assert_eq!(events[0].kind, EventKind::Fault);
    assert_eq!(events[1].kind, EventKind::Fault);
    assert_eq!(events[2].kind, EventKind::Warn);
    assert_eq!(events[3].kind, EventKind::Warn);
    assert_eq!(events[4].kind, EventKind::Info);
}

#[test]
fn settings_projection_requires_object_result_and_rejects_invalid_ranges() {
    assert!(parse_settings(&json!({})).is_none());
    assert!(parse_settings(&json!({"result": "not-an-object"})).is_none());

    let settings = parse_settings(&json!({
        "result": {
            "resource.cycle_interval_ms": -1,
            "simulation.time_scale": 4294967296_u64
        }
    }))
    .expect("settings object");
    assert_eq!(settings.cycle_interval_ms, None);
    assert_eq!(settings.simulation_time_scale, 1);
    assert_eq!(settings.log_level, "info");
    assert_eq!(settings.control_mode, "ro");
}

#[test]
fn console_config_preserves_recognized_layout_order_and_positive_refresh() {
    let root = unique_console_root("valid");
    fs::create_dir_all(&root).expect("create console root");
    fs::write(
        root.join("runtime.toml"),
        "[console]\nlayout = [\"watch\", \"UNKNOWN\", \"cycle\", \"IO\"]\nrefresh_ms = 750\n",
    )
    .expect("write console config");

    let config = load_console_config(&root);
    assert_eq!(
        config.layout,
        Some(vec![PanelKind::Watch, PanelKind::Cycle, PanelKind::Io])
    );
    assert_eq!(config.refresh_ms, Some(750));
    fs::remove_dir_all(root).expect("remove console root");
}

#[test]
fn console_config_falls_back_for_missing_malformed_or_unknown_only_layout() {
    let missing = unique_console_root("missing");
    fs::create_dir_all(&missing).expect("create missing config root");
    let config = load_console_config(&missing);
    assert_eq!(config.layout, None);
    assert_eq!(config.refresh_ms, None);
    fs::remove_dir_all(missing).expect("remove missing config root");

    let malformed = unique_console_root("malformed");
    fs::create_dir_all(&malformed).expect("create malformed config root");
    fs::write(malformed.join("runtime.toml"), "[console\n").expect("write malformed config");
    assert_eq!(load_console_config(&malformed).layout, None);
    fs::remove_dir_all(malformed).expect("remove malformed config root");

    let unknown = unique_console_root("unknown");
    fs::create_dir_all(&unknown).expect("create unknown config root");
    fs::write(
        unknown.join("runtime.toml"),
        "[console]\nlayout = [\"mystery\"]\n",
    )
    .expect("write unknown config");
    assert_eq!(load_console_config(&unknown).layout, None);
    fs::remove_dir_all(unknown).expect("remove unknown config root");
}

#[test]
fn console_config_rejects_zero_and_negative_refresh_intervals() {
    for (label, refresh) in [("zero", "0"), ("negative", "-1")] {
        let root = unique_console_root(label);
        fs::create_dir_all(&root).expect("create refresh config root");
        fs::write(
            root.join("runtime.toml"),
            format!("[console]\nrefresh_ms = {refresh}\n"),
        )
        .expect("write refresh config");
        assert_eq!(load_console_config(&root).refresh_ms, None, "{label}");
        fs::remove_dir_all(root).expect("remove refresh config root");
    }
}

#[test]
fn cycle_history_is_bounded_and_rounds_to_tenths() {
    let mut state = state();
    state.data.status = Some(StatusSnapshot {
        cycle_last: 1.26,
        ..StatusSnapshot::default()
    });
    for _ in 0..125 {
        update_cycle_history(&mut state);
    }
    assert_eq!(state.cycle_history.len(), 120);
    assert!(state.cycle_history.iter().all(|sample| *sample == 13));

    state.data.status.as_mut().expect("status").cycle_last = 0.0;
    update_cycle_history(&mut state);
    assert_eq!(state.cycle_history.back(), Some(&1));
}

#[test]
fn cycle_history_ignores_absent_non_finite_and_negative_samples() {
    let mut state = state();
    update_cycle_history(&mut state);
    assert!(state.cycle_history.is_empty());

    for sample in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.1] {
        state.data.status = Some(StatusSnapshot {
            cycle_last: sample,
            ..StatusSnapshot::default()
        });
        update_cycle_history(&mut state);
    }
    assert!(state.cycle_history.is_empty());
}

#[test]
fn alerts_retain_only_the_five_newest_entries() {
    let mut state = state();
    for index in 0..8 {
        push_alert(
            &mut state,
            &format!("alert-{index}"),
            Style::default().fg(COLOR_INFO),
        );
    }
    assert_eq!(state.alerts.len(), 5);
    let text = state
        .alerts
        .iter()
        .flat_map(|line| line.segments.iter().map(|(text, _)| text.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        text,
        ["alert-3", "alert-4", "alert-5", "alert-6", "alert-7"]
    );
}

#[test]
fn event_alert_identity_includes_timestamp_kind_and_message() {
    let mut state = state();
    state.data.events = vec![EventSnapshot {
        label: "E42".to_string(),
        kind: EventKind::Warn,
        timestamp: Some("2026-07-30T10:00:00Z".to_string()),
        message: "first".to_string(),
    }];
    update_event_alerts(&mut state);
    assert_eq!(state.alerts.len(), 1);

    state.data.events = vec![EventSnapshot {
        label: "E42".to_string(),
        kind: EventKind::Fault,
        timestamp: Some("2026-07-30T10:00:01Z".to_string()),
        message: "second".to_string(),
    }];
    update_event_alerts(&mut state);
    assert_eq!(state.alerts.len(), 2);
    assert!(output_text(&state).is_empty());
}

#[test]
fn informational_events_are_seen_without_becoming_alerts() {
    let mut state = state();
    state.data.events = vec![EventSnapshot {
        label: "I1".to_string(),
        kind: EventKind::Info,
        timestamp: None,
        message: "started".to_string(),
    }];
    update_event_alerts(&mut state);
    assert!(state.alerts.is_empty());
    assert_eq!(state.seen_events.len(), 1);
}

#[test]
fn read_only_mode_never_activates_prompt_or_sends_control_requests() {
    let (mut client, requests, server) = scripted_client(Vec::new());
    let mut state = state();
    assert!(!handle_key(
        KeyEvent::from(KeyCode::Char('/')),
        &mut client,
        &mut state,
        true,
    )
    .expect("read-only slash"));
    assert!(!state.prompt.active);
    assert!(output_text(&state).contains("Read-only mode."));
    assert!(handle_key(
        KeyEvent::from(KeyCode::Char('q')),
        &mut client,
        &mut state,
        true,
    )
    .expect("read-only quit"));
    assert!(requests.lock().expect("lock request log").is_empty());
    finish_scripted_client(client, server);
}

#[test]
fn disabled_debug_controls_do_not_issue_shortcut_requests() {
    let (mut client, requests, server) = scripted_client(Vec::new());
    let mut state = state();
    state.debug_controls = false;
    for key in ['p', 'r', 's', 'o', 'u'] {
        assert!(!handle_key(
            KeyEvent::from(KeyCode::Char(key)),
            &mut client,
            &mut state,
            false,
        )
        .expect("blocked debug shortcut"));
    }
    assert!(output_text(&state).contains("Debug controls disabled."));
    assert!(requests.lock().expect("lock request log").is_empty());
    finish_scripted_client(client, server);
}

#[test]
fn prompt_escape_and_control_c_return_to_inactive_normal_mode() {
    let (mut client, _, server) = scripted_client(Vec::new());
    let mut state = state();
    state.prompt.activate_with("/io set ");
    state.prompt.mode = PromptMode::SettingsValue(SettingKey::LogLevel);
    handle_prompt_key(KeyEvent::from(KeyCode::Esc), &mut client, &mut state)
        .expect("escape prompt");
    assert!(!state.prompt.active);
    assert_eq!(state.prompt.mode, PromptMode::Normal);

    state.prompt.activate_with("/status");
    state.prompt.mode = PromptMode::Menu(MenuKind::Control);
    handle_prompt_key(
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        &mut client,
        &mut state,
    )
    .expect("control-c prompt");
    assert!(!state.prompt.active);
    assert_eq!(state.prompt.mode, PromptMode::Normal);
    finish_scripted_client(client, server);
}

#[test]
fn confirmation_accepts_only_yes_and_cancels_every_other_response() {
    let (mut client, requests, server) = scripted_client(vec![json!({"ok": true, "result": {}})]);
    let mut state = state();
    assert!(
        !handle_prompt_confirm("later", ConfirmAction::Shutdown, &mut client, &mut state,)
            .expect("cancel shutdown")
    );
    assert!(output_text(&state).contains("Cancelled."));
    assert!(requests.lock().expect("lock request log").is_empty());

    assert!(
        !handle_prompt_confirm(" YES ", ConfirmAction::RestartCold, &mut client, &mut state,)
            .expect("confirm cold restart")
    );
    let captured = requests.lock().expect("lock request log");
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0]["type"], "restart");
    assert_eq!(captured[0]["params"]["mode"], "cold");
    drop(captured);
    finish_scripted_client(client, server);
}

#[test]
fn fetch_transaction_uses_stable_order_ids_limit_and_auth() {
    let responses = vec![
        json!({"result": {"state": "running"}}),
        json!({"result": []}),
        json!({"result": []}),
        json!({"result": []}),
        json!({"result": {}}),
    ];
    let (mut client, requests, server) = scripted_client(responses);
    client.token = Some("secret-token".to_string());
    let data = fetch_data(&mut client).expect("fetch complete console snapshot");
    assert!(data.status.is_some());
    assert!(data.tasks.is_empty());
    assert!(data.io.is_empty());
    assert!(data.events.is_empty());
    assert!(data.settings.is_some());

    let captured = requests.lock().expect("lock request log");
    assert_eq!(captured.len(), 5);
    assert_eq!(
        captured
            .iter()
            .map(|request| request["type"].as_str().expect("request type"))
            .collect::<Vec<_>>(),
        [
            "status",
            "tasks.stats",
            "io.list",
            "events.tail",
            "config.get"
        ]
    );
    assert_eq!(
        captured
            .iter()
            .map(|request| request["id"].as_u64().expect("request id"))
            .collect::<Vec<_>>(),
        [1, 2, 3, 4, 5]
    );
    assert_eq!(captured[3]["params"]["limit"], 20);
    assert!(captured
        .iter()
        .all(|request| request["auth"] == "secret-token"));
    drop(captured);
    finish_scripted_client(client, server);
}
