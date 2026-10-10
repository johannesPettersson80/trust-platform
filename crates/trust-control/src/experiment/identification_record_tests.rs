//! Tests of the Test ledger and of the samples Identify takes from a Test's runtime recording. The assertions follow
//! `control-studio-experiment.test.ts` (the ledger, the roles, the runtime samples and the retained prefix); the
//! others are worked out by hand.
#![allow(clippy::float_cmp)] // The samples carry exact times and values on purpose.

use super::*;
use crate::experiment::plan::{
    build_control_experiment_plan, ControlExperimentPlanResult, ControlExperimentRequestKind,
    ControllerMode, ExperimentSample, PlanInput, PlanSignal,
};
use crate::loop_definition::LoopDefinition;
use crate::recording::evidence::recording_evidence;
use crate::recording::fixtures::{binding, loop_definition, loop_with_disturbance, run, sample};
use crate::recording::{Acquisition, Completeness, EvidenceSource, RecordedRun};
use crate::signals::Quality;

/// The fixture loop with the units and the output limit of the `TypeScript` Test recording: setpoint and measurement in
/// °C, the manipulated variable in %, and output limits of 0 and 35.
fn runtime_loop() -> LoopDefinition {
    let mut loop_definition = loop_definition();
    loop_definition
        .signals
        .setpoint
        .as_mut()
        .expect("the setpoint is set")
        .unit = "°C".to_string();
    loop_definition.signals.process_variable.unit = "°C".to_string();
    loop_definition.limits.output_maximum = Some(35.0);
    loop_definition
}

/// The complete runtime recording of the `TypeScript` tests (`runtimeOwnedRecording()`): three scans 20 ms apart, the
/// second one with a bad measurement, and the manipulated input at its limit from the second scan on.
fn runtime_recording() -> RecordedRun {
    run(
        vec![
            binding("setpoint", "%ML0", "LREAL"),
            binding("processVariable", "%IL0", "LREAL"),
            binding("manipulatedVariable", "%QL0", "LREAL"),
        ],
        vec![
            sample(
                11,
                Some(0),
                &[Some(50.0), Some(48.0), Some(32.0)],
                &[Quality::Good; 3],
            ),
            sample(
                12,
                Some(20_000_000),
                &[Some(55.0), Some(48.5), Some(35.0)],
                &[Quality::Good, Quality::Bad, Quality::Good],
            ),
            sample(
                13,
                Some(40_000_000),
                &[Some(55.0), Some(49.0), Some(35.0)],
                &[Quality::Good; 3],
            ),
        ],
    )
}

fn row(
    time: f64,
    input: f64,
    output: f64,
    quality: SampleQuality,
    input_unit: &str,
    saturated: bool,
) -> ExperimentIdentificationSample {
    ExperimentIdentificationSample {
        time_seconds: time,
        input,
        output,
        quality,
        input_unit: Some(input_unit.to_string()),
        output_unit: Some("°C".to_string()),
        clipped: None,
        saturated: saturated.then_some(true),
    }
}

/// A plain sample of a Test (`{ timeSeconds, input, output, quality: "good" }`).
fn plain(input: f64, output: f64) -> ExperimentIdentificationSample {
    ExperimentIdentificationSample {
        time_seconds: 0.0,
        input,
        output,
        quality: SampleQuality::Good,
        input_unit: None,
        output_unit: None,
        clipped: None,
        saturated: None,
    }
}

/// The 30 samples of the `TypeScript` ledger test (`identificationSamples(amplitude)`).
fn identification_samples(amplitude: f64) -> Vec<ExperimentIdentificationSample> {
    (0_u32..30)
        .map(|index| {
            let time = f64::from(index);
            let step = index >= 5;
            ExperimentIdentificationSample {
                time_seconds: time,
                input: if step { amplitude } else { 0.0 },
                output: if step {
                    amplitude * 0.2 * (1.0 - (-(time - 5.0) / 4.0).exp())
                } else {
                    0.0
                },
                quality: SampleQuality::Good,
                input_unit: Some("%".to_string()),
                output_unit: Some("%".to_string()),
                clipped: None,
                saturated: None,
            }
        })
        .collect()
}

fn terminal(
    experiment_id: &str,
    status: ExperimentTerminalStatus,
    stop_reason: Option<&str>,
    restoration: TerminalRestoration,
) -> ExperimentIdentificationTerminal {
    ExperimentIdentificationTerminal {
        experiment_id: experiment_id.to_string(),
        status,
        stop_reason: stop_reason.map(str::to_string),
        restoration,
        restoration_reason: None,
    }
}

/// A plan the review accepts for `kind` at scan `scan`: Auto for a setpoint step, Manual otherwise.
fn reviewed_plan(kind: ControlExperimentRequestKind, scan: u64) -> ControlExperimentPlan {
    let loop_definition = loop_definition();
    let controller_mode = if kind == ControlExperimentRequestKind::SetpointStep {
        ControllerMode::Auto
    } else {
        ControllerMode::Manual
    };
    let live = |symbol: &str, address: &str, value: f64| PlanSignal {
        symbol: symbol.to_string(),
        address: Some(address.to_string()),
        value,
        unit: "%".to_string(),
        quality: Quality::Good,
    };
    let sample = ExperimentSample {
        scan,
        source: EvidenceSource::SimulatedPlant,
        controller_mode,
        setpoint: Some(live("LevelSP", "%ML0", 50.0)),
        process_variable: live("LevelPV", "%IL0", 48.0),
        manipulated_variable: live("ValveMV", "%QL0", 32.0),
        additional: Vec::new(),
    };
    match build_control_experiment_plan(&PlanInput {
        loop_definition: &loop_definition,
        sample: Some(&sample),
        kind,
        amplitude: 5.0,
        duration_seconds: 30.0,
        physical_topology_reviewed: false,
        seed: None,
        process_center: None,
        process_noise: None,
        hold_seconds: None,
    }) {
        ControlExperimentPlanResult::Accepted { plan } => *plan,
        ControlExperimentPlanResult::Rejected { reason } => panic!("the plan is refused: {reason}"),
    }
}

#[test]
fn a_test_is_recorded_once_with_its_role_and_its_qualification() {
    let setpoint = ControlExperimentRequestKind::SetpointStep;
    let plan = reviewed_plan(setpoint, 41);
    let mut ledger = ExperimentIdentificationLedger::default();
    let failed = ledger
        .record(
            &plan,
            &terminal(
                &plan.id,
                ExperimentTerminalStatus::Failed,
                Some("stale_runtime_data"),
                TerminalRestoration::Restored,
            ),
            &[plain(50.0, 48.0)],
            None,
        )
        .expect("the first record is taken");
    assert_eq!(failed.role, IdentificationRole::Estimation);
    assert_eq!(failed.outcome, IdentificationOutcome::Failed);
    assert_eq!(
        failed.qualification,
        IdentificationQualification::NotQualified
    );
    assert_eq!(ledger.records().len(), 1);
    assert_eq!(
        ledger.retry(&failed.experiment_id, &plan),
        Err(ExperimentLedgerError::NotFresh),
        "a failed Test may be retried only from a newly reviewed plan"
    );

    let retry_plan = reviewed_plan(setpoint, 42);
    let retried = ledger
        .retry(&failed.experiment_id, &retry_plan)
        .expect("a fresh plan retries");
    assert_eq!(retried.id, retry_plan.id);
    let estimation = ledger
        .record(
            &retried,
            &terminal(
                &retried.id,
                ExperimentTerminalStatus::Completed,
                Some("duration_elapsed"),
                TerminalRestoration::Restored,
            ),
            &[plain(50.0, 48.0)],
            None,
        )
        .expect("the retry is recorded");
    assert_eq!(estimation.role, IdentificationRole::Estimation);
    assert_eq!(
        ledger
            .identification_records()
            .iter()
            .map(|record| record.experiment_id.as_str())
            .collect::<Vec<_>>(),
        [retried.id.as_str()]
    );

    let validation_plan = reviewed_plan(setpoint, 43);
    let validation = ledger
        .record(
            &validation_plan,
            &terminal(
                &validation_plan.id,
                ExperimentTerminalStatus::Completed,
                Some("duration_elapsed"),
                TerminalRestoration::Restored,
            ),
            &[plain(50.0, 48.0)],
            None,
        )
        .expect("the validation is recorded");
    assert_eq!(validation.role, IdentificationRole::Validation);
}

#[test]
fn the_ledger_refuses_evidence_of_another_plan_a_second_record_and_a_retry_of_a_completed_test() {
    let plan = reviewed_plan(ControlExperimentRequestKind::SetpointStep, 41);
    let mut ledger = ExperimentIdentificationLedger::default();
    assert_eq!(
        ledger.record(
            &plan,
            &terminal(
                "experiment:other",
                ExperimentTerminalStatus::Completed,
                None,
                TerminalRestoration::Restored
            ),
            &[plain(50.0, 48.0)],
            None,
        ),
        Err(ExperimentLedgerError::EvidenceMismatch)
    );
    ledger
        .record(
            &plan,
            &terminal(
                &plan.id,
                ExperimentTerminalStatus::Completed,
                None,
                TerminalRestoration::Restored,
            ),
            &[plain(50.0, 48.0)],
            None,
        )
        .expect("the first record is taken");
    assert_eq!(
        ledger.record(
            &plan,
            &terminal(
                &plan.id,
                ExperimentTerminalStatus::Completed,
                None,
                TerminalRestoration::Restored
            ),
            &[plain(50.0, 48.0)],
            None,
        ),
        Err(ExperimentLedgerError::DuplicateRecord)
    );
    assert_eq!(
        ledger.retry(
            &plan.id,
            &reviewed_plan(ControlExperimentRequestKind::SetpointStep, 42)
        ),
        Err(ExperimentLedgerError::NotRetryable),
        "only a failed record may be retried"
    );
}

#[test]
fn incomplete_evidence_makes_a_completed_test_fail_to_qualify() {
    let plan = reviewed_plan(ControlExperimentRequestKind::SetpointStep, 41);
    let mut incomplete: Acquisition = runtime_recording().acquisition;
    incomplete.completeness = Completeness::Incomplete;
    incomplete.missing_scan_ranges = vec![crate::recording::ScanRange {
        first: 12,
        last: 12,
    }];
    let mut ledger = ExperimentIdentificationLedger::default();
    let record = ledger
        .record(
            &plan,
            &terminal(
                &plan.id,
                ExperimentTerminalStatus::Completed,
                None,
                TerminalRestoration::Restored,
            ),
            &[plain(50.0, 48.0)],
            Some(&incomplete),
        )
        .expect("the record is taken");
    assert_eq!(record.outcome, IdentificationOutcome::Failed);
    assert_eq!(
        record.qualification,
        IdentificationQualification::NotQualified
    );
    assert_eq!(
        identification_record_for_role(ledger.records(), IdentificationRole::Estimation),
        None
    );
}

#[test]
fn identify_takes_the_completed_test_of_each_role_and_never_a_failed_one() {
    let mut ledger = ExperimentIdentificationLedger::default();
    let plan = reviewed_plan(ControlExperimentRequestKind::MvStep, 41);
    let failed = ledger
        .record(
            &plan,
            &terminal(
                &plan.id,
                ExperimentTerminalStatus::Failed,
                None,
                TerminalRestoration::Restored,
            ),
            &identification_samples(5.0),
            None,
        )
        .expect("the failed test is taken");
    let estimation_plan = reviewed_plan(ControlExperimentRequestKind::MvStep, 42);
    let estimation = ledger
        .record(
            &estimation_plan,
            &terminal(
                &estimation_plan.id,
                ExperimentTerminalStatus::Completed,
                None,
                TerminalRestoration::Restored,
            ),
            &identification_samples(5.0),
            None,
        )
        .expect("the estimation is taken");
    let validation_plan = reviewed_plan(ControlExperimentRequestKind::MvStep, 43);
    let validation = ledger
        .record(
            &validation_plan,
            &terminal(
                &validation_plan.id,
                ExperimentTerminalStatus::Completed,
                None,
                TerminalRestoration::Restored,
            ),
            &identification_samples(7.0),
            None,
        )
        .expect("the validation is taken");
    assert_eq!(
        identification_record_for_role(ledger.records(), IdentificationRole::Estimation)
            .map(|record| record.experiment_id.as_str()),
        Some(estimation.experiment_id.as_str())
    );
    assert_eq!(
        identification_record_for_role(ledger.records(), IdentificationRole::Validation)
            .map(|record| record.experiment_id.as_str()),
        Some(validation.experiment_id.as_str())
    );
    assert_ne!(
        identification_record_for_role(ledger.records(), IdentificationRole::Estimation)
            .map(|record| record.experiment_id.as_str()),
        Some(failed.experiment_id.as_str()),
        "failed Test records must not reach Identify"
    );
}

#[test]
fn identify_admits_only_a_complete_runtime_recording_and_its_samples_with_their_saturation() {
    let loop_definition = runtime_loop();
    let complete = runtime_recording();
    assert_eq!(
        identification_samples_from_runtime_recording(
            ControlExperimentKind::MvStep,
            &complete,
            &loop_definition,
            None
        ),
        Ok(vec![
            row(0.0, 32.0, 48.0, SampleQuality::Good, "%", false),
            row(0.02, 35.0, 48.5, SampleQuality::Bad, "%", true),
            row(0.04, 35.0, 49.0, SampleQuality::Good, "%", true),
        ])
    );
    let setpoint: Vec<(f64, Option<String>)> = identification_samples_from_runtime_recording(
        ControlExperimentKind::SetpointStep,
        &complete,
        &loop_definition,
        None,
    )
    .expect("the setpoint samples")
    .into_iter()
    .map(|sample| (sample.input, sample.input_unit))
    .collect();
    assert_eq!(
        setpoint,
        [
            (50.0, Some("°C".to_string())),
            (55.0, Some("°C".to_string())),
            (55.0, Some("°C".to_string()))
        ]
    );

    let mut incomplete = runtime_recording();
    incomplete.acquisition.method = "ui_observed_snapshots".to_string();
    incomplete.acquisition.completeness = Completeness::Incomplete;
    incomplete.acquisition.actual_cycle_timing_available = false;
    assert_eq!(
        identification_samples_from_runtime_recording(
            ControlExperimentKind::MvStep,
            &incomplete,
            &loop_definition,
            None
        ),
        Err(IdentificationSamplesError::Incomplete(
            recording_evidence(&incomplete, None).reason
        )),
        "an incomplete acquisition is refused with the evidence reason"
    );

    let mut missed = runtime_recording();
    missed.samples = vec![missed.samples[0].clone(), {
        let mut late = missed.samples[2].clone();
        late.scan = 14;
        late
    }];
    let refused = identification_samples_from_runtime_recording(
        ControlExperimentKind::MvStep,
        &missed,
        &loop_definition,
        None,
    );
    let Err(IdentificationSamplesError::Incomplete(reason)) = &refused else {
        panic!("a missed execution is refused, got {refused:?}");
    };
    assert!(
        reason.contains("record period"),
        "the reason names the missed execution: {reason}"
    );

    let mut single = runtime_recording();
    single.samples.truncate(1);
    let verdict = recording_evidence(&single, None);
    assert_eq!(verdict.completeness, Completeness::Incomplete);
    assert_eq!(
        identification_samples_from_runtime_recording(
            ControlExperimentKind::MvStep,
            &single,
            &loop_definition,
            None
        ),
        Err(IdentificationSamplesError::Incomplete(verdict.reason)),
        "a one-sample run is refused: it is not complete"
    );

    let start = "Start acknowledgement was not confirmed before Extension Host reload.";
    let unconfirmed = identification_samples_from_runtime_recording(
        ControlExperimentKind::MvStep,
        &complete,
        &loop_definition,
        Some(start),
    );
    let Err(IdentificationSamplesError::Incomplete(reason)) = &unconfirmed else {
        panic!("a run from an unconfirmed start is refused, got {unconfirmed:?}");
    };
    assert!(
        reason.starts_with("Start acknowledgement unconfirmed ("),
        "{reason}"
    );
    assert!(
        reason.contains(start),
        "the reason keeps the start failure: {reason}"
    );
}

#[test]
fn an_incomplete_aborted_test_keeps_its_received_prefix_outside_identify() {
    let loop_definition = runtime_loop();
    let plan = reviewed_plan(ControlExperimentRequestKind::MvStep, 41);
    let mut recording = runtime_recording();
    recording.acquisition.completeness = Completeness::Incomplete;
    recording.acquisition.actual_cycle_timing_available = false;
    recording.acquisition.missing_scan_ranges = vec![crate::recording::ScanRange {
        first: 12,
        last: 12,
    }];
    recording.acquisition.reason =
        "Runtime acquisition stopped after abort; scan 12 is unavailable.".to_string();
    let retained = retained_identification_samples_from_runtime_recording(
        ControlExperimentKind::MvStep,
        &recording,
        &loop_definition,
    )
    .expect("the received prefix is retained");
    let mut ledger = ExperimentIdentificationLedger::default();
    let failed = ledger
        .record(
            &plan,
            &terminal(
                &plan.id,
                ExperimentTerminalStatus::Aborted,
                Some("user_abort"),
                TerminalRestoration::Restored,
            ),
            &retained,
            Some(&recording.acquisition),
        )
        .expect("the aborted test is recorded");
    assert_eq!(failed.outcome, IdentificationOutcome::Failed);
    assert_eq!(
        failed.qualification,
        IdentificationQualification::NotQualified
    );
    assert_eq!(
        failed
            .acquisition
            .as_ref()
            .map(|acquisition| acquisition.completeness),
        Some(Completeness::Incomplete)
    );
    assert_eq!(
        failed
            .samples
            .iter()
            .map(|sample| sample.input)
            .collect::<Vec<_>>(),
        [32.0, 35.0, 35.0]
    );
    assert_eq!(
        identification_record_for_role(ledger.records(), IdentificationRole::Estimation),
        None
    );
    assert_eq!(
        ledger.retry(&failed.experiment_id, &plan),
        Err(ExperimentLedgerError::NotFresh)
    );
}

#[test]
fn a_load_step_takes_its_single_declared_disturbance_as_the_input() {
    // The measurement is in °C, so the rows of this test carry the same output unit as the others.
    let mut loop_definition = loop_with_disturbance();
    loop_definition.signals.process_variable.unit = "°C".to_string();
    let recording = run(
        vec![
            binding("setpoint", "%ML0", "LREAL"),
            binding("processVariable", "%IL0", "LREAL"),
            binding("manipulatedVariable", "%QL0", "LREAL"),
            binding("feed-flow", "%QL8", "LREAL"),
        ],
        (1_u64..=3)
            .map(|scan| {
                let time = i64::try_from(scan - 1).expect("three scans") * 100_000_000;
                sample(
                    scan,
                    Some(time),
                    &[Some(50.0), Some(48.0), Some(32.0), Some(60.0)],
                    &[Quality::Good; 4],
                )
            })
            .collect(),
    );
    let samples = identification_samples_from_runtime_recording(
        ControlExperimentKind::LoadStep,
        &recording,
        &loop_definition,
        None,
    )
    .expect("the load step has its disturbance");
    assert_eq!(
        samples,
        [
            row(0.0, 60.0, 48.0, SampleQuality::Good, "%", false),
            row(0.1, 60.0, 48.0, SampleQuality::Good, "%", false),
            row(0.2, 60.0, 48.0, SampleQuality::Good, "%", false),
        ]
    );
    assert_eq!(
        identification_samples_from_runtime_recording(
            ControlExperimentKind::LoadStep,
            &recording,
            &runtime_loop(),
            None
        ),
        Err(IdentificationSamplesError::DisturbanceCount),
        "a load step needs a declared disturbance"
    );
}

#[test]
fn a_sample_without_a_runtime_time_cannot_be_placed() {
    let mut recording = runtime_recording();
    recording.samples[1].runtime_time_ns = None;
    assert_eq!(
        retained_identification_samples_from_runtime_recording(
            ControlExperimentKind::MvStep,
            &recording,
            &runtime_loop()
        ),
        Err(IdentificationSamplesError::InvalidSamples)
    );
}
