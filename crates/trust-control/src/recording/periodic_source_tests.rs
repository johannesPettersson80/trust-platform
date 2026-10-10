use super::{retained_periodic_recording_source, RetainedPeriodicRecordingSource};
use crate::loop_definition::LoopDefinition;
use crate::recording::fixtures::{binding, loop_definition, run, sample};
use crate::recording::{Completeness, RecordedRun};
use crate::signals::Quality;

/// The setpoint of every sample of these recordings.
const SETPOINT: f64 = 50.0;

/// The recording identifier and loop revision of the fixture runs.
const RECORDING_ID: &str = "rec-fixture";
const REVISION: &str = "sha256:loop-1";

/// Runtime times in nanoseconds of scans that follow `cycles_ns`, starting at 0.
fn times_ns(cycles_ns: &[i64]) -> Vec<i64> {
    let mut time = 0;
    let mut times = vec![time];
    for cycle in cycles_ns {
        time += cycle;
        times.push(time);
    }
    times
}

/// A recorded run of the scans `scans`, at the runtime times `times`, with the applied input and measurement of
/// each sample given by `input` and `pv` (the sample index).
fn recording(
    scans: &[u64],
    times: &[i64],
    input: impl Fn(usize) -> f64,
    pv: impl Fn(usize) -> f64,
) -> RecordedRun {
    let bindings = vec![
        binding("setpoint", "%ML0", "LREAL"),
        binding("processVariable", "%IL0", "LREAL"),
        binding("manipulatedVariable", "%QL0", "LREAL"),
    ];
    let samples = scans
        .iter()
        .zip(times)
        .enumerate()
        .map(|(index, (scan, time))| {
            sample(
                *scan,
                Some(*time),
                &[Some(SETPOINT), Some(pv(index)), Some(input(index))],
                &[Quality::Good; 3],
            )
        })
        .collect();
    run(bindings, samples)
}

/// The fixture recording of the `TypeScript` tests: 32 scans of 0.1 s with the input `1 + index/100` and the
/// measurement `2 + index/50`.
fn steady_recording() -> RecordedRun {
    let scans: Vec<u64> = (1..=32).collect();
    let times = times_ns(&[100_000_000; 31]);
    recording(
        &scans,
        &times,
        |index| 1.0 + f64::from(u32::try_from(index).unwrap_or(0)) / 100.0,
        |index| 2.0 + f64::from(u32::try_from(index).unwrap_or(0)) / 50.0,
    )
}

fn retained(
    recorded: &RecordedRun,
    loop_definition: &LoopDefinition,
) -> Option<RetainedPeriodicRecordingSource> {
    retained_periodic_recording_source(
        Some(recorded),
        loop_definition,
        RECORDING_ID,
        REVISION,
        None,
    )
}

#[track_caller]
fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-9,
        "{actual} is not {expected}"
    );
}

#[test]
fn a_steady_recording_is_retained_at_its_median_cycle_with_each_scan_placed_in_time() {
    let source =
        retained(&steady_recording(), &loop_definition()).expect("the recording is retained");
    assert_close(source.sample_time_seconds, 0.1);
    assert_eq!(source.scans.len(), 32);
    assert_close(source.scans[31].time_seconds, 3.1);
    assert_close(source.scans[0].input, 1.0);
    assert_close(source.scans[31].output, 2.0 + 31.0 / 50.0);
    assert_eq!(source.recording_id, RECORDING_ID);
    assert_eq!(source.recording_revision, REVISION);
}

#[test]
fn a_longer_cycle_places_the_later_scans_after_it_and_the_sample_time_stays_the_median() {
    // The cycle into the sixth scan takes 0.105 s; the median of the 31 cycles is still 0.1 s.
    let scans: Vec<u64> = (1..=32).collect();
    let mut cycles = [100_000_000_i64; 31];
    cycles[4] = 105_000_000;
    let recorded = recording(&scans, &times_ns(&cycles), |_| 1.0, |_| 2.0);
    let source =
        retained(&recorded, &loop_definition()).expect("the jittered recording is retained");
    assert_close(source.sample_time_seconds, 0.1);
    // The scans are placed on the 0.1 s grid they resample onto: the grid points 0.5 s and 0.6 s, not the scan times.
    assert_close(source.scans[4].time_seconds, 0.4);
    assert_close(source.scans[5].time_seconds, 0.5);
    assert_close(source.scans[6].time_seconds, 0.6);
    assert_eq!(
        source.scans.len(),
        32,
        "the 3.105 s record has 32 grid points"
    );
}

#[test]
fn a_cycle_longer_than_one_and_a_half_median_cycles_is_a_gap_and_is_refused() {
    // A 0.2 s cycle into the sixteenth scan is longer than 1.5 × 0.1 s; the median cycle stays 0.1 s.
    let scans: Vec<u64> = (1..=32).collect();
    let mut cycles = [100_000_000_i64; 31];
    cycles[14] = 200_000_000;
    let gapped = recording(&scans, &times_ns(&cycles), |_| 1.0, |_| 2.0);
    assert!(
        retained(&gapped, &loop_definition()).is_none(),
        "a 0.2 s gap cannot be bridged"
    );
}

#[test]
fn a_recording_that_is_incomplete_or_is_another_recording_is_refused() {
    let loop_definition = loop_definition();
    let mut incomplete = steady_recording();
    incomplete.acquisition.completeness = Completeness::Incomplete;
    assert!(
        retained(&incomplete, &loop_definition).is_none(),
        "an incomplete recording is not used"
    );
    assert!(
        retained_periodic_recording_source(
            Some(&steady_recording()),
            &loop_definition,
            "other",
            REVISION,
            None
        )
        .is_none(),
        "the recording must carry the identifier it is asked for"
    );
    assert!(
        retained_periodic_recording_source(
            Some(&steady_recording()),
            &loop_definition,
            RECORDING_ID,
            "sha256:other",
            None
        )
        .is_none(),
        "the recording must carry the revision it is asked for"
    );
    assert!(retained_periodic_recording_source(
        None,
        &loop_definition,
        RECORDING_ID,
        REVISION,
        None
    )
    .is_none());
}

#[test]
fn an_imported_recording_is_held_to_its_provenance_revision() {
    let loop_definition = loop_definition();
    let imported = retained_periodic_recording_source(
        Some(&steady_recording()),
        &loop_definition,
        RECORDING_ID,
        "sha256:imported",
        Some("sha256:imported"),
    );
    assert!(
        imported.is_some(),
        "the provenance revision replaces the loop revision of the runs"
    );
    assert!(
        retained_periodic_recording_source(
            Some(&steady_recording()),
            &loop_definition,
            RECORDING_ID,
            REVISION,
            Some("sha256:imported"),
        )
        .is_none(),
        "a different provenance revision is refused"
    );
}

#[test]
fn a_short_recording_or_one_with_a_bad_scan_is_refused() {
    let loop_definition = loop_definition();
    let scans: Vec<u64> = (1..=31).collect();
    let short = recording(&scans, &times_ns(&[100_000_000; 30]), |_| 1.0, |_| 2.0);
    assert!(
        retained(&short, &loop_definition).is_none(),
        "a periodic source keeps at least 32 samples"
    );

    let mut forced = steady_recording();
    forced.samples[7].quality[2] = Quality::Forced;
    assert!(
        retained(&forced, &loop_definition).is_none(),
        "a forced sample is not good"
    );

    let on_limit = recording(
        &(1..=32).collect::<Vec<u64>>(),
        &times_ns(&[100_000_000; 31]),
        |index| if index == 3 { 100.0 } else { 1.0 },
        |_| 2.0,
    );
    assert!(
        retained(&on_limit, &loop_definition).is_none(),
        "an input on an output limit is saturated"
    );

    let mut missing = steady_recording();
    missing.samples[2].values[1] = None;
    assert!(
        retained(&missing, &loop_definition).is_none(),
        "a missing measurement is not used"
    );

    let mut missed = steady_recording();
    missed.samples[12].scan += 1;
    assert!(
        retained(&missed, &loop_definition).is_none(),
        "a missed execution breaks the steady record period"
    );

    let mut cycles = [100_000_000_i64; 31];
    cycles[1] = 0;
    let stalled = recording(
        &(1..=32).collect::<Vec<u64>>(),
        &times_ns(&cycles),
        |_| 1.0,
        |_| 2.0,
    );
    assert!(
        retained(&stalled, &loop_definition).is_none(),
        "a cycle that does not advance is refused"
    );
}

#[test]
fn a_live_recording_of_64_scans_is_retained_at_its_steady_cycle() {
    // The live recording of a completed experiment: 64 scans 0.1 s apart, the input swinging 70 to 80 and the
    // measurement at 35 (`three-page-analysis` fixture).
    let scans: Vec<u64> = (100..164).collect();
    let times = times_ns(&[100_000_000; 63]);
    let recorded = recording(
        &scans,
        &times,
        |index| 75.0 + 5.0 * (0.02 * f64::from(u32::try_from(index).unwrap_or(0))).sin(),
        |_| 35.0,
    );
    let source = retained(&recorded, &loop_definition()).expect("the project run is retained");
    assert_close(source.sample_time_seconds, 0.1);
    assert_eq!(source.scans.len(), 64);
}

#[test]
fn jittered_cycles_of_100_and_110_ms_are_retained_with_their_median() {
    // The cycles alternate 110 and 100 ms (`jittered-sampling` fixture): the median of the 39 cycles is 110 ms.
    let scans: Vec<u64> = (1..=40).collect();
    let cycles: Vec<i64> = (1..40)
        .map(|index| {
            if index % 2 == 1 {
                110_000_000
            } else {
                100_000_000
            }
        })
        .collect();
    let recorded = recording(
        &scans,
        &times_ns(&cycles),
        |index| 1.0 + f64::from(u32::try_from(index).unwrap_or(0)) / 100.0,
        |index| 2.0 + f64::from(u32::try_from(index).unwrap_or(0)) / 50.0,
    );
    let source =
        retained(&recorded, &loop_definition()).expect("the jittered recording is retained");
    assert!(
        (source.sample_time_seconds - 0.105).abs() <= 0.01,
        "sample time {}",
        source.sample_time_seconds
    );
    // The 39 cycles cover 4.1 s; on the 0.11 s grid that is 38 points, within the 35 to 42 the TypeScript test accepts.
    assert!(
        (35..=42).contains(&source.scans.len()),
        "{} grid points",
        source.scans.len()
    );

    // A 0.25 s cycle in the jittered record is a gap, longer than 1.5 × 0.11 s, and is refused.
    let mut gapped = cycles.clone();
    gapped[19] = 250_000_000;
    let gapped = recording(&scans, &times_ns(&gapped), |_| 1.0, |_| 2.0);
    assert!(
        retained(&gapped, &loop_definition()).is_none(),
        "a 0.25 s cycle is a gap"
    );
}
