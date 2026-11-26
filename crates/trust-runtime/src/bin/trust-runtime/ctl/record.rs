//! `trust-runtime ctl record`: records the resource's next cycles into a cycle trace and saves it
//! as a `.trusttrace` file (specification 11 §6.8.1, "Cycle traces"; specification 48 RU-59).
//!
//! `trace.record` starts the recording, and `trace.read` is asked until it has ended. Its trace is
//! then read part by part, as the client reads every trace ([`trace::read_file`]): each part's
//! CRC-32 and the whole trace's SHA-256 are checked before the file is written. Ctrl+C sends `trace.stop`, and the trace is saved with what it holds; a second
//! Ctrl+C ends the command at once.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context as _};
use serde_json::{json, Value};
use signal_hook::consts::signal::SIGINT;
use trust_runtime_client::trace::{self, EndedTrace, TraceState};
use trust_runtime_client::ControlClient;
use trust_runtime_services::live_values::value_grammar::literal::{parse_literal, Literal};

use super::install::{connect, step_failed};
use super::ResolvedControlTarget;

/// How often the recording's state is asked while it runs.
const STATE_POLL: Duration = Duration::from_millis(100);

/// The exit status of a command that a second Ctrl+C ends: 128 and SIGINT's number, as a shell
/// reports an interrupted command.
const INTERRUPTED: i32 = 130;

/// What a recording records until: `trace.record`'s goal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Goal {
    /// That many cycles, or with a task that many of its runs.
    Cycles(u64),
    /// The cycles within that many nanoseconds of logical time from the first.
    DurationNs(u64),
}

/// Records the next cycles of the resource at `target` until `cycles` cycles, or with `task` that
/// task's runs, or the cycles of the logical time `duration`, and saves the trace in `out`, by
/// default `trace-<UTC date and time>.trusttrace` in the current folder.
pub(super) fn run(
    target: &ResolvedControlTarget,
    cycles: Option<u64>,
    duration: Option<&str>,
    task: Option<&str>,
    out: Option<&Path>,
) -> anyhow::Result<()> {
    let goal = goal(cycles, duration)?;
    let interrupted = interrupt_flag()?;
    let (mut client, _) = connect(&target.endpoint, target.auth_token.as_deref())?;
    let (trace, reserved) = start(&mut client, goal, task)?;
    eprintln!("recording trace {trace}, {reserved} bytes reserved; Ctrl+C ends it");
    let ended = wait_for_end(&mut client, trace, &interrupted)?;
    let file = trace::read_file(&mut client, &json!(trace), &ended)
        .map_err(|error| step_failed("trace.read", &error))?;
    let path = out.map_or_else(|| default_file("trace", now_utc_secs()), Path::to_path_buf);
    std::fs::write(&path, &file).with_context(|| format!("cannot write {}", path.display()))?;
    outln!(
        "recorded {} cycles ({}) to {}, {} bytes",
        ended.cycles,
        ended.reason,
        path.display(),
        file.len()
    );
    Ok(())
}

/// Saves the flight recorder's trace of the last fault of the runtime at `target` in `out`, by
/// default `fault-<UTC date and time>.trusttrace` in the current folder (specification 11 §6.8.1,
/// "Cycle traces"), read and checked as a recording's trace is.
///
/// # Errors
///
/// A runtime that keeps no fault's trace, a trace that cannot be read whole, and a file that
/// cannot be written.
pub(super) fn run_fault(target: &ResolvedControlTarget, out: Option<&Path>) -> anyhow::Result<()> {
    let (mut client, _) = connect(&target.endpoint, target.auth_token.as_deref())?;
    let Some((ended, file)) =
        trace::fault_trace(&mut client).map_err(|error| step_failed("trace.read", &error))?
    else {
        bail!(
            "the runtime keeps no fault's trace: no fault since it started, or no flight recorder \
             ([runtime.trace] flight_recorder in runtime.toml)"
        );
    };
    let path = out.map_or_else(|| default_file("fault", now_utc_secs()), Path::to_path_buf);
    std::fs::write(&path, &file).with_context(|| format!("cannot write {}", path.display()))?;
    outln!(
        "saved the fault's trace of {} cycles ({}) to {}, {} bytes",
        ended.cycles,
        ended.reason,
        path.display(),
        file.len()
    );
    Ok(())
}

/// The recording's goal: `--cycles`, at least 1, or `--duration`.
fn goal(cycles: Option<u64>, duration: Option<&str>) -> anyhow::Result<Goal> {
    match (cycles, duration) {
        (Some(cycles), None) if cycles > 0 => Ok(Goal::Cycles(cycles)),
        (Some(_), None) => bail!("--cycles takes a number of at least 1"),
        (None, Some(duration)) => duration_ns(duration).map(Goal::DurationNs),
        _ => bail!("give exactly one of --cycles and --duration"),
    }
}

/// `--duration`'s nanoseconds: a duration as IEC 61131-3 writes one (Table 8), with or without its
/// `T#` prefix, of at least 1 nanosecond.
fn duration_ns(text: &str) -> anyhow::Result<u64> {
    let text = text.trim();
    let literal = if text.contains('#') {
        text.to_owned()
    } else {
        format!("T#{text}")
    };
    match parse_literal(&literal) {
        Ok(Literal::Duration { nanos, .. }) if nanos > 0 => Ok(nanos.unsigned_abs()),
        _ => bail!(
            "--duration takes a duration of at least 1 ns, such as 500ms, 2s or T#1m30s: {text}"
        ),
    }
}

/// The flag that the first Ctrl+C sets; a second Ctrl+C ends the process at once with
/// [`INTERRUPTED`].
fn interrupt_flag() -> anyhow::Result<Arc<AtomicBool>> {
    let flag = Arc::new(AtomicBool::new(false));
    // In this order, the shutdown sees the flag that the first Ctrl+C set only on the second.
    signal_hook::flag::register_conditional_shutdown(SIGINT, INTERRUPTED, Arc::clone(&flag))
        .map_err(|error| anyhow!("cannot watch for Ctrl+C: {error}"))?;
    signal_hook::flag::register(SIGINT, Arc::clone(&flag))
        .map_err(|error| anyhow!("cannot watch for Ctrl+C: {error}"))?;
    Ok(flag)
}

/// Starts the recording (`trace.record`): its id, and the bytes the runtime reserved for it.
fn start(client: &mut ControlClient, goal: Goal, task: Option<&str>) -> anyhow::Result<(u64, u64)> {
    let mut params = match goal {
        Goal::Cycles(cycles) => json!({ "cycles": cycles }),
        Goal::DurationNs(nanos) => json!({ "duration_ns": nanos }),
    };
    if let Some(task) = task {
        params["task"] = json!(task);
    }
    let reply = client
        .request("trace.record", Some(params))
        .map_err(|error| step_failed("trace.record", &error))?;
    let number = |key: &str| reply.get(key).and_then(Value::as_u64);
    match (number("trace"), number("reserved_bytes")) {
        (Some(trace), Some(reserved)) => Ok((trace, reserved)),
        _ => bail!("trace.record answered without the trace or its reserved bytes: {reply}"),
    }
}

/// Asks the recording's state until it has ended. Once `interrupted` is set, it sends
/// `trace.stop`, which ends the recording and answers its state.
fn wait_for_end(
    client: &mut ControlClient,
    trace: u64,
    interrupted: &AtomicBool,
) -> anyhow::Result<EndedTrace> {
    let mut stopped = false;
    loop {
        let request = if !stopped && interrupted.load(Ordering::SeqCst) {
            stopped = true;
            "trace.stop"
        } else {
            "trace.read"
        };
        let state = client
            .request(request, Some(json!({ "trace": trace })))
            .map_err(|error| step_failed(request, &error))?;
        match TraceState::from_json(&state).map_err(|error| step_failed(request, &error))? {
            TraceState::Ended(ended) => return Ok(ended),
            TraceState::Recording { .. } => {}
            TraceState::None => bail!("the runtime answered no recording's state: {state}"),
        }
        std::thread::sleep(STATE_POLL);
    }
}

/// `<prefix>-<UTC date and time>.trusttrace` for the UTC time `utc_secs`, the date and time in the
/// basic form of ISO 8601, as the runtime names a trace's file.
fn default_file(prefix: &str, utc_secs: i64) -> PathBuf {
    PathBuf::from(trust_runtime::control::trace_file_name(prefix, utc_secs))
}

/// The UTC time now, in whole seconds from 1970.
fn now_utc_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|since| i64::try_from(since.as_secs()).ok())
        .unwrap_or(0)
}

#[cfg(test)]
#[path = "record_tests.rs"]
mod tests;
