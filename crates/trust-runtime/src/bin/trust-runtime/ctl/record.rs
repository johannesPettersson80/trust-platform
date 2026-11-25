//! `trust-runtime ctl record`: records the resource's next cycles into a cycle trace and saves it
//! as a `.trusttrace` file (specification 11 §6.8.1, "Cycle traces"; specification 48 RU-59).
//!
//! `trace.record` starts the recording, and `trace.read` is asked until it has ended. Its trace is
//! then read part by part. Each part's CRC-32 and the whole trace's SHA-256 are checked before the
//! file is written. Ctrl+C sends `trace.stop`, and the trace is saved with what it holds; a second
//! Ctrl+C ends the command at once.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context as _};
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use signal_hook::consts::signal::SIGINT;
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

/// A recording that has ended, as `trace.read` gives its state.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Ended {
    /// Why it ended (specification 48 Appendix C, C.3).
    reason: String,
    /// The cycles it holds.
    cycles: u64,
    /// The bytes of its file.
    bytes: usize,
    /// Its file's SHA-256, in hexadecimal.
    sha256: String,
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
    let file = read_trace(&mut client, &json!(trace), &ended)?;
    let path = out.map_or_else(|| default_file("trace", now_utc_secs()), Path::to_path_buf);
    std::fs::write(&path, &file).with_context(|| format!("cannot write {}", path.display()))?;
    println!(
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
    let state = client
        .request("trace.read", Some(json!({ "trace": "fault" })))
        .map_err(|error| step_failed("trace.read", &error))?;
    if state.get("state").and_then(Value::as_str) == Some("none") {
        bail!(
            "the runtime keeps no fault's trace: no fault since it started, or no flight recorder \
             ([runtime.trace] flight_recorder in runtime.toml)"
        );
    }
    let ended = ended(&state)?
        .ok_or_else(|| anyhow!("the runtime answered a fault's trace that is still recording"))?;
    let file = read_trace(&mut client, &json!("fault"), &ended)?;
    let path = out.map_or_else(|| default_file("fault", now_utc_secs()), Path::to_path_buf);
    std::fs::write(&path, &file).with_context(|| format!("cannot write {}", path.display()))?;
    println!(
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
) -> anyhow::Result<Ended> {
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
        if let Some(ended) = ended(&state)? {
            return Ok(ended);
        }
        std::thread::sleep(STATE_POLL);
    }
}

/// The ended recording that `state` describes, and `None` while it records.
fn ended(state: &Value) -> anyhow::Result<Option<Ended>> {
    match state.get("state").and_then(Value::as_str) {
        Some("recording") => Ok(None),
        Some("ended") => {
            let text = |key: &str| state.get(key).and_then(Value::as_str).map(str::to_owned);
            let number = |key: &str| state.get(key).and_then(Value::as_u64);
            let bytes = number("bytes").and_then(|bytes| usize::try_from(bytes).ok());
            match (text("reason"), number("cycles"), bytes, text("sha256")) {
                (Some(reason), Some(cycles), Some(bytes), Some(sha256)) => Ok(Some(Ended {
                    reason,
                    cycles,
                    bytes,
                    sha256,
                })),
                _ => bail!(
                    "the runtime answered an ended trace without its reason, cycles, bytes or \
                     SHA-256: {state}"
                ),
            }
        }
        _ => bail!("the runtime answered no recording's state: {state}"),
    }
}

/// The ended trace's file, read part by part from offset 0, each part checked ([`part`]), and the
/// whole checked against the length and the SHA-256 that its state gave.
fn read_trace(client: &mut ControlClient, trace: &Value, ended: &Ended) -> anyhow::Result<Vec<u8>> {
    let mut file = Vec::with_capacity(ended.bytes);
    while file.len() < ended.bytes {
        let reply = client
            .request(
                "trace.read",
                Some(json!({ "trace": trace, "offset": file.len() })),
            )
            .map_err(|error| step_failed("trace.read", &error))?;
        let bytes = part(&reply, file.len())?;
        file.extend_from_slice(&bytes);
    }
    let sha256 = hex::encode(Sha256::digest(&file));
    if file.len() != ended.bytes || sha256 != ended.sha256 {
        bail!(
            "the trace read has {} bytes and the SHA-256 {sha256}, not the {} bytes and the \
             SHA-256 {} that it ended with",
            file.len(),
            ended.bytes,
            ended.sha256
        );
    }
    Ok(file)
}

/// The bytes of the part that `reply` carries, asked for at `offset`: its offset, its next offset
/// and its CRC-32 agree with its bytes (specification 34 PORT-05).
fn part(reply: &Value, offset: usize) -> anyhow::Result<Vec<u8>> {
    let number = |key: &str| {
        reply
            .get(key)
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
    };
    let data = reply
        .get("data")
        .and_then(Value::as_str)
        .and_then(|data| STANDARD.decode(data).ok());
    let crc32 = reply.get("crc32").and_then(Value::as_str);
    let (Some(at), Some(next), Some(bytes), Some(crc32)) =
        (number("offset"), number("next_offset"), data, crc32)
    else {
        bail!(
            "the runtime answered a part without its offset, next offset, data or CRC-32: {reply}"
        );
    };
    if at != offset || bytes.is_empty() || next != offset + bytes.len() {
        bail!(
            "the part asked for at {offset} came from {at} with {} bytes and the next offset \
             {next}",
            bytes.len()
        );
    }
    let checksum = format!("{:08x}", crc32fast::hash(&bytes));
    if checksum != crc32 {
        bail!("the part at {offset} has the CRC-32 {checksum}, not the {crc32} it came with");
    }
    Ok(bytes)
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
