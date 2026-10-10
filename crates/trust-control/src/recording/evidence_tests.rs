//! The evidence of a stopped recording (`runtimeRecording.ts`): the claims of the answer, its samples and its verdict.

use super::*;
use crate::recording::fixtures::{binding, run, sample};
use crate::recording::ScanRange;
use crate::signals::{Quality, SignalBinding};

fn bindings() -> Vec<SignalBinding> {
    vec![
        binding("processVariable", "%IL0", "LREAL"),
        binding("valid", "%MX16.0", "BOOL"),
    ]
}

/// The two consecutive scans of a complete recording: the fixture's run.
fn two_scans() -> RecordedRun {
    run(
        bindings(),
        vec![
            sample(
                1,
                Some(0),
                &[Some(2.5), Some(1.0)],
                &[Quality::Good, Quality::Good],
            ),
            sample(
                2,
                Some(10_000_000),
                &[Some(2.5), Some(1.0)],
                &[Quality::Good, Quality::Good],
            ),
        ],
    )
}

#[test]
fn a_complete_acquisition_of_two_steady_samples_is_complete_evidence() {
    let evidence = recording_evidence(&two_scans(), None);
    assert_eq!(evidence.completeness, Completeness::Complete);
    assert_eq!(
        evidence.reason,
        "Complete recording — runtime captured 2 scans. Stop: stopped."
    );
}

#[test]
fn a_single_sample_is_never_complete_evidence() {
    let mut single = two_scans();
    single.samples.truncate(1);
    single.acquisition.reason = "Captured every scan.".into();
    let evidence = recording_evidence(&single, None);
    assert_eq!(evidence.completeness, Completeness::Incomplete);
    assert_eq!(
        evidence.reason,
        "Incomplete recording — Captured every scan."
    );
}

#[test]
fn a_missed_execution_makes_a_complete_claim_incomplete() {
    let mut gap = two_scans();
    gap.samples.push(sample(
        4,
        Some(30_000_000),
        &[Some(2.5), Some(1.0)],
        &[Quality::Good, Quality::Good],
    ));
    let evidence = recording_evidence(&gap, None);
    assert_eq!(evidence.completeness, Completeness::Incomplete);
    assert_eq!(
        evidence.reason,
        "Incomplete recording — the sample sequence changes its record period, so an execution is missing."
    );
}

#[test]
fn an_incomplete_acquisition_keeps_the_runtimes_reason() {
    let mut incomplete = two_scans();
    incomplete.acquisition.completeness = Completeness::Incomplete;
    incomplete.acquisition.reason = "execution 2 is missing".into();
    let evidence = recording_evidence(&incomplete, None);
    assert_eq!(evidence.completeness, Completeness::Incomplete);
    assert_eq!(
        evidence.reason,
        "Incomplete recording — execution 2 is missing"
    );
}

#[test]
fn an_unconfirmed_start_is_incomplete_whatever_the_runtime_claims() {
    let evidence = recording_evidence(
        &two_scans(),
        Some("Start acknowledgement was not confirmed before Extension Host reload."),
    );
    assert_eq!(
        evidence.completeness,
        Completeness::Incomplete,
        "a start that was not acknowledged cannot upgrade to complete evidence (E11.4)"
    );
    assert!(
        evidence
            .reason
            .starts_with("Start acknowledgement unconfirmed (Start acknowledgement was not confirmed before Extension Host reload.); stopped data retained."),
        "{}",
        evidence.reason
    );
    assert!(
        evidence
            .reason
            .contains("Complete recording — runtime captured 2 scans."),
        "the stopped data keeps its own reason: {}",
        evidence.reason
    );
}

#[test]
fn a_complete_claim_with_samples_that_do_not_step_forward_contradicts_itself() {
    let mut unordered = two_scans();
    unordered.samples[1].runtime_time_ns = Some(0);
    assert!(
        check_run_samples(&unordered).is_err(),
        "a complete acquisition steps forward (E4.5)"
    );
    unordered.acquisition.completeness = Completeness::Incomplete;
    unordered.acquisition.reason = "a non-increasing time".into();
    assert!(
        check_run_samples(&unordered).is_ok(),
        "an incomplete acquisition keeps its samples as evidence (E4.5)"
    );
}

#[test]
fn a_bool_value_other_than_zero_or_one_is_refused_whatever_the_acquisition_claims() {
    let mut bad = two_scans();
    bad.samples[0].values[1] = Some(2.0);
    assert!(check_run_samples(&bad).is_err());
    bad.acquisition.completeness = Completeness::Incomplete;
    assert!(
        check_run_samples(&bad).is_err(),
        "a BOOL value is 0 or 1 in every acquisition (E0.3)"
    );
}

#[test]
fn the_claims_of_a_stop_answer_agree_with_its_samples() {
    assert!(check_acquisition_claims(&two_scans()).is_ok());

    let mut count = two_scans();
    count.acquisition.observed_sample_count = 3;
    let failure = check_acquisition_claims(&count).expect_err("the count is wrong");
    assert!(failure.message.contains("acquisition data"), "{failure:?}");

    let mut method = two_scans();
    method.acquisition.method = "ui_observed_snapshots".into();
    assert!(check_acquisition_claims(&method).is_err());

    let mut missing = two_scans();
    missing.acquisition.missing_scan_ranges = vec![ScanRange { first: 1, last: 1 }];
    let failure = check_acquisition_claims(&missing).expect_err("a complete claim has a gap");
    assert!(failure.message.contains("acquisition"), "{failure:?}");

    let mut timing = two_scans();
    timing.acquisition.actual_cycle_timing_available = false;
    assert!(check_acquisition_claims(&timing).is_err());

    let mut reversed = two_scans();
    reversed.acquisition.completeness = Completeness::Incomplete;
    reversed.acquisition.missing_scan_ranges = vec![ScanRange { first: 3, last: 2 }];
    let failure = check_acquisition_claims(&reversed).expect_err("a range runs backwards");
    assert!(
        failure.message.contains("claims are inconsistent"),
        "{failure:?}"
    );
}
