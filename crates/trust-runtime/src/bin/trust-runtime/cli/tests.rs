#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory, Parser};

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
    fn parse_check_json_and_ci_flags() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "check",
            "--project",
            "project",
            "--sources",
            "src",
            "--json",
            "--ci",
        ]);
        match cli.command.expect("command") {
            Command::Check {
                project,
                sources,
                json,
                ci,
            } => {
                assert_eq!(project, Some(PathBuf::from("project")));
                assert_eq!(sources, Some(PathBuf::from("src")));
                assert!(json);
                assert!(ci);
            }
            other => panic!("expected check command, got {other:?}"),
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
    fn runtime_help_names_trust_dev_workbench_commands_and_removal_window() {
        let mut help = Vec::new();
        Cli::command()
            .write_long_help(&mut help)
            .expect("render trust-runtime help");
        let text = String::from_utf8(help).expect("help is utf-8");

        assert!(text.contains("Workbench commands:"));
        assert!(text.contains("use trust-dev test/docs/commit/agent serve"));
        assert!(text.contains("no earlier than 2026-10-05"));
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
    fn parse_ads_import_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "ads",
            "import",
            "--config",
            "ads.toml",
            "--snapshot",
            "ads/snapshots/line1.symbols.json",
            "--output",
            "src/generated/ads_generated.st",
            "--force",
            "--json",
        ]);
        match cli.command.expect("command") {
            Command::Ads { action } => match action {
                AdsAction::Import {
                    config,
                    snapshots,
                    output,
                    force,
                    json,
                } => {
                    assert_eq!(config, PathBuf::from("ads.toml"));
                    assert_eq!(
                        snapshots,
                        vec![PathBuf::from("ads/snapshots/line1.symbols.json")]
                    );
                    assert_eq!(output, PathBuf::from("src/generated/ads_generated.st"));
                    assert!(force);
                    assert!(json);
                }
                other => panic!("expected ads import action, got {other:?}"),
            },
            other => panic!("expected ads command, got {other:?}"),
        }
    }

    #[test]
    fn parse_ads_validate_offline_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "ads",
            "validate",
            "--offline",
            "--config",
            "ads.toml",
            "--snapshot",
            "ads/snapshots/line1.symbols.json",
            "--generated",
            "src/generated/ads_generated.st",
            "--json",
        ]);
        match cli.command.expect("command") {
            Command::Ads { action } => match action {
                AdsAction::Validate {
                    offline,
                    live,
                    config,
                    snapshots,
                    generated,
                    json,
                } => {
                    assert!(offline);
                    assert!(!live);
                    assert_eq!(config, PathBuf::from("ads.toml"));
                    assert_eq!(
                        snapshots,
                        vec![PathBuf::from("ads/snapshots/line1.symbols.json")]
                    );
                    assert_eq!(generated, PathBuf::from("src/generated/ads_generated.st"));
                    assert!(json);
                }
                other => panic!("expected ads validate action, got {other:?}"),
            },
            other => panic!("expected ads command, got {other:?}"),
        }
    }

    #[test]
    fn parse_ads_validate_live_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "ads",
            "validate",
            "--live",
            "--config",
            "ads.toml",
            "--generated",
            "src/generated/ads_generated.st",
            "--json",
        ]);
        match cli.command.expect("command") {
            Command::Ads { action } => match action {
                AdsAction::Validate {
                    offline,
                    live,
                    config,
                    snapshots,
                    generated,
                    json,
                } => {
                    assert!(!offline);
                    assert!(live);
                    assert_eq!(config, PathBuf::from("ads.toml"));
                    assert!(snapshots.is_empty());
                    assert_eq!(generated, PathBuf::from("src/generated/ads_generated.st"));
                    assert!(json);
                }
                other => panic!("expected ads validate action, got {other:?}"),
            },
            other => panic!("expected ads command, got {other:?}"),
        }
    }

    #[test]
    fn parse_ads_discover_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "ads",
            "discover",
            "--target",
            "192.168.10.5",
            "--target-net-id",
            "5.23.91.12.1.1",
            "--no-broadcast",
            "--json",
        ]);
        match cli.command.expect("command") {
            Command::Ads { action } => match action {
                AdsAction::Discover {
                    target,
                    target_net_id,
                    ams_port,
                    no_broadcast,
                    json,
                } => {
                    assert_eq!(target.as_deref(), Some("192.168.10.5"));
                    assert_eq!(target_net_id.as_deref(), Some("5.23.91.12.1.1"));
                    assert_eq!(ams_port, 851);
                    assert!(no_broadcast);
                    assert!(json);
                }
                other => panic!("expected ads discover action, got {other:?}"),
            },
            other => panic!("expected ads command, got {other:?}"),
        }
    }

    #[test]
    fn parse_fleet_runtime_add_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "fleet",
            "runtime",
            "add",
            "--fleet-root",
            "fleet",
            "--name",
            "cell_1",
            "--template",
            "empty",
            "--control-port",
            "9910",
            "--web-port",
            "18080",
            "--json",
        ]);
        match cli.command.expect("command") {
            Command::Fleet { action } => match action {
                FleetAction::Runtime { action } => match action {
                    FleetRuntimeAction::Add {
                        fleet_root,
                        name,
                        template,
                        control_port,
                        web_port,
                        json,
                    } => {
                        assert_eq!(fleet_root, PathBuf::from("fleet"));
                        assert_eq!(name, "cell_1");
                        assert_eq!(template, FleetRuntimeTemplateArg::Empty);
                        assert_eq!(control_port, Some(9910));
                        assert_eq!(web_port, Some(18080));
                        assert!(json);
                    }
                    other => panic!("expected fleet runtime add, got {other:?}"),
                },
                other => panic!("expected fleet runtime action, got {other:?}"),
            },
            other => panic!("expected fleet command, got {other:?}"),
        }

        let automatic = Cli::parse_from([
            "trust-runtime",
            "fleet",
            "runtime",
            "add",
            "--fleet-root",
            "fleet",
            "--name",
            "automatic",
        ]);
        match automatic.command.expect("automatic command") {
            Command::Fleet {
                action:
                    FleetAction::Runtime {
                        action:
                            FleetRuntimeAction::Add {
                                control_port,
                                web_port,
                                ..
                            },
                    },
            } => {
                assert_eq!(control_port, None);
                assert_eq!(web_port, None);
            }
            other => panic!("expected automatic fleet runtime add, got {other:?}"),
        }
    }

    #[test]
    fn parse_fleet_list_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "fleet",
            "list",
            "--fleet-root",
            "fleet",
            "--json",
        ]);
        match cli.command.expect("command") {
            Command::Fleet { action } => match action {
                FleetAction::List { fleet_root, json } => {
                    assert_eq!(fleet_root, PathBuf::from("fleet"));
                    assert!(json);
                }
                other => panic!("expected fleet list action, got {other:?}"),
            },
            other => panic!("expected fleet command, got {other:?}"),
        }
    }

    #[test]
    fn parse_fleet_runtime_lifecycle_commands() {
        let start = Cli::parse_from([
            "trust-runtime",
            "fleet",
            "runtime",
            "start",
            "--fleet-root",
            "fleet",
            "--name",
            "cell",
            "--json",
        ]);
        match start.command.expect("command") {
            Command::Fleet { action } => match action {
                FleetAction::Runtime { action } => match action {
                    FleetRuntimeAction::Start {
                        fleet_root,
                        name,
                        json,
                    } => {
                        assert_eq!(fleet_root, PathBuf::from("fleet"));
                        assert_eq!(name, "cell");
                        assert!(json);
                    }
                    other => panic!("expected fleet runtime start, got {other:?}"),
                },
                other => panic!("expected fleet runtime action, got {other:?}"),
            },
            other => panic!("expected fleet command, got {other:?}"),
        }

        let stop = Cli::parse_from([
            "trust-runtime",
            "fleet",
            "runtime",
            "stop",
            "--fleet-root",
            "fleet",
            "--name",
            "cell",
            "--json",
        ]);
        match stop.command.expect("command") {
            Command::Fleet { action } => match action {
                FleetAction::Runtime { action } => match action {
                    FleetRuntimeAction::Stop {
                        fleet_root,
                        name,
                        json,
                    } => {
                        assert_eq!(fleet_root, PathBuf::from("fleet"));
                        assert_eq!(name, "cell");
                        assert!(json);
                    }
                    other => panic!("expected fleet runtime stop, got {other:?}"),
                },
                other => panic!("expected fleet runtime action, got {other:?}"),
            },
            other => panic!("expected fleet command, got {other:?}"),
        }

        let status = Cli::parse_from([
            "trust-runtime",
            "fleet",
            "runtime",
            "status",
            "--fleet-root",
            "fleet",
            "--name",
            "cell",
            "--json",
        ]);
        match status.command.expect("command") {
            Command::Fleet { action } => match action {
                FleetAction::Runtime { action } => match action {
                    FleetRuntimeAction::Status {
                        fleet_root,
                        name,
                        json,
                    } => {
                        assert_eq!(fleet_root, PathBuf::from("fleet"));
                        assert_eq!(name, "cell");
                        assert!(json);
                    }
                    other => panic!("expected fleet runtime status, got {other:?}"),
                },
                other => panic!("expected fleet runtime action, got {other:?}"),
            },
            other => panic!("expected fleet command, got {other:?}"),
        }

        let logs = Cli::parse_from([
            "trust-runtime",
            "fleet",
            "runtime",
            "logs",
            "--fleet-root",
            "fleet",
            "--name",
            "cell",
            "--lines",
            "25",
            "--json",
        ]);
        match logs.command.expect("command") {
            Command::Fleet { action } => match action {
                FleetAction::Runtime { action } => match action {
                    FleetRuntimeAction::Logs {
                        fleet_root,
                        name,
                        lines,
                        json,
                    } => {
                        assert_eq!(fleet_root, PathBuf::from("fleet"));
                        assert_eq!(name, "cell");
                        assert_eq!(lines, 25);
                        assert!(json);
                    }
                    other => panic!("expected fleet runtime logs, got {other:?}"),
                },
                other => panic!("expected fleet runtime action, got {other:?}"),
            },
            other => panic!("expected fleet command, got {other:?}"),
        }
    }

    #[test]
    fn parse_ads_browse_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "ads",
            "browse",
            "--config",
            "ads.toml",
            "--connection",
            "line1",
            "--json",
        ]);
        match cli.command.expect("command") {
            Command::Ads { action } => match action {
                AdsAction::Browse {
                    config,
                    connection,
                    json,
                } => {
                    assert_eq!(config, PathBuf::from("ads.toml"));
                    assert_eq!(connection.as_deref(), Some("line1"));
                    assert!(json);
                }
                other => panic!("expected ads browse action, got {other:?}"),
            },
            other => panic!("expected ads command, got {other:?}"),
        }
    }

    #[test]
    fn parse_ads_doctor_guarded_write_probe_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "ads",
            "doctor",
            "--target",
            "192.168.10.5",
            "--target-net-id",
            "5.23.91.12.1.1",
            "--write-symbol",
            "GVL.Setpoint",
            "--write-type",
            "REAL",
            "--write-value",
            "12.5",
            "--json",
        ]);
        match cli.command.expect("command") {
            Command::Ads { action } => match action {
                AdsAction::Doctor {
                    target,
                    target_net_id,
                    ams_port,
                    write_symbol,
                    write_type,
                    write_value,
                    json,
                } => {
                    assert_eq!(target, "192.168.10.5");
                    assert_eq!(target_net_id.as_deref(), Some("5.23.91.12.1.1"));
                    assert_eq!(ams_port, 851);
                    assert_eq!(write_symbol.as_deref(), Some("GVL.Setpoint"));
                    assert_eq!(write_type.as_deref(), Some("REAL"));
                    assert_eq!(write_value.as_deref(), Some("12.5"));
                    assert!(json);
                }
                other => panic!("expected ads doctor action, got {other:?}"),
            },
            other => panic!("expected ads command, got {other:?}"),
        }
    }

    #[test]
    fn parse_comm_schema_protocol_filter_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "comm",
            "schema",
            "--protocol",
            "opcua",
            "--json",
        ]);
        match cli.command.expect("command") {
            Command::Comm { action } => match action {
                CommAction::Schema { protocol, json } => {
                    assert_eq!(protocol.as_deref(), Some("opcua"));
                    assert!(json);
                }
                other => panic!("expected comm schema action, got {other:?}"),
            },
            other => panic!("expected comm command, got {other:?}"),
        }
    }

    #[test]
    fn parse_comm_apply_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "comm",
            "apply",
            "--project",
            "project",
            "--protocol",
            "modbus-tcp",
            "--params",
            r#"{"address":"127.0.0.1:502"}"#,
            "--action",
            "add",
            "--instance-id",
            "modbus_tcp:1",
            "--json",
        ]);
        match cli.command.expect("command") {
            Command::Comm { action } => match action {
                CommAction::Apply {
                    project,
                    protocol,
                    params,
                    action,
                    instance_id,
                    json,
                } => {
                    assert_eq!(project, PathBuf::from("project"));
                    assert_eq!(protocol, "modbus-tcp");
                    assert_eq!(params, r#"{"address":"127.0.0.1:502"}"#);
                    assert_eq!(action, CommApplyCliAction::Add);
                    assert_eq!(instance_id.as_deref(), Some("modbus_tcp:1"));
                    assert!(json);
                }
                other => panic!("expected comm apply action, got {other:?}"),
            },
            other => panic!("expected comm command, got {other:?}"),
        }
    }

    #[test]
    fn parse_comm_discover_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "comm",
            "discover",
            "--protocol",
            "modbus-tcp",
            "--origin",
            "runtime",
            "--cidr",
            "192.168.1.0/24",
            "--timeout-ms",
            "250",
            "--unit-id",
            "17",
            "--probe-read-address",
            "400",
            "--probe-read-quantity",
            "2",
            "--json",
        ]);
        match cli.command.expect("command") {
            Command::Comm { action } => match action {
                CommAction::Discover {
                    protocol,
                    origin,
                    cidr,
                    host,
                    adapter,
                    timeout_ms,
                    unit_id,
                    probe_read_address,
                    probe_read_quantity,
                    passive,
                    json,
                } => {
                    assert_eq!(protocol, "modbus-tcp");
                    assert_eq!(origin, CommDiscoverOriginArg::Runtime);
                    assert_eq!(cidr.as_deref(), Some("192.168.1.0/24"));
                    assert!(host.is_none());
                    assert!(adapter.is_none());
                    assert_eq!(timeout_ms, Some(250));
                    assert_eq!(unit_id, Some(17));
                    assert_eq!(probe_read_address, Some(400));
                    assert_eq!(probe_read_quantity, Some(2));
                    assert!(passive);
                    assert!(json);
                }
                other => panic!("expected comm discover action, got {other:?}"),
            },
            other => panic!("expected comm command, got {other:?}"),
        }
    }

    #[test]
    fn parse_comm_browse_symbols_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "comm",
            "browse-symbols",
            "--protocol",
            "ads",
            "--target",
            r#"{"host":"192.168.1.10","ams_net_id":"5.23.91.12.1.1"}"#,
            "--connection-name",
            "line1",
            "--json",
        ]);
        match cli.command.expect("command") {
            Command::Comm { action } => match action {
                CommAction::BrowseSymbols {
                    protocol,
                    project,
                    target,
                    snapshot_file,
                    instance_id,
                    kind,
                    connection_name,
                    json,
                } => {
                    assert_eq!(protocol, "ads");
                    assert!(project.is_none());
                    assert_eq!(
                        target.as_deref(),
                        Some(r#"{"host":"192.168.1.10","ams_net_id":"5.23.91.12.1.1"}"#)
                    );
                    assert!(snapshot_file.is_none());
                    assert!(instance_id.is_none());
                    assert_eq!(kind, "symbols");
                    assert_eq!(connection_name.as_deref(), Some("line1"));
                    assert!(json);
                }
                other => panic!("expected comm browse-symbols action, got {other:?}"),
            },
            other => panic!("expected comm command, got {other:?}"),
        }
    }

    #[test]
    fn parse_ads_doctor_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "ads",
            "doctor",
            "--target",
            "192.168.10.5",
            "--target-net-id",
            "5.23.91.12.1.1",
            "--json",
        ]);
        match cli.command.expect("command") {
            Command::Ads { action } => match action {
                AdsAction::Doctor {
                    target,
                    target_net_id,
                    ams_port,
                    write_symbol,
                    write_type,
                    write_value,
                    json,
                } => {
                    assert_eq!(target, "192.168.10.5");
                    assert_eq!(target_net_id.as_deref(), Some("5.23.91.12.1.1"));
                    assert_eq!(ams_port, 851);
                    assert!(write_symbol.is_none());
                    assert!(write_type.is_none());
                    assert!(write_value.is_none());
                    assert!(json);
                }
                other => panic!("expected ads doctor action, got {other:?}"),
            },
            other => panic!("expected ads command, got {other:?}"),
        }
    }

    #[test]
    fn parse_ads_route_script_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "ads",
            "route-script",
            "--route-name",
            "trust-runtime-line1",
            "--target",
            "192.168.10.5",
            "--target-net-id",
            "5.23.91.12.1.1",
            "--local-ip",
            "192.168.10.20",
            "--local-net-id",
            "192.168.10.20.1.1",
            "--format",
            "staticroutes",
            "--json",
        ]);
        match cli.command.expect("command") {
            Command::Ads { action } => match action {
                AdsAction::RouteScript {
                    route_name,
                    target,
                    target_net_id,
                    local_ip,
                    local_net_id,
                    format,
                    json,
                    ..
                } => {
                    assert_eq!(route_name, "trust-runtime-line1");
                    assert_eq!(target, "192.168.10.5");
                    assert_eq!(target_net_id, "5.23.91.12.1.1");
                    assert_eq!(local_ip, "192.168.10.20");
                    assert_eq!(local_net_id, "192.168.10.20.1.1");
                    assert_eq!(format, AdsRouteArtifactFormat::Staticroutes);
                    assert!(json);
                }
                other => panic!("expected ads route-script action, got {other:?}"),
            },
            other => panic!("expected ads command, got {other:?}"),
        }
    }

    #[test]
    fn parse_ads_add_route_requires_no_password_argument() {
        let error = Cli::try_parse_from([
            "trust-runtime",
            "ads",
            "add-route",
            "--route-name",
            "trust-runtime-line1",
            "--target",
            "192.168.10.5",
            "--target-net-id",
            "5.23.91.12.1.1",
            "--local-ip",
            "192.168.10.20",
            "--local-net-id",
            "192.168.10.20.1.1",
            "--password",
            "secret",
        ])
        .expect_err("password argv must not parse");
        assert_eq!(error.kind(), clap::error::ErrorKind::UnknownArgument);
    }

    #[test]
    fn parse_ads_add_route_password_stdin_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "ads",
            "add-route",
            "--route-name",
            "trust-runtime-line1",
            "--target",
            "192.168.10.5",
            "--target-net-id",
            "5.23.91.12.1.1",
            "--local-ip",
            "192.168.10.20",
            "--local-net-id",
            "192.168.10.20.1.1",
            "--username",
            "Administrator",
            "--password-stdin",
            "--json",
        ]);
        match cli.command.expect("command") {
            Command::Ads { action } => match action {
                AdsAction::AddRoute {
                    route_name,
                    target,
                    target_net_id,
                    local_ip,
                    local_net_id,
                    username,
                    password_stdin,
                    json,
                    ..
                } => {
                    assert_eq!(route_name, "trust-runtime-line1");
                    assert_eq!(target, "192.168.10.5");
                    assert_eq!(target_net_id, "5.23.91.12.1.1");
                    assert_eq!(local_ip, "192.168.10.20");
                    assert_eq!(local_net_id, "192.168.10.20.1.1");
                    assert_eq!(username, "Administrator");
                    assert!(password_stdin);
                    assert!(json);
                }
                other => panic!("expected ads add-route action, got {other:?}"),
            },
            other => panic!("expected ads command, got {other:?}"),
        }
    }

    #[test]
    fn parse_ads_route_remove_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "ads",
            "route-remove",
            "--route-name",
            "trust-runtime-line1",
            "--json",
        ]);
        match cli.command.expect("command") {
            Command::Ads { action } => match action {
                AdsAction::RouteRemove { route_name, json } => {
                    assert_eq!(route_name, "trust-runtime-line1");
                    assert!(json);
                }
                other => panic!("expected ads route-remove action, got {other:?}"),
            },
            other => panic!("expected ads command, got {other:?}"),
        }
    }

    #[test]
    fn parse_ads_import_symbols_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "ads",
            "import-symbols",
            "--target",
            "192.168.10.5",
            "--target-net-id",
            "5.23.91.12.1.1",
            "--connection",
            "line1",
            "--name-prefix",
            "line1_",
            "--include",
            "MAIN.*",
            "--out",
            "ads.toml",
            "--snapshot-out",
            "ads/snapshots/line1.symbols.json",
            "--existing-snapshot",
            "ads/snapshots/line0.symbols.json",
            "--gen",
            "src/generated/ads_generated.st",
            "--force",
            "--dry-run",
            "--json",
        ]);
        match cli.command.expect("command") {
            Command::Ads { action } => match action {
                AdsAction::ImportSymbols {
                    target,
                    target_net_id,
                    connection,
                    name_prefix,
                    include_patterns,
                    out,
                    snapshot_out,
                    existing_snapshots,
                    generated,
                    force,
                    dry_run,
                    json,
                    ..
                } => {
                    assert_eq!(target, "192.168.10.5");
                    assert_eq!(target_net_id.as_deref(), Some("5.23.91.12.1.1"));
                    assert_eq!(connection, "line1");
                    assert_eq!(name_prefix.as_deref(), Some("line1_"));
                    assert_eq!(include_patterns, vec!["MAIN.*"]);
                    assert_eq!(out, PathBuf::from("ads.toml"));
                    assert_eq!(
                        snapshot_out,
                        Some(PathBuf::from("ads/snapshots/line1.symbols.json"))
                    );
                    assert_eq!(
                        existing_snapshots,
                        vec![PathBuf::from("ads/snapshots/line0.symbols.json")]
                    );
                    assert_eq!(generated, PathBuf::from("src/generated/ads_generated.st"));
                    assert!(force);
                    assert!(dry_run);
                    assert!(json);
                }
                other => panic!("expected ads import-symbols action, got {other:?}"),
            },
            other => panic!("expected ads command, got {other:?}"),
        }
    }

    #[test]
    fn parse_ads_server_status_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "ads",
            "server",
            "status",
            "--endpoint",
            "tcp://127.0.0.1:9000",
            "--token",
            "secret",
            "--json",
        ]);
        match cli.command.expect("command") {
            Command::Ads { action } => match action {
                AdsAction::Server { action } => match action {
                    AdsServerAction::Status {
                        endpoint,
                        token,
                        json,
                        ..
                    } => {
                        assert_eq!(endpoint.as_deref(), Some("tcp://127.0.0.1:9000"));
                        assert_eq!(token.as_deref(), Some("secret"));
                        assert!(json);
                    }
                    other => panic!("expected ads server status action, got {other:?}"),
                },
                other => panic!("expected ads server action, got {other:?}"),
            },
            other => panic!("expected ads command, got {other:?}"),
        }
    }

    #[test]
    fn parse_ads_server_symbols_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "ads",
            "server",
            "symbols",
            "--project",
            ".",
        ]);
        match cli.command.expect("command") {
            Command::Ads { action } => match action {
                AdsAction::Server { action } => match action {
                    AdsServerAction::Symbols { project, json, .. } => {
                        assert_eq!(project, Some(PathBuf::from(".")));
                        assert!(!json);
                    }
                    other => panic!("expected ads server symbols action, got {other:?}"),
                },
                other => panic!("expected ads server action, got {other:?}"),
            },
            other => panic!("expected ads command, got {other:?}"),
        }
    }

    #[test]
    fn parse_ads_server_doctor_external_evidence_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "ads",
            "server",
            "doctor",
            "--endpoint",
            "tcp://127.0.0.1:9000",
            "--external-kind",
            "pyads",
            "--external-name",
            "lab-smoke",
            "--json",
        ]);
        match cli.command.expect("command") {
            Command::Ads { action } => match action {
                AdsAction::Server { action } => match action {
                    AdsServerAction::Doctor {
                        endpoint,
                        external_kind,
                        external_name,
                        json,
                        ..
                    } => {
                        assert_eq!(endpoint.as_deref(), Some("tcp://127.0.0.1:9000"));
                        assert_eq!(external_kind.as_deref(), Some("pyads"));
                        assert_eq!(external_name.as_deref(), Some("lab-smoke"));
                        assert!(json);
                    }
                    other => panic!("expected ads server doctor action, got {other:?}"),
                },
                other => panic!("expected ads server action, got {other:?}"),
            },
            other => panic!("expected ads command, got {other:?}"),
        }
    }

    #[test]
    fn parse_ads_server_route_script_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "ads",
            "server",
            "route-script",
            "--route-name",
            "trust-runtime-server",
            "--server-ip",
            "192.168.10.20",
            "--server-net-id",
            "192.168.10.20.1.1",
            "--ads-port",
            "851",
            "--format",
            "gui",
            "--json",
        ]);
        match cli.command.expect("command") {
            Command::Ads { action } => match action {
                AdsAction::Server { action } => match action {
                    AdsServerAction::RouteScript {
                        route_name,
                        server_ip,
                        server_net_id,
                        ads_port,
                        format,
                        json,
                    } => {
                        assert_eq!(route_name, "trust-runtime-server");
                        assert_eq!(server_ip, "192.168.10.20");
                        assert_eq!(server_net_id, "192.168.10.20.1.1");
                        assert_eq!(ads_port, 851);
                        assert_eq!(format, AdsRouteArtifactFormat::Gui);
                        assert!(json);
                    }
                    other => panic!("expected ads server route-script action, got {other:?}"),
                },
                other => panic!("expected ads server action, got {other:?}"),
            },
            other => panic!("expected ads command, got {other:?}"),
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
        let cli = Cli::parse_from(["trust-runtime", "agent", "serve", "--project", "workspace"]);
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
