use super::*;

use super::control_config::{
    config_set, is_bool_value, parse_bool_value, parse_toml_value, set_simple_response,
    set_toml_value,
};
use super::io_nav::{handle_io_command, open_io_select};
use super::settings::{apply_setting, settings_menu_lines};

use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

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

fn io_state() -> UiState {
    let mut state = state();
    state.data.io = vec![
        IoEntry {
            name: "Input".to_string(),
            address: "IX0.0".to_string(),
            value: "false".to_string(),
            direction: "IN".to_string(),
        },
        IoEntry {
            name: "OutputBool".to_string(),
            address: "QX0.0".to_string(),
            value: "Bool(false)".to_string(),
            direction: "OUT".to_string(),
        },
        IoEntry {
            name: "OutputWord".to_string(),
            address: "QW2".to_string(),
            value: "17".to_string(),
            direction: "out".to_string(),
        },
    ];
    state
}

fn output_text(state: &UiState) -> String {
    state
        .prompt
        .output
        .iter()
        .flat_map(|line| line.segments.iter().map(|(text, _)| text.as_str()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn scripted_client(
    responses: Vec<serde_json::Value>,
) -> (
    ControlClient,
    Arc<Mutex<Vec<serde_json::Value>>>,
    JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind command control server");
    let address = listener.local_addr().expect("read command server address");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&requests);
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept command client");
        let reader_stream = stream.try_clone().expect("clone command stream");
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
                serde_json::from_str::<serde_json::Value>(&line).expect("decode command request");
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
                .expect("write command response");
        }
    });
    let client = ControlClient::connect(ControlEndpoint::Tcp(address), None)
        .expect("connect command client");
    (client, requests, server)
}

fn finish_client(client: ControlClient, server: JoinHandle<()>) {
    drop(client);
    server.join().expect("join command control server");
}

#[test]
fn boolean_setting_aliases_are_trimmed_case_insensitive_and_closed() {
    for source in ["true", " TRUE ", "1", "yes", "On", "enable", "ENABLED"] {
        assert_eq!(parse_bool_value(source), Some(true), "source={source:?}");
    }
    for source in ["false", " FALSE ", "0", "no", "Off", "disable", "DISABLED"] {
        assert_eq!(parse_bool_value(source), Some(false), "source={source:?}");
    }
    for source in ["", "2", "truthy", "enabled later"] {
        assert_eq!(parse_bool_value(source), None, "source={source:?}");
    }
}

#[test]
fn toml_scalar_parser_preserves_boolean_integer_and_trimmed_string_types() {
    assert_eq!(parse_toml_value(" TRUE "), toml::Value::Boolean(true));
    assert_eq!(parse_toml_value("false"), toml::Value::Boolean(false));
    assert_eq!(parse_toml_value(" -42 "), toml::Value::Integer(-42));
    assert_eq!(
        parse_toml_value("  debug  "),
        toml::Value::String("debug".to_string())
    );
    assert_eq!(
        parse_toml_value("1.5"),
        toml::Value::String("1.5".to_string())
    );
}

#[test]
fn nested_toml_update_creates_tables_and_preserves_siblings() {
    let mut document: toml::Value =
        toml::from_str("title = \"line-a\"\n[runtime]\nkeep = 7\n").expect("parse TOML");
    set_toml_value(&mut document, "runtime.web.auth", "token").expect("set nested value");
    set_toml_value(&mut document, "runtime.enabled", "true").expect("set Boolean value");

    assert_eq!(document["title"].as_str(), Some("line-a"));
    assert_eq!(document["runtime"]["keep"].as_integer(), Some(7));
    assert_eq!(document["runtime"]["web"]["auth"].as_str(), Some("token"));
    assert_eq!(document["runtime"]["enabled"].as_bool(), Some(true));
}

#[test]
fn nested_toml_update_rejects_traversing_a_scalar() {
    let mut document: toml::Value =
        toml::from_str("runtime = \"not-a-table\"\n").expect("parse TOML");
    let error = set_toml_value(&mut document, "runtime.web.auth", "token")
        .expect_err("scalar traversal must fail");
    assert!(error.to_string().contains("invalid toml path"));
    assert_eq!(document["runtime"].as_str(), Some("not-a-table"));
}

#[test]
fn boolean_io_detection_accepts_only_canonical_boolean_renderings() {
    for source in ["true", " FALSE ", "Bool(true)", "Bool(false)"] {
        assert!(is_bool_value(source), "source={source:?}");
    }
    for source in ["", "1", "not Bool(true)", "MyBool(false)", "Boolish(true)"] {
        assert!(!is_bool_value(source), "source={source:?}");
    }
}

#[test]
fn simple_response_distinguishes_success_protocol_error_and_transport_error() {
    let mut state = state();
    set_simple_response(
        &mut state,
        Ok(json!({"ok": true, "result": {}})),
        "accepted",
    );
    assert_eq!(output_text(&state), "accepted");

    set_simple_response(
        &mut state,
        Ok(json!({"ok": false, "error": "denied"})),
        "must not appear",
    );
    assert_eq!(output_text(&state), "denied");

    set_simple_response(
        &mut state,
        Err(anyhow::anyhow!("connection closed")),
        "must not appear",
    );
    assert!(output_text(&state).contains("connection closed"));
}

#[test]
fn config_set_preserves_error_and_restart_required_truth() {
    let (mut client, requests, server) = scripted_client(vec![
        json!({"ok": false, "error": "read only"}),
        json!({"ok": true, "result": {"restart_required": ["control.mode"]}}),
    ]);
    let denied = config_set(&mut client, json!({"control.mode": "rw"}));
    assert!(!denied.ok);
    assert!(!denied.restart_required);
    assert_eq!(denied.error.as_deref(), Some("read only"));

    let accepted = config_set(&mut client, json!({"control.mode": "rw"}));
    assert!(accepted.ok);
    assert!(accepted.restart_required);
    assert_eq!(accepted.error, None);
    assert_eq!(requests.lock().expect("lock request log").len(), 2);
    finish_client(client, server);
}

#[test]
fn settings_navigation_wraps_over_all_entries() {
    let mut state = state();
    state.settings_index = 0;
    move_settings_selection(&mut state, -1);
    assert_eq!(state.settings_index, 8);
    move_settings_selection(&mut state, 1);
    assert_eq!(state.settings_index, 0);
    assert_eq!(settings_menu_lines(&state, 0).len(), 11);
}

#[test]
fn settings_selection_is_one_based_and_invalid_choice_retains_mode() {
    let (mut client, _, server) = scripted_client(Vec::new());
    let mut state = state();
    state.prompt.mode = PromptMode::SettingsSelect;
    assert!(!handle_settings_select("999", &mut client, &mut state)
        .expect("reject invalid setting selection"));
    assert_eq!(state.prompt.mode, PromptMode::SettingsSelect);
    assert!(output_text(&state).contains("Invalid choice."));

    assert!(!handle_settings_select("1", &mut client, &mut state).expect("select first setting"));
    assert_eq!(
        state.prompt.mode,
        PromptMode::SettingsValue(SettingKey::PlcName)
    );
    assert!(state.prompt.active);
    finish_client(client, server);
}

#[test]
fn project_owned_settings_fail_without_project_path() {
    let (mut client, requests, server) = scripted_client(Vec::new());
    let mut state = state();
    let name = apply_setting(SettingKey::PlcName, "line-a", &mut client, &mut state)
        .expect("apply PLC name");
    assert!(!name.ok);
    assert!(!name.restart_required);
    assert!(name.message.contains("Project path required"));

    let cycle = apply_setting(SettingKey::CycleInterval, "20", &mut client, &mut state)
        .expect("apply cycle interval");
    assert!(!cycle.ok);
    assert!(!cycle.restart_required);
    assert!(cycle.message.contains("Project path required"));
    assert!(requests.lock().expect("lock request log").is_empty());
    finish_client(client, server);
}

#[test]
fn invalid_boolean_setting_fails_before_control_request() {
    let (mut client, requests, server) = scripted_client(Vec::new());
    let mut state = state();
    for key in [SettingKey::DiscoveryEnabled, SettingKey::MeshEnabled] {
        let result = apply_setting(key, "sometimes", &mut client, &mut state)
            .expect("reject invalid Boolean");
        assert!(!result.ok);
        assert!(!result.restart_required);
        assert_eq!(result.message, "Use true/false.");
    }
    assert!(requests.lock().expect("lock request log").is_empty());
    finish_client(client, server);
}

#[test]
fn live_setting_does_not_report_saved_after_control_rejection() {
    let (mut client, requests, server) =
        scripted_client(vec![json!({"ok": false, "error": "policy denied"})]);
    let mut state = state();
    let result = apply_setting(SettingKey::LogLevel, "debug", &mut client, &mut state)
        .expect("apply live log level");
    assert!(!result.ok);
    assert!(!result.restart_required);
    assert!(result.message.contains("policy denied"));
    assert_eq!(requests.lock().expect("lock request log").len(), 1);
    finish_client(client, server);
}

#[test]
fn restart_setting_does_not_open_restart_path_after_control_rejection() {
    let (mut client, _, server) = scripted_client(vec![json!({"ok": false, "error": "read only"})]);
    let mut state = state();
    let result = apply_setting(SettingKey::ControlMode, "rw", &mut client, &mut state)
        .expect("apply control mode");
    assert!(!result.ok);
    assert!(!result.restart_required);
    assert!(result.message.contains("read only"));
    finish_client(client, server);
}

#[test]
fn watch_command_requires_name_and_duplicate_watch_is_idempotent() {
    let (mut client, _, server) = scripted_client(Vec::new());
    let mut state = state();
    execute_command("/watch", &mut client, &mut state).expect("missing watch name");
    assert!(output_text(&state).contains("Usage: /watch <name>"));

    execute_command("/watch MAIN.count", &mut client, &mut state).expect("add watch");
    execute_command("/watch MAIN.count", &mut client, &mut state).expect("repeat watch");
    assert_eq!(state.watch_list, ["MAIN.count"]);
    execute_command("/unwatch all", &mut client, &mut state).expect("clear watches");
    assert!(state.watch_list.is_empty());
    assert!(state.watch_values.is_empty());
    finish_client(client, server);
}

#[test]
fn beginner_mode_blocks_non_beginner_command_before_mutation() {
    let (mut client, requests, server) = scripted_client(Vec::new());
    let mut state = state();
    state.beginner_mode = true;
    state
        .alerts
        .push_back(PromptLine::plain("retain", Style::default().fg(COLOR_INFO)));
    execute_command("/clear", &mut client, &mut state).expect("block advanced clear");
    assert_eq!(state.alerts.len(), 1);
    assert!(output_text(&state).contains("Beginner mode"));
    assert!(requests.lock().expect("lock request log").is_empty());
    finish_client(client, server);
}

#[test]
fn io_selection_filters_mutating_actions_to_outputs_and_wraps() {
    let mut state = io_state();
    open_io_select(IoActionKind::Set, &mut state);
    assert_eq!(state.prompt.mode, PromptMode::IoSelect(IoActionKind::Set));
    let text = output_text(&state);
    assert!(!text.contains("IX0.0"));
    assert!(text.contains("QX0.0"));
    assert!(text.contains("QW2"));

    state.io_index = 0;
    move_io_selection(&mut state, IoActionKind::Set, -1);
    assert_eq!(state.io_index, 1);
    move_io_selection(&mut state, IoActionKind::Set, 1);
    assert_eq!(state.io_index, 0);

    open_io_select(IoActionKind::Read, &mut state);
    assert!(output_text(&state).contains("IX0.0"));
}

#[test]
fn io_selection_zero_and_out_of_range_are_explicit_errors() {
    let (mut client, _, server) = scripted_client(Vec::new());
    let mut state = io_state();
    open_io_select(IoActionKind::Set, &mut state);
    handle_io_select("0", IoActionKind::Set, &mut client, &mut state)
        .expect("reject zero selection");
    assert_eq!(state.prompt.mode, PromptMode::IoSelect(IoActionKind::Set));
    assert!(output_text(&state).contains("Invalid choice."));

    handle_io_select("3", IoActionKind::Set, &mut client, &mut state)
        .expect("reject out-of-range selection");
    assert_eq!(state.prompt.mode, PromptMode::IoSelect(IoActionKind::Set));
    assert!(output_text(&state).contains("Invalid choice."));
    finish_client(client, server);
}

#[test]
fn boolean_and_non_boolean_outputs_choose_distinct_value_paths() {
    let (mut client, _, server) = scripted_client(Vec::new());
    let mut state = io_state();
    open_io_select(IoActionKind::Set, &mut state);
    handle_io_select("1", IoActionKind::Set, &mut client, &mut state)
        .expect("select Boolean output");
    assert_eq!(state.prompt.mode, PromptMode::IoValueSelect);
    assert_eq!(state.io_pending_address.as_deref(), Some("QX0.0"));
    assert!(output_text(&state).contains("TRUE"));
    assert!(output_text(&state).contains("FALSE"));

    open_io_select(IoActionKind::Set, &mut state);
    handle_io_select("2", IoActionKind::Set, &mut client, &mut state)
        .expect("select non-Boolean output");
    assert_eq!(state.prompt.mode, PromptMode::Normal);
    assert!(state.prompt.active);
    assert_eq!(state.prompt.input, "/io set QW2 ");
    assert!(output_text(&state).contains("Enter value:"));
    finish_client(client, server);
}

#[test]
fn force_marker_changes_only_after_successful_control_response() {
    let (mut client, _, server) = scripted_client(vec![
        json!({"ok": false, "error": "denied"}),
        json!({"ok": true, "result": {}}),
    ]);
    let mut state = io_state();
    handle_io_command(vec!["force", "QX0.0", "true"], &mut client, &mut state)
        .expect("rejected force");
    assert!(!state.forced_io.contains("QX0.0"));
    assert!(output_text(&state).contains("denied"));

    handle_io_command(vec!["force", "QW2", "17"], &mut client, &mut state).expect("accepted force");
    assert!(state.forced_io.contains("QW2"));
    assert!(output_text(&state).contains("I/O forced."));
    finish_client(client, server);
}

#[test]
fn unforce_marker_is_retained_until_control_response_succeeds() {
    let (mut client, _, server) = scripted_client(vec![
        json!({"ok": false, "error": "denied"}),
        json!({"ok": true, "result": {}}),
    ]);
    let mut state = io_state();
    state.forced_io.insert("QX0.0".to_string());
    state.forced_io.insert("QW2".to_string());

    handle_io_command(vec!["unforce", "QX0.0"], &mut client, &mut state).expect("rejected unforce");
    assert!(state.forced_io.contains("QX0.0"));
    handle_io_command(vec!["unforce", "QW2"], &mut client, &mut state).expect("accepted unforce");
    assert!(!state.forced_io.contains("QW2"));
    finish_client(client, server);
}

#[test]
fn unforce_all_retains_failed_markers_and_reports_partial_failure() {
    let (mut client, _, server) = scripted_client(vec![
        json!({"ok": true, "result": {}}),
        json!({"ok": false, "error": "release denied"}),
    ]);
    let mut state = io_state();
    state.forced_io.insert("QX0.0".to_string());
    state.forced_io.insert("QW2".to_string());
    handle_io_command(vec!["unforce", "all"], &mut client, &mut state)
        .expect("partially rejected unforce-all");
    assert_eq!(state.forced_io.len(), 1);
    assert!(output_text(&state).contains("release denied"));
    assert!(!output_text(&state).contains("All forced I/O released."));
    finish_client(client, server);
}

#[test]
fn unknown_io_subcommand_is_visible_and_does_not_issue_request() {
    let (mut client, requests, server) = scripted_client(Vec::new());
    let mut state = io_state();
    handle_io_command(vec!["mystery"], &mut client, &mut state)
        .expect("reject unknown I/O command");
    assert!(output_text(&state).contains("Unknown /io command."));
    assert!(requests.lock().expect("lock request log").is_empty());
    finish_client(client, server);
}
