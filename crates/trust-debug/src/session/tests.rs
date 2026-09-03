#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Source;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use trust_runtime::debug::SourceLocation;
    use trust_runtime::io::IoAddress;
    use trust_runtime::value::Value as RuntimeValue;

    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

    fn temp_source_path(label: &str) -> std::path::PathBuf {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let mut dir = std::env::temp_dir();
        dir.push(format!("trust-debug-{label}-{id}"));
        let _ = std::fs::create_dir_all(&dir);
        let mut path = dir;
        path.push("main.st");
        path
    }

    fn temp_project_root(label: &str) -> std::path::PathBuf {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let mut dir = std::env::temp_dir();
        dir.push(format!("trust-debug-{label}-{id}"));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn expands_brace_globs() {
        let patterns = expand_braces("**/*.{st,ST,pou,POU}");
        assert_eq!(patterns.len(), 4);
        assert!(patterns.contains(&"**/*.st".to_string()));
        assert!(patterns.contains(&"**/*.ST".to_string()));
        assert!(patterns.contains(&"**/*.pou".to_string()));
        assert!(patterns.contains(&"**/*.POU".to_string()));
    }

    #[test]
    fn expands_nested_braces() {
        let patterns = expand_braces("a{b,c}d{e,f}");
        let mut sorted = patterns.clone();
        sorted.sort();
        assert_eq!(sorted, vec!["abde", "abdf", "acde", "acdf"]);
    }

    #[test]
    fn session_resolves_breakpoints_to_statement_start() {
        let mut runtime = Runtime::new();
        let source = "x := 1;\n  y := 2;\n";
        let x_start = source.find("x := 1;").unwrap();
        let x_end = x_start + "x := 1;".len();
        let y_start = source.find("y := 2;").unwrap();
        let y_end = y_start + "y := 2;".len();
        runtime.register_statement_locations(
            0,
            vec![
                SourceLocation::new(0, x_start as u32, x_end as u32),
                SourceLocation::new(0, y_start as u32, y_end as u32),
            ],
        );

        let mut session = DebugSession::new(runtime);
        session.register_source("main.st", 0, source);

        let args = SetBreakpointsArguments {
            source: Source {
                name: Some("main".into()),
                path: Some("main.st".into()),
                source_reference: None,
            },
            breakpoints: Some(vec![SourceBreakpoint {
                line: 2,
                column: Some(1),
                condition: None,
                hit_condition: None,
                log_message: None,
            }]),
            lines: None,
            source_modified: None,
        };

        let response = session.set_breakpoints(&args);
        assert_eq!(response.breakpoints.len(), 1);
        let breakpoint = &response.breakpoints[0];
        assert!(breakpoint.verified);
        assert_eq!(breakpoint.line, Some(2));
        assert_eq!(breakpoint.column, Some(3));
    }

    #[test]
    fn session_snaps_breakpoints_inside_indent() {
        let mut runtime = Runtime::new();
        let source = "x := 1;\n  y := 2;\n";
        let x_start = source.find("x := 1;").unwrap();
        let x_end = x_start + "x := 1;".len();
        let y_start = source.find("y := 2;").unwrap();
        let y_end = y_start + "y := 2;".len();
        runtime.register_statement_locations(
            0,
            vec![
                SourceLocation::new(0, x_start as u32, x_end as u32),
                SourceLocation::new(0, y_start as u32, y_end as u32),
            ],
        );

        let mut session = DebugSession::new(runtime);
        session.register_source("main.st", 0, source);

        let args = SetBreakpointsArguments {
            source: Source {
                name: Some("main".into()),
                path: Some("main.st".into()),
                source_reference: None,
            },
            breakpoints: Some(vec![SourceBreakpoint {
                line: 2,
                column: Some(2),
                condition: None,
                hit_condition: None,
                log_message: None,
            }]),
            lines: None,
            source_modified: None,
        };

        let response = session.set_breakpoints(&args);
        assert_eq!(response.breakpoints.len(), 1);
        let breakpoint = &response.breakpoints[0];
        assert!(breakpoint.verified);
        assert_eq!(breakpoint.line, Some(2));
        assert_eq!(breakpoint.column, Some(3));
    }

    #[test]
    fn session_accepts_logpoint_templates() {
        let mut runtime = Runtime::new();
        let source = "x := 1;\n";
        let x_start = source.find("x := 1;").unwrap();
        let x_end = x_start + "x := 1;".len();
        runtime.register_statement_locations(
            0,
            vec![SourceLocation::new(0, x_start as u32, x_end as u32)],
        );

        let mut session = DebugSession::new(runtime);
        session.register_source("main.st", 0, source);

        let args = SetBreakpointsArguments {
            source: Source {
                name: Some("main".into()),
                path: Some("main.st".into()),
                source_reference: None,
            },
            breakpoints: Some(vec![SourceBreakpoint {
                line: 1,
                column: Some(1),
                condition: None,
                hit_condition: None,
                log_message: Some("x={x}".into()),
            }]),
            lines: None,
            source_modified: None,
        };

        let response = session.set_breakpoints(&args);
        assert_eq!(response.breakpoints.len(), 1);
        assert!(response.breakpoints[0].verified);
    }

    #[test]
    fn session_rejects_invalid_log_message() {
        let mut runtime = Runtime::new();
        let source = "x := 1;\n";
        let x_start = source.find("x := 1;").unwrap();
        let x_end = x_start + "x := 1;".len();
        runtime.register_statement_locations(
            0,
            vec![SourceLocation::new(0, x_start as u32, x_end as u32)],
        );

        let mut session = DebugSession::new(runtime);
        session.register_source("main.st", 0, source);

        let args = SetBreakpointsArguments {
            source: Source {
                name: Some("main".into()),
                path: Some("main.st".into()),
                source_reference: None,
            },
            breakpoints: Some(vec![SourceBreakpoint {
                line: 1,
                column: Some(1),
                condition: None,
                hit_condition: None,
                log_message: Some("{".into()),
            }]),
            lines: None,
            source_modified: None,
        };

        let response = session.set_breakpoints(&args);
        assert_eq!(response.breakpoints.len(), 1);
        assert!(!response.breakpoints[0].verified);
    }

    #[test]
    fn parse_hit_condition_supports_basic_operators() {
        assert_eq!(parse_hit_condition("3"), Some(HitCondition::Equal(3)));
        assert_eq!(parse_hit_condition(">= 4"), Some(HitCondition::AtLeast(4)));
        assert_eq!(
            parse_hit_condition("> 5"),
            Some(HitCondition::GreaterThan(5))
        );
        assert_eq!(parse_hit_condition("==6"), Some(HitCondition::Equal(6)));
        assert!(parse_hit_condition("nope").is_none());
    }

    #[test]
    fn session_reload_revalidates_breakpoints() {
        let path = temp_source_path("reload");
        let source_v1 = r#"PROGRAM Main
VAR
    x : INT;
END_VAR
x := INT#1;
END_PROGRAM
"#;
        std::fs::write(&path, source_v1).unwrap();

        let mut session = DebugSession::new(Runtime::new());
        session.set_program_path(path.to_string_lossy().to_string());
        session
            .reload_program(Some(path.to_string_lossy().as_ref()))
            .unwrap();

        let args = SetBreakpointsArguments {
            source: Source {
                name: Some(path.to_string_lossy().to_string()),
                path: Some(path.to_string_lossy().to_string()),
                source_reference: None,
            },
            breakpoints: Some(vec![SourceBreakpoint {
                line: 5,
                column: Some(1),
                condition: None,
                hit_condition: None,
                log_message: None,
            }]),
            lines: None,
            source_modified: None,
        };
        let response = session.set_breakpoints(&args);
        assert_eq!(response.breakpoints.len(), 1);
        assert_eq!(response.breakpoints[0].line, Some(5));

        let source_v2 = format!("\n{source_v1}");
        std::fs::write(&path, source_v2).unwrap();
        let updated = session.reload_program(None).unwrap();
        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0].line, Some(6));
    }

    #[test]
    fn session_reload_clears_breakpoints_without_requests() {
        let path = temp_source_path("reload_clear");
        let source = r#"PROGRAM Main
VAR
    x : INT;
END_VAR
x := INT#1;
END_PROGRAM
"#;
        std::fs::write(&path, source).unwrap();

        let mut session = DebugSession::new(Runtime::new());
        session.set_program_path(path.to_string_lossy().to_string());
        session
            .reload_program(Some(path.to_string_lossy().as_ref()))
            .unwrap();

        let args = SetBreakpointsArguments {
            source: Source {
                name: Some(path.to_string_lossy().to_string()),
                path: Some(path.to_string_lossy().to_string()),
                source_reference: None,
            },
            breakpoints: Some(vec![SourceBreakpoint {
                line: 5,
                column: Some(1),
                condition: None,
                hit_condition: None,
                log_message: None,
            }]),
            lines: None,
            source_modified: None,
        };
        let _ = session.set_breakpoints(&args);
        assert_eq!(session.control.breakpoint_count(), 1);

        session.clear_requested_breakpoints();
        session.reload_program(None).unwrap();
        assert_eq!(session.control.breakpoint_count(), 0);
    }

    #[test]
    fn source_display_name_is_project_relative_but_path_stays_absolute() {
        let project_root = temp_project_root("source_display");
        let src = project_root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        let main_path = src.join("main.st");
        let absolute_path = canonicalize_lossy(&main_path);
        std::fs::write(&absolute_path, "PROGRAM Main\nEND_PROGRAM\n").unwrap();

        let mut session = DebugSession::new(Runtime::new());
        session.update_source_options(SourceOptionsUpdate {
            root: Some(project_root.to_string_lossy().to_string()),
            include_globs: None,
            exclude_globs: None,
            ignore_pragmas: None,
        });
        session.register_source(absolute_path.to_string_lossy().to_string(), 0, "");

        let source = session.source_for_file_id(0).expect("source for file id");
        assert_eq!(source.name.as_deref(), Some("src/main.st"));
        let source_path = source.path.as_deref().expect("DAP source path");
        assert!(
            std::path::Path::new(source_path).is_absolute(),
            "DAP path must stay absolute so VS Code can open the stack frame source"
        );
        assert_eq!(
            std::fs::canonicalize(source_path).expect("canonical DAP source path"),
            std::fs::canonicalize(&absolute_path).expect("canonical registered source path"),
            "DAP path must identify the registered source file"
        );
    }

    #[test]
    fn session_reload_applies_project_io_toml_drivers() {
        let project_root = temp_project_root("reload_io");
        let src = project_root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        let main_path = src.join("Main.st");
        let config_path = src.join("Configuration.st");
        std::fs::write(
            &main_path,
            r#"PROGRAM MainProgram
VAR
    e_stop : BOOL;
    running : BOOL;
END_VAR

running := FALSE;
END_PROGRAM
"#,
        )
        .unwrap();
        std::fs::write(
            &config_path,
            r#"CONFIGURATION Config
TASK Cycle (INTERVAL := T#10ms, PRIORITY := 1);
PROGRAM P1 WITH Cycle : MainProgram;
VAR_CONFIG
    P1.e_stop AT %IX0.0 : BOOL;
    P1.running AT %QX0.0 : BOOL;
END_VAR
END_CONFIGURATION
"#,
        )
        .unwrap();
        std::fs::write(
            project_root.join("io.toml"),
            r#"[io]

[[io.drivers]]
name = "loopback"
params = { input_count = 1, output_count = 1, scan_period_ms = 10 }
"#,
        )
        .unwrap();

        let mut session = DebugSession::new(Runtime::new());
        session.update_source_options(SourceOptionsUpdate {
            root: Some(project_root.to_string_lossy().to_string()),
            include_globs: None,
            exclude_globs: None,
            ignore_pragmas: None,
        });
        session
            .reload_program(Some(config_path.to_string_lossy().as_ref()))
            .unwrap();

        let output = IoAddress::parse("%QX0.0").unwrap();
        let input = IoAddress::parse("%IX0.0").unwrap();
        session.debug_control().force_io(output.clone(), RuntimeValue::Bool(true));

        let handle = session.runtime_handle();
        let mut runtime = handle.lock().unwrap();
        runtime.execute_cycle().unwrap();
        runtime.execute_cycle().unwrap();

        assert_eq!(runtime.io().read(&output).unwrap(), RuntimeValue::Bool(true));
        assert_eq!(
            runtime.io().read(&input).unwrap(),
            RuntimeValue::Bool(true),
            "loopback io.toml driver should mirror forced output into the next input cycle"
        );
        let snapshot = runtime.io().snapshot();
        assert_eq!(
            snapshot.inputs[0].source.as_deref(),
            Some("Loopback I/O"),
            "debug-session simulator path should annotate configured input provenance"
        );
        assert_eq!(
            snapshot.outputs[0].source.as_deref(),
            Some("Loopback I/O"),
            "debug-session simulator path should annotate configured output provenance"
        );
    }

    include!("mqtt_mapping_tests.rs");

    #[test]
    fn session_reload_validates_project_ads_toml_bindings() {
        let project_root = temp_project_root("reload_ads");
        let src = project_root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        let main_path = src.join("Main.st");
        let config_path = src.join("Configuration.st");
        std::fs::write(&main_path, "PROGRAM Main\nEND_PROGRAM\n").unwrap();
        std::fs::write(
            &config_path,
            r#"CONFIGURATION Config
RESOURCE R ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Main WITH MainTask : Main;
END_RESOURCE
END_CONFIGURATION
"#,
        )
        .unwrap();
        std::fs::write(
            project_root.join("runtime.toml"),
            r#"[bundle]
version = 1

[resource]
name = "R"
cycle_interval_ms = 100

[runtime.control]
endpoint = "unix:///tmp/trust-debug-reload-ads.sock"
mode = "debug"
debug_enabled = true

[runtime.web]
enabled = false
listen = "127.0.0.1:8080"
auth = "local"
tls = false

[runtime.tls]
mode = "disabled"
require_remote = false

[runtime.discovery]
enabled = false
service_name = "truST"
advertise = false
interfaces = []

[runtime.mesh]
enabled = false
listen = "0.0.0.0:5200"
tls = false
publish = []

[runtime.ads]
enabled = true
config_path = "ads.toml"
worker_tick_interval_ms = 20

[runtime.opcua]
enabled = false
listen = "0.0.0.0:4840"
endpoint_path = "/"
namespace_uri = "urn:trust:runtime"
publish_interval_ms = 250
max_nodes = 128
expose = []
security_policy = "basic256sha256"
security_mode = "sign_and_encrypt"
allow_anonymous = false

[runtime.observability]
enabled = false
sample_interval_ms = 1000
mode = "all"
include = []
history_path = "history/historian.jsonl"
max_entries = 20000
prometheus_enabled = true
prometheus_path = "/metrics"

[runtime.log]
level = "info"

[runtime.retain]
mode = "none"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 1000
action = "halt"

[runtime.fault]
policy = "halt"
"#,
        )
        .unwrap();
        std::fs::write(
            project_root.join("ads.toml"),
            r#"[[connections]]
name = "line1"
target_net_id = "5.23.91.12.1.1"
host = "192.168.10.5"
ams_port = 851
transport = "plain"
insecure_transport = true

[[connections.points]]
var = "missing_ads_global"
symbol = "MAIN.Temperature"
type = "REAL"
mode = "poll"
"#,
        )
        .unwrap();

        let mut session = DebugSession::new(Runtime::new());
        session.update_source_options(SourceOptionsUpdate {
            root: Some(project_root.to_string_lossy().to_string()),
            include_globs: None,
            exclude_globs: None,
            ignore_pragmas: None,
        });
        let err = session
            .reload_program(Some(config_path.to_string_lossy().as_ref()))
            .expect_err("debug reload must validate ADS bindings before launch");
        assert!(
            err.to_string()
                .contains("failed to resolve declared global 'missing_ads_global'"),
            "unexpected ADS binding error: {err}"
        );
    }

    #[test]
    fn session_revalidates_breakpoints_after_source_registration() {
        let source = r#"PROGRAM Main
VAR
    x : INT := 0;
END_VAR
IF x = 0 THEN
    x := x + 1;
END_IF;
END_PROGRAM
"#;
        let harness = TestHarness::from_source(source).unwrap();
        let mut session = DebugSession::new(harness.into_runtime());

        let line_index = source
            .lines()
            .position(|line| line.contains("x := x + 1;"))
            .unwrap();
        let line = line_index as u32 + 1;
        let args = SetBreakpointsArguments {
            source: Source {
                name: Some("main".into()),
                path: Some("main.st".into()),
                source_reference: None,
            },
            breakpoints: Some(vec![SourceBreakpoint {
                line,
                column: Some(1),
                condition: None,
                hit_condition: None,
                log_message: None,
            }]),
            lines: None,
            source_modified: None,
        };

        let response = session.set_breakpoints(&args);
        assert!(!response.breakpoints[0].verified);

        session.register_source("main.st", 0, source);
        let updated = session.revalidate_breakpoints();
        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0].line, Some(line));
        assert!(updated[0].verified);
    }

    #[test]
    fn session_resolves_if_header_breakpoint_to_if_statement() {
        let source = r#"PROGRAM Main
VAR
    startCmd : BOOL := TRUE;
    x : INT := 0;
END_VAR
IF startCmd THEN
    x := x + 1;
END_IF;
END_PROGRAM
"#;
        let harness = TestHarness::from_source(source).unwrap();
        let mut session = DebugSession::new(harness.into_runtime());
        session.register_source("main.st", 0, source);

        let if_line = source
            .lines()
            .position(|line| line.trim_start().starts_with("IF startCmd THEN"))
            .unwrap() as u32
            + 1;
        let args = SetBreakpointsArguments {
            source: Source {
                name: Some("main".into()),
                path: Some("main.st".into()),
                source_reference: None,
            },
            breakpoints: Some(vec![SourceBreakpoint {
                line: if_line,
                column: Some(1),
                condition: None,
                hit_condition: None,
                log_message: None,
            }]),
            lines: None,
            source_modified: None,
        };

        let response = session.set_breakpoints(&args);
        assert_eq!(response.breakpoints.len(), 1);
        let bp = &response.breakpoints[0];
        assert!(bp.verified);
        assert_eq!(bp.line, Some(if_line));
        assert_eq!(bp.column, Some(1));
    }
}
