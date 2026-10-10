//! Shared fixtures of the identification tests after the recorded model: connected records built from their JSON words, with
//! a verdict that stands in for the evaluation (stage V3). Test-only.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::identification::connected_record::RuntimeVerificationResult;
use crate::identification::independent_validation::{RequirementStatus, RunEvaluation};

/// The evaluation of a fixture record: its verdict only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Verdict(pub RequirementStatus);

impl RunEvaluation for Verdict {
    fn status(&self) -> RequirementStatus {
        self.0
    }
}

/// One fixture sample: its time, its sequence value and each channel's value in the order of the channels (`None` is a
/// missing value, recorded as bad).
pub type Row = (f64, u64, Vec<Option<f64>>);

/// What a fixture record is made of.
#[derive(Debug, Clone)]
pub struct Spec<'a> {
    /// The recording's identifier.
    pub id: &'a str,
    /// The project revision the record was taken on.
    pub project_revision: &'a str,
    /// The model revision of the run's conditions.
    pub model_revision: &'a str,
    /// The experiment kind of the reviewed plan, as the plan words it (for example `setpoint_step`).
    pub kind: &'a str,
    /// The channels: symbol and unit.
    pub channels: &'a [(&'a str, &'a str)],
    /// The samples.
    pub rows: &'a [Row],
    /// The steady baseline window of the request, when it has one.
    pub baseline: Option<(f64, f64)>,
    /// The verdict of the evaluation.
    pub verdict: RequirementStatus,
}

/// A connected record of a setpoint step with the default revisions and no baseline window.
#[must_use]
pub fn setpoint_record(
    id: &str,
    channels: &[(&str, &str)],
    rows: &[Row],
    verdict: RequirementStatus,
) -> RuntimeVerificationResult<Verdict> {
    build(&Spec {
        id,
        project_revision: "sha256:project",
        model_revision: "connected-plant:tank",
        kind: "setpoint_step",
        channels,
        rows,
        baseline: None,
        verdict,
    })
}

/// The connected record of a `Spec`.
#[must_use]
pub fn build(spec: &Spec<'_>) -> RuntimeVerificationResult<Verdict> {
    let first_sequence = spec.rows.first().map_or(0, |row| row.1);
    let last_sequence = spec.rows.last().map_or(0, |row| row.1);
    let samples: Vec<Value> = spec
        .rows
        .iter()
        .map(|(time, sequence, values)| {
            let readings: serde_json::Map<String, Value> = spec
                .channels
                .iter()
                .zip(values)
                .map(|((symbol, _), value)| {
                    let reading = match value {
                        Some(value) => json!({ "value": value, "quality": "good" }),
                        None => json!({ "value": null, "quality": "bad" }),
                    };
                    ((*symbol).to_owned(), reading)
                })
                .collect();
            json!({ "timeSeconds": time, "sequence": sequence, "values": readings })
        })
        .collect();
    let channel_list: Vec<Value> = spec
        .channels
        .iter()
        .map(|(symbol, unit)| json!({ "id": symbol, "unit": unit }))
        .collect();
    let mut request = json!({
        "execution": "connected-runtime",
        "loop_revision": "sha256:loop",
        "acquisition": {
            "recordingId": spec.id,
            "programRevision": "sha256:program",
            "method": "runtime_scan_buffer",
            "completeness": "complete",
            "observedSampleCount": spec.rows.len(),
            "scanRange": { "first": first_sequence, "last": last_sequence },
            "missingScanRanges": [],
            "droppedObservedSamples": 0,
            "actualCycleTimingAvailable": true,
            "freshnessAvailable": false,
            "reason": "Complete recording."
        },
        "experiment": setpoint_plan_json(spec.kind),
    });
    if let Some((start, end)) = spec.baseline {
        request["baselineStartSeconds"] = json!(start);
        request["baselineEndSeconds"] = json!(end);
    }
    let times: Vec<f64> = spec.rows.iter().map(|row| row.0).collect();
    let duration = times.last().copied().unwrap_or(0.0);
    let value = json!({
        "projectRevision": spec.project_revision,
        "request": request,
        "run": {
            "id": spec.id,
            "evidence": "ST-executed",
            "parameters": {},
            "conditions": {
                "initialState": {},
                "events": [],
                "durationSeconds": duration,
                "sampleTimesSeconds": times,
                "seed": 1,
                "modelRevision": spec.model_revision,
                "programRevision": "sha256:program",
                "parameterSchemaRevision": "sha256:loop"
            },
            "channels": channel_list,
            "samples": samples
        },
        "evaluation": Verdict(spec.verdict)
    });
    serde_json::from_value(value).expect("the fixture record decodes")
}

/// The reviewed plan of an experiment of the given kind on the fixture loop, as the connected record stores it.
fn setpoint_plan_json(kind: &str) -> Value {
    json!({
        "id": "experiment:tank:11:setpoint_step",
        "source": "simulation",
        "kind": kind,
        "affectedSignal": "LevelSP",
        "valueType": "LREAL",
        "from": 50.0,
        "to": 55.0,
        "unit": "%",
        "minimum": 0.0,
        "maximum": 100.0,
        "durationMs": 20.0,
        "expectedScan": 11,
        "stopConditions": [],
        "restorationValue": 50.0,
        "requiresPhysicalApproval": false,
        "trajectory": []
    })
}

/// A fitted transfer model of one input and one output named `MV` and `PV` (both in the loop's units), with the given
/// evidence and the numerator and denominator (descending powers).
#[must_use]
pub fn fitted_model(
    evidence: Vec<String>,
    numerator: Vec<f64>,
    denominator: Vec<f64>,
) -> crate::model::GeneralModel {
    use crate::model::{ModelChannel, ModelIdentity, ModelSource, ModelValidation, OperatingRange};
    let channel = |id: &str, unit: &str| ModelChannel {
        id: id.to_owned(),
        unit: unit.to_owned(),
        operating_range: OperatingRange {
            minimum: 0.0,
            maximum: 100.0,
        },
    };
    crate::model::GeneralModel::TransferFunction {
        identity: ModelIdentity {
            revision: "sha256:fitted".to_owned(),
            source: ModelSource::Fitted,
            validation: ModelValidation::Validated,
            evidence,
            inputs: vec![channel("MV", "%")],
            outputs: vec![channel("PV", "degC")],
            delay_seconds: 0.0,
            operating_point: None,
        },
        transfer: trust_control_math::model::TransferFunction {
            numerator,
            denominator,
        },
    }
}
