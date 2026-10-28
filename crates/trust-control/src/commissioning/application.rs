//! What Apply writes and what Restore asks for (specification 35, B6.1, B6.4 and C0, C1; protocol annex E7.3 and E7.7).
//!
//! The decision part of `controllerApplication.ts`: the record a validated `Control.PID` candidate writes, the request that
//! carries it and the checks it passes before it is sent, the decoding of the runtime's answers, and the rule that decides from
//! one status read whether a queued write is confirmed. The runtime calls and the polling loop belong to the language server;
//! nothing here reads a clock or talks to a runtime.

use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

use super::texts;
use crate::failure::{Cause, Failure, NextAction};
use crate::tuning::candidate::{validate_parameters, AntiWindup, ControlPidParameters};

/// Where a controller write goes (`source` of E7.7): a simulated target, or a physical one that needs approval (A2.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplySource {
    /// A simulated target: no approval is needed.
    Simulation,
    /// A physical target: each write needs the engineer's approval immediately before it starts (A2.5).
    Physical,
}

impl ApplySource {
    /// The word the runtime reads for this source (E7.7).
    #[must_use]
    pub fn wire(self) -> &'static str {
        match self {
            Self::Simulation => "simulation",
            Self::Physical => "physical",
        }
    }
}

/// The record `control.pid.apply` writes (C0, C1; E7.7): the twelve fields of the shipped `PIDParameters`, in the order and with
/// the names the TypeScript record used. A candidate writes every field, so a term its family does not use is an explicit zero.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ControlPidWireRecord {
    /// The proportional gain `Kp`.
    pub proportional_gain: f64,
    /// The integral gain `Ki`.
    pub integral_gain: f64,
    /// The derivative gain `Kd`.
    pub derivative_gain: f64,
    /// The proportional setpoint weight `β`.
    pub proportional_setpoint_weight: f64,
    /// The derivative setpoint weight `γ`.
    pub derivative_setpoint_weight: f64,
    /// The bias of the output.
    pub bias: f64,
    /// The smallest output.
    pub output_minimum: f64,
    /// The largest output.
    pub output_maximum: f64,
    /// The smallest rate of change of the output per second.
    pub rate_minimum: f64,
    /// The largest rate of change of the output per second.
    pub rate_maximum: f64,
    /// The tracking time `Tt` in seconds.
    pub tracking_time: f64,
    /// The anti-windup method.
    pub anti_windup: AntiWindup,
}

impl ControlPidWireRecord {
    /// The record a validated candidate writes: the candidate's twelve fields, unchanged (`controllerApplication.ts`).
    #[must_use]
    pub fn from_parameters(parameters: &ControlPidParameters) -> Self {
        Self {
            proportional_gain: parameters.proportional_gain,
            integral_gain: parameters.integral_gain,
            derivative_gain: parameters.derivative_gain,
            proportional_setpoint_weight: parameters.proportional_setpoint_weight,
            derivative_setpoint_weight: parameters.derivative_setpoint_weight,
            bias: parameters.bias,
            output_minimum: parameters.output_minimum,
            output_maximum: parameters.output_maximum,
            rate_minimum: parameters.rate_minimum,
            rate_maximum: parameters.rate_maximum,
            tracking_time: parameters.tracking_time,
            anti_windup: parameters.anti_windup,
        }
    }
}

/// The request that writes a validated candidate to a `Control.PID` (`control.pid.apply`, E7.7). Its fields carry the runtime's
/// names; a physical source carries the engineer's approval (A2.5).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ControllerApplyRequest {
    /// The controller's instance path, for example `Main.TIC101`.
    pub instance_path: String,
    /// `simulation` or `physical`.
    pub source: &'static str,
    /// The active revision the candidate was reviewed against; the write is refused when it has changed.
    pub expected_revision: String,
    /// The complete record.
    pub parameters: ControlPidWireRecord,
    /// The engineer's approval, required on a physical target.
    pub physical_approved: bool,
}

impl ControllerApplyRequest {
    /// The request for a candidate, checked before it is sent: a physical source has its approval, the candidate passes the
    /// record's checks (C0), and the instance path and the reviewed revision are not blank.
    ///
    /// # Errors
    /// Returns the refusal of the first check that fails: the missing approval, the record's refusal, or a blank field.
    pub fn new(
        instance_path: &str,
        source: ApplySource,
        expected_revision: &str,
        candidate: &ControlPidParameters,
        physical_approved: bool,
    ) -> Result<Self, Failure> {
        if source == ApplySource::Physical && !physical_approved {
            return Err(Failure::new(
                &texts::application_physical_approval_required(),
                Cause::Approval,
                NextAction::Approve,
            ));
        }
        validate_parameters(candidate)
            .map_err(|refusal| Failure::new(&refusal.0, Cause::Settings, NextAction::OpenLimits))?;
        Ok(Self {
            instance_path: non_blank(
                instance_path,
                "instance path",
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            )?,
            source: source.wire(),
            expected_revision: non_blank(
                expected_revision,
                "reviewed revision",
                Cause::Settings,
                NextAction::Validate,
            )?,
            parameters: ControlPidWireRecord::from_parameters(candidate),
            physical_approved,
        })
    }
}

/// The request that reads a `Control.PID`'s status (`control.pid.status`, E7.7).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ControllerStatusRequest {
    /// The controller's instance path.
    pub instance_path: String,
}

impl ControllerStatusRequest {
    /// The request for a controller's status.
    ///
    /// # Errors
    /// The instance path is blank.
    pub fn new(instance_path: &str) -> Result<Self, Failure> {
        Ok(Self {
            instance_path: non_blank(
                instance_path,
                "instance path",
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            )?,
        })
    }
}

/// The request that returns a `Control.PID` to the revision Apply retained (`control.pid.rollback`, E7.7). A physical target needs
/// the engineer's approval (A2.5); the runtime decides from its own target fact whether it does.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ControllerRollbackRequest {
    /// The controller's instance path.
    pub instance_path: String,
    /// The active revision the rollback was decided against.
    pub expected_active_revision: String,
    /// The revision to return to, as Apply retained it.
    pub previous_revision: String,
    /// The engineer's approval, carried on a physical target.
    pub physical_approved: bool,
}

impl ControllerRollbackRequest {
    /// The request that returns a controller to its retained revision.
    ///
    /// # Errors
    /// A revision or the instance path is blank.
    pub fn new(
        instance_path: &str,
        expected_active_revision: &str,
        previous_revision: &str,
        physical_approved: bool,
    ) -> Result<Self, Failure> {
        Ok(Self {
            instance_path: non_blank(
                instance_path,
                "instance path",
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            )?,
            expected_active_revision: non_blank(
                expected_active_revision,
                "active revision",
                Cause::Implementation,
                NextAction::None,
            )?,
            previous_revision: non_blank(
                previous_revision,
                "previous revision",
                Cause::Implementation,
                NextAction::None,
            )?,
            physical_approved,
        })
    }
}

/// The controller's status as the rules read it (`control.pid.status`, E7.7).
#[derive(Debug, Clone, PartialEq)]
pub struct ControllerStatus {
    /// The revision the controller holds.
    pub active_revision: String,
    /// The write queued and not yet read back, when there is one.
    pub pending_revision: Option<String>,
    /// Whether the runtime has confirmed the record at a scan boundary.
    pub confirmed: bool,
    /// The record's values, as the runtime reports them.
    pub parameters: serde_json::Map<String, Value>,
}

/// Decodes the runtime's status answer (`ControllerApplicationClient.status`). An empty pending revision is no pending write.
///
/// # Errors
/// The parameters are not an object, or the active revision is not a non-blank text.
pub fn decode_status(response: &Value) -> Result<ControllerStatus, Failure> {
    let Some(parameters) = response.get("parameters").and_then(Value::as_object) else {
        return Err(Failure::new(
            &texts::application_status_parameters_invalid(),
            Cause::Runtime,
            NextAction::None,
        ));
    };
    Ok(ControllerStatus {
        active_revision: revision_answer(response.get("active_revision"))?,
        pending_revision: response
            .get("pending_revision")
            .and_then(Value::as_str)
            .filter(|pending| !pending.is_empty())
            .map(str::to_owned),
        confirmed: response.get("confirmed").and_then(Value::as_bool) == Some(true),
        parameters: parameters.clone(),
    })
}

/// The revisions that an accepted write names (`control.pid.apply`'s answer, E7.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteAccepted {
    /// The revision the write replaces; Restore returns to it (B6.4).
    pub previous_revision: String,
    /// The revision the write queues; it is active once the runtime confirms it.
    pub pending_revision: String,
}

/// Decodes the answer to `control.pid.apply`.
///
/// # Errors
/// The answer does not name a non-blank previous revision and a non-blank pending revision.
pub fn decode_apply_answer(response: &Value) -> Result<WriteAccepted, Failure> {
    Ok(WriteAccepted {
        previous_revision: revision_answer(response.get("previous_revision"))?,
        pending_revision: revision_answer(response.get("pending_revision"))?,
    })
}

/// Decodes the answer to `control.pid.rollback`: the revision it queues.
///
/// # Errors
/// The answer does not name a non-blank pending revision.
pub fn decode_rollback_answer(response: &Value) -> Result<String, Failure> {
    revision_answer(response.get("pending_revision"))
}

/// Whether a queued write is confirmed after one status read (C1 "Atomic write"; E7.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirmation {
    /// The runtime holds the reviewed revision and has confirmed it at a scan boundary.
    Confirmed,
    /// Not yet: the write is queued, or the record is not read back. Read the status again.
    Pending,
}

/// Decides from one status read whether the reviewed write is confirmed (`waitForConfirmedRevision`). A queued revision is never
/// labelled applied before the complete structure is read back (C1), and another write's revision is never taken for the candidate.
///
/// # Errors
/// Another write has replaced the reviewed candidate, or the record changed with no write pending.
pub fn confirmation_step(
    status: &ControllerStatus,
    reviewed_revision: &str,
) -> Result<Confirmation, Failure> {
    if status.confirmed && status.active_revision == reviewed_revision {
        return Ok(Confirmation::Confirmed);
    }
    match status.pending_revision.as_deref() {
        Some(pending) if pending != reviewed_revision => Err(Failure::new(
            &texts::application_candidate_replaced(pending, reviewed_revision),
            Cause::Runtime,
            NextAction::Validate,
        )),
        None if status.active_revision != reviewed_revision => Err(Failure::new(
            &texts::application_not_confirmed(&status.active_revision, reviewed_revision),
            Cause::Runtime,
            NextAction::Validate,
        )),
        _ => Ok(Confirmation::Pending),
    }
}

/// The failure of a confirmation that the window did not reach (A7): the runtime may still confirm the write, so the engineer waits.
#[must_use]
pub fn confirmation_timed_out(window_ms: f64) -> Failure {
    Failure::new(
        &texts::application_confirmation_timed_out(window_ms),
        Cause::Runtime,
        NextAction::Wait,
    )
}

/// The result of an applied write (`ApplicationResult`, B6.1): the revision it replaced and the revision the runtime confirmed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppliedRevisions {
    /// The revision the write replaced, which Restore returns to.
    pub previous_revision: String,
    /// The revision the runtime confirmed as active.
    pub active_revision: String,
}

impl AppliedRevisions {
    /// The result of a write the runtime has confirmed: its pending revision is now the active one.
    #[must_use]
    pub fn confirmed(accepted: &WriteAccepted) -> Self {
        Self {
            previous_revision: accepted.previous_revision.clone(),
            active_revision: accepted.pending_revision.clone(),
        }
    }
}

/// The text of a required request field: it must not be blank, and it is kept as given (`required` of `controllerApplication.ts`).
fn non_blank(
    text: &str,
    field: &str,
    cause: Cause,
    next_action: NextAction,
) -> Result<String, Failure> {
    if text.trim().is_empty() {
        return Err(Failure::new(
            &texts::application_request_blank(field),
            cause,
            next_action,
        ));
    }
    Ok(text.to_owned())
}

/// A revision the runtime answered with: a non-blank text (`required` of `controllerApplication.ts`).
fn revision_answer(value: Option<&Value>) -> Result<String, Failure> {
    value
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            Failure::new(
                &texts::application_revision_invalid(),
                Cause::Runtime,
                NextAction::None,
            )
        })
}

#[cfg(test)]
#[path = "application_tests.rs"]
mod tests;
