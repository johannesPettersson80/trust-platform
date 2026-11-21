//! `trust-dev rust bench`'s command line (specification 48 RU-48); what the bench measures and
//! records is proven by `tests/rust_bench_cli.rs` and trust-runtime's `native::rust_pous` tests.

use clap::Parser;

use super::DEFAULT_SCANS;
use crate::{Cli, Command, RustAction};

fn bench(arguments: &[&str]) -> (Option<std::path::PathBuf>, u32) {
    let cli = Cli::try_parse_from(["trust-dev", "rust", "bench"].iter().chain(arguments))
        .expect("the command line parses");
    let Command::Rust {
        action: RustAction::Bench { project, scans, .. },
    } = cli.command
    else {
        panic!("rust bench");
    };
    (project, scans)
}

#[test]
fn the_command_line_names_the_project_and_the_scans_of_each_case() {
    // RU-48: 1,000 scans a case unless `--scans` says otherwise. Catches a default that measures
    // fewer calls than the specification gives and a `--scans` that does not reach the run.
    assert_eq!(DEFAULT_SCANS, 1_000);
    assert_eq!(bench(&[]), (None, 1_000));
    assert_eq!(
        bench(&["--project", "line", "--scans", "50"]),
        (Some(std::path::PathBuf::from("line")), 50)
    );
}

#[test]
fn a_device_bench_names_its_target() {
    // RU-48: a bench on a device runs the image built for the device's target, as `trust-dev test
    // --device` does, so `--device` without `--target`, or `--target` alone, is refused. Catches a
    // device bench that would build for this computer.
    for arguments in [
        &["rust", "bench", "--device", "/dev/ttyACM0"][..],
        &["rust", "bench", "--target", "esp32c6"][..],
    ] {
        assert!(
            Cli::try_parse_from(std::iter::once("trust-dev").chain(arguments.iter().copied()))
                .is_err(),
            "{arguments:?}"
        );
    }
    let cli = Cli::try_parse_from([
        "trust-dev",
        "rust",
        "bench",
        "--device",
        "tcp://127.0.0.1:5555",
        "--target",
        "esp32c6",
    ])
    .expect("the command line parses");
    let Command::Rust {
        action:
            RustAction::Bench {
                device,
                target,
                token,
                ..
            },
    } = cli.command
    else {
        panic!("rust bench");
    };
    assert_eq!(
        (device.as_deref(), target.as_deref(), token),
        (Some("tcp://127.0.0.1:5555"), Some("esp32c6"), None)
    );
}
