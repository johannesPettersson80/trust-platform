#![allow(clippy::float_cmp)]

use serde_json::json;

use super::*;
use crate::commissioning::confirmation::confirmation_window_ms;
use crate::commissioning::observation::{exact_f64, AuxiliaryValue, ObservedChannel};
use crate::limits::{record_period_executions, RECORDING_SAMPLE_LIMIT};
use crate::loop_definition::{AuxiliarySignal, Direction, LoopSignal, Range};
use crate::recording::channels::ChannelQuality;
use crate::validation::run_validation::{RunCheck, RunRequirement, Window};

fn scan(elapsed_seconds: f64, sequence: u64, process_value: Option<f64>) -> ResponseSample {
    ResponseSample {
        elapsed_seconds,
        scan: sequence,
        controller_in_auto: true,
        setpoint: Some(5.0),
        process_value,
        applied_output: Some(2.0),
        quality: ChannelQuality::Good,
        additional: Vec::new(),
    }
}

/// A scan that records the process variable and the secondary pressure, the auxiliary channel of role PV.
fn with_pressure(process_value: Option<f64>, pressure: Option<f64>) -> ResponseSample {
    ResponseSample {
        additional: vec![AuxiliaryValue {
            id: "pv-secondary".to_owned(),
            unit: "bar".to_owned(),
            value: pressure,
            quality: ChannelQuality::Good,
        }],
        ..scan(1.0, 2, process_value)
    }
}

fn room_range() -> PermittedRange {
    PermittedRange {
        symbol: "P1.RoomC".to_owned(),
        unit: "degC".to_owned(),
        minimum: 0.0,
        maximum: 10.0,
        auxiliary_id: None,
    }
}

fn pressure_range() -> PermittedRange {
    PermittedRange {
        symbol: "P2.Pressure".to_owned(),
        unit: "bar".to_owned(),
        minimum: 1.0,
        maximum: 3.0,
        auxiliary_id: Some("pv-secondary".to_owned()),
    }
}

fn loop_signal(symbol: &str, unit: &str, minimum: f64, maximum: f64) -> LoopSignal {
    LoopSignal {
        symbol: symbol.to_owned(),
        address: None,
        iec_type: "LREAL".to_owned(),
        direction: Direction::Read,
        unit: unit.to_owned(),
        range: Range { minimum, maximum },
    }
}

fn auxiliary_signal(id: &str, role: AuxiliaryRole, symbol: &str, unit: &str) -> AuxiliarySignal {
    AuxiliarySignal {
        id: id.to_owned(),
        channel: "channel".to_owned(),
        role,
        symbol: symbol.to_owned(),
        address: None,
        iec_type: "LREAL".to_owned(),
        direction: Direction::Read,
        unit: unit.to_owned(),
        range: Range {
            minimum: 0.0,
            maximum: 100.0,
        },
    }
}

fn signals_with(additional: Vec<AuxiliarySignal>) -> Signals {
    Signals {
        setpoint: None,
        process_variable: loop_signal("P1.RoomC", "degC", 0.0, 10.0),
        manipulated_variable: loop_signal("P1.CompressorPct", "%", 0.0, 100.0),
        actuator: None,
        additional: Some(additional),
    }
}

/// The criteria of a response that holds the target 5 for 20 s on a 500 ms task.
fn criteria(process_maximum: f64, maximum_error: f64) -> CommissioningCriteria {
    CommissioningCriteria {
        target: 5.0,
        duration_seconds: 20.0,
        settling_time_seconds: 5.0,
        maximum_error,
        task_period_ms: 500.0,
        process_range: Range {
            minimum: 0.0,
            maximum: process_maximum,
        },
        output_limits: Range {
            minimum: -20.0,
            maximum: 20.0,
        },
        switching: None,
    }
}

fn recording(samples: Vec<ResponseSample>, completeness: Completeness) -> ObservedRecording {
    ObservedRecording {
        completeness,
        samples,
        output_unit: "V".to_owned(),
        evidence: RunEvidence::PhysicalRecorded,
    }
}

/// An observation with one bounds requirement on the process variable `PV` from 0 s to `end`.
fn bounded_pv(id: &str, end: f64, minimum: f64, maximum: f64) -> CommissioningObservation {
    CommissioningObservation {
        channels: vec![ObservedChannel {
            id: "PV".to_owned(),
            source: "processVariable".to_owned(),
            unit: "degC".to_owned(),
        }],
        requirements: vec![RunRequirement {
            id: id.to_owned(),
            channel: "PV".to_owned(),
            window: Window { start: 0.0, end },
            check: RunCheck::Bounds { minimum, maximum },
        }],
    }
}

#[test]
fn a_process_variable_outside_its_range_ends_the_observation_naming_it() {
    // Ported from control-studio-apply-process-time.test.ts (B6.3, B6.7): the departure names the signal and its values.
    let refusal = check_permitted_ranges(&[scan(0.5, 1, Some(12.5))], &[room_range()])
        .expect_err("the departure is refused");
    assert!(matches!(refusal, ObservationRefusal::PermittedRangeLeft(_)));
    assert_eq!(
        refusal.failure().message,
        "P1.RoomC left its permitted range (12.5 °C outside 0 °C..10 °C)"
    );
    assert_eq!(refusal.failure().cause, Cause::ModelOrData);
    assert_eq!(refusal.failure().next_action, NextAction::None);
}

#[test]
fn a_secondary_output_outside_its_own_range_ends_the_observation_naming_that_output() {
    let refusal = check_permitted_ranges(
        &[with_pressure(Some(5.0), Some(3.5))],
        &[room_range(), pressure_range()],
    )
    .expect_err("the secondary departure is refused");
    assert_eq!(
        refusal.failure().message,
        "P2.Pressure left its permitted range (3.5 bar outside 1 bar..3 bar)"
    );
    assert_eq!(refusal.failure().next_action, NextAction::None);
}

#[test]
fn a_secondary_output_inside_its_own_range_does_not_end_the_observation() {
    assert!(check_permitted_ranges(
        &[with_pressure(Some(5.0), Some(2.0))],
        &[room_range(), pressure_range()],
    )
    .is_ok());
}

#[test]
fn a_secondary_departure_in_the_stopped_recording_is_named() {
    let refusal = check_permitted_ranges(
        &[with_pressure(Some(5.0), Some(0.5))],
        &[room_range(), pressure_range()],
    )
    .expect_err("the stopped recording is checked");
    assert_eq!(
        refusal.failure().message,
        "P2.Pressure left its permitted range (0.5 bar outside 1 bar..3 bar)"
    );
}

#[test]
fn a_scan_without_a_value_of_an_output_is_no_departure() {
    assert!(check_permitted_ranges(&[scan(0.5, 1, None)], &[room_range()]).is_ok());
    assert!(check_permitted_ranges(
        &[with_pressure(Some(5.0), None)],
        &[room_range(), pressure_range()]
    )
    .is_ok());
    // A value that is not a number is refused by the recorded-data assessment, not as a departure (as the TypeScript).
    assert!(check_permitted_ranges(&[scan(0.5, 1, Some(f64::NAN))], &[room_range()]).is_ok());
}

#[test]
fn the_permitted_ranges_are_the_process_variable_then_the_pv_and_state_auxiliaries() {
    let signals = signals_with(vec![
        auxiliary_signal(
            "pv-secondary",
            AuxiliaryRole::ProcessVariable,
            "P2.Pressure",
            "bar",
        ),
        auxiliary_signal("state-speed", AuxiliaryRole::State, "M1.Speed", "rpm"),
        auxiliary_signal(
            "mv-echo",
            AuxiliaryRole::ManipulatedVariable,
            "M1.Echo",
            "%",
        ),
    ]);
    let ranges = observed_ranges_of(&signals);
    let identifiers: Vec<Option<&str>> = ranges
        .iter()
        .map(|range| range.auxiliary_id.as_deref())
        .collect();
    assert_eq!(
        identifiers,
        vec![None, Some("pv-secondary"), Some("state-speed")]
    );
    assert_eq!(ranges[0].symbol, "P1.RoomC");
}

#[test]
fn the_response_covers_the_reviewed_duration_from_the_adopted_target() {
    // Ported from control-studio-apply-process-time.test.ts (B6.3): the target is adopted at 2 s, the duration is 4 s, so the
    // sample at 6 s is the last one of the response.
    assert!(!response_covers_duration(Some(2.0), 5.0, 4_000.0));
    assert!(response_covers_duration(Some(2.0), 6.0, 4_000.0));
    assert!(!response_covers_duration(None, 60.0, 4_000.0));
}

#[test]
fn a_response_entered_at_30_s_is_covered_once_it_reaches_the_duration_from_the_entry() {
    // B6.3, A4.2: the response starts at 30 s and its samples are 47.5 s apart. The reviewed 9,500 s from the entry is reached at the
    // sample at 9,530 s; the one before it, at 9,482.5 s, is one record period short.
    let duration_ms = 9_500_000.0;
    assert!(!response_covers_duration(Some(30.0), 9_482.5, duration_ms));
    assert!(response_covers_duration(Some(30.0), 9_530.0, duration_ms));
    // The planned recording still holds that sample: its executions end 23.75 s after it, at 9,553.75 s (9,553,750 ms).
    assert!(planned_recording_ms(duration_ms, 100.0) >= 9_553_750.0);
}

#[test]
fn the_runtime_counts_as_lost_after_five_seconds_without_a_newer_scan() {
    // Ported from control-studio-apply-process-time.test.ts (A3.4): no newer value for more than 5 s is the connection loss.
    assert!(!connection_lost(0.0, 5_000.0, 0.0));
    assert!(connection_lost(0.0, 5_001.0, 0.0));
    // A7: on a followed recording with a 2 s record period the timeout is three of those intervals, 6 s, when that is longer.
    assert!(!connection_lost(0.0, 6_000.0, 2_000.0));
    assert!(connection_lost(0.0, 6_001.0, 2_000.0));
}

#[test]
fn a_response_is_averaged_when_its_samples_are_more_than_one_and_a_half_executions_apart() {
    let samples = |spacing: f64| {
        (0..5)
            .map(|index| scan(f64::from(index) * spacing, 1, Some(5.0)))
            .collect::<Vec<_>>()
    };
    // A 500 ms task: samples 0.5 s apart are one execution apart; samples 1 s apart are two executions apart (A4.2).
    assert!(!response_is_averaged(&samples(0.5), 500.0));
    assert!(response_is_averaged(&samples(1.0), 500.0));
}

#[test]
fn a_long_response_recorded_at_a_record_period_is_assessed_with_its_observed_bounds() {
    // Ported from control-studio-apply-process-time.test.ts (A4.2, E4.4, B6.3): a 0.5 s task recorded two executions per sample,
    // so the samples are 1 s apart, and the last one averages fewer executions. The observed bounds hold, so the error is 0.
    let mut samples: Vec<ResponseSample> = (0_u32..20)
        .map(|index| scan(f64::from(index), 2 * (u64::from(index) + 1), Some(5.0)))
        .collect();
    samples.push(scan(19.75, 41, Some(5.0)));
    let observed = bounded_pv("bounds", 20.0, 0.0, 10.0);
    assert_eq!(
        assess_observation(
            &recording(samples, Completeness::Complete),
            &criteria(10.0, 0.1),
            Some(&observed),
        ),
        Ok(0.0)
    );
}

#[test]
fn the_error_of_a_good_response_is_the_largest_settled_error_without_observed_requirements() {
    // The process settles at 5 with one excursion to 12 that the settling time has passed (the error is then 7).
    let samples: Vec<ResponseSample> = (0_u32..=20)
        .map(|index| {
            let value = if index == 10 { 12.0 } else { 5.0 };
            scan(f64::from(index), u64::from(index) + 1, Some(value))
        })
        .collect();
    let criteria = CommissioningCriteria {
        task_period_ms: 1_000.0,
        ..criteria(20.0, 10.0)
    };
    assert_eq!(
        assess_observation(&recording(samples, Completeness::Complete), &criteria, None),
        Ok(7.0)
    );
}

#[test]
fn an_incomplete_recording_is_a_failed_requirement_that_offers_restore() {
    // Ported from control-studio-apply-process-time.test.ts (B6.3): the failure is a settings failure with the restore action.
    let refusal = assess_observation(
        &recording(vec![scan(0.0, 1, Some(5.0))], Completeness::Incomplete),
        &criteria(10.0, 0.1),
        None,
    )
    .expect_err("an incomplete recording is refused");
    assert!(matches!(refusal, ObservationRefusal::RequirementFailed(_)));
    assert_eq!(refusal.failure().cause, Cause::Settings);
    assert_eq!(refusal.failure().next_action, NextAction::Restore);
}

#[test]
fn an_observed_requirement_that_fails_is_a_failed_requirement_that_offers_restore() {
    // The process variable reaches 12 at 10 s, above the observed bound of 10. The recorded response itself passes (its error
    // is 7, within 10), so the requirement is what fails.
    let samples: Vec<ResponseSample> = (0_u32..=20)
        .map(|index| {
            let value = if index == 10 { 12.0 } else { 5.0 };
            scan(f64::from(index), u64::from(index) + 1, Some(value))
        })
        .collect();
    let observed = bounded_pv("pv-bounds", 20.0, 0.0, 10.0);
    let criteria = CommissioningCriteria {
        task_period_ms: 1_000.0,
        ..criteria(20.0, 10.0)
    };
    let refusal = assess_observation(
        &recording(samples, Completeness::Complete),
        &criteria,
        Some(&observed),
    )
    .expect_err("the failed requirement is refused");
    assert!(matches!(refusal, ObservationRefusal::RequirementFailed(_)));
    assert!(
        refusal
            .failure()
            .message
            .starts_with("Commissioning pv-bounds: "),
        "the requirement is named: {}",
        refusal.failure().message
    );
    assert_eq!(refusal.failure().cause, Cause::Settings);
    assert_eq!(refusal.failure().next_action, NextAction::Restore);
}

#[test]
fn a_response_without_scans_is_refused_as_missing() {
    let span = ObservedResponseSpan {
        duration_seconds: 20.0,
        averaged: false,
    };
    let failure = assess_commissioning_observation(
        &[],
        &bounded_pv("bounds", 20.0, 0.0, 10.0),
        span,
        RunEvidence::PhysicalRecorded,
    )
    .expect_err("a response without scans is refused");
    assert_eq!(
        failure.message,
        "Commissioning channel observations are missing."
    );
    assert_eq!(failure.next_action, NextAction::Restore);
}

#[test]
fn the_observation_journal_names_its_start_and_its_planned_duration() {
    // Ported from control-studio-apply-process-time.test.ts (B6.3): the identity names the start and the duration.
    let journal = ObservationJournal {
        started_at_process_ns: 5_000_000_000,
        duration_ms: 4_000.0,
    };
    assert_eq!(
        serde_json::to_value(journal).expect("the journal serializes"),
        json!({"startedAtProcessNs": 5_000_000_000_u64, "durationMs": 4_000.0})
    );
}

#[test]
fn an_observation_is_in_progress_only_while_journalled_with_its_recording() {
    // Ported from control-studio-apply-reattach.test.ts (A8.5).
    assert!(observation_in_progress(JournalStage::Recording, true, true));
    assert!(observation_in_progress(JournalStage::Transfer, true, true));
    assert!(
        !observation_in_progress(JournalStage::Transfer, false, true),
        "before the observation starts, the reviewed recovery applies"
    );
    assert!(
        !observation_in_progress(JournalStage::Recording, true, false),
        "without a recording identity nothing can be reattached"
    );
    assert!(
        !observation_in_progress(JournalStage::Completed, true, true),
        "a completed observation is not followed again"
    );
    assert!(!observation_in_progress(
        JournalStage::Recovered,
        true,
        true
    ));
    assert!(!observation_in_progress(JournalStage::Retired, true, true));
}

#[test]
fn a_recording_the_runtime_no_longer_holds_is_an_incomplete_observation_that_offers_restore() {
    // Ported from control-studio-apply-reattach.test.ts (A8.6): the observation is incomplete, and Restore is offered.
    let refusal = not_held_refusal("Recording identity is unavailable.");
    assert!(matches!(refusal, ObservationRefusal::NotHeld(_)));
    assert_eq!(refusal.failure().next_action, NextAction::Restore);
    assert!(refusal
        .failure()
        .message
        .contains("no longer holds the recording"));
}

#[test]
fn only_the_recording_unavailable_code_says_the_runtime_no_longer_holds_the_recording() {
    assert!(recording_not_held("recording_unavailable"));
    assert!(!recording_not_held("scan_reset"));
}

#[test]
fn a_detached_observation_offers_no_action() {
    let refusal = detached_refusal();
    assert!(matches!(refusal, ObservationRefusal::Detached(_)));
    assert_eq!(refusal.failure().next_action, NextAction::None);
    assert_eq!(
        refusal.failure().message,
        "The panel closed while the observation was reattached; the runtime keeps recording it."
    );
}

#[test]
fn the_recording_holds_the_target_change_allowance_and_three_record_periods() {
    // 4 s on a 100 ms task: the 10 s confirmation floor and two executions (the allowance of the change, A7, B6.3), and three record
    // periods of at most a 200th of the planned length plus one execution (A4.2). Solved, the planned length is 2,900,000 / 197 ms.
    // Before the record periods were added the same case was 14.2 s.
    assert!((planned_recording_ms(4_000.0, 100.0) - 2_900_000.0 / 197.0).abs() < 1e-6);
    // On a 2 s task the window is three executions plus the 5 s connection-loss timeout, 11 s, and the allowance adds two executions.
    assert!((planned_recording_ms(4_000.0, 2_000.0) - 25_000.0 * 200.0 / 197.0).abs() < 1e-6);
}

// The planned length is a whole number of ms, as the start takes it, so the cast is exact for these durations.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
#[test]
fn the_planned_recording_exceeds_the_duration_by_the_allowance_and_three_longest_record_periods() {
    // The longest record period the start can choose settles over the whole recording, which keeps 200 samples in all (A4.2).
    for (duration_ms, task_ms) in [
        (1_000_u64, 100_u64),
        (4_000, 100),
        (60_000, 2_000),
        (9_500_000, 100),
        (86_400_000, 100),
    ] {
        let planned = planned_recording_ms(exact_f64(duration_ms), exact_f64(task_ms));
        let planned_whole = planned.ceil() as u64;
        let executions = record_period_executions(
            planned_whole,
            task_ms,
            RECORDING_SAMPLE_LIMIT,
            Some(planned_whole),
        );
        let record = exact_f64(executions * task_ms);
        let allowance = confirmation_window_ms(exact_f64(task_ms)) + 2.0 * exact_f64(task_ms);
        assert!(
            planned - exact_f64(duration_ms) >= allowance + 3.0 * record,
            "a {duration_ms} ms response on a {task_ms} ms task has {planned} ms planned"
        );
    }
}
