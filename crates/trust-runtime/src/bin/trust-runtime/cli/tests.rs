#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parse_build_ci_flag() {
        let cli = Cli::parse_from(["trust-runtime", "build", "--ci"]);
        match cli.command.expect("command") {
            Command::Build { ci, .. } => assert!(ci),
            other => panic!("expected build command, got {other:?}"),
        }
    }

    #[test]
    fn parse_validate_ci_flag() {
        let cli = Cli::parse_from(["trust-runtime", "validate", "--project", "project", "--ci"]);
        match cli.command.expect("command") {
            Command::Validate { ci, .. } => assert!(ci),
            other => panic!("expected validate command, got {other:?}"),
        }
    }

    #[test]
    fn parse_test_ci_flag() {
        let cli = Cli::parse_from(["trust-runtime", "test", "--project", "project", "--ci"]);
        match cli.command.expect("command") {
            Command::Test { ci, .. } => assert!(ci),
            other => panic!("expected test command, got {other:?}"),
        }
    }

    #[test]
    fn parse_docs_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "docs",
            "--project",
            "project",
            "--out-dir",
            "out",
            "--format",
            "markdown",
        ]);
        match cli.command.expect("command") {
            Command::Docs {
                project,
                out_dir,
                format,
            } => {
                assert_eq!(project, Some(PathBuf::from("project")));
                assert_eq!(out_dir, Some(PathBuf::from("out")));
                assert_eq!(format, DocsFormat::Markdown);
            }
            other => panic!("expected docs command, got {other:?}"),
        }
    }

    #[test]
    fn parse_plcopen_export_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "plcopen",
            "export",
            "--project",
            "project",
            "--output",
            "out.xml",
            "--json",
        ]);
        match cli.command.expect("command") {
            Command::Plcopen { action } => match action {
                PlcopenAction::Export {
                    project,
                    output,
                    target,
                    json,
                } => {
                    assert_eq!(project, Some(PathBuf::from("project")));
                    assert_eq!(output, Some(PathBuf::from("out.xml")));
                    assert_eq!(target, PlcopenExportTargetArg::Generic);
                    assert!(json);
                }
                other => panic!("expected plcopen export action, got {other:?}"),
            },
            other => panic!("expected plcopen command, got {other:?}"),
        }
    }

    #[test]
    fn parse_plcopen_export_target_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "plcopen",
            "export",
            "--project",
            "project",
            "--target",
            "siemens",
        ]);
        match cli.command.expect("command") {
            Command::Plcopen { action } => match action {
                PlcopenAction::Export { target, .. } => {
                    assert_eq!(target, PlcopenExportTargetArg::Siemens);
                }
                other => panic!("expected plcopen export action, got {other:?}"),
            },
            other => panic!("expected plcopen command, got {other:?}"),
        }
    }

    #[test]
    fn parse_plcopen_import_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "plcopen",
            "import",
            "--input",
            "interop/plcopen.xml",
            "--project",
            "project",
            "--json",
        ]);
        match cli.command.expect("command") {
            Command::Plcopen { action } => match action {
                PlcopenAction::Import {
                    input,
                    project,
                    json,
                } => {
                    assert_eq!(input, PathBuf::from("interop/plcopen.xml"));
                    assert_eq!(project, Some(PathBuf::from("project")));
                    assert!(json);
                }
                other => panic!("expected plcopen import action, got {other:?}"),
            },
            other => panic!("expected plcopen command, got {other:?}"),
        }
    }

    #[test]
    fn parse_play_simulation_flags() {
        let cli = Cli::parse_from(["trust-runtime", "play", "--simulation", "--time-scale", "8"]);
        match cli.command.expect("command") {
            Command::Play {
                simulation,
                time_scale,
                ..
            } => {
                assert!(simulation);
                assert_eq!(time_scale, 8);
            }
            other => panic!("expected play command, got {other:?}"),
        }
    }

    #[test]
    fn parse_run_execution_backend_flag() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "run",
            "--project",
            "project",
            "--execution-backend",
            "vm",
        ]);
        match cli.command.expect("command") {
            Command::Run {
                execution_backend, ..
            } => assert_eq!(execution_backend, Some(ExecutionBackendArg::Vm)),
            other => panic!("expected run command, got {other:?}"),
        }
    }

    #[test]
    fn parse_run_execution_backend_rejects_interpreter_flag() {
        let err = Cli::try_parse_from([
            "trust-runtime",
            "run",
            "--project",
            "project",
            "--execution-backend",
            "interpreter",
        ])
        .expect_err("interpreter backend should be rejected by CLI");
        assert!(err.to_string().contains("invalid value 'interpreter'"));
    }

    #[test]
    fn parse_play_execution_backend_rejects_interpreter_flag() {
        let err = Cli::try_parse_from([
            "trust-runtime",
            "play",
            "--execution-backend",
            "interpreter",
        ])
        .expect_err("interpreter backend should be rejected by CLI");
        assert!(err.to_string().contains("invalid value 'interpreter'"));
    }

    #[test]
    fn parse_hmi_init_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "hmi",
            "--project",
            "project",
            "init",
            "--style",
            "classic",
        ]);
        match cli.command.expect("command") {
            Command::Hmi { project, action } => {
                assert_eq!(project, Some(PathBuf::from("project")));
                match action {
                    HmiAction::Init { style, force } => {
                        assert_eq!(style, HmiStyleArg::Classic);
                        assert!(!force);
                    }
                    other => panic!("expected hmi init action, got {other:?}"),
                }
            }
            other => panic!("expected hmi command, got {other:?}"),
        }
    }

    #[test]
    fn parse_hmi_update_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "hmi",
            "--project",
            "project",
            "update",
            "--style",
            "mint",
        ]);
        match cli.command.expect("command") {
            Command::Hmi { project, action } => {
                assert_eq!(project, Some(PathBuf::from("project")));
                match action {
                    HmiAction::Update { style } => assert_eq!(style, HmiStyleArg::Mint),
                    other => panic!("expected hmi update action, got {other:?}"),
                }
            }
            other => panic!("expected hmi command, got {other:?}"),
        }
    }

    #[test]
    fn parse_hmi_reset_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "hmi",
            "--project",
            "project",
            "reset",
            "--style",
            "industrial",
        ]);
        match cli.command.expect("command") {
            Command::Hmi { project, action } => {
                assert_eq!(project, Some(PathBuf::from("project")));
                match action {
                    HmiAction::Reset { style } => assert_eq!(style, HmiStyleArg::Industrial),
                    other => panic!("expected hmi reset action, got {other:?}"),
                }
            }
            other => panic!("expected hmi command, got {other:?}"),
        }
    }

    #[test]
    fn parse_setup_cancel_mode() {
        let cli = Cli::parse_from(["trust-runtime", "setup", "--mode", "cancel"]);
        match cli.command.expect("command") {
            Command::Setup { mode, .. } => assert_eq!(mode, Some(SetupModeArg::Cancel)),
            other => panic!("expected setup command, got {other:?}"),
        }
    }

    #[test]
    fn parse_ide_serve_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "ide",
            "serve",
            "--project",
            "workspace",
            "--listen",
            "127.0.0.1:19081",
        ]);
        match cli.command.expect("command") {
            Command::Ide { action } => match action {
                ConfigUiAction::Serve { project, listen } => {
                    assert_eq!(project, Some(PathBuf::from("workspace")));
                    assert_eq!(listen, "127.0.0.1:19081");
                }
            },
            other => panic!("expected ide command, got {other:?}"),
        }
    }

    #[test]
    fn parse_config_ui_serve_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "config-ui",
            "serve",
            "--project",
            "workspace",
            "--listen",
            "127.0.0.1:19081",
        ]);
        match cli.command.expect("command") {
            Command::ConfigUi { action } => match action {
                ConfigUiAction::Serve { project, listen } => {
                    assert_eq!(project, Some(PathBuf::from("workspace")));
                    assert_eq!(listen, "127.0.0.1:19081");
                }
            },
            other => panic!("expected config-ui command, got {other:?}"),
        }
    }

    #[test]
    fn parse_agent_serve_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "agent",
            "serve",
            "--project",
            "workspace",
        ]);
        match cli.command.expect("command") {
            Command::Agent { action } => match action {
                AgentAction::Serve { project } => {
                    assert_eq!(project, Some(PathBuf::from("workspace")));
                }
            },
            other => panic!("expected agent command, got {other:?}"),
        }
    }

    #[test]
    fn parse_registry_private_init_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "registry",
            "init",
            "--root",
            "registry",
            "--visibility",
            "private",
            "--token",
            "secret",
        ]);
        match cli.command.expect("command") {
            Command::Registry { action } => match action {
                RegistryAction::Init {
                    root,
                    visibility,
                    token,
                } => {
                    assert_eq!(root, PathBuf::from("registry"));
                    assert_eq!(visibility, RegistryVisibilityArg::Private);
                    assert_eq!(token, Some("secret".to_string()));
                }
                other => panic!("expected registry init action, got {other:?}"),
            },
            other => panic!("expected registry command, got {other:?}"),
        }
    }

    #[test]
    fn parse_conformance_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "conformance",
            "--suite-root",
            "conformance",
            "--output",
            "summary.json",
            "--update-expected",
            "--filter",
            "timers",
        ]);
        match cli.command.expect("command") {
            Command::Conformance {
                suite_root,
                output,
                update_expected,
                filter,
            } => {
                assert_eq!(suite_root, Some(PathBuf::from("conformance")));
                assert_eq!(output, Some(PathBuf::from("summary.json")));
                assert!(update_expected);
                assert_eq!(filter.as_deref(), Some("timers"));
            }
            other => panic!("expected conformance command, got {other:?}"),
        }
    }

    #[test]
    fn parse_bench_project_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "bench",
            "project",
            "--project",
            "examples/plcopen_motion_single_axis_demo",
            "--samples",
            "300",
            "--warmup-cycles",
            "50",
            "--watch",
            "g_motion_demo_completed_sequences",
            "--watch",
            "g_motion_demo_last_error",
            "--output",
            "json",
        ]);
        match cli.command.expect("command") {
            Command::Bench { action } => match action {
                BenchAction::Project {
                    project,
                    samples,
                    warmup_cycles,
                    watch,
                    tier1,
                    output,
                } => {
                    assert_eq!(
                        project,
                        PathBuf::from("examples/plcopen_motion_single_axis_demo")
                    );
                    assert_eq!(samples, 300);
                    assert_eq!(warmup_cycles, 50);
                    assert_eq!(
                        watch,
                        vec![
                            "g_motion_demo_completed_sequences",
                            "g_motion_demo_last_error"
                        ]
                    );
                    assert!(!tier1);
                    assert_eq!(output, BenchOutputFormat::Json);
                }
                other => panic!("expected bench project action, got {other:?}"),
            },
            other => panic!("expected bench command, got {other:?}"),
        }
    }

    #[test]
    fn parse_bench_project_command_with_tier1() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "bench",
            "project",
            "--project",
            "examples/plcopen_motion_single_axis_demo",
            "--tier1",
        ]);
        match cli.command.expect("command") {
            Command::Bench { action } => match action {
                BenchAction::Project { tier1, .. } => {
                    assert!(tier1);
                }
                other => panic!("expected bench project action, got {other:?}"),
            },
            other => panic!("expected bench command, got {other:?}"),
        }
    }

    #[test]
    fn parse_bench_t0_shm_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "bench",
            "t0-shm",
            "--samples",
            "120",
            "--payload-bytes",
            "48",
            "--output",
            "json",
        ]);
        match cli.command.expect("command") {
            Command::Bench { action } => match action {
                BenchAction::T0Shm {
                    samples,
                    payload_bytes,
                    output,
                } => {
                    assert_eq!(samples, 120);
                    assert_eq!(payload_bytes, 48);
                    assert_eq!(output, BenchOutputFormat::Json);
                }
                other => panic!("expected bench t0-shm action, got {other:?}"),
            },
            other => panic!("expected bench command, got {other:?}"),
        }
    }

    #[test]
    fn parse_bench_mesh_zenoh_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "bench",
            "mesh-zenoh",
            "--samples",
            "64",
            "--payload-bytes",
            "96",
            "--loss-rate",
            "0.05",
            "--reorder-rate",
            "0.15",
        ]);
        match cli.command.expect("command") {
            Command::Bench { action } => match action {
                BenchAction::MeshZenoh {
                    samples,
                    payload_bytes,
                    loss_rate,
                    reorder_rate,
                    output,
                } => {
                    assert_eq!(samples, 64);
                    assert_eq!(payload_bytes, 96);
                    assert_eq!(loss_rate, 0.05);
                    assert_eq!(reorder_rate, 0.15);
                    assert_eq!(output, BenchOutputFormat::Table);
                }
                other => panic!("expected bench mesh-zenoh action, got {other:?}"),
            },
            other => panic!("expected bench command, got {other:?}"),
        }
    }

}
