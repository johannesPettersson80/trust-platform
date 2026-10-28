//! Tests of the recorded-run evaluation (`run_validation.rs`). The assertions of `control-studio-run-validation.test.ts` are
//! kept with their inputs and expected values, each test naming the `TypeScript` test it ports. The rules that the `TypeScript`
//! test does not reach (N8.3 window ends, the settling reading, the mean error, Apply) have their own tests.

use std::collections::BTreeMap;

use super::*;
use crate::identification::connected_record::{
    RunEvent, RunEvidence, ValidationChannel, ValidationSample, ValidationValue,
};
use crate::limits::ISOLATED_SIMULATION_SAMPLE_LIMIT;

const TOLERANCE: f64 = 1e-12;
/// The samples of the long run: one every 10 ms over 120 s (the isolated run keeps up to 100,000 samples, A7).
const LONG_COUNT: usize = 12_001;

/// Asserts that `actual` lies within the test tolerance of `expected`.
fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= TOLERANCE,
        "{actual} is not {expected}"
    );
}

/// The index of a test sample, as a number.
fn at(index: usize) -> f64 {
    f64::from(u32::try_from(index).expect("a test index fits in u32"))
}

/// The time of sample `index` of the long run: one sample every 10 ms.
fn time_of(index: usize) -> f64 {
    at(index) / 100.0
}

/// The sequence value of sample `index`: the scans of the test runs count from 10.
fn sequence_of(index: usize) -> u64 {
    10 + u64::try_from(index).expect("a test index fits in u64")
}

fn channel(id: &str, unit: &str) -> ValidationChannel {
    ValidationChannel {
        id: id.to_owned(),
        unit: unit.to_owned(),
    }
}

/// A good sample of the response channel at `time_seconds`.
fn response(time_seconds: f64, sequence: u64, value: f64) -> ValidationSample {
    ValidationSample {
        time_seconds,
        sequence,
        values: BTreeMap::from([(
            "response".to_owned(),
            ValidationValue {
                value: Some(value),
                quality: ChannelQuality::Good,
            },
        )]),
    }
}

/// The conditions of the test runs, with the given duration and sample times.
fn conditions(duration_seconds: f64, sample_times_seconds: Vec<f64>) -> RunConditions {
    RunConditions {
        initial_state: BTreeMap::from([("storage".to_owned(), 0.0)]),
        events: vec![RunEvent {
            at_seconds: 0.0,
            channel: "command".to_owned(),
            value: 1.0,
        }],
        duration_seconds,
        sample_times_seconds,
        seed: 31,
        model_revision: "plant-1".to_owned(),
        program_revision: "st-1".to_owned(),
        parameter_schema_revision: "params-1".to_owned(),
    }
}

/// The `TypeScript` test's run: five samples one second apart over 4 s, the response in metres, with the given values.
fn run_of(values: &[f64]) -> ValidationRun {
    ValidationRun {
        id: "run-a".to_owned(),
        evidence: RunEvidence::StExecuted,
        parameters: BTreeMap::from([("gain".to_owned(), 2.0)]),
        conditions: conditions(4.0, vec![0.0, 1.0, 2.0, 3.0, 4.0]),
        channels: vec![channel("response", "m")],
        samples: values
            .iter()
            .enumerate()
            .map(|(index, &value)| response(at(index), sequence_of(index), value))
            .collect(),
    }
}

/// The run of the record-period test: nine executions averaged three per sample, at 1, 4 and 7 s of an 8 s run (A4.2, E4.4).
fn averaged_run() -> ValidationRun {
    ValidationRun {
        id: "averaged".to_owned(),
        evidence: RunEvidence::StExecuted,
        parameters: BTreeMap::new(),
        conditions: RunConditions {
            initial_state: BTreeMap::new(),
            events: Vec::new(),
            ..conditions(8.0, vec![1.0, 4.0, 7.0])
        },
        channels: vec![channel("response", "m")],
        samples: vec![
            response(1.0, 3, 0.5),
            response(4.0, 6, 1.0),
            response(7.0, 9, 1.0),
        ],
    }
}

/// The run of the channel test: the response and `count − 1` state channels, each holding 0 at every sample.
fn wide_run(count: usize) -> ValidationRun {
    let mut source = run_of(&[0.0, 1.0, 1.0, 1.0, 1.0]);
    for index in 1..count {
        let id = format!("state-{}", index - 1);
        source.channels.push(channel(&id, "1"));
        for sample in &mut source.samples {
            sample.values.insert(
                id.clone(),
                ValidationValue {
                    value: Some(0.0),
                    quality: ChannelQuality::Good,
                },
            );
        }
    }
    source
}

fn window(start: f64, end: f64) -> Window {
    Window { start, end }
}

/// The `TypeScript` test's `error` requirement, against the constant 1 over the window 1 s to 4 s.
fn error_with(statistic: ErrorStatistic, maximum: f64) -> RunRequirement {
    RunRequirement {
        id: "error".to_owned(),
        channel: "response".to_owned(),
        window: window(1.0, 4.0),
        check: RunCheck::Error {
            reference: ErrorReference::Constant(1.0),
            statistic,
            maximum,
        },
    }
}

/// The `TypeScript` test's `error` requirement: the largest error against 1 is at most 0.1.
fn error_requirement() -> RunRequirement {
    error_with(ErrorStatistic::Maximum, 0.1)
}

/// The evaluation of a run that the test expects to be accepted, with the record period not declared.
fn evaluate(run: &ValidationRun, requirements: &[RunRequirement]) -> RecordedRunEvaluation {
    evaluate_recorded_run(run, requirements, false).expect("the run is accepted")
}

/// The refusal that the test expects.
fn refusal<T: std::fmt::Debug>(result: Result<T, Failure>) -> Failure {
    result.expect_err("the run is refused")
}

#[test]
fn evaluates_a_run_recorded_at_a_record_period_at_the_mean_times_of_its_executions() {
    // TypeScript test: "evaluates a run recorded at a record period, whose samples sit at the mean times of their executions".
    let whole_run = RunRequirement {
        id: "whole-run".to_owned(),
        channel: "response".to_owned(),
        window: window(0.0, 8.0),
        check: RunCheck::Error {
            reference: ErrorReference::Constant(1.0),
            statistic: ErrorStatistic::Maximum,
            maximum: 0.6,
        },
    };
    let whole = evaluate_recorded_run(&averaged_run(), &[whole_run], true)
        .expect("an averaged run is accepted");
    assert_eq!(whole.status, RequirementStatus::Pass);
    assert_close(whole.requirements[0].value.expect("a value"), 0.5);
    // A run that does not declare its record period must start at 0.
    assert_eq!(
        refusal(evaluate_recorded_run(&averaged_run(), &[], false)).message,
        "Invalid immutable experiment conditions."
    );
}

#[test]
fn accepts_every_channel_an_isolated_run_records_up_to_64() {
    // TypeScript test: "accepts every channel an isolated run records, up to 64".
    assert_eq!(
        evaluate(&wide_run(64), &[error_requirement()]).status,
        RequirementStatus::Pass
    );
    assert_eq!(
        refusal(evaluate_recorded_run(
            &wide_run(65),
            &[error_requirement()],
            false
        ))
        .message,
        "Invalid run channels or sample structure."
    );
}

#[test]
fn evaluates_engineer_declared_error_and_keeps_its_own_copy_of_the_run() {
    // TypeScript test: "evaluates engineer-declared error and immutable run evidence". The TypeScript freezes the run; here
    // the evaluation owns its copy, so a later change to the source does not reach it.
    let mut source = run_of(&[0.0, 1.0, 1.0, 1.0, 1.0]);
    let result = evaluate(&source, &[error_requirement()]);
    assert_eq!(result.status, RequirementStatus::Pass);
    assert_close(result.requirements[0].value.expect("a value"), 0.0);
    source
        .conditions
        .initial_state
        .insert("storage".to_owned(), 99.0);
    assert_close(result.run.conditions.initial_state["storage"], 0.0);
}

#[test]
fn an_unobservable_settling_deadline_is_refused() {
    // TypeScript test: "freezes experiment conditions and rejects an unobservable settling deadline". The deadline of 3 s
    // plus the hold of 2 s is longer than the 4 s window, so the late settling cannot be observed (N8.3).
    let late = RunRequirement {
        id: "late".to_owned(),
        channel: "response".to_owned(),
        window: window(0.0, 4.0),
        check: RunCheck::Settling {
            target: 1.0,
            band: 0.1,
            hold_seconds: 2.0,
            deadline_seconds: 3.0,
        },
    };
    let failure = refusal(evaluate_recorded_run(
        &run_of(&[0.0, 1.0, 1.0, 1.0, 1.0]),
        &[late],
        false,
    ));
    assert_eq!(
        failure.message,
        "Invalid requirement calculation or limits."
    );
    assert_eq!(failure.next_action, NextAction::OpenRequirements);
}

#[test]
fn computes_rms_error_from_the_declared_recorded_window() {
    // TypeScript test: "computes RMS error in Studio from the declared recorded window". The window 1 s to 4 s holds the
    // values 2, 4, 2, 4 against 1, so the errors are 1, 3, 1, 3 and the RMS error is √5.
    let values = [0.0, 2.0, 4.0, 2.0, 4.0];
    let rms = evaluate(&run_of(&values), &[error_with(ErrorStatistic::Rms, 2.3)]);
    assert_eq!(rms.status, RequirementStatus::Pass);
    assert_close(rms.requirements[0].value.expect("a value"), 5.0_f64.sqrt());
    assert_eq!(
        evaluate(&run_of(&values), &[error_with(ErrorStatistic::Rms, 2.1)]).status,
        RequirementStatus::Fail
    );
}

#[test]
fn evaluates_directional_overshoot_settling_limits_rates_saturation_and_switching() {
    // TypeScript test: "evaluates directional overshoot, settling, limits, rates, saturation and switching".
    let requirement = |id: &str, check: RunCheck| RunRequirement {
        id: id.to_owned(),
        channel: "response".to_owned(),
        window: window(0.0, 4.0),
        check,
    };
    let requirements = [
        requirement(
            "overshoot",
            RunCheck::Overshoot {
                initial: 0.0,
                target: 1.0,
                maximum: 0.1,
            },
        ),
        requirement(
            "settle",
            RunCheck::Settling {
                target: 1.0,
                band: 0.05,
                hold_seconds: 2.0,
                deadline_seconds: 2.0,
            },
        ),
        requirement(
            "bounds",
            RunCheck::Bounds {
                minimum: 0.0,
                maximum: 1.0,
            },
        ),
        requirement(
            "rate",
            RunCheck::Rate {
                maximum: 1.0,
                direction: None,
            },
        ),
        requirement(
            "sat",
            RunCheck::Saturation {
                minimum: -1.0,
                maximum: 1.0,
                maximum_seconds: 3.0,
            },
        ),
        requirement(
            "switch",
            RunCheck::Switching {
                threshold: 0.2,
                maximum: 1,
            },
        ),
    ];
    let result = evaluate(&run_of(&[0.0, 1.0, 1.0, 1.0, 1.0]), &requirements);
    assert_eq!(result.status, RequirementStatus::Pass);
    for (row, expected) in result
        .requirements
        .iter()
        .zip([0.0, 1.0, 0.0, 1.0, 3.0, 1.0])
    {
        assert_close(row.value.expect("a value"), expected);
    }
    assert_eq!(
        evaluate(&run_of(&[0.0, 1.4, 1.0, 1.0, 1.0]), &requirements).status,
        RequirementStatus::Fail
    );
    let down = [requirement(
        "down",
        RunCheck::Overshoot {
            initial: 2.0,
            target: 1.0,
            maximum: 0.1,
        },
    )];
    assert_eq!(
        evaluate(&run_of(&[2.0, 0.8, 1.0, 1.0, 1.0]), &down).status,
        RequirementStatus::Fail
    );
}

#[test]
fn missing_forced_and_gapped_coverage_is_inconclusive() {
    // TypeScript test: "makes missing, forced and gapped coverage inconclusive".
    let mut forced = run_of(&[0.0, 1.0, 1.0, 1.0, 1.0]);
    forced.samples[2]
        .values
        .get_mut("response")
        .expect("the response value")
        .quality = ChannelQuality::Forced;
    let mut gapped = run_of(&[0.0, 1.0, 1.0, 1.0, 1.0]);
    gapped.samples[2].sequence += 1;
    let mut truncated = run_of(&[0.0, 1.0, 1.0, 1.0, 1.0]);
    truncated.samples.truncate(4);
    for source in [forced, gapped, truncated] {
        let result = evaluate(&source, &[error_requirement()]);
        assert_eq!(result.status, RequirementStatus::Inconclusive);
        assert_close(
            result.requirements[0].limit.expect("the declared limit"),
            0.1,
        );
    }
}

#[test]
fn requires_actual_timing_and_complete_references_while_evaluating_independent_violations() {
    // TypeScript test: "requires actual timing and complete references while evaluating independent violations".
    let mut timing = run_of(&[0.0, 1.0, 1.0, 1.0, 1.0]);
    timing.samples[2].time_seconds = 2.01;
    let jittered = evaluate(&timing, &[error_requirement()]);
    assert_eq!(
        jittered.status,
        RequirementStatus::Pass,
        "a 10 ms shift is normal task jitter, not a gap"
    );
    assert_close(jittered.requirements[0].value.expect("a value"), 0.0);

    let mut gapped = run_of(&[0.0, 1.0, 1.0, 1.0, 1.0]);
    gapped.samples[3].time_seconds = 3.9;
    let gap = evaluate(&gapped, &[error_requirement()]);
    assert_eq!(gap.status, RequirementStatus::Inconclusive);
    assert_eq!(
        gap.requirements[0].reason,
        "The recording has a gap of 1.9 s at 2 s."
    );

    let mut not_a_number = run_of(&[0.0, 1.0, 1.0, 1.0, 1.0]);
    not_a_number.samples[2]
        .values
        .get_mut("response")
        .expect("the response value")
        .value = Some(f64::NAN);
    assert_eq!(
        evaluate(&not_a_number, &[error_requirement()]).status,
        RequirementStatus::Inconclusive
    );

    // The reference channel carries no values at the samples, so its requirement is Inconclusive, while the other
    // requirement, which needs only the response, is evaluated and fails.
    let mut source = run_of(&[0.0, 2.0, 2.0, 2.0, 2.0]);
    source.channels.push(channel("target", "m"));
    let reference = RunRequirement {
        check: RunCheck::Error {
            reference: ErrorReference::Channel {
                channel: "target".to_owned(),
            },
            statistic: ErrorStatistic::Maximum,
            maximum: 0.1,
        },
        ..error_requirement()
    };
    let observed = RunRequirement {
        id: "observed-error".to_owned(),
        ..error_requirement()
    };
    let result = evaluate(&source, &[reference, observed]);
    let statuses: Vec<RequirementStatus> =
        result.requirements.iter().map(|row| row.status).collect();
    assert_eq!(
        statuses,
        [RequirementStatus::Inconclusive, RequirementStatus::Fail]
    );
    assert_eq!(result.status, RequirementStatus::Fail);

    assert_eq!(
        refusal(evaluate_recorded_run(
            &run_of(&[0.0, 1.0, 1.0, 1.0, 1.0]),
            &[error_requirement(), error_requirement()],
            false
        ))
        .message,
        "Requirement IDs must be unique."
    );
    assert_eq!(
        evaluate(&run_of(&[0.0, 1.0, 1.0, 1.0, 1.0]), &[]).status,
        RequirementStatus::Inconclusive
    );
    let mean = evaluate(
        &run_of(&[0.0, 0.8, 1.2, 1.0, 1.0]),
        &[error_with(ErrorStatistic::Mean, 0.1)],
    );
    assert_eq!(mean.status, RequirementStatus::Pass);
    assert_close(mean.requirements[0].value.expect("a value"), 0.1);
}

#[test]
fn ranks_only_matched_qualifying_runs_and_rejects_changed_evidence_or_conditions() {
    // TypeScript test: "ranks only matched qualifying runs and rejects changed evidence or conditions".
    let current = run_of(&[0.0, 1.05, 1.05, 1.05, 1.05]);
    let candidate = run_of(&[0.0, 1.0, 1.0, 1.0, 1.0]);
    let requirements = [error_requirement()];
    let comparison =
        compare_recorded_runs(&current, &candidate, &requirements, false, Some("error"))
            .expect("the runs compare");
    assert_eq!(comparison.status, RequirementStatus::Pass);
    assert_eq!(comparison.preferred, Some(Preference::Candidate));

    let model_predicted = ValidationRun {
        evidence: RunEvidence::ModelPredicted,
        ..candidate.clone()
    };
    assert_eq!(
        compare_recorded_runs(
            &current,
            &model_predicted,
            &requirements,
            false,
            Some("error")
        )
        .expect("the runs compare")
        .status,
        RequirementStatus::Inconclusive
    );
    // Parameter values may differ between candidates; only their identities must match (N8.6).
    let other_gain = ValidationRun {
        parameters: BTreeMap::from([("gain".to_owned(), 3.0)]),
        ..candidate.clone()
    };
    assert_eq!(
        compare_recorded_runs(&current, &other_gain, &requirements, false, Some("error"))
            .expect("the runs compare")
            .preferred,
        Some(Preference::Candidate)
    );
    let mut reseeded = candidate.clone();
    reseeded.conditions.seed += 1;
    assert_eq!(
        compare_recorded_runs(&current, &reseeded, &requirements, false, Some("error"))
            .expect("the runs compare")
            .status,
        RequirementStatus::Inconclusive
    );
    let worse = run_of(&[0.0, 2.0, 2.0, 2.0, 2.0]);
    assert_eq!(
        compare_recorded_runs(&candidate, &worse, &requirements, false, Some("error"))
            .expect("the runs compare")
            .preferred,
        Some(Preference::Current)
    );
}

#[test]
fn decodes_and_evaluates_a_run_longer_than_12000_samples() {
    // TypeScript test: "decodes and evaluates a run longer than 12,000 samples". The run's last sample alone deviates, so the
    // maximum error is found only if the decoded run is evaluated to its end.
    let long_run = |last: f64| ValidationRun {
        conditions: conditions(
            time_of(LONG_COUNT - 1),
            (0..LONG_COUNT).map(time_of).collect(),
        ),
        samples: (0..LONG_COUNT)
            .map(|index| {
                let value = if index == LONG_COUNT - 1 { last } else { 1.0 };
                response(time_of(index), sequence_of(index), value)
            })
            .collect(),
        ..run_of(&[0.0, 1.0, 1.0, 1.0, 1.0])
    };
    let requirement = RunRequirement {
        window: window(0.0, time_of(LONG_COUNT - 1)),
        ..error_requirement()
    };
    let clean = evaluate_recorded_run(&long_run(1.0), std::slice::from_ref(&requirement), false)
        .expect("the long run is accepted");
    assert_eq!(clean.run.samples.len(), LONG_COUNT);
    assert_eq!(clean.status, RequirementStatus::Pass);
    let late = evaluate_recorded_run(&long_run(1.5), std::slice::from_ref(&requirement), false)
        .expect("the long run is accepted");
    assert_eq!(late.status, RequirementStatus::Fail);
    assert_close(late.requirements[0].value.expect("a value"), 0.5);
}

#[test]
fn refuses_a_run_one_sample_above_the_isolated_run_limit() {
    // TypeScript test: "refuses a run one sample above the isolated-run limit".
    let over =
        usize::try_from(ISOLATED_SIMULATION_SAMPLE_LIMIT + 1).expect("the limit fits in usize");
    let long_grid = ValidationRun {
        conditions: conditions(time_of(over - 1), (0..over).map(time_of).collect()),
        samples: Vec::new(),
        ..run_of(&[0.0, 1.0, 1.0, 1.0, 1.0])
    };
    assert_eq!(
        refusal(evaluate_recorded_run(
            &long_grid,
            &[error_requirement()],
            false
        ))
        .message,
        "Invalid immutable experiment conditions."
    );
    let long_samples = ValidationRun {
        samples: (0..over)
            .map(|index| response(time_of(index), sequence_of(index), 1.0))
            .collect(),
        ..run_of(&[0.0, 1.0, 1.0, 1.0, 1.0])
    };
    assert_eq!(
        refusal(evaluate_recorded_run(
            &long_samples,
            &[error_requirement()],
            false
        ))
        .message,
        "Invalid run channels or sample structure."
    );
}

#[test]
fn window_ends_lie_on_the_sample_grid_unless_the_run_is_averaged() {
    // N8.3: each window starts and ends on the declared sample grid. A run recorded at a record period keeps the TypeScript
    // reading, because its samples sit at mean times (E4.4), so the same window is accepted there.
    let off_grid = RunRequirement {
        window: window(0.5, 3.0),
        ..error_requirement()
    };
    assert_eq!(
        refusal(evaluate_recorded_run(
            &run_of(&[0.0, 1.0, 1.0, 1.0, 1.0]),
            &[off_grid],
            false
        ))
        .message,
        "Invalid requirement identity, channel or sample window."
    );
    let off_grid_averaged = RunRequirement {
        window: window(2.5, 7.5),
        ..error_requirement()
    };
    let result = evaluate_recorded_run(&averaged_run(), &[off_grid_averaged], true)
        .expect("an averaged run keeps its window ends");
    assert_eq!(result.status, RequirementStatus::Pass);
}

#[test]
fn settling_takes_the_start_of_the_final_run_inside_the_band_as_the_typescript_does() {
    // The response leaves the band at 1 s and returns at 2 s, and stays. The TypeScript takes the final entry (2 s), so the
    // requirement passes. N8.3 reads the first entry (0 s) instead, which the samples after it contradict, so it would fail.
    // This test pins the TypeScript reading; the difference is listed in the report for a decision.
    let settle = RunRequirement {
        id: "settle".to_owned(),
        channel: "response".to_owned(),
        window: window(0.0, 4.0),
        check: RunCheck::Settling {
            target: 1.0,
            band: 0.05,
            hold_seconds: 2.0,
            deadline_seconds: 2.0,
        },
    };
    let result = evaluate(&run_of(&[1.0, 0.0, 1.0, 1.0, 1.0]), &[settle]);
    assert_eq!(result.status, RequirementStatus::Pass);
    assert_close(result.requirements[0].value.expect("a value"), 2.0);
}

#[test]
fn settling_fails_when_the_arrival_is_late_or_the_hold_is_not_met_or_never_arrives() {
    let settling = |hold_seconds: f64, deadline_seconds: f64| RunRequirement {
        id: "settle".to_owned(),
        channel: "response".to_owned(),
        window: window(0.0, 4.0),
        check: RunCheck::Settling {
            target: 1.0,
            band: 0.05,
            hold_seconds,
            deadline_seconds,
        },
    };
    // The response arrives at 3 s, after the 2 s deadline.
    let late = evaluate(&run_of(&[0.0, 0.0, 0.0, 1.0, 1.0]), &[settling(1.0, 2.0)]);
    assert_eq!(late.status, RequirementStatus::Fail);
    assert_eq!(late.requirements[0].status, RequirementStatus::Fail);
    assert_close(late.requirements[0].value.expect("a value"), 3.0);
    assert_eq!(
        late.requirements[0].reason,
        "Observed measurement exceeds the declared limit."
    );
    // The response arrives at 2 s, but a hold of 3 s needs the arrival to be 3 s or earlier in the 4 s window.
    let short_hold = evaluate(&run_of(&[0.0, 0.0, 1.0, 1.0, 1.0]), &[settling(3.0, 1.0)]);
    assert_eq!(short_hold.status, RequirementStatus::Fail);
    assert_eq!(
        short_hold.requirements[0].reason,
        "No observed arrival remained in band for the required hold through the end of the window."
    );
    // The last sample lies outside the band, so no arrival remains.
    let never = evaluate(&run_of(&[1.0, 1.0, 1.0, 1.0, 0.0]), &[settling(1.0, 2.0)]);
    assert_eq!(never.status, RequirementStatus::Fail);
    assert_eq!(
        never.requirements[0].reason,
        "No observed arrival remained in band for the required hold through the end of the window."
    );
}

#[test]
fn mean_error_reports_the_signed_mean_of_the_errors() {
    // N8.3 "Mean error", which the TypeScript does not have. The value is reported with its sign, and the verdict compares its
    // magnitude with the maximum, so a bias fails in either direction. A one-sided test would pass a large negative bias.
    let negative = evaluate(
        &run_of(&[0.0, 0.0, 0.0, 0.0, 0.0]),
        &[error_with(ErrorStatistic::MeanError, 0.1)],
    );
    assert_close(negative.requirements[0].value.expect("a value"), -1.0);
    assert_eq!(negative.status, RequirementStatus::Fail);
    // A bias of +0.05 (1.05 against the reference 1) is within the limit 0.1 and passes.
    let small = evaluate(
        &run_of(&[0.0, 1.05, 1.05, 1.05, 1.05]),
        &[error_with(ErrorStatistic::MeanError, 0.1)],
    );
    assert_close(small.requirements[0].value.expect("a value"), 0.05);
    assert_eq!(small.status, RequirementStatus::Pass);
    let biased = evaluate(
        &run_of(&[0.0, 1.5, 1.5, 1.5, 1.5]),
        &[error_with(ErrorStatistic::MeanError, 0.1)],
    );
    assert_close(biased.requirements[0].value.expect("a value"), 0.5);
    assert_eq!(biased.status, RequirementStatus::Fail);
}

#[test]
fn comparison_without_an_objective_and_with_an_undeclared_one() {
    // TypeScript test: the ranking refusals of "compareRecordedRuns" (N8.6).
    let plain = run_of(&[0.0, 1.0, 1.0, 1.0, 1.0]);
    let without = compare_recorded_runs(&plain, &plain, &[error_requirement()], false, None)
        .expect("the runs compare");
    assert_eq!(without.status, RequirementStatus::Pass);
    assert_eq!(without.preferred, None);
    assert_eq!(without.reason, "No ranking objective was declared.");
    assert_eq!(
        refusal(compare_recorded_runs(
            &plain,
            &plain,
            &[error_requirement()],
            false,
            Some("missing")
        ))
        .message,
        "Ranking objective must name a declared requirement."
    );
    let both_fail = compare_recorded_runs(
        &run_of(&[0.0, 2.0, 2.0, 2.0, 2.0]),
        &run_of(&[0.0, 3.0, 3.0, 3.0, 3.0]),
        &[error_requirement()],
        false,
        Some("error"),
    )
    .expect("the runs compare");
    assert_eq!(both_fail.status, RequirementStatus::Fail);
    assert_eq!(both_fail.preferred, None);
    assert_eq!(
        both_fail.reason,
        "Neither run satisfies all requirements; no ranking is available."
    );
}

#[test]
fn apply_needs_a_passing_model_check_and_a_passing_candidate_evaluation() {
    let passing = evaluate(&run_of(&[0.0, 1.0, 1.0, 1.0, 1.0]), &[error_requirement()]);
    let failing = evaluate(&run_of(&[0.0, 2.0, 2.0, 2.0, 2.0]), &[error_requirement()]);
    let empty = evaluate(&run_of(&[0.0, 1.0, 1.0, 1.0, 1.0]), &[]);
    assert_eq!(passing.status, RequirementStatus::Pass);
    assert_eq!(failing.status, RequirementStatus::Fail);
    assert_eq!(empty.status, RequirementStatus::Inconclusive);
    assert!(apply_eligible(RequirementStatus::Pass, &passing));
    assert!(!apply_eligible(RequirementStatus::Fail, &passing));
    assert!(!apply_eligible(RequirementStatus::Inconclusive, &passing));
    assert!(!apply_eligible(RequirementStatus::Pass, &failing));
    assert!(!apply_eligible(RequirementStatus::Pass, &empty));
}

#[test]
fn the_finding_names_the_reason_of_the_first_requirement_that_does_not_pass() {
    let failing = evaluate(&run_of(&[0.0, 2.0, 2.0, 2.0, 2.0]), &[error_requirement()]);
    assert_eq!(
        failing.finding(),
        Some("Observed measurement exceeds the declared limit.")
    );
    let passing = evaluate(&run_of(&[0.0, 1.0, 1.0, 1.0, 1.0]), &[error_requirement()]);
    assert_eq!(passing.finding(), None);
}
