//! Replaying a cycle trace in a simulator over the control link (specification 48 RU-60;
//! specification 11 §6.8.1, "Replaying a trace"): the trace's upload, the request, and the answer
//! as a tool reports it. `trust-dev replay` and `trust_sim::Replay` share it.

use std::fmt;
use std::fmt::Write as _;

use base64::Engine as _;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{ControlClient, ControlError};

/// Uploads `trace`, the bytes of a `.trusttrace` file, and replays it in the simulator that
/// `client` is connected to: `transfer.begin` `{"for": "trace"}`, the parts of the size it
/// answers, each with its CRC-32, then `simulator.replay` naming the upload (specification 11
/// §6.8.1).
///
/// # Errors
///
/// The connection failed; the simulator refused the upload or the replay, with its code
/// (`trace_invalid`, `simulator_started`, `clock_not_manual`); or an answer is not what the
/// request answers.
pub fn replay(client: &mut ControlClient, trace: &[u8]) -> Result<ReplayAnswer, ControlError> {
    let mut digest = String::new();
    for byte in Sha256::digest(trace) {
        // Writing to a `String` cannot fail.
        let _ = write!(digest, "{byte:02x}");
    }
    let begun = client.request(
        "transfer.begin",
        Some(json!({"bytes": trace.len(), "sha256": digest, "for": "trace"})),
    )?;
    let transfer = begun
        .get("transfer")
        .cloned()
        .ok_or_else(|| ControlError::Malformed("transfer.begin names no transfer".into()))?;
    let part_bytes = begun
        .get("part_bytes")
        .and_then(Value::as_u64)
        .and_then(|bytes| usize::try_from(bytes).ok())
        .filter(|bytes| *bytes > 0)
        .ok_or_else(|| ControlError::Malformed("transfer.begin names no part size".into()))?;
    for (index, part) in trace.chunks(part_bytes).enumerate() {
        client.request(
            "transfer.part",
            Some(json!({
                "transfer": transfer,
                "offset": index * part_bytes,
                "data": base64::engine::general_purpose::STANDARD.encode(part),
                "crc32": format!("{:08x}", crc32fast::hash(part)),
            })),
        )?;
    }
    let answer = client.request("simulator.replay", Some(json!({"transfer": transfer})))?;
    ReplayAnswer::from_json(&answer)
}

/// What a replay found: the answer of `simulator.replay` (specification 11 §6.8.1), which its
/// [`Display`](fmt::Display) words as a tool reports it (specification 48 RU-60, "The result").
#[derive(Debug, Clone, PartialEq)]
pub struct ReplayAnswer {
    /// Whether every cycle matched.
    pub matched: bool,
    /// The cycles the replay ran.
    pub cycles: u64,
    /// What was replayed.
    pub kind: ReplayKind,
    /// The first cycle that differs, none when every cycle matched.
    pub first_difference: Option<FirstDifference>,
    /// How many cycles after the first that differs differ too.
    pub later_differing_cycles: u64,
    /// What the replay could not carry over or apply, each once.
    pub notes: Vec<String>,
}

/// What was replayed, from the sources' hash and the target of the trace and of the build (RU-60,
/// "What was replayed").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayKind {
    /// The same sources, built for the same target.
    SameProgram,
    /// The same sources, built for another target.
    SameSourcesOtherTarget,
    /// Other sources.
    ChangedProgram,
}

/// The first cycle whose outputs or fault differ from the trace's.
#[derive(Debug, Clone, PartialEq)]
pub struct FirstDifference {
    /// The cycle's place in the trace, from 0.
    pub cycle: u64,
    /// Its logical time, in nanoseconds.
    pub time_ns: i64,
    /// How it differs.
    pub difference: Difference,
}

/// How the first cycle that differs differs.
#[derive(Debug, Clone, PartialEq)]
pub enum Difference {
    /// The outputs that differ.
    Outputs(Vec<OutputDifference>),
    /// The fault that ended the recorded cycle and the one that ended the replayed cycle, either
    /// of which may be none.
    Faults {
        /// The trace's fault.
        trace: Option<String>,
        /// The build's fault.
        build: Option<String>,
    },
}

/// An output variable whose value differs after a cycle.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputDifference {
    /// The variable's path, as the debugger names it.
    pub path: String,
    /// Its address, such as `%QW2`.
    pub address: String,
    /// The trace's value, as `variables.read` gives a value; null when there is none.
    pub trace: Value,
    /// The build's value, likewise.
    pub build: Value,
}

impl ReplayAnswer {
    /// The answer of `simulator.replay`, `{"matched", "cycles", "kind", "first_difference",
    /// "later_differing_cycles", "notes"}`.
    ///
    /// # Errors
    ///
    /// [`ControlError::Malformed`] for an answer without one of these, or with one of another
    /// kind.
    pub fn from_json(answer: &Value) -> Result<Self, ControlError> {
        let malformed =
            |what: &str| ControlError::Malformed(format!("simulator.replay's answer: {what}"));
        let number = |name: &str| {
            answer
                .get(name)
                .and_then(Value::as_u64)
                .ok_or_else(|| malformed(&format!("{name} is not a count")))
        };
        let kind = match answer.get("kind").and_then(Value::as_str) {
            Some("same_program") => ReplayKind::SameProgram,
            Some("same_sources_other_target") => ReplayKind::SameSourcesOtherTarget,
            Some("changed_program") => ReplayKind::ChangedProgram,
            _ => return Err(malformed("kind names no kind of replay")),
        };
        let first_difference = match answer.get("first_difference") {
            None | Some(Value::Null) => None,
            Some(first) => Some(first_difference(first).ok_or_else(|| {
                malformed("first_difference is not a cycle with its outputs or faults")
            })?),
        };
        let notes = answer
            .get("notes")
            .and_then(Value::as_array)
            .and_then(|notes| {
                notes
                    .iter()
                    .map(|note| note.as_str().map(str::to_string))
                    .collect::<Option<Vec<_>>>()
            })
            .ok_or_else(|| malformed("notes is not a list of texts"))?;
        Ok(Self {
            matched: answer
                .get("matched")
                .and_then(Value::as_bool)
                .ok_or_else(|| malformed("matched is not true or false"))?,
            cycles: number("cycles")?,
            kind,
            first_difference,
            later_differing_cycles: number("later_differing_cycles")?,
            notes,
        })
    }

    /// The exit code of `trust-dev replay` (RU-60, "The result"): 0 for a replay that matches
    /// every cycle, 1 for one that differs.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        u8::from(!self.matched)
    }
}

/// The first difference of an answer: `{"cycle", "time_ns"}` with `outputs` or `faults`.
fn first_difference(first: &Value) -> Option<FirstDifference> {
    let difference = match (first.get("outputs"), first.get("faults")) {
        (Some(outputs), None) => Difference::Outputs(
            outputs
                .as_array()?
                .iter()
                .map(|output| {
                    Some(OutputDifference {
                        path: output.get("path")?.as_str()?.to_string(),
                        address: output.get("address")?.as_str()?.to_string(),
                        trace: output.get("trace")?.clone(),
                        build: output.get("build")?.clone(),
                    })
                })
                .collect::<Option<Vec<_>>>()?,
        ),
        (None, Some(faults)) => {
            let fault = |side: &str| match faults.get(side)? {
                Value::Null => Some(None),
                Value::String(text) => Some(Some(text.clone())),
                _ => None,
            };
            Difference::Faults {
                trace: fault("trace")?,
                build: fault("build")?,
            }
        }
        _ => return None,
    };
    Some(FirstDifference {
        cycle: first.get("cycle")?.as_u64()?,
        time_ns: first.get("time_ns")?.as_i64()?,
        difference,
    })
}

impl fmt::Display for ReplayAnswer {
    /// The report (RU-60, "The result"): the cycles replayed and what was replayed; at the first
    /// cycle that differs, the cycle and its logical time, each output that differs with its name,
    /// its address, the trace's value and the build's, or the two faults, and how many later
    /// cycles differ too; for the same program, that every difference is a determinism fault or an
    /// input the trace did not hold (specification 44 NX-04); then the notes.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self.kind {
            ReplayKind::SameProgram => "the same program",
            ReplayKind::SameSourcesOtherTarget => "the same program for another device",
            ReplayKind::ChangedProgram => "a changed program",
        };
        let cycles = plural(self.cycles, "cycle", "cycles");
        match &self.first_difference {
            None => writeln!(
                out,
                "Replayed {cycles} of {kind}: every cycle's outputs match the trace."
            )?,
            Some(first) => {
                writeln!(out, "Replayed {cycles} of {kind}.")?;
                writeln!(
                    out,
                    "The first difference is at cycle {}, at the logical time {}:",
                    first.cycle,
                    logical_time(first.time_ns)
                )?;
                match &first.difference {
                    Difference::Outputs(outputs) => {
                        for output in outputs {
                            writeln!(
                                out,
                                "  {} ({}): the trace has {}, the build {}",
                                output.path,
                                output.address,
                                value_text(&output.trace),
                                value_text(&output.build)
                            )?;
                        }
                    }
                    Difference::Faults { trace, build } => writeln!(
                        out,
                        "  the trace's cycle ended with {}, the build's with {}",
                        fault_text(trace.as_deref()),
                        fault_text(build.as_deref())
                    )?,
                }
                match self.later_differing_cycles {
                    0 => writeln!(out, "No later cycle differs.")?,
                    1 => writeln!(out, "1 later cycle differs too.")?,
                    later => writeln!(out, "{later} later cycles differ too.")?,
                }
                if self.kind != ReplayKind::ChangedProgram {
                    writeln!(
                        out,
                        "The build is {kind}, so each difference is a determinism fault or an \
                         input the trace does not hold (specification 44 NX-04)."
                    )?;
                }
            }
        }
        if !self.notes.is_empty() {
            writeln!(out, "Notes:")?;
            for note in &self.notes {
                writeln!(out, "  {note}")?;
            }
        }
        Ok(())
    }
}

/// `count` with the singular or the plural noun.
fn plural(count: u64, one: &str, many: &str) -> String {
    if count == 1 {
        format!("1 {one}")
    } else {
        format!("{count} {many}")
    }
}

/// A logical time in milliseconds, its fraction written out only when it has one: `30 ms`,
/// `1.5 ms`.
fn logical_time(time_ns: i64) -> String {
    let millis = time_ns / 1_000_000;
    let fraction = (time_ns % 1_000_000).unsigned_abs();
    if fraction == 0 {
        return format!("{millis} ms");
    }
    let digits = format!("{fraction:06}");
    format!("{millis}.{} ms", digits.trim_end_matches('0'))
}

/// A value as a report writes it: a text without its quotes, none as `no value`, every other as
/// `variables.read` gives it.
fn value_text(value: &Value) -> String {
    match value {
        Value::Null => "no value".to_string(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// A fault as a report writes it, none as `no fault`.
fn fault_text(fault: Option<&str>) -> &str {
    fault.unwrap_or("no fault")
}
