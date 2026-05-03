use super::*;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use serde_json::json;
use std::net::TcpListener;
use std::thread;

fn test_client() -> ControlClient {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test control socket");
    let addr = listener.local_addr().expect("read local addr");

    thread::spawn(move || {
        let Ok((stream, _)) = listener.accept() else {
            return;
        };
        let reader_stream = stream.try_clone().expect("clone stream for request reader");
        let mut reader = io::BufReader::new(reader_stream);
        let mut writer = stream;
        loop {
            let mut request = String::new();
            match reader.read_line(&mut request) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            let id = serde_json::from_str::<serde_json::Value>(&request)
                .ok()
                .and_then(|value| value.get("id").cloned())
                .unwrap_or_else(|| json!(1));
            let response = json!({
                "id": id,
                "ok": true,
                "result": {}
            });
            if writer
                .write_all(response.to_string().as_bytes())
                .and_then(|_| writer.write_all(b"\n"))
                .and_then(|_| writer.flush())
                .is_err()
            {
                break;
            }
        }
    });

    ControlClient::connect(ControlEndpoint::Tcp(addr), None).expect("connect test client")
}

fn sample_state() -> UiState {
    let mut forced_io = HashSet::new();
    forced_io.insert("QX0.0".to_string());

    UiState {
        data: UiData {
            status: Some(StatusSnapshot {
                state: "running".to_string(),
                fault: "none".to_string(),
                resource: "SIM_RESOURCE".to_string(),
                uptime_ms: 12_345,
                cycle_min: 0.2,
                cycle_avg: 0.4,
                cycle_max: 0.9,
                cycle_last: 0.5,
                overruns: 0,
                faults: 0,
                drivers: vec![DriverSnapshot {
                    name: "sim".to_string(),
                    status: "ok".to_string(),
                    error: None,
                }],
                debug_enabled: true,
                control_mode: "rw".to_string(),
                simulation_mode: "production".to_string(),
                simulation_time_scale: 1,
                simulation_warning: String::new(),
            }),
            tasks: vec![TaskSnapshot {
                name: "MainTask".to_string(),
                last_ms: 0.5,
                avg_ms: 0.4,
                max_ms: 0.9,
                overruns: 0,
            }],
            io: vec![
                IoEntry {
                    name: "MotorStart".to_string(),
                    address: "QX0.0".to_string(),
                    value: "true".to_string(),
                    direction: "OUT".to_string(),
                },
                IoEntry {
                    name: "LevelLow".to_string(),
                    address: "IX0.1".to_string(),
                    value: "false".to_string(),
                    direction: "IN".to_string(),
                },
            ],
            events: vec![EventSnapshot {
                label: "EVT001".to_string(),
                kind: EventKind::Info,
                timestamp: Some("2026-01-01T00:00:00Z".to_string()),
                message: "Started".to_string(),
            }],
            settings: Some(SettingsSnapshot {
                cycle_interval_ms: Some(100),
                log_level: "info".to_string(),
                watchdog_enabled: true,
                watchdog_timeout_ms: 500,
                watchdog_action: "halt".to_string(),
                fault_policy: "safe_halt".to_string(),
                retain_mode: "none".to_string(),
                retain_save_interval_ms: None,
                web_listen: "127.0.0.1:8080".to_string(),
                web_auth: "local".to_string(),
                discovery_enabled: false,
                mesh_enabled: false,
                mesh_publish: Vec::new(),
                mesh_subscribe: Vec::new(),
                control_mode: "rw".to_string(),
                simulation_enabled: false,
                simulation_time_scale: 1,
                simulation_mode: "production".to_string(),
                simulation_warning: String::new(),
            }),
        },
        pending_confirm: None,
        beginner_mode: false,
        debug_controls: true,
        prompt: PromptState::new(),
        layout: vec![
            PanelKind::Cycle,
            PanelKind::Io,
            PanelKind::Status,
            PanelKind::Events,
        ],
        focus: None,
        panel_page: 0,
        settings_index: 0,
        menu_index: 0,
        io_index: 0,
        io_value_index: 0,
        io_pending_address: None,
        io_pending_action: None,
        cycle_history: VecDeque::from([2, 4, 6, 8]),
        watch_list: vec!["Main.counter".to_string()],
        watch_values: vec![("Main.counter".to_string(), "42".to_string())],
        forced_io,
        alerts: VecDeque::new(),
        seen_events: HashSet::new(),
        connected: true,
        bundle_root: None,
    }
}

fn render_snapshot(state: &UiState, no_input: bool, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| render_ui(frame.area(), frame, state, no_input))
        .expect("draw ui");
    let mut lines = Vec::new();
    let buffer = terminal.backend().buffer();
    for y in 0..height {
        let mut line = String::new();
        for x in 0..width {
            line.push_str(buffer[(x, y)].symbol());
        }
        lines.push(line.trim_end().to_string());
    }
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

fn prompt_output_text(state: &UiState) -> String {
    state
        .prompt
        .output
        .iter()
        .flat_map(|line| line.segments.iter().map(|(text, _)| text.as_str()))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn parse_status_includes_simulation_mode_fields() {
    let response = json!({
        "result": {
            "state": "running",
            "resource": "SIM_RESOURCE",
            "simulation_mode": "simulation",
            "simulation_time_scale": 12,
            "simulation_warning": "Simulation mode active (time scale x12). Not for live hardware.",
            "metrics": {
                "cycle_ms": {
                    "min": 0.1,
                    "avg": 0.2,
                    "max": 0.3,
                    "last": 0.2
                },
                "overruns": 0,
                "faults": 0
            }
        }
    });

    let status = parse_status(&response).expect("status parse");
    assert_eq!(status.simulation_mode, "simulation");
    assert_eq!(status.simulation_time_scale, 12);
    assert!(status.simulation_warning.contains("Not for live hardware"));
}

#[test]
fn parse_status_accepts_io_drivers_field() {
    let response = json!({
        "result": {
            "state": "running",
            "resource": "runtime-a",
            "io_drivers": [{ "name": "loopback", "status": "ok" }],
            "metrics": {
                "cycle_ms": { "min": 0.1, "avg": 0.2, "max": 0.4, "last": 0.3 },
                "overruns": 0,
                "faults": 0
            }
        }
    });
    let status = parse_status(&response).expect("status parse");
    assert_eq!(status.drivers.len(), 1);
    assert_eq!(status.drivers[0].name, "loopback");
    assert_eq!(status.drivers[0].status, "ok");
}

#[test]
fn parse_settings_includes_simulation_fields() {
    let response = json!({
        "result": {
            "log.level": "info",
            "simulation.enabled": true,
            "simulation.time_scale": 6,
            "simulation.mode": "simulation",
            "simulation.warning": "Simulation mode active (time scale x6). Not for live hardware."
        }
    });

    let settings = parse_settings(&response).expect("settings parse");
    assert!(settings.simulation_enabled);
    assert_eq!(settings.simulation_time_scale, 6);
    assert_eq!(settings.simulation_mode, "simulation");
    assert!(settings
        .simulation_warning
        .contains("Not for live hardware"));
}

#[test]
fn parse_settings_includes_cycle_interval_field() {
    let response = json!({
        "result": {
            "resource.cycle_interval_ms": 100
        }
    });
    let settings = parse_settings(&response).expect("settings parse");
    assert_eq!(settings.cycle_interval_ms, Some(100));
}

#[test]
fn parse_snapshot_includes_tasks_io_and_events() {
    let status_response = json!({
        "result": {
            "state": "running",
            "resource": "SIM_RESOURCE",
            "metrics": {
                "cycle_ms": { "min": 0.1, "avg": 0.2, "max": 0.4, "last": 0.3 },
                "overruns": 2,
                "faults": 1
            },
            "drivers": [{ "name": "sim", "status": "ok" }],
            "control_mode": "rw"
        }
    });
    let tasks_response = json!({
        "result": [
            { "name": "MainTask", "last_ms": 0.5, "avg_ms": 0.4, "max_ms": 0.9, "overruns": 0 }
        ]
    });
    let io_response = json!({
        "result": [
            { "name": "MotorStart", "address": "QX0.0", "value": true, "direction": "OUT" }
        ]
    });
    let events_response = json!({
        "result": [
            { "code": "EVT001", "level": "warn", "message": "Cycle near limit" }
        ]
    });
    let settings_response = json!({
        "result": {
            "log.level": "debug",
            "web.listen": "127.0.0.1:8080",
            "control.mode": "rw",
            "mesh.publish": ["Main.speed"],
            "mesh.subscribe": [{ "topic": "line/a", "alias": "Main.in" }]
        }
    });

    let status = parse_status(&status_response).expect("parse status");
    let tasks = parse_tasks(&tasks_response);
    let io = parse_io(&io_response);
    let events = parse_events(&events_response);
    let settings = parse_settings(&settings_response).expect("parse settings");

    let snapshot = format!(
        "status={} resource={} last={:.1} overruns={}\n\
tasks={} first={} avg={:.1}\n\
io={} first={} value={}\n\
events={} first={} kind={:?}\n\
settings log={} web={} control={} publish={} subscribe={}",
        status.state,
        status.resource,
        status.cycle_last,
        status.overruns,
        tasks.len(),
        tasks.first().map(|task| task.name.as_str()).unwrap_or("-"),
        tasks.first().map(|task| task.avg_ms).unwrap_or_default(),
        io.len(),
        io.first()
            .map(|entry| entry.address.as_str())
            .unwrap_or("-"),
        io.first().map(|entry| entry.value.as_str()).unwrap_or("-"),
        events.len(),
        events
            .first()
            .map(|event| event.label.as_str())
            .unwrap_or("-"),
        events
            .first()
            .map(|event| event.kind)
            .unwrap_or(EventKind::Info),
        settings.log_level,
        settings.web_listen,
        settings.control_mode,
        settings.mesh_publish.join(","),
        settings
            .mesh_subscribe
            .iter()
            .map(|(topic, alias)| format!("{topic}->{alias}"))
            .collect::<Vec<_>>()
            .join(",")
    );

    assert_eq!(
        snapshot,
        "status=running resource=SIM_RESOURCE last=0.3 overruns=2\n\
tasks=1 first=MainTask avg=0.4\n\
io=1 first=QX0.0 value=true\n\
events=1 first=EVT001 kind=Warn\n\
settings log=debug web=127.0.0.1:8080 control=rw publish=Main.speed subscribe=line/a->Main.in"
    );
}

#[test]
fn render_dashboard_snapshot_matches_layout() {
    let mut state = sample_state();
    state.prompt.set_output(vec![PromptLine::plain(
        "Monitoring",
        Style::default().fg(COLOR_INFO),
    )]);

    let snapshot = render_snapshot(&state, true, 80, 20);
    let excerpt = snapshot
        .lines()
        .take(8)
        .collect::<Vec<_>>()
        .join("\n")
        .replace('│', "|")
        .replace('─', "-")
        .replace(['┌', '┐', '└', '┘', '┬', '┴', '├', '┤', '┼'], "+");

    assert_eq!(
        excerpt,
        "+ Cycle Time ------------------------------------------------------------------+\n\
|   █                                                                          |\n\
|  ▄█                                                                          |\n\
|  ██                                                                          |\n\
| ███                                                                          |\n\
|▄███                                                                          |\n\
|████                                                                          |\n\
|min 0.2ms  avg 0.4ms  max 0.9ms  last 0.5ms                                   |"
    );
}

#[test]
fn input_navigation_handles_prompt_and_read_only_mode() {
    let mut client = test_client();
    let mut state = sample_state();

    let should_exit = handle_key(
        KeyEvent::from(KeyCode::Char('/')),
        &mut client,
        &mut state,
        false,
    )
    .expect("open prompt");
    assert!(!should_exit);
    assert!(state.prompt.active);
    assert!(state.prompt.showing_suggestions);

    state.prompt.showing_suggestions = false;
    state.prompt.history = vec!["status".to_string(), "help".to_string()];
    state.prompt.input.clear();
    state.prompt.cursor = 0;

    handle_key(KeyEvent::from(KeyCode::Up), &mut client, &mut state, false).expect("history up");
    assert_eq!(state.prompt.input, "help");
    handle_key(KeyEvent::from(KeyCode::Up), &mut client, &mut state, false)
        .expect("history up again");
    assert_eq!(state.prompt.input, "status");
    handle_key(
        KeyEvent::from(KeyCode::Down),
        &mut client,
        &mut state,
        false,
    )
    .expect("history down");
    assert_eq!(state.prompt.input, "help");

    let mut readonly_state = sample_state();
    handle_key(
        KeyEvent::from(KeyCode::Char('/')),
        &mut client,
        &mut readonly_state,
        true,
    )
    .expect("readonly slash");
    assert!(!readonly_state.prompt.active);
    assert!(prompt_output_text(&readonly_state).contains("Read-only mode."));
    assert!(handle_key(
        KeyEvent::from(KeyCode::Char('q')),
        &mut client,
        &mut readonly_state,
        true,
    )
    .expect("readonly quit"));
}

#[test]
fn command_routing_covers_settings_beginner_guard_and_pause() {
    let mut client = test_client();
    let mut state = sample_state();

    assert!(!execute_command("/settings", &mut client, &mut state).expect("settings command"));
    assert_eq!(state.prompt.mode, PromptMode::SettingsSelect);
    assert!(state.prompt.active);

    let mut beginner_state = sample_state();
    beginner_state.beginner_mode = true;
    execute_command("/log", &mut client, &mut beginner_state).expect("blocked command");
    assert!(prompt_output_text(&beginner_state).contains("Beginner mode"));

    execute_command("/p", &mut client, &mut state).expect("pause shortcut");
    assert!(prompt_output_text(&state).contains("Paused."));
}
