// Held values and sequence values are compared exactly on purpose; the scan indices are small counts.
#![allow(clippy::float_cmp, clippy::cast_precision_loss)]

use super::{evenly_sampled, ChannelReading, EvenSamplingError, TimedRecordedScan};
use crate::signals::Quality;

fn scan(time_seconds: f64, sequence: u64, values: &[f64]) -> TimedRecordedScan {
    TimedRecordedScan {
        time_seconds,
        sequence,
        readings: values
            .iter()
            .map(|value| ChannelReading {
                value: Some(*value),
                quality: Quality::Good,
            })
            .collect(),
    }
}

/// The scan times of a 100 ms task in a 10 ms resource: the interval `i`, from scan `i` to scan `i + 1`, is 0.11 s
/// when `i % 3 == 2` and 0.1 s otherwise (the generated-loop test).
fn generated_loop_times(count: usize) -> Vec<f64> {
    let mut times = vec![0.0];
    for index in 0..count - 1 {
        let interval = if index % 3 == 2 { 0.11 } else { 0.1 };
        let last = times.last().copied().unwrap_or(0.0);
        times.push(last + interval);
    }
    times
}

/// Execution times that alternate 0.11 s and 0.1 s, as the jittered-sampling test records them.
fn alternating_times(count: usize) -> Vec<f64> {
    let mut times = vec![0.0];
    for index in 1..count {
        let interval = if index % 2 == 1 { 0.11 } else { 0.1 };
        let last = times.last().copied().unwrap_or(0.0);
        times.push(last + interval);
    }
    times
}

#[test]
fn a_run_already_on_its_grid_is_returned_unchanged() {
    let scans: Vec<TimedRecordedScan> = (0..40_usize)
        .map(|index| scan(index as f64 * 0.1, 7 + index as u64, &[index as f64]))
        .collect();
    let sampled = evenly_sampled(&scans, None, &[]).expect("the run is on its grid");
    assert!((sampled.step_seconds - 0.1).abs() <= 1e-9);
    assert_eq!(sampled.scans, scans);
}

#[test]
fn a_live_plant_recording_is_compared_on_an_even_grid_with_held_values() {
    let times = generated_loop_times(40);
    let scans: Vec<TimedRecordedScan> = times
        .iter()
        .enumerate()
        .map(|(index, &time)| scan(time, 7 + index as u64, &[index as f64]))
        .collect();
    let sampled = evenly_sampled(&scans, None, &[]).expect("the run is refused or placed");
    assert!(
        (sampled.step_seconds - 0.1).abs() <= 1e-9,
        "the median spacing: {}",
        sampled.step_seconds
    );
    for (index, placed) in sampled.scans.iter().enumerate() {
        assert!(
            (placed.time_seconds - index as f64 * 0.1).abs() <= 1e-9,
            "grid time {index}"
        );
        assert_eq!(placed.sequence, 7 + index as u64);
    }
    let held: Vec<f64> = sampled.scans[..5]
        .iter()
        .map(|placed| placed.readings[0].value.expect("a value"))
        .collect();
    assert_eq!(
        held,
        [0.0, 1.0, 2.0, 2.0, 3.0],
        "each grid time holds the latest scan at or before it, as a PLC holds a value"
    );
}

#[test]
fn a_missing_scan_and_a_scan_out_of_time_order_are_irregular() {
    let times = generated_loop_times(40);
    let missing: Vec<TimedRecordedScan> = times
        .iter()
        .enumerate()
        .map(|(index, &time)| {
            scan(
                time,
                if index == 5 { 99 } else { 7 + index as u64 },
                &[index as f64],
            )
        })
        .collect();
    assert_eq!(
        evenly_sampled(&missing, None, &[]),
        Err(EvenSamplingError::Irregular),
        "a scan whose sequence skips is missing"
    );
    let shifted: Vec<TimedRecordedScan> = times
        .iter()
        .enumerate()
        .map(|(index, &time)| {
            scan(
                if index == 20 { time + 0.2 } else { time },
                7 + index as u64,
                &[index as f64],
            )
        })
        .collect();
    assert_eq!(
        evenly_sampled(&shifted, None, &[]),
        Err(EvenSamplingError::Irregular),
        "a scan that runs back in time is irregular"
    );
}

#[test]
fn a_gap_of_more_than_one_and_a_half_spacings_is_named() {
    let times = generated_loop_times(40);
    let gapped: Vec<TimedRecordedScan> = times
        .iter()
        .enumerate()
        .map(|(index, &time)| {
            scan(
                if index >= 20 { time + 0.2 } else { time },
                7 + index as u64,
                &[index as f64],
            )
        })
        .collect();
    match evenly_sampled(&gapped, None, &[]) {
        Err(EvenSamplingError::Gap(gap)) => {
            assert!(
                (gap.length_seconds - 0.3).abs() <= 1e-9,
                "gap of {} s",
                gap.length_seconds
            );
            assert!((gap.start_seconds - times[19]).abs() <= 1e-9);
        }
        other => panic!("expected a gap, found {other:?}"),
    }
}

#[test]
fn a_run_with_fewer_than_thirty_scans_is_too_few() {
    let scans: Vec<TimedRecordedScan> = (0..29_usize)
        .map(|index| scan(index as f64 * 0.1, index as u64, &[0.0]))
        .collect();
    assert_eq!(
        evenly_sampled(&scans, None, &[]),
        Err(EvenSamplingError::TooFew)
    );
}

#[test]
fn a_linear_channel_reproduces_a_ramp_and_a_held_channel_takes_whole_scan_values() {
    let times = alternating_times(40);
    let scans: Vec<TimedRecordedScan> = times
        .iter()
        .enumerate()
        .map(|(index, &time)| scan(time, 7 + index as u64, &[time, index as f64]))
        .collect();
    let sampled = evenly_sampled(&scans, None, &[0]).expect("the jittered run is placed");
    for placed in &sampled.scans {
        let ramp = placed.readings[0].value.expect("a value");
        assert!(
            (ramp - placed.time_seconds).abs() <= 1e-9,
            "a linear output reproduces a ramp"
        );
        let held = placed.readings[1].value.expect("a value");
        assert_eq!(
            held.fract(),
            0.0,
            "the held command takes whole scan values"
        );
    }
}
