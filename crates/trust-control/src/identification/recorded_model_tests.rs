//! Tests of the model fitted from recordings (`recordedModelIdentification.ts`), and of the gain sign check
//! (`checkGainSign`). The excitation fits of the recordings are pinned here, and the `TypeScript`'s relay and closed-loop
//! fits of `fitRecordedModel` are ported with the excitation fixtures of `excitation_tests.rs`.

#![allow(clippy::cast_precision_loss)] // Sample indices are small counts, far below 2^53.

use serde_json::json;

use super::{check_gain_sign, fit_recorded_model, RecordedFitError};
use crate::identification::automatic::AutomaticIdentificationResult;
use crate::identification::connected_record::RuntimeVerificationResult;
use crate::identification::data::IdentificationSample;
use crate::identification::excitation::tests::{
    noisy_setpoint_record, relay_record, setpoint_step_record, NoisySetpointOptions, RelayOptions,
    SetpointStepOptions,
};
use crate::identification::independent_validation::RequirementStatus;
use crate::identification::record_fixtures::{build, setpoint_record, Row, Spec, Verdict};
use crate::identification::texts::MISSING_NONZERO_STEP;
use crate::loop_definition::{Action, LoopDefinition};
use crate::model::GeneralModel;

const CHANNELS: &[(&str, &str)] = &[("ValveMV", "%"), ("LevelPV", "degC")];

/// The fixture loop with its controller action and the units of its measurement and its plant input.
fn loop_with(action: Action) -> LoopDefinition {
    let mut loop_definition = crate::recording::fixtures::loop_definition();
    loop_definition.controller.action = action;
    loop_definition.signals.process_variable.unit = "degC".to_owned();
    loop_definition.signals.manipulated_variable.unit = "%".to_owned();
    loop_definition
}

/// The fixture loop with the identification policy the `TypeScript` fits declare: a maximum normalised error of 0.1 (N3.2).
/// The other policy fields are the ones a stored policy needs to decode; they do not change the excitation fit.
fn policy_loop() -> LoopDefinition {
    let mut loop_definition = loop_with(Action::Direct);
    loop_definition.identification_policy = Some(json!({
        "maximumNormalizedError": 0.1,
        "openLoopPermitted": true,
        "sustainedHoldPermitted": true,
        "information": "basic"
    }));
    loop_definition
}

/// Forty samples with the input and the output constant: no excitation, so no identification can succeed.
fn constant_rows() -> Vec<Row> {
    (0..40_u64)
        .map(|index| (index as f64 * 0.1, index, vec![Some(50.0), Some(20.0)]))
        .collect()
}

/// Forty samples in which the plant input steps from 50 to 60 at the twentieth sample, and the output follows it as a ramp.
fn stepping_rows() -> Vec<Row> {
    (0..40_u64)
        .map(|index| {
            let input = if index < 20 { 50.0 } else { 60.0 };
            let output = 20.0 + 0.5 * (index as f64 - 19.0).clamp(0.0, 10.0);
            (index as f64 * 0.1, index, vec![Some(input), Some(output)])
        })
        .collect()
}

/// The connected record of generated scans on the fixture channels: the plant input (`ValveMV`) and the measured output
/// (`LevelPV`) at the time of each scan, with the baseline window of the request when there is one.
fn record_of(
    id: &str,
    kind: &str,
    scans: &[IdentificationSample],
    baseline: Option<(f64, f64)>,
) -> RuntimeVerificationResult<Verdict> {
    let rows: Vec<Row> = scans
        .iter()
        .enumerate()
        .map(|(index, sample)| {
            let sequence = u64::try_from(index).expect("a fixture record is short");
            (
                sample.time_seconds,
                sequence,
                vec![Some(sample.input), Some(sample.output)],
            )
        })
        .collect();
    build(&Spec {
        id,
        project_revision: "sha256:project",
        model_revision: "connected-plant:project",
        kind,
        channels: CHANNELS,
        rows: &rows,
        baseline,
        verdict: RequirementStatus::Pass,
    })
}

/// Asserts that `actual` lies within `tolerance` of `expected` (the `near` helper of the `TypeScript` tests).
fn near(actual: f64, expected: f64, tolerance: f64, label: &str) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{label}: expected {expected} ± {tolerance}, got {actual}"
    );
}

fn refusal_text<T: std::fmt::Debug>(result: Result<T, RecordedFitError>) -> String {
    match result {
        Err(RecordedFitError::Refused(refusal)) => refusal.0,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// The model of a fit that gives one. Any other result fails the test, and the panic names what the fit gave.
fn fitted_model(
    result: Result<AutomaticIdentificationResult<Verdict>, RecordedFitError>,
) -> GeneralModel {
    match result {
        Ok(AutomaticIdentificationResult::Model { model, .. }) => *model,
        other => panic!("expected a fitted model, got {other:?}"),
    }
}

#[test]
fn the_gain_must_agree_with_the_controller_action() {
    let direct = loop_with(Action::Direct);
    let message = check_gain_sign(&direct, -0.42, false)
        .expect_err("a falling output on a direct loop is refused");
    assert!(
        message.0.starts_with("The identified gain is −0.42"),
        "{}",
        message.0
    );
    assert!(
        message
            .0
            .contains("the measurement falls when the output rises"),
        "{}",
        message.0
    );
    assert!(check_gain_sign(&direct, 0.42, false).is_ok());
    let reverse = loop_with(Action::Reverse);
    assert!(check_gain_sign(&reverse, -0.42, false).is_ok());
    let refused = check_gain_sign(&reverse, 0.42, false)
        .expect_err("a rising output on a reverse loop is refused");
    assert!(
        refused
            .0
            .contains("the measurement rises when the output rises"),
        "{}",
        refused.0
    );
}

#[test]
fn a_zero_gain_is_refused_with_its_gain_and_unit_whatever_the_action() {
    // Specification N5.7 and N6.3: a zero gain is refused, with the gain and its unit and the next action. The unit of
    // the test loop is degC per %, and a zero gain has no sign to contradict the action.
    for action in [Action::Direct, Action::Reverse] {
        let Err(refusal) = check_gain_sign(&loop_with(action), 0.0, false) else {
            panic!("a zero gain is refused");
        };
        assert!(
            refusal.0.starts_with("The identified gain is 0 degC/%: "),
            "{refusal:?}"
        );
    }
    assert!(check_gain_sign(&loop_with(Action::Direct), 2.0, false).is_ok());
}

#[test]
fn a_fit_needs_one_recording_or_two() {
    let loop_definition = loop_with(Action::Direct);
    let none: Vec<RuntimeVerificationResult<Verdict>> = Vec::new();
    assert_eq!(
        refusal_text(fit_recorded_model(&loop_definition, &none)),
        "One estimation recording or two independent recordings are required."
    );
    let record = setpoint_record("one", CHANNELS, &constant_rows(), RequirementStatus::Pass);
    let three = vec![record.clone(), record.clone(), record];
    assert_eq!(
        refusal_text(fit_recorded_model(&loop_definition, &three)),
        "One estimation recording or two independent recordings are required."
    );
}

#[test]
fn several_channels_are_not_fitted_here() {
    let mut loop_definition = loop_with(Action::Direct);
    loop_definition.channels = Some(json!({
        "inputs": [{ "signalId": "manipulatedVariable", "modelChannelId": "mv" }, { "signalId": "manipulatedVariable" }],
        "outputs": [{ "signalId": "processVariable", "modelChannelId": "pv" }]
    }));
    let record = setpoint_record("one", CHANNELS, &constant_rows(), RequirementStatus::Pass);
    match fit_recorded_model(&loop_definition, &[record]) {
        Err(RecordedFitError::Parked { path }) => assert!(path.contains("multichannel")),
        other => panic!("expected the parked multichannel fit, got {other:?}"),
    }
}

#[test]
fn an_observation_is_fitted_as_an_excitation_and_refused_for_its_missing_step() {
    // C12 and B3.5: a project-scripted recording follows B3.5, so an observation gets the excitation fit (N2.2), not the
    // parked state-model fallback. A record whose plant input never changes has no nonzero step: N2.6 refuses it, naming
    // the missing step, and no other fit replaces that refusal.
    let loop_definition = loop_with(Action::Direct);
    let record = build(&Spec {
        id: "observation",
        project_revision: "sha256:project",
        model_revision: "connected-plant:tank",
        kind: "observation",
        channels: CHANNELS,
        rows: &constant_rows(),
        baseline: None,
        verdict: RequirementStatus::Pass,
    });
    assert_eq!(
        refusal_text(fit_recorded_model(&loop_definition, &[record])),
        MISSING_NONZERO_STEP
    );
}

#[test]
fn a_project_scenario_without_an_input_change_is_refused_for_its_missing_step() {
    // C12 and B3.5: the scenario's recording is fitted from its recorded plant input as an excitation, so one that never
    // moves the plant input is refused for its missing step, as an observation is.
    let loop_definition = loop_with(Action::Direct);
    let record = build(&Spec {
        id: "scenario",
        project_revision: "sha256:project",
        model_revision: "connected-plant:tank",
        kind: "project_sequence",
        channels: CHANNELS,
        rows: &constant_rows(),
        baseline: None,
        verdict: RequirementStatus::Pass,
    });
    assert_eq!(
        refusal_text(fit_recorded_model(&loop_definition, &[record])),
        MISSING_NONZERO_STEP
    );
}

#[test]
fn a_project_scenario_that_moves_the_plant_input_is_fitted_as_an_excitation() {
    // C12 and B3.5: a scenario that steps the plant input is fitted from the recorded input, so it is not refused for a
    // missing step and it is not parked. The outcome is pinned. The excitation fit (N2.2) fits every form to the recorded
    // input, and the first order is the simplest acceptable one (N3.3): its normalized error is 0.0554 (N3.2), its final
    // set 0.0454 and its reached fraction 0.9235 (A10.3). The first order with delay collapses to a zero delay, so the
    // resolved-delay rule does not apply. The numbers come from v1-final-P2-A1/excitation_outcome.py, which uses C2's
    // replica of the grid fit (fit_replica.py): K 0.5414009690, tau 0.5343534462 s, delay 0, operating point u0 50, y0 20.
    let loop_definition = loop_with(Action::Direct);
    let record = build(&Spec {
        id: "scenario",
        project_revision: "sha256:project",
        model_revision: "connected-plant:tank",
        kind: "project_sequence",
        channels: CHANNELS,
        rows: &stepping_rows(),
        baseline: None,
        verdict: RequirementStatus::Pass,
    });
    let model = fitted_model(fit_recorded_model(&loop_definition, &[record]));
    let GeneralModel::TransferFunction { identity, transfer } = model else {
        panic!("the first order is a transfer function");
    };
    near(
        transfer.numerator[0],
        0.541_400_968_977_369,
        1e-6,
        "process gain K",
    );
    near(
        transfer.denominator[0],
        0.534_353_446_161_993_7,
        1e-6,
        "time constant tau",
    );
    near(
        transfer.denominator[1],
        1.0,
        1e-12,
        "the constant of the lag",
    );
    near(identity.delay_seconds, 0.0, 1e-12, "no dead time");
    let point = identity
        .operating_point
        .expect("the excitation fit gives its operating point");
    near(
        point.inputs[0],
        50.0,
        1e-9,
        "u0, the mean input before the step",
    );
    near(
        point.outputs[0],
        20.0,
        1e-9,
        "y0, the mean output before the step",
    );
}

#[test]
fn identifies_a_relay_recording_as_a_delayed_first_order_transfer_function() {
    // control-studio-excitation-identification.test.ts: "identifies a relay recording as a delayed first-order transfer
    // function" (81). Inputs: the loop's policy with maximumNormalizedError 0.1; relayRecord {gain 2, tau 5, delay 1, u0 0,
    // y0 1, d 1, band 0.1, noise 0.02}, 601 scans on 0.1 s, one run of the relay kind. Expected: a transfer function with
    // delay 1 ± 0.15, K 2 ± 0.1, tau 5 ± 0.5, and an operating point of input 0 and output 1 ± 0.02. The grid fit
    // (v1-final-P2-A1/ports.py, C2's replica) gives the first order with delay: K 1.96562, tau 4.92045, L 1.00000, normalized
    // error 0.0117 within the limit (N3.2), chosen as the simplest acceptable form (N3.3).
    let relay = relay_record(&RelayOptions {
        gain: 2.0,
        tau: 5.0,
        delay: 1.0,
        u0: 0.0,
        y0: 1.0,
        d: 1.0,
        band: 0.1,
        noise: 0.02,
    });
    let record = record_of("relay", "relay", &relay, None);
    let model = fitted_model(fit_recorded_model(&policy_loop(), &[record]));
    let GeneralModel::TransferFunction { identity, transfer } = model else {
        panic!("a relay test must first try the simple models, which give a transfer function");
    };
    near(identity.delay_seconds, 1.0, 0.15, "dead time L");
    near(
        transfer.numerator[0] / transfer.denominator[1],
        2.0,
        0.1,
        "process gain K",
    );
    near(
        transfer.denominator[0] / transfer.denominator[1],
        5.0,
        0.5,
        "time constant tau",
    );
    let point = identity
        .operating_point
        .expect("the excitation fit gives its operating point");
    assert_eq!(point.inputs.len(), 1, "one operating input");
    near(
        point.inputs[0],
        0.0,
        1e-12,
        "the operating input is the baseline 0",
    );
    assert_eq!(point.outputs.len(), 1, "one operating output");
    near(point.outputs[0], 1.0, 0.02, "baseline output");
}

#[test]
fn fits_a_noisy_closed_loop_setpoint_step_from_the_steady_baseline_window_its_record_names() {
    // control-studio-excitation-identification.test.ts: "fits a noisy closed-loop setpoint step from the steady baseline
    // window its record names" (115). Inputs: maximumNormalizedError 0.1; noisySetpointStepRecord {gain 1.5, tau 4, delay
    // 0.5, kp 1, ki 0.3, noise 0.02, stepAt 100}, 401 scans on 0.1 s; one run of the setpoint-step kind with the baseline
    // window 5 s to 9.95 s. Expected: a transfer function with K 1.5 ± 0.1. The grid fit (v1-final-P2-A1/ports.py) gives the
    // first order with delay: K 1.50044, tau 4.00220, L 0.50000, which the resolved-delay rule chooses (N3.3), normalized
    // error 0.0023.
    let noisy = noisy_setpoint_record(&NoisySetpointOptions {
        gain: 1.5,
        tau: 4.0,
        delay: 0.5,
        kp: 1.0,
        ki: 0.3,
        noise: 0.02,
        step_at: 100,
    });
    let record = record_of(
        "noisy-setpoint-step",
        "setpoint_step",
        &noisy,
        Some((5.0, 9.95)),
    );
    let model = fitted_model(fit_recorded_model(&policy_loop(), &[record]));
    let GeneralModel::TransferFunction { transfer, .. } = model else {
        panic!("the closed-loop step gives a transfer function");
    };
    let last = transfer.denominator.len() - 1;
    near(
        transfer.numerator[0] / transfer.denominator[last],
        1.5,
        0.1,
        "process gain K",
    );
}

#[test]
fn fits_the_simple_delayed_first_order_model_to_the_closed_loop_step_that_automatic_runs() {
    // control-studio-excitation-identification.test.ts: "fits the simple delayed first-order model to the closed-loop step
    // that Automatic runs" (132). Inputs: maximumNormalizedError 0.1; setpointStepRecord {gain 1.5, tau 4, delay 0.5, kp 1,
    // ki 0.3}, 401 scans on 0.1 s with no noise; one run of the setpoint-step kind with no baseline window. Expected: a
    // transfer function with K 1.5 ± 0.1 and tau plus delay 4.5 ± 0.3 (the lag may take the delay). The grid fit
    // (v1-final-P2-A1/ports.py) gives the first order with delay: K 1.500001, tau 4.000034, L 0.500000, normalized error
    // 1.4e-6.
    let closed_loop = setpoint_step_record(&SetpointStepOptions {
        gain: 1.5,
        tau: 4.0,
        delay: 0.5,
        kp: 1.0,
        ki: 0.3,
    });
    let record = record_of("setpoint-step", "setpoint_step", &closed_loop, None);
    let model = fitted_model(fit_recorded_model(&policy_loop(), &[record]));
    let GeneralModel::TransferFunction { identity, transfer } = model else {
        panic!("the closed-loop step gives a transfer function");
    };
    near(
        transfer.numerator[0] / transfer.denominator[1],
        1.5,
        0.1,
        "process gain K",
    );
    near(
        transfer.denominator[0] / transfer.denominator[1] + identity.delay_seconds,
        4.5,
        0.3,
        "lag plus delay tau + L",
    );
}

#[test]
fn two_recordings_without_excitation_name_the_failed_two_recording_fit() {
    let loop_definition = loop_with(Action::Direct);
    let first = setpoint_record("first", CHANNELS, &constant_rows(), RequirementStatus::Pass);
    let second = setpoint_record(
        "second",
        CHANNELS,
        &constant_rows(),
        RequirementStatus::Pass,
    );
    match fit_recorded_model(&loop_definition, &[first, second]) {
        Err(RecordedFitError::StateModelRequired { attempts }) => {
            assert_eq!(attempts.len(), 1, "one failed attempt");
            assert!(
                attempts[0].starts_with("Two-recording fit: "),
                "{}",
                attempts[0]
            );
            assert!(
                attempts[0].ends_with('.'),
                "a sentence ends with one full stop"
            );
        }
        other => panic!("expected the state fallback after the failed fit, got {other:?}"),
    }
}
