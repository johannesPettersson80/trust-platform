//! Loops and recorded runs shared by the tests of the recording and experiment modules. Test builds only.
//!
//! The loops are the fixtures of the `TypeScript` tests (`loop()`, `boundLoop()`, `holdLoop()`,
//! `loopWithDisturbance()`), written as the loop file's JSON and decoded without validation.

use serde_json::{json, Value};

use crate::loop_definition::{AuxiliarySignal, LoopDefinition};
use crate::recording::{
    Acquisition, Completeness, RecordedRun, RecordedSample, ScanRange, StopReason,
};
use crate::signals::{Quality, SignalBinding};

/// The JSON of the fixture loop (`loop()`): a level in percent with a setpoint, a measurement and a manipulated
/// variable, output limits of 0 and 100 and a 10 ms nominal period.
fn base_json() -> Value {
    json!({
        "schemaVersion": 2,
        "id": "loop:tank",
        "name": "Tank level",
        "revision": "sha256:loop-1",
        "projectId": "project:fixture",
        "runtimeId": "runtime:local",
        "controller": {
            "instance": "TankController",
            "type": "Control.PID",
            "revision": "sha256:controller-1",
            "action": "direct",
            "taskPeriodMs": 10.0
        },
        "signals": {
            "setpoint": {
                "symbol": "LevelSP", "address": "%ML0", "iecType": "LREAL",
                "direction": "read", "unit": "%", "range": { "minimum": 0.0, "maximum": 100.0 }
            },
            "processVariable": {
                "symbol": "LevelPV", "address": "%IL0", "iecType": "LREAL",
                "direction": "read", "unit": "%", "range": { "minimum": 0.0, "maximum": 100.0 }
            },
            "manipulatedVariable": {
                "symbol": "ValveMV", "address": "%QL0", "iecType": "LREAL",
                "direction": "write", "unit": "%", "range": { "minimum": 0.0, "maximum": 100.0 }
            }
        },
        "limits": { "outputMinimum": 0.0, "outputMaximum": 100.0 },
        "observedSampleTiming": { "nominalPeriodMs": 10.0 }
    })
}

fn decode(value: Value) -> LoopDefinition {
    serde_json::from_value(value).expect("the fixture loop decodes")
}

/// The fixture loop (`loop()`).
#[must_use]
pub fn loop_definition() -> LoopDefinition {
    decode(base_json())
}

/// The fixture loop with a parameter record in task `MainTask` and the Manual-mode signal `P1.Run` (`boundLoop()`).
#[must_use]
pub fn bound_loop() -> LoopDefinition {
    let mut value = base_json();
    value["parameterBinding"] = parameter_binding(
        "MainTask",
        &[json!({
            "symbol": "TankController.Kp", "iec_type": "LREAL", "unit": "1",
            "convention": "proportional gain", "minimum": 0.0, "maximum": 10.0
        })],
    );
    decode(value)
}

/// The Lesson 14 step test (`holdLoop()`): a 5 s baseline, then a hold of 20 to 40 s that the experiment parameter
/// `P1.HoldSeconds` declares, in a 100 ms loop.
#[must_use]
pub fn hold_loop() -> LoopDefinition {
    let mut value = base_json();
    value["observedSampleTiming"] = json!({ "nominalPeriodMs": 100.0 });
    value["experiment"] = json!({
        "execution": "project-scripted",
        "kind": "observation",
        "amplitude": 0.0,
        "durationSeconds": 45.1,
        "seed": 1,
        "baselineSeconds": 5.0,
        "holdParameter": "P1.HoldSeconds",
        "activation": {
            "symbol": "P1.Run", "address": "%MX64.0", "iecType": "BOOL", "direction": "write"
        }
    });
    value["parameterBinding"] = parameter_binding(
        "MainTask",
        &[json!({
            "symbol": "P1.HoldSeconds", "iec_type": "REAL", "unit": "s",
            "convention": "held step duration", "minimum": 20.0, "maximum": 40.0
        })],
    );
    decode(value)
}

/// The fixture loop with a writeable disturbance `FeedFlow` at `%QL8` (`loopWithDisturbance()`).
#[must_use]
pub fn loop_with_disturbance() -> LoopDefinition {
    let mut value = base_json();
    value["signals"]["additional"] = json!([serde_json::to_value(auxiliary_signal(
        "feed-flow",
        "reactor",
        "disturbance",
        "FeedFlow",
        "%QL8",
        "write",
        "%",
    ))
    .expect("the fixture signal encodes")]);
    decode(value)
}

fn parameter_binding(task: &str, parameters: &[Value]) -> Value {
    json!({
        "loop_id": "loop:tank",
        "task_name": task,
        "parameters": parameters,
        "mode": { "symbol": "P1.Run", "manual_value": false }
    })
}

/// An additional signal of the fixture loop: a 0 to 100 signal in a channel with a role, at an address.
#[must_use]
pub fn auxiliary_signal(
    id: &str,
    channel: &str,
    role: &str,
    symbol: &str,
    address: &str,
    direction: &str,
    unit: &str,
) -> AuxiliarySignal {
    serde_json::from_value(json!({
        "id": id,
        "channel": channel,
        "role": role,
        "symbol": symbol,
        "address": address,
        "iecType": "LREAL",
        "direction": direction,
        "unit": unit,
        "range": { "minimum": 0.0, "maximum": 100.0 }
    }))
    .expect("the fixture signal decodes")
}

/// A binding of a recorded run that reads `address` as `iec_type` (annex E4.2).
#[must_use]
pub fn binding(id: &str, address: &str, iec_type: &str) -> SignalBinding {
    SignalBinding {
        id: id.to_string(),
        address: Some(address.to_string()),
        symbol: None,
        iec_type: iec_type.to_string(),
    }
}

/// One sample of a recorded run with no forced value: `values` and `quality` in the bindings' order, at the
/// runtime time `runtime_time_ns`.
#[must_use]
pub fn sample(
    scan: u64,
    runtime_time_ns: Option<i64>,
    values: &[Option<f64>],
    quality: &[Quality],
) -> RecordedSample {
    RecordedSample {
        scan,
        runtime_time_ns,
        captured_at_ms: 1_000 + scan,
        values: values.to_vec(),
        forced: vec![false; values.len()],
        quality: quality.to_vec(),
        gap: false,
    }
}

/// A complete recorded run of `samples` over `bindings`, stopped by a stop request (annex E4.7).
#[must_use]
pub fn run(bindings: Vec<SignalBinding>, samples: Vec<RecordedSample>) -> RecordedRun {
    let acquisition = Acquisition {
        method: "runtime_scan_buffer".to_string(),
        completeness: Completeness::Complete,
        observed_sample_count: u64::try_from(samples.len()).unwrap_or(u64::MAX),
        scan_range: scan_range(&samples),
        missing_scan_ranges: Vec::new(),
        dropped_observed_samples: 0,
        actual_cycle_timing_available: true,
        freshness_available: false,
        reason: "Complete recording.".to_string(),
    };
    RecordedRun {
        recording_id: "rec-fixture".to_string(),
        loop_revision: "sha256:loop-1".to_string(),
        program_revision: Some("sha256:program".to_string()),
        bindings,
        stop_reason: StopReason::Stopped,
        samples,
        acquisition,
    }
}

fn scan_range(samples: &[RecordedSample]) -> Option<ScanRange> {
    Some(ScanRange {
        first: samples.first()?.scan,
        last: samples.last()?.scan,
    })
}
