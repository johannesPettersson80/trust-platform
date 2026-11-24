//! Replaying a cycle trace in the simulator: the control request `simulator.replay` (specification
//! 11 §6.8.1, "Replaying a trace"; specification 48 RU-60). The trace is uploaded first with
//! `transfer.begin {"for": "trace"}`, and the request names that upload. A simulator replays it
//! before its first cycle ([`ResourcePort::replay_trace`](crate::ports::ResourcePort::replay_trace)); a runtime on the wall clock, every
//! other, refuses it as `clock_not_manual`, the default.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde_json::{json, Value as JsonValue};

use crate::dispatch::transfers::{take_complete, transfer_member};
use crate::ports::ControlHost;
use crate::protocol::ControlResponse;
use crate::simulator_advance::CLOCK_NOT_MANUAL;
use crate::variables::json_value;
use trust_runtime_core::value::Value;

#[cfg(test)]
mod tests;

/// The error code of a simulator that has already run a cycle.
pub const SIMULATOR_STARTED: &str = "simulator_started";
/// The error code of a trace whose format version, required section or checksum fails.
pub const TRACE_INVALID: &str = "trace_invalid";

/// What was replayed, from the sources' hash and the target of the trace and of the build (RU-60,
/// "What was replayed"): an image's build time makes every build's SHA-256 its own, so the image's
/// hash says nothing of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayKind {
    /// The same sources, built for the same target.
    SameProgram,
    /// The same sources, built for another target.
    SameSourcesOtherTarget,
    /// Other sources.
    ChangedProgram,
}

impl ReplayKind {
    /// The kind of a replay of a trace recorded from sources `trace_sources` for `trace_target` by
    /// a build of `build_sources` for `build_target`.
    #[must_use]
    pub fn of(
        trace_sources: &[u8; 32],
        trace_target: u32,
        build_sources: &[u8; 32],
        build_target: u32,
    ) -> Self {
        match (trace_sources == build_sources, trace_target == build_target) {
            (true, true) => Self::SameProgram,
            (true, false) => Self::SameSourcesOtherTarget,
            (false, _) => Self::ChangedProgram,
        }
    }

    /// The kind as the answer names it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::SameProgram => "same_program",
            Self::SameSourcesOtherTarget => "same_sources_other_target",
            Self::ChangedProgram => "changed_program",
        }
    }
}

/// An output variable whose value differs after a cycle: its path and address, and the trace's
/// value and the build's, each as `variables.read` gives a value.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputDifference {
    /// The variable's path, as the debugger names it.
    pub path: String,
    /// Its address, such as `%QW2`.
    pub address: String,
    /// The trace's value, none when its bytes are no value of the variable's type in the build.
    pub trace: Option<Value>,
    /// The build's value, likewise.
    pub build: Option<Value>,
}

/// How the first cycle that differs differs.
#[derive(Debug, Clone, PartialEq)]
pub enum Difference {
    /// The outputs that differ.
    Outputs(Vec<OutputDifference>),
    /// The trace's fault and the build's, either of which may be none.
    Faults {
        /// The fault that ended the recorded cycle.
        trace: Option<String>,
        /// The fault that ended the replayed cycle.
        build: Option<String>,
    },
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

/// What a replay found: the answer of `simulator.replay` (specification 11 §6.8.1).
#[derive(Debug, Clone, PartialEq)]
pub struct ReplayReport {
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

impl ReplayReport {
    /// Whether every cycle matched.
    #[must_use]
    pub fn matched(&self) -> bool {
        self.first_difference.is_none()
    }

    /// The answer, `{"matched", "cycles", "kind", "first_difference", "later_differing_cycles",
    /// "notes"}`.
    #[must_use]
    pub fn to_json(&self) -> JsonValue {
        let first_difference = self
            .first_difference
            .as_ref()
            .map_or(JsonValue::Null, |first| {
                let mut difference = json!({"cycle": first.cycle, "time_ns": first.time_ns});
                match &first.difference {
                    Difference::Outputs(outputs) => {
                        difference["outputs"] = outputs
                            .iter()
                            .map(|output| {
                                json!({
                                    "path": output.path,
                                    "address": output.address,
                                    "trace": output.trace.as_ref().map_or(JsonValue::Null, json_value),
                                    "build": output.build.as_ref().map_or(JsonValue::Null, json_value),
                                })
                            })
                            .collect();
                    }
                    Difference::Faults { trace, build } => {
                        difference["faults"] = json!({"trace": trace, "build": build});
                    }
                }
                difference
            });
        json!({
            "matched": self.matched(),
            "cycles": self.cycles,
            "kind": self.kind.name(),
            "first_difference": first_difference,
            "later_differing_cycles": self.later_differing_cycles,
            "notes": self.notes,
        })
    }
}

/// Why a runtime replays no trace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayRefusal {
    /// The runtime runs on the wall clock: every runtime but a simulator.
    ClockNotManual,
    /// The simulator has already run a cycle.
    SimulatorStarted,
    /// The trace does not read: the reader's reason.
    TraceInvalid(String),
    /// The replay cannot run here: the text says why.
    Unavailable(String),
}

/// Answers `simulator.replay {"transfer"}`: takes the complete upload the request names, which ends
/// it, and replays it as a cycle trace.
#[must_use]
pub fn request<H: ControlHost>(id: u64, params: Option<&JsonValue>, host: &H) -> ControlResponse {
    let Some(params) = params else {
        return ControlResponse::error(id, "missing params".to_string());
    };
    let transfer = match transfer_member(id, params) {
        Ok(transfer) => transfer,
        Err(refusal) => return refusal,
    };
    let (trace, _) = match take_complete(id, transfer, host) {
        Ok(upload) => upload,
        Err(refusal) => return refusal,
    };
    match host.replay_trace(trace) {
        Ok(report) => ControlResponse::ok(id, report.to_json()),
        Err(ReplayRefusal::ClockNotManual) => ControlResponse::error_with_code(
            id,
            "the runtime runs on the wall clock: only a simulator replays a trace \
             (`trust-runtime simulate`)"
                .to_string(),
            CLOCK_NOT_MANUAL,
        ),
        Err(ReplayRefusal::SimulatorStarted) => ControlResponse::error_with_code(
            id,
            "the simulator has run a cycle: a replay starts before the first, at the logical \
             time 0"
                .to_string(),
            SIMULATOR_STARTED,
        ),
        Err(ReplayRefusal::TraceInvalid(reason)) => ControlResponse::error_with_code(
            id,
            ["the trace cannot be replayed: ", &reason].concat(),
            TRACE_INVALID,
        ),
        Err(ReplayRefusal::Unavailable(reason)) => ControlResponse::error(id, reason),
    }
}
