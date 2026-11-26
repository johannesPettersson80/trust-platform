//! `trust-dev replay <trace> [--project <path>]` (specification 48 RU-60): builds the project from
//! its sources as they are now, replays the trace in a simulator of it (`trust-runtime simulate`,
//! the installed runtime that trust-sim finds) without the simulation's plants, and prints the
//! report. It ends with 0 when every cycle matched, 1 when a cycle differs, and 2 with the reason
//! when the trace or the build cannot be replayed.

use std::path::{Path, PathBuf};

use trust_runtime::bundle::detect_bundle_path;

use crate::style;

/// The exit code of a trace or a build that cannot be replayed (RU-60, "The result").
const CANNOT_REPLAY: i32 = 2;

/// Runs `trust-dev replay`: prints the report, and ends the process with 1 when a cycle differs
/// and with 2 when the trace or the build cannot be replayed.
///
/// # Errors
///
/// Only the current folder that cannot be read when no project is named.
pub(crate) fn run_replay(trace: &Path, project: Option<PathBuf>) -> anyhow::Result<()> {
    let root = match project {
        Some(path) => path,
        None => detect_bundle_path(None).unwrap_or(std::env::current_dir()?),
    };
    match replay(trace, &root) {
        Ok(answer) => {
            out!("{answer}");
            if !answer.matched {
                std::process::exit(i32::from(answer.exit_code()));
            }
            Ok(())
        }
        Err(reason) => {
            eprintln!("{}", style::error(format!("Error: {reason}")));
            std::process::exit(CANNOT_REPLAY);
        }
    }
}

/// The answer of a replay of the trace at `trace` in a simulator of the project at `root`, built
/// as it is now by the installed runtime.
fn replay(trace: &Path, root: &Path) -> trust_sim::Result<trust_sim::ReplayAnswer> {
    let bytes = std::fs::read(trace).map_err(|error| trust_sim::Error::Trace {
        path: trace.to_path_buf(),
        message: error.to_string(),
    })?;
    trust_sim::Project::open(root)?.simulator()?.replay(&bytes)
}
