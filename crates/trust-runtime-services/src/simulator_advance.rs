//! The simulator's clock: the control request `simulator.advance` (specification 11 §6.8.1,
//! "Simulator for machine tests"), which every runtime answers alike (specification 34 STATE-08).
//! A simulator runs the cycles its test asks for ([`ResourcePort::advance_simulator`]); a runtime
//! on the wall clock, every other, refuses it as `clock_not_manual`, the default.

use alloc::format;
use alloc::string::{String, ToString};

use serde_json::{json, Value as JsonValue};
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::value::Duration;

use crate::ports::{RecordsPort, ResourcePort};
use crate::protocol::ControlResponse;
use crate::status::{invalid_params, newest_fault, validate_object_fields};

#[cfg(test)]
mod tests;

/// The members a `simulator.advance` request's parameters may have: exactly one of them.
const MEMBERS: &[&str] = &["cycles", "duration_ns"];

/// The error code of a runtime that runs on the wall clock.
pub const CLOCK_NOT_MANUAL: &str = "clock_not_manual";

/// How far `simulator.advance` moves the simulator's logical time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Advance {
    /// That many cycles of the resource, one every cycle interval of logical time.
    Cycles(u64),
    /// The cycles due within that much logical time, after which the time is at its end.
    Duration(Duration),
}

/// What an advance did: the logical time after it and the cycles it ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Advanced {
    /// The logical time after the advance.
    pub time: Duration,
    /// The cycles the advance ran.
    pub cycles: u64,
}

/// Why a runtime runs no advance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdvanceRefusal {
    /// The runtime runs on the wall clock: every runtime but a simulator.
    ClockNotManual,
    /// The scan thread ended, hangs, or did not finish a cycle: the text says which.
    Unavailable(String),
}

/// Answers `simulator.advance`: the cycles run, and the logical time and the resource's state after
/// them, `{"time_ns", "cycles", "state"}`. A cycle that adds a fault event ends the advance, and the
/// answer adds that event as `faults` lists it (`fault`).
#[must_use]
pub fn request<H: ResourcePort + RecordsPort>(
    id: u64,
    params: Option<&JsonValue>,
    host: &H,
) -> ControlResponse {
    let advance = match parse(params) {
        Ok(advance) => advance,
        Err(reason) => return invalid_params(id, &reason),
    };
    let before = newest_fault(host);
    let faulted = || newest_fault(host) != before;
    match host.advance_simulator(advance, &faulted) {
        Ok(advanced) => {
            let mut result = json!({
                "time_ns": advanced.time.as_nanos(),
                "cycles": advanced.cycles,
                "state": format!("{:?}", host.resource_state()).to_ascii_lowercase(),
            });
            let after = newest_fault(host);
            if after != before
                && let Some(fault) = after
            {
                result["fault"] = fault;
            }
            ControlResponse::ok(id, result)
        }
        Err(AdvanceRefusal::ClockNotManual) => ControlResponse::error_with_code(
            id,
            "the runtime runs on the wall clock: only a simulator advances its time \
             (`trust-runtime simulate`)"
                .to_string(),
            CLOCK_NOT_MANUAL,
        ),
        Err(AdvanceRefusal::Unavailable(reason)) => ControlResponse::error(id, reason),
    }
}

/// Whether `state` lets a simulator run its next cycle: a running resource.
#[must_use]
pub fn runs_cycles(state: ResourceState) -> bool {
    matches!(state, ResourceState::Running)
}

fn parse(params: Option<&JsonValue>) -> Result<Advance, String> {
    let params = params.ok_or("missing params")?;
    validate_object_fields(params, MEMBERS, MEMBERS)?;
    match (params.get("cycles"), params.get("duration_ns")) {
        (Some(cycles), None) => cycles
            .as_u64()
            .filter(|cycles| *cycles > 0)
            .map(Advance::Cycles)
            .ok_or_else(|| "cycles must be a whole number of at least 1".to_string()),
        (None, Some(duration)) => duration
            .as_u64()
            .and_then(|nanos| i64::try_from(nanos).ok())
            .map(|nanos| Advance::Duration(Duration::from_nanos(nanos)))
            .ok_or_else(|| "duration_ns must be a whole number of nanoseconds".to_string()),
        _ => Err("give exactly one of cycles and duration_ns".to_string()),
    }
}
