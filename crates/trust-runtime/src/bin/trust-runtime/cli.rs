//! CLI definitions for trust-runtime.

use clap::{ArgAction, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use std::path::PathBuf;

include!("cli/commands.rs");
include!("cli/output_formats.rs");
include!("cli/agent.rs");
include!("cli/bench.rs");
include!("cli/hmi.rs");
include!("cli/setup.rs");
include!("cli/plcopen.rs");
include!("cli/registry.rs");
include!("cli/control.rs");
include!("cli/tests.rs");
