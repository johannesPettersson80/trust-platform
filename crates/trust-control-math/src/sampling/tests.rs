#![allow(clippy::float_cmp)] // Held indices and copied samples are compared exactly on purpose.

use super::*;

/// The scan times of a 100 ms task in a 10 ms resource, built as the generated-loop test builds them:
/// interval `i`, from scan `i` to scan `i + 1`, is 0.11 s when `i % 3 == 2` and 0.1 s otherwise.
fn task_jitter_times(count: usize) -> Vec<f64> {
    let mut times = vec![0.0];
    for index in 0..count - 1 {
        let interval = if index % 3 == 2 { 0.11 } else { 0.1 };
        times.push(times[index] + interval);
    }
    times
}

/// Scan times alternating 0.11 s and 0.1 s, as the jittered-sampling tests record them.
fn alternating_jitter_times(count: usize) -> Vec<f64> {
    let mut times = vec![0.0];
    for index in 1..count {
        let interval = if index % 2 == 1 { 0.11 } else { 0.1 };
        times.push(times[index - 1] + interval);
    }
    times
}

fn sample(time_seconds: f64, input: f64, output: f64) -> EvenSample {
    EvenSample {
        time_seconds,
        input,
        output,
        quality: SampleQuality::Good,
        clipped: false,
        saturated: false,
    }
}

#[test]
fn median_spacing_is_the_middle_interval_and_the_upper_one_for_an_even_count() {
    // Intervals 0.1, 0.11, 0.1, 0.1, 0.11: the sorted middle (index 2) is 0.1.
    let times = [0.0, 0.1, 0.21, 0.31, 0.41, 0.52];
    assert!((median_spacing(&times) - 0.1).abs() <= 1e-12);
    // Intervals 0.2, 0.1, 0.3, 0.4 sort to 0.1, 0.2, 0.3, 0.4: index 2 is 0.3 (upper median).
    let uneven = [0.0, 0.2, 0.3, 0.6, 1.0];
    assert!((median_spacing(&uneven) - 0.3).abs() <= 1e-12);
    assert_eq!(median_spacing(&[1.0]), 0.0, "fewer than two times");
}

#[test]
fn first_gap_reports_the_interval_that_is_longer_than_one_and_a_half_steps() {
    // Thirteen scans at 0, 0.1, …, 1.2 s, then nine from 1.62 s: the gap starts at 1.2 s and lasts
    // 0.42 s.
    let early: Vec<f64> = (0..13).map(|index| f64::from(index) * 0.1).collect();
    let late: Vec<f64> = (0..9).map(|index| 1.62 + f64::from(index) * 0.1).collect();
    let times: Vec<f64> = early.iter().chain(&late).copied().collect();
    let gap = first_gap(&times, 0.1).expect("the gap is found");
    assert!((gap.start_seconds - 1.2).abs() <= 1e-9);
    assert!((gap.length_seconds - 0.42).abs() <= 1e-9);
    assert_eq!(first_gap(&task_jitter_times(40), 0.1), None);
}

#[test]
fn even_grid_holds_the_latest_scan_at_or_before_each_grid_time() {
    // The live-plant pattern: 40 scans, 0.1 s apart with a 0.11 s interval every third. The grid at
    // 0.1 s runs to 4.03 s / 0.1 + 1 = 41 points, and the first five hold scans 0, 1, 2, 2, 3.
    let times = task_jitter_times(40);
    let grid = even_grid(&times, 0.1).expect("the record can be placed");
    assert_eq!(grid.len(), 41);
    let held: Vec<usize> = grid.iter().take(5).map(|point| point.index).collect();
    assert_eq!(held, vec![0, 1, 2, 2, 3]);
    assert!((grid[40].time_seconds - 4.0).abs() <= 1e-9);
}

#[test]
fn evenly_sampled_grid_places_jittered_scans_and_refuses_gaps_and_short_records() {
    let times = task_jitter_times(40);
    let placed = evenly_sampled_grid(&times, None, 30).expect("the record is placed");
    assert!(
        (placed.step_seconds - 0.1).abs() < 1e-9,
        "the median spacing"
    );
    assert!(!placed.aligned);
    assert_eq!(placed.points.len(), 41);

    let mut gapped = times.clone();
    gapped[20] += 0.2;
    assert!(
        evenly_sampled_grid(&gapped, None, 30).is_err(),
        "a gap is refused"
    );
    assert_eq!(
        evenly_sampled_grid(&times[..10], None, 30),
        Err(SampleError::TooFewSamples { minimum: 30 })
    );
    let backwards = [0.0, 0.2, 0.1];
    assert_eq!(
        evenly_sampled_grid(&backwards, Some(0.1), 1),
        Err(SampleError::IrregularTimes)
    );
}

#[test]
fn a_linear_output_reproduces_a_ramp_between_jittered_scans() {
    // The measured output is its own time, so interpolating it at each grid time gives the grid time.
    let times = alternating_jitter_times(40);
    let placed = evenly_sampled_grid(&times, None, 30).expect("the record is placed");
    for point in &placed.points {
        let (held, next) = (times[point.index], times.get(point.index + 1).copied());
        let value = match next {
            Some(next) if point.fraction > 0.0 => interpolate(held, next, point.fraction),
            _ => held,
        };
        assert!(
            (value - point.time_seconds).abs() < 1e-8,
            "a ramp at {}",
            point.time_seconds
        );
    }
}

#[test]
fn resampling_interpolates_outputs_and_holds_inputs() {
    // Times 0, 0.1, 0.25 with outputs 0, 1, 2.5 on a 0.1 s grid: the point at 0.2 s lies two thirds
    // of the way from 0.1 s to 0.25 s, so its output is 1 + (2/3)·1.5 = 2 and its input is held at 6.
    let samples = [
        sample(0.0, 5.0, 0.0),
        sample(0.1, 6.0, 1.0),
        sample(0.25, 7.0, 2.5),
    ];
    let resampled = resampled_evenly(&samples, Some(0.1)).expect("the record is placed");
    let outputs: Vec<f64> = resampled.iter().map(|entry| entry.output).collect();
    let inputs: Vec<f64> = resampled.iter().map(|entry| entry.input).collect();
    assert_eq!(outputs.len(), 3);
    for (value, reference) in outputs.iter().zip([0.0, 1.0, 2.0]) {
        assert!((value - reference).abs() <= 1e-9);
    }
    assert_eq!(inputs, vec![5.0, 6.0, 6.0]);
}

#[test]
fn block_averaging_never_mixes_the_two_sides_of_a_step() {
    // 12,000 samples at 0.01 s intervals; the input steps at 30 s, the sample at index 3000.
    let samples: Vec<EvenSample> = (0..12_000)
        .map(|index| {
            let time = f64::from(index) / 100.0;
            sample(time, if index < 3000 { 0.0 } else { 1.0 }, time)
        })
        .collect();
    let averaged = block_averaged(&samples, 5000, &[30.0]).expect("the limit is positive");
    // Size 3 gives 3000/3 = 1000 blocks before the step and 9000/3 = 3000 after it: 4000 in all.
    assert_eq!(averaged.len(), 4000);
    assert!(averaged
        .iter()
        .all(|entry| entry.input == 0.0 || entry.input == 1.0));
    assert!(averaged
        .windows(2)
        .all(|pair| pair[1].time_seconds > pair[0].time_seconds));
    assert_eq!(
        block_averaged(&samples, 0, &[]),
        Err(SampleError::InvalidLimit)
    );
}

#[test]
fn a_block_holds_the_flags_of_any_member() {
    // Four samples with one clipped sample, averaged to two blocks of two.
    let mut samples = [
        sample(0.0, 1.0, 1.0),
        sample(1.0, 1.0, 2.0),
        sample(2.0, 1.0, 3.0),
        sample(3.0, 1.0, 4.0),
    ];
    samples[1].clipped = true;
    let averaged = block_averaged(&samples, 2, &[]).expect("two blocks");
    assert_eq!(averaged.len(), 2);
    assert_eq!(averaged[0].quality, SampleQuality::Bad);
    assert!(averaged[0].clipped);
    assert!((averaged[0].time_seconds - 0.5).abs() <= 1e-12);
    assert!((averaged[0].output - 1.5).abs() <= 1e-12);
    assert_eq!(averaged[1].quality, SampleQuality::Good);
    assert!(!averaged[1].clipped);
}

#[test]
fn the_statistics_over_a_record_are_hand_computed() {
    assert_eq!(largest(&[3.0, -1.0, 7.0], f64::NEG_INFINITY), 7.0);
    assert_eq!(largest(&[], 0.5), 0.5, "the floor when no value exceeds it");
    assert_eq!(smallest(&[3.0, -1.0, 7.0]), -1.0);
    assert_eq!(smallest(&[]), f64::INFINITY);
    assert!((root_sum_square(&[3.0, 4.0]) - 5.0).abs() <= 1e-12);
}
