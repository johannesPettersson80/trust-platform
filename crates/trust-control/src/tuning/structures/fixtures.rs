//! Fixtures of the structure tests: the models of the TypeScript tests (Specification 35, C6 to C9), the requirements of each test, and
//! the support that makes no prediction. With no prediction, a design takes the declared response's formula, as N5.12 allows when no
//! model prediction is available, so the tests check the rules that do not depend on the engine's predictor.

use trust_control_math::model::{OperatingPoint, StateSpace, TransferFunction};

use super::path::PathSupport;
use crate::model::{
    GeneralModel, ModelChannel, ModelIdentity, ModelSource, ModelValidation, OperatingRange,
};
use crate::tuning::placement::{PlacementRefusal, PolePlacement};
use crate::tuning::plant::TuningPlant;
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::shortfall::{ResponsePredictor, TimePoint, Trial};
use crate::tuning::structure::{ParallelGains, Structure};
use crate::tuning::typed_pid::TypedDesign;

/// A model channel from `minimum` to `maximum` in `unit`.
pub fn channel(id: &str, unit: &str, minimum: f64, maximum: f64) -> ModelChannel {
    ModelChannel {
        id: id.to_owned(),
        unit: unit.to_owned(),
        operating_range: OperatingRange { minimum, maximum },
    }
}

/// The identity of a fixture model: a validated fit with its channels, delay and operating point.
fn identity(
    inputs: Vec<ModelChannel>,
    outputs: Vec<ModelChannel>,
    delay_seconds: f64,
    operating_point: Option<OperatingPoint>,
) -> ModelIdentity {
    ModelIdentity {
        revision: "sha256:fixture".to_owned(),
        source: ModelSource::Fitted,
        validation: ModelValidation::Validated,
        evidence: vec!["sha256:recording".to_owned()],
        inputs,
        outputs,
        delay_seconds,
        operating_point,
    }
}

/// A continuous state-space model; its states are named `state0`, `state1`, and so on.
fn continuous(
    system: StateSpace,
    inputs: Vec<ModelChannel>,
    outputs: Vec<ModelChannel>,
    delay_seconds: f64,
    operating_point: Option<OperatingPoint>,
) -> GeneralModel {
    let states = (0..system.states())
        .map(|index| channel(&format!("state{index}"), "1", -1e3, 1e3))
        .collect();
    GeneralModel::ContinuousStateSpace {
        identity: identity(inputs, outputs, delay_seconds, operating_point),
        states,
        system,
    }
}

/// An operating point with the given inputs and outputs.
fn point(inputs: Vec<f64>, outputs: Vec<f64>) -> OperatingPoint {
    OperatingPoint { inputs, outputs }
}

/// The heater of the feedforward tests: 0.2 °C per % through an 8 s lag, and −0.1 °C per kW through a 3 s lag, at 35 °C with 80 %
/// and 10 kW (`heaterModel` of `control-studio-feedforward-fixtures.ts`).
pub fn heater_model() -> GeneralModel {
    let system = StateSpace {
        a: vec![vec![-1.0 / 8.0, 0.0], vec![0.0, -1.0 / 3.0]],
        b: vec![vec![0.2 / 8.0, 0.0], vec![0.0, -0.1 / 3.0]],
        c: vec![vec![1.0, 1.0]],
        d: vec![vec![0.0, 0.0]],
    };
    continuous(
        system,
        vec![
            channel("P1.HeaterPct", "%", 0.0, 100.0),
            channel("P1.LoadKw", "kW", 0.0, 50.0),
        ],
        vec![channel("P1.WaterC", "degC", 0.0, 80.0)],
        0.0,
        Some(point(vec![80.0, 10.0], vec![35.0])),
    )
}

/// The heater whose disturbance reaches the water through two 1.5 s lags, so its path has a delay (the feedforward test of
/// a slow measured disturbance).
pub fn slow_load_model() -> GeneralModel {
    let system = StateSpace {
        a: vec![
            vec![-1.0 / 8.0, 0.0, 0.0],
            vec![0.0, -1.0 / 1.5, 0.0],
            vec![0.0, 1.0 / 1.5, -1.0 / 1.5],
        ],
        b: vec![vec![0.2 / 8.0, 0.0], vec![0.0, -0.1 / 1.5], vec![0.0, 0.0]],
        c: vec![vec![1.0, 0.0, 1.0]],
        d: vec![vec![0.0, 0.0]],
    };
    continuous(
        system,
        vec![
            channel("P1.HeaterPct", "%", 0.0, 100.0),
            channel("P1.LoadKw", "kW", 0.0, 50.0),
        ],
        vec![channel("P1.WaterC", "degC", 0.0, 80.0)],
        0.0,
        Some(point(vec![80.0, 10.0], vec![35.0])),
    )
}

/// The vessel of the cascade test: a jacket heated by the plant input, and the vessel that follows the jacket with 10 s
/// (`model` of `control-studio-cascade-together.test.ts`). The jacket is the first output and the vessel the second.
pub fn vessel_model() -> GeneralModel {
    let system = StateSpace {
        a: vec![vec![-0.5, 0.0], vec![0.1, -0.1]],
        b: vec![vec![0.1], vec![0.0]],
        c: vec![vec![1.0, 0.0], vec![0.0, 1.0]],
        d: vec![vec![0.0], vec![0.0]],
    };
    continuous(
        system,
        vec![channel("P1.HeaterPct", "%", 0.0, 100.0)],
        vec![
            channel("P1.JacketC", "degC", 0.0, 90.0),
            channel("P1.VesselC", "degC", 0.0, 90.0),
        ],
        0.0,
        Some(point(vec![50.0], vec![40.0, 40.0])),
    )
}

/// Two zones that share a wall, with the decentralized test's matrices: K = [[0.225, 0.075], [0.075, 0.225]] °C per %.
pub fn zones_model() -> GeneralModel {
    let system = StateSpace {
        a: vec![vec![-0.15, 0.05], vec![0.05, -0.15]],
        b: vec![vec![0.03, 0.0], vec![0.0, 0.03]],
        c: vec![vec![1.0, 0.0], vec![0.0, 1.0]],
        d: vec![vec![0.0, 0.0], vec![0.0, 0.0]],
    };
    continuous(
        system,
        vec![
            channel("u1", "%", -100.0, 100.0),
            channel("u2", "%", -100.0, 100.0),
        ],
        vec![
            channel("y1", "degC", -100.0, 100.0),
            channel("y2", "degC", -100.0, 100.0),
        ],
        0.0,
        None,
    )
}

/// The crossed pair of the refusal test: K = [[0.2, 0.5], [0.5, 0.2]], so the direct pairing has a negative diagonal relative gain.
pub fn crossed_zones_model() -> GeneralModel {
    let system = StateSpace {
        a: vec![vec![-1.0, 0.0], vec![0.0, -1.0]],
        b: vec![vec![0.2, 0.5], vec![0.5, 0.2]],
        c: vec![vec![1.0, 0.0], vec![0.0, 1.0]],
        d: vec![vec![0.0, 0.0], vec![0.0, 0.0]],
    };
    continuous(
        system,
        vec![
            channel("u1", "%", -100.0, 100.0),
            channel("u2", "%", -100.0, 100.0),
        ],
        vec![
            channel("y1", "degC", -100.0, 100.0),
            channel("y2", "degC", -100.0, 100.0),
        ],
        0.0,
        None,
    )
}

/// The two zones of the decoupler test: heat crosses the shared wall, with K = −A⁻¹B = [[0.085, 0.026], [0.026, 0.085]] °C per %.
pub fn decoupler_zones_model() -> GeneralModel {
    let system = StateSpace {
        a: vec![vec![-0.13, 0.04], vec![0.04, -0.13]],
        b: vec![vec![0.01, 0.0], vec![0.0, 0.01]],
        c: vec![vec![1.0, 0.0], vec![0.0, 1.0]],
        d: vec![vec![0.0, 0.0], vec![0.0, 0.0]],
    };
    continuous(
        system,
        vec![
            channel("P1.Heater1Pct", "%", 0.0, 100.0),
            channel("P1.Heater2Pct", "%", 0.0, 100.0),
        ],
        vec![
            channel("P1.Zone1C", "degC", 0.0, 100.0),
            channel("P1.Zone2C", "degC", 0.0, 100.0),
        ],
        0.0,
        Some(point(vec![50.0, 50.0], vec![35.0, 35.0])),
    )
}

/// The heater of the Smith predictor test: 0.2/(5s + 1) with a delay of one second, at 35 °C with 75 %.
pub fn smith_heater_model() -> GeneralModel {
    GeneralModel::TransferFunction {
        identity: identity(
            vec![channel("P1.HeaterPct", "%", 0.0, 100.0)],
            vec![channel("P1.WaterC", "degC", 0.0, 80.0)],
            1.0,
            Some(point(vec![75.0], vec![35.0])),
        ),
        transfer: TransferFunction {
            numerator: vec![0.2],
            denominator: vec![5.0, 1.0],
        },
    }
}

/// The requirements of a test: the reference change, the maximum error, the settling band, the settling time and the duration,
/// with no declared formula method and no declared objective.
pub fn requirements(
    reference_change: f64,
    maximum_error: f64,
    settling_band: f64,
    settling_time_seconds: f64,
    duration_seconds: f64,
) -> TuningRequirements {
    TuningRequirements {
        reference_change,
        maximum_error,
        settling_band,
        settling_time_seconds,
        duration_seconds,
        constant_input_disturbance: None,
        measurement_noise_band: None,
        gain_uncertainty_fraction: None,
        transfer_tolerance: None,
        maximum_overshoot_percent: None,
        tuning_method: None,
        tuning_objective: None,
        automatic_criteria: None,
    }
}

/// The support of the tests: no response prediction and no pole placement. Every design then takes the declared response's formula.
pub struct NoSupport;

impl PathSupport for NoSupport {
    fn predictor(
        &self,
        _path: &TuningPlant,
        _operating_input: f64,
        _operating_output: f64,
    ) -> Box<dyn ResponsePredictor + '_> {
        Box::new(NoPrediction)
    }

    fn placement(&self, _path: &TuningPlant) -> Box<dyn PolePlacement + '_> {
        Box::new(NoPlacement)
    }
}

/// A predictor that predicts nothing, so every candidate goes unpredicted (N5.12).
struct NoPrediction;

impl ResponsePredictor for NoPrediction {
    fn predict(
        &self,
        _design: &TypedDesign,
        _trial: Trial,
        _requirements: &TuningRequirements,
    ) -> Option<Vec<TimePoint>> {
        None
    }
}

/// A placement that has no form for any structure.
struct NoPlacement;

impl PolePlacement for NoPlacement {
    fn gains(
        &self,
        _family: Structure,
        _envelope: f64,
        _settling_time_seconds: f64,
    ) -> Result<ParallelGains, PlacementRefusal> {
        Err(PlacementRefusal::NoPlacementForm)
    }
}
