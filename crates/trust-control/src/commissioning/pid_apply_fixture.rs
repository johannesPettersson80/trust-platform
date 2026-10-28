//! The Control.PID apply fixture of the commissioning tests (test only): the loop, the reading and the records of the TypeScript apply
//! tests (`control-studio-pid-fixtures.ts`, `control-studio-apply-process-time.test.ts`), so that the rules are tested on the same numbers.
//! Declared under `cfg(test)` by `commissioning.rs`.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::commissioning_result::{TransferState, TransferStatus};
use super::parameter_application::{ParameterReview, ReviewKind};
use super::parameter_status::{ParameterStatus, RecordState};
use super::pid_commissioning::{PidCampaign, PidRunProfile, PidStart, ReviewSetup};
use super::transfer::TransferValue;
use crate::failure::Failure;
use crate::loop_definition::LoopDefinition;
use crate::model::GeneralModel;
use crate::validation::engineer_verification_plan::EngineerRequirements;

/// The measured output's noise of the TypeScript tests (`noise = 0.05`, A6.4).
pub(crate) const NOISE: f64 = 0.05;

/// The Control.PID loop of the apply tests (`control-studio-pid-fixtures.ts`, `pidLoop` with `pidInitialization`): the set point
/// `Main.SP` and the measurement `Main.PV` in m, the manipulated variable `Main.MV` in V within ±20, a 20 ms task, the parallel PID gains
/// `Main.Kp`, `Main.Ki` and `Main.Kd` within ±1e6, the states `Main.Integral` and `Main.PreviousError`, and the mode `Main.Manual`, TRUE in
/// Manual. `additional` lists the loop's further signals; `setpoint_type` is the set point's IEC type.
#[allow(clippy::needless_pass_by_value)] // A test fixture: it takes the JSON literals its callers write in place.
pub(crate) fn pid_loop(setpoint_type: &str, additional: Value) -> LoopDefinition {
    let signals = json!({
        "setpoint": { "symbol": "Main.SP", "address": "%ML0", "iecType": setpoint_type, "direction": "read",
            "unit": "m", "range": { "minimum": -20, "maximum": 20 } },
        "processVariable": { "symbol": "Main.PV", "address": "%IL0", "iecType": "LREAL", "direction": "read",
            "unit": "m", "range": { "minimum": -20, "maximum": 20 } },
        "manipulatedVariable": { "symbol": "Main.MV", "address": "%QL0", "iecType": "LREAL", "direction": "write",
            "unit": "V", "range": { "minimum": -20, "maximum": 20 } },
        "additional": additional
    });
    let file = json!({
        "schemaVersion": 2, "id": "loop:pid-apply", "name": "PID apply", "revision": "sha256:pid-apply-1",
        "projectId": "project:pid-apply", "runtimeId": "runtime:pid-apply",
        "controller": {
            "instance": "Main", "type": "Control.PID", "revision": "sha256:pid-controller-1", "action": "direct",
            "design": "pid", "taskPeriodMs": 20,
            "law": { "kind": "pid", "parameterRecordSymbol": "Main.Parameters", "proportionalGainSymbol": "Main.Kp",
                "integralGainSymbol": "Main.Ki", "derivativeGainSymbol": "Main.Kd" },
            "manualOutputSignal": "Main.ManualCommand", "manualOutputAddress": "%ML8", "manualOutputType": "LREAL",
            "pidInitialization": { "kind": "parallel-pid-v1",
                "integralState": { "symbol": "Main.Integral", "iecType": "LREAL", "unit": "V", "minimum": -20, "maximum": 20 },
                "derivativeState": { "symbol": "Main.PreviousError", "iecType": "LREAL", "unit": "m", "minimum": -20, "maximum": 20 },
                "proportionalSetpointWeight": 1, "derivativeSetpointWeight": 1, "maximumSnapshotAgeMs": 100 }
        },
        "signals": signals,
        "limits": { "outputMinimum": -20, "outputMaximum": 20 },
        "observedSampleTiming": { "nominalPeriodMs": 20, "maximumJitterMs": 1 },
        "parameterBinding": { "loop_id": "loop:pid-apply", "task_name": "MainTask",
            "parameters": [
                { "symbol": "Main.Kp", "iec_type": "LREAL", "unit": "V/m", "convention": "proportional gain", "minimum": -1e6, "maximum": 1e6 },
                { "symbol": "Main.Ki", "iec_type": "LREAL", "unit": "V/(m*s)", "convention": "integral gain", "minimum": -1e6, "maximum": 1e6 },
                { "symbol": "Main.Kd", "iec_type": "LREAL", "unit": "V*s/m", "convention": "derivative gain", "minimum": -1e6, "maximum": 1e6 }
            ],
            "mode": { "symbol": "Main.Manual", "manual_value": true } }
    });
    LoopDefinition::decode(&file.to_string()).expect("the apply fixture loop decodes")
}

/// A further measured output of role PV, `P2.Pressure` in bar (B1.4), as the apply tests add it.
pub(crate) fn secondary_signal() -> Value {
    json!([{ "id": "pv-secondary", "channel": "secondary", "role": "PV", "symbol": "P2.Pressure", "address": "%IL8",
        "iecType": "LREAL", "direction": "read", "unit": "bar", "range": { "minimum": 1, "maximum": 3 } }])
}

/// The values of a reading of the apply fixture: the set point `Main.SP` 2, the measurement `Main.PV` 2.004 (the validated point is 2), the
/// output `Main.MV` at `output`, the controller states at zero, the gains `Main.Kp` 1, `Main.Ki` 0 and `Main.Kd` 0 in force, and
/// `Main.Manual` at `manual`.
pub(crate) fn values(manual: bool, output: f64) -> BTreeMap<String, TransferValue> {
    let mut readings: BTreeMap<String, TransferValue> = [
        ("Main.SP", 2.0),
        ("Main.PV", 2.004),
        ("Main.MV", output),
        ("Main.ManualCommand", 0.0),
        ("Main.Integral", 0.0),
        ("Main.PreviousError", 0.0),
        ("Main.Kp", 1.0),
        ("Main.Ki", 0.0),
        ("Main.Kd", 0.0),
    ]
    .into_iter()
    .map(|(symbol, value)| (symbol.to_owned(), TransferValue::Number(value)))
    .collect();
    readings.insert("Main.Manual".to_owned(), TransferValue::Boolean(manual));
    readings
}

/// A transfer status that reports `values`, as the runtime reports a transfer (annex E6.3).
pub(crate) fn status(values: BTreeMap<String, TransferValue>) -> TransferStatus {
    TransferStatus {
        program_revision: "program".to_owned(),
        binding_revision: "transfer".to_owned(),
        values,
        runtime_time_ns: Some(1_000_000_000),
        status: TransferState::Idle,
        reason: None,
        supported_tracking: Some(vec![
            "parallel-pid-v1".to_owned(),
            "proportional".to_owned(),
        ]),
        transaction: None,
    }
}

/// The parameter record of the apply fixture at the gains `kp`, `ki` and `kd`, in force under `active`.
pub(crate) fn record(kp: f64, ki: f64, kd: f64, active: &str) -> ParameterStatus {
    ParameterStatus {
        supports_request_id: Some(true),
        transaction_id: Some("apply-1".to_owned()),
        program_revision: "program".to_owned(),
        binding_revision: "parameters".to_owned(),
        active_revision: active.to_owned(),
        values: BTreeMap::from([
            ("Main.Kp".to_owned(), kp),
            ("Main.Ki".to_owned(), ki),
            ("Main.Kd".to_owned(), kd),
        ]),
        status: RecordState::Applied,
        previous_revision: None,
        previous_values: None,
        reason: None,
    }
}

/// The candidate of the TypeScript tests, `Kp 2, Ki 0.5, Kd 0`, reviewed against the record at `Kp 1` (`review`).
pub(crate) fn review() -> ParameterReview {
    ParameterReview {
        transaction_id: "apply-1".to_owned(),
        observed: record(1.0, 0.0, 0.0, "original"),
        values: BTreeMap::from([
            ("Main.Kp".to_owned(), 2.0),
            ("Main.Ki".to_owned(), 0.5),
            ("Main.Kd".to_owned(), 0.0),
        ]),
        kind: ReviewKind::Apply,
    }
}

/// Begins and accepts a parallel PID review of the apply fixture at `observed` (`PidCommissioning.review`), with the measured output's
/// noise `noise`.
pub(crate) fn accept_at<'a>(
    loop_definition: &'a LoopDefinition,
    model: &'a GeneralModel,
    task: &'a EngineerRequirements,
    noise: Option<f64>,
    observed: &TransferStatus,
) -> Result<PidCampaign<'a>, Failure> {
    let setup = ReviewSetup {
        loop_definition,
        model,
        task,
        profile: PidRunProfile::ParallelPid,
        review: review(),
        reference_hold: None,
        process_noise: noise,
    };
    PidStart::begin(setup, observed)?.accept(observed)
}

/// The campaign the apply fixture starts from: found in Auto, with its output at 0 V, and the measured noise [`NOISE`].
pub(crate) fn campaign<'a>(
    loop_definition: &'a LoopDefinition,
    model: &'a GeneralModel,
    task: &'a EngineerRequirements,
) -> PidCampaign<'a> {
    accept_at(
        loop_definition,
        model,
        task,
        Some(NOISE),
        &status(values(false, 0.0)),
    )
    .expect("the apply fixture is reviewed at its starting point")
}

/// The refusal of a rule that refuses its input.
pub(crate) fn refusal<T>(result: Result<T, Failure>) -> Failure {
    result.err().expect("the rule refuses this input")
}
