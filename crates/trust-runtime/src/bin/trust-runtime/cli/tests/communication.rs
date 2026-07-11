use super::*;
use clap::{CommandFactory, Parser};

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

mod fleet;
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
                target_net_id,
                ams_port,
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
                assert!(target_net_id.is_none());
                assert!(ams_port.is_none());
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
fn parse_comm_ads_manual_discover_identity_and_port() {
    let matches = Cli::command()
        .try_get_matches_from([
            "trust-runtime",
            "comm",
            "discover",
            "--protocol",
            "ads",
            "--host",
            "192.0.2.5",
            "--target-net-id",
            "5.23.91.12.1.1",
            "--ams-port",
            "852",
            "--json",
        ])
        .expect("comm ADS discovery must accept a manual AMS identity and logical port");
    let (_, comm) = matches.subcommand().expect("comm subcommand");
    let (_, discover) = comm.subcommand().expect("discover subcommand");

    assert_eq!(
        discover.get_one::<String>("target_net_id").map(String::as_str),
        Some("5.23.91.12.1.1")
    );
    assert_eq!(discover.get_one::<u16>("ams_port"), Some(&852));
}

#[test]
fn parse_comm_ads_discover_rejects_zero_logical_port() {
    let error = Cli::command()
        .try_get_matches_from([
            "trust-runtime",
            "comm",
            "discover",
            "--protocol",
            "ads",
            "--host",
            "192.0.2.5",
            "--ams-port",
            "0",
        ])
        .expect_err("logical ADS port zero must be rejected by clap");

    assert!(error.to_string().contains("--ams-port"));
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
