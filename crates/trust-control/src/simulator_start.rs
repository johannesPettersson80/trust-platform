//! The Start simulator action (specification 35, §0.1a and A2.8): the page asks for the local simulator, and the engine decides
//! whether the request is refused at once, joins a start that is still finishing, or lets the extension start the simulator. The
//! extension starts it through the runtime lifecycle service and reports the outcome; the engine then answers the reply the page
//! shows. The engine starts nothing itself. A live request that finds no runtime running is refused with the next action that applies
//! ([`runtime_not_running`]): Start simulator when the window's runtime is the local simulator, else Connect runtime.

#![allow(clippy::module_name_repetitions)] // The types name their concept: the simulator start.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::failure::{Cause, Failure, NextAction};
use crate::identification::connected_run::runtime_required;

/// The runtime mode of the local simulator, as the runtime lifecycle service reports it; an attached runtime is `online`.
pub const SIMULATE: &str = "simulate";

/// The sentences of the replies (specification 35, A2.8).
pub const SCOPE_CHANGED: &str = "The Control Studio scope or loop revision changed. Reopen the current loop before starting the simulator.";
/// A start for another loop or revision is still finishing.
pub const ANOTHER_START: &str = "Another simulator start is still finishing. Wait and retry.";
/// The loop's runtime is not the local simulator.
pub const NOT_SIMULATOR: &str =
    "Start is available only for the local simulator. Connect to a physical runtime explicitly.";
/// The simulator runs already.
pub const ALREADY_RUNNING: &str = "Simulator is already running.";
/// The lifecycle service is starting the simulator.
pub const IN_PROGRESS: &str = "Simulator start is already in progress.";
/// The simulator started, and the lifecycle service said nothing.
pub const STARTED: &str = "Simulator started.";
/// The simulator did not start, and the lifecycle service said nothing.
pub const NOT_STARTED: &str = "The simulator could not start.";
/// A live request found the local simulator not running (A2.8, E11.1).
pub const SIMULATOR_NOT_RUNNING: &str = "Start the simulator before running a live action.";

/// The start the extension has in flight, relative to the request (the extension keeps the start's promise).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum PendingStart {
    /// No start is in flight.
    Idle,
    /// A start for the same panel and loop revision is in flight: the request joins it.
    SameScope,
    /// A start for another panel or loop revision is in flight.
    OtherScope,
}

/// The arguments of `controlStudio/simulatorStart`: the runtime the page asked to start, the runtime the loop is bound to, the
/// runtime state the lifecycle service reports, and the start in flight.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SimulatorStartRequest {
    /// The runtime the loop is bound to.
    pub loop_runtime_id: String,
    /// The runtime the page named in its message; absent when it named none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_runtime_id: Option<String>,
    /// The mode the page named in its message; absent when it named none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_runtime_mode: Option<String>,
    /// The runtime mode the lifecycle service reports: `simulate` or `online`.
    pub runtime_mode: String,
    /// Whether a runtime is running.
    pub running: bool,
    /// Whether the lifecycle service is starting a runtime.
    pub starting: bool,
    /// The start the extension has in flight.
    pub pending: PendingStart,
}

/// What the extension does with a start request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum SimulatorStartDecision {
    /// Start the simulator through the lifecycle service, then report the outcome.
    Start,
    /// Wait for the start in flight and reply with its reply.
    Join,
    /// Reply with `reply` at once.
    Reply,
}

/// The answer of `controlStudio/simulatorStart`: what the extension does, and the reply when it replies at once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SimulatorStartAnswer {
    /// What the extension does.
    pub decision: SimulatorStartDecision,
    /// The reply the page receives; absent when the extension starts the simulator or joins a start in flight.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply: Option<SimulatorStartReply>,
}

/// The reply the page receives for a start: `ok` with the sentence it shows, or the failure with its kind and sentence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SimulatorStartReply {
    /// Whether the simulator runs.
    pub ok: bool,
    /// The sentence the page shows when the start succeeded; absent on a failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// The failure; absent when the start succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<SimulatorStartFailure>,
}

/// A refused start: its kind and the sentence that says why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SimulatorStartFailure {
    /// Why the start was refused or failed.
    pub kind: SimulatorStartFailureKind,
    /// The sentence the page shows.
    pub message: String,
}

/// The kinds of a refused or failed start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SimulatorStartFailureKind {
    /// The request does not name this loop's runtime and the simulate mode.
    InvalidScope,
    /// A start is already in progress.
    AlreadyStarting,
    /// The loop's runtime is not the local simulator.
    NotSimulator,
    /// The lifecycle service could not start the simulator.
    StartFailed,
}

/// What the extension reports after it started the simulator (`controlStudio/simulatorStartResult`): `ok` with the lifecycle's
/// sentence, or the lifecycle's failure sentence (or the text of an error it threw).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SimulatorStartOutcome {
    /// Whether the lifecycle service started the simulator.
    pub ok: bool,
    /// The lifecycle service's sentence when it started the simulator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// The lifecycle service's failure sentence when it did not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_message: Option<String>,
}

/// Decides a start request (specification 35, A2.8). In order: the request must name the loop's runtime and the simulate mode; a
/// start in flight for the same panel and revision is joined, and one for another refuses; a runtime that is not the simulator
/// refuses; a running simulator answers at once; a start in progress refuses; otherwise the extension starts the simulator.
#[must_use]
pub fn decide(request: &SimulatorStartRequest) -> SimulatorStartAnswer {
    if request.requested_runtime_id.as_deref() != Some(request.loop_runtime_id.as_str())
        || request.requested_runtime_mode.as_deref() != Some(SIMULATE)
    {
        return refused(SimulatorStartFailureKind::InvalidScope, SCOPE_CHANGED);
    }
    match request.pending {
        PendingStart::SameScope => SimulatorStartAnswer {
            decision: SimulatorStartDecision::Join,
            reply: None,
        },
        PendingStart::OtherScope => {
            refused(SimulatorStartFailureKind::AlreadyStarting, ANOTHER_START)
        }
        PendingStart::Idle => decide_when_idle(request),
    }
}

/// The decision when no start is in flight: the runtime's mode, then whether it runs, then whether it is starting.
fn decide_when_idle(request: &SimulatorStartRequest) -> SimulatorStartAnswer {
    if request.runtime_mode != SIMULATE {
        return refused(SimulatorStartFailureKind::NotSimulator, NOT_SIMULATOR);
    }
    if request.running {
        return answered(SimulatorStartReply {
            ok: true,
            message: Some(ALREADY_RUNNING.to_string()),
            failure: None,
        });
    }
    if request.starting {
        return refused(SimulatorStartFailureKind::AlreadyStarting, IN_PROGRESS);
    }
    SimulatorStartAnswer {
        decision: SimulatorStartDecision::Start,
        reply: None,
    }
}

/// The reply to the extension's report of its start (`controlStudio/simulatorStartResult`): the lifecycle service's sentence when
/// it started the simulator (or `Simulator started.` when it gave none), or a start failure with its sentence.
#[must_use]
pub fn outcome(outcome: &SimulatorStartOutcome) -> SimulatorStartReply {
    if outcome.ok {
        let message = non_empty(outcome.message.as_deref()).unwrap_or(STARTED);
        return SimulatorStartReply {
            ok: true,
            message: Some(message.to_string()),
            failure: None,
        };
    }
    let message = non_empty(outcome.failure_message.as_deref()).unwrap_or(NOT_STARTED);
    SimulatorStartReply {
        ok: false,
        message: None,
        failure: Some(SimulatorStartFailure {
            kind: SimulatorStartFailureKind::StartFailed,
            message: message.to_string(),
        }),
    }
}

/// The refusal of a live request that carries no runtime connection, because no runtime is running (A2.8, E11.1). `runtime_mode` is
/// the mode of the window's runtime lifecycle, as the extension reports it with the request. When it is the local simulator
/// (`simulate`), the next action starts the simulator, through the Start simulator action. Otherwise the runtime is attached through
/// its endpoint, or the request names no mode (the command line), and the next action connects a runtime, which the engineer connects
/// or starts ([`runtime_required`]); the engine never starts the simulator for a physical target.
#[must_use]
pub fn runtime_not_running(runtime_mode: Option<&str>) -> Failure {
    if runtime_mode == Some(SIMULATE) {
        return Failure::new(
            SIMULATOR_NOT_RUNNING,
            Cause::Runtime,
            NextAction::StartSimulator,
        );
    }
    runtime_required()
}

fn non_empty(text: Option<&str>) -> Option<&str> {
    text.filter(|text| !text.is_empty())
}

fn refused(kind: SimulatorStartFailureKind, message: &str) -> SimulatorStartAnswer {
    answered(SimulatorStartReply {
        ok: false,
        message: None,
        failure: Some(SimulatorStartFailure {
            kind,
            message: message.to_string(),
        }),
    })
}

fn answered(reply: SimulatorStartReply) -> SimulatorStartAnswer {
    SimulatorStartAnswer {
        decision: SimulatorStartDecision::Reply,
        reply: Some(reply),
    }
}

#[cfg(test)]
#[path = "simulator_start_tests.rs"]
mod tests;
