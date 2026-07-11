use super::*;

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
