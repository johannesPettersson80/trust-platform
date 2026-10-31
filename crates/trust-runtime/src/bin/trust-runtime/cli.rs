//! CLI definitions for trust-runtime.

use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use std::path::PathBuf;

include!("cli/commands.rs");
include!("cli/output_formats.rs");
include!("cli/agent.rs");
include!("cli/bench.rs");
include!("cli/comm.rs");
include!("cli/plant.rs");
include!("cli/discover.rs");
include!("cli/identify.rs");
include!("cli/fleet.rs");
include!("cli/ads.rs");
include!("cli/setup.rs");
include!("cli/plcopen.rs");
include!("cli/registry.rs");
include!("cli/control.rs");
include!("cli/firmware.rs");
include!("cli/toolchain.rs");
include!("cli/tests.rs");
include!("cli/docs_tests.rs");
