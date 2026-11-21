//! `trust-dev rust bench`'s command line (specification 48 RU-48); what the bench measures and
//! records is proven by `tests/rust_bench_cli.rs` and trust-runtime's `native::rust_pous` tests.

use clap::Parser;

use super::DEFAULT_SCANS;
use crate::{Cli, Command, RustAction};

fn bench(arguments: &[&str]) -> (Option<std::path::PathBuf>, u32) {
    let cli = Cli::try_parse_from(["trust-dev", "rust", "bench"].iter().chain(arguments))
        .expect("the command line parses");
    let Command::Rust {
        action: RustAction::Bench { project, scans },
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
