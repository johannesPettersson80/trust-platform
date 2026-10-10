use serde_json::json;

use super::{
    applied_mv, capture_additional_channels, follow_samples, recorded_channel_identity,
    sample_quality, valid_recorded_channels, ChannelQuality, RecordedChannel,
};
use crate::identification::campaign::FollowSample;
use crate::loop_definition::{AuxiliaryRole, LoopDefinition, LoopSignal};
use crate::recording::fixtures::{auxiliary_signal, binding, loop_definition, run, sample};
use crate::recording::{RecordedRun, RecordedSample};
use crate::signals::{Quality, SignalBinding};

/// The readings of a loop with a quality flag `valid` and a disturbance `feed-flow` in the same channel `reactor`,
/// and a level `level` in another channel: `valid` and `feed-flow` as given, `level` at 40.
fn readings(valid: Option<f64>, feed: Option<f64>) -> impl Fn(&str) -> Option<f64> {
    move |id| match id {
        "valid" => valid,
        "feed-flow" => feed,
        "level" => Some(40.0),
        _ => None,
    }
}

fn forced_feed(id: &str) -> bool {
    id == "feed-flow"
}

fn nothing_forced(_: &str) -> bool {
    false
}

fn flagged_loop() -> LoopDefinition {
    let mut loop_definition = loop_definition();
    loop_definition.signals.additional = Some(vec![
        auxiliary_signal(
            "valid", "reactor", "quality", "Valid", "%IX0.0", "read", "%",
        ),
        auxiliary_signal(
            "feed-flow",
            "reactor",
            "disturbance",
            "FeedFlow",
            "%QL8",
            "write",
            "%",
        ),
        auxiliary_signal("level", "tank", "state", "LevelState", "%IL8", "read", "%"),
    ]);
    loop_definition
}

fn qualities(channels: &[RecordedChannel]) -> Vec<ChannelQuality> {
    channels.iter().map(|channel| channel.quality).collect()
}

fn three_signals() -> Vec<SignalBinding> {
    vec![
        binding("setpoint", "%ML0", "LREAL"),
        binding("processVariable", "%IL0", "LREAL"),
        binding("manipulatedVariable", "%QL0", "LREAL"),
    ]
}

fn channel(
    id: &str,
    channel: &str,
    role: AuxiliaryRole,
    unit: &str,
    value: Option<f64>,
) -> RecordedChannel {
    RecordedChannel {
        id: id.to_string(),
        channel: channel.to_string(),
        role,
        unit: unit.to_string(),
        value,
        quality: ChannelQuality::Good,
    }
}

#[test]
fn a_flag_that_is_not_one_makes_its_channel_bad_and_a_forced_channel_is_forced() {
    let loop_definition = flagged_loop();
    // The flag reads 0 in the reactor channel, so both reactor signals are bad; the tank channel is good.
    let lowered = capture_additional_channels(
        &loop_definition,
        readings(Some(0.0), Some(32.0)),
        nothing_forced,
    );
    assert_eq!(
        qualities(&lowered),
        [
            ChannelQuality::Bad,
            ChannelQuality::Bad,
            ChannelQuality::Good
        ]
    );
    assert_eq!(lowered[1].channel, "reactor");
    assert_eq!(lowered[1].value, Some(32.0));
    // A raised flag leaves the channel good, and a forced signal is forced whatever its value.
    let forced = capture_additional_channels(
        &loop_definition,
        readings(Some(1.0), Some(32.0)),
        forced_feed,
    );
    assert_eq!(
        qualities(&forced),
        [
            ChannelQuality::Good,
            ChannelQuality::Forced,
            ChannelQuality::Good
        ]
    );
    // A missing value is bad.
    let missing =
        capture_additional_channels(&loop_definition, readings(Some(1.0), None), nothing_forced);
    assert_eq!(missing[1].quality, ChannelQuality::Bad);
    assert_eq!(missing[1].value, None);
}

#[test]
fn the_actuator_is_the_first_channel_in_the_primary_channel() {
    let mut loop_definition = loop_definition();
    loop_definition.signals.actuator = Some(
        serde_json::from_value::<LoopSignal>(json!({
            "symbol": "Valve.Position", "address": "%IL4", "iecType": "LREAL",
            "direction": "read", "unit": "%", "range": { "minimum": 0.0, "maximum": 100.0 }
        }))
        .expect("the fixture actuator decodes"),
    );
    let channels = capture_additional_channels(
        &loop_definition,
        |id| (id == "actuator").then_some(31.5),
        nothing_forced,
    );
    assert_eq!(channels.len(), 1);
    assert_eq!(
        channels[0],
        channel(
            "actuator",
            "primary",
            AuxiliaryRole::Actuator,
            "%",
            Some(31.5)
        )
    );
}

#[test]
fn a_sample_is_bad_when_any_value_is_missing_or_not_good_and_forced_when_any_is_forced() {
    let loop_definition = loop_definition();
    let good_sample = |values: [Option<f64>; 3], quality: [Quality; 3]| {
        run(three_signals(), vec![sample(1, Some(0), &values, &quality)])
    };
    let quality_of =
        |recorded: &RecordedRun| sample_quality(recorded, &loop_definition, &recorded.samples[0]);
    assert_eq!(
        quality_of(&good_sample(
            [Some(50.0), Some(48.0), Some(32.0)],
            [Quality::Good; 3]
        )),
        ChannelQuality::Good
    );
    assert_eq!(
        quality_of(&good_sample(
            [Some(50.0), Some(48.0), Some(32.0)],
            [Quality::Good, Quality::Forced, Quality::Bad]
        )),
        ChannelQuality::Forced,
        "a forced value makes the sample forced, even when another value is bad"
    );
    assert_eq!(
        quality_of(&good_sample(
            [Some(50.0), None, Some(32.0)],
            [Quality::Good; 3]
        )),
        ChannelQuality::Bad
    );
    assert_eq!(
        quality_of(&good_sample(
            [Some(50.0), Some(48.0), Some(32.0)],
            [Quality::Good, Quality::Missing, Quality::Good]
        )),
        ChannelQuality::Bad
    );
    let mut forced_flag = good_sample([Some(50.0), Some(48.0), Some(32.0)], [Quality::Good; 3]);
    forced_flag.samples[0].forced[1] = true;
    assert_eq!(quality_of(&forced_flag), ChannelQuality::Forced);
}

#[test]
fn a_lowered_quality_flag_makes_the_whole_sample_bad() {
    // The sample-level rule reads the loop's quality flags as the captured channels do (a flag that is not 1).
    let loop_definition = flagged_loop();
    let bindings = vec![
        binding("setpoint", "%ML0", "LREAL"),
        binding("processVariable", "%IL0", "LREAL"),
        binding("manipulatedVariable", "%QL0", "LREAL"),
        binding("valid", "%IX0.0", "BOOL"),
    ];
    let recorded = run(
        bindings,
        vec![sample(
            1,
            Some(0),
            &[Some(50.0), Some(48.0), Some(32.0), Some(0.0)],
            &[Quality::Good; 4],
        )],
    );
    assert_eq!(
        sample_quality(&recorded, &loop_definition, &recorded.samples[0]),
        ChannelQuality::Bad,
        "a sensor-quality signal that reads 0 is bad identification data"
    );
}

#[test]
fn the_applied_input_is_the_actuator_when_the_loop_declares_one() {
    let loop_definition = loop_definition();
    let mut with_actuator = loop_definition.clone();
    with_actuator.signals.actuator = Some(
        serde_json::from_value::<LoopSignal>(json!({
            "symbol": "Valve.Position", "address": "%IL4", "iecType": "LREAL",
            "direction": "read", "unit": "%", "range": { "minimum": 0.0, "maximum": 100.0 }
        }))
        .expect("the fixture actuator decodes"),
    );
    let mut bindings = three_signals();
    bindings.push(binding("actuator", "%IL4", "LREAL"));
    let recorded = run(
        bindings,
        vec![sample(
            1,
            Some(0),
            &[Some(50.0), Some(48.0), Some(32.0), Some(31.5)],
            &[Quality::Good; 4],
        )],
    );
    let first: &RecordedSample = &recorded.samples[0];
    assert_eq!(
        applied_mv(&recorded, &with_actuator, first),
        Some(31.5),
        "the actuator reached 31.5"
    );
    assert_eq!(
        applied_mv(&recorded, &loop_definition, first),
        Some(32.0),
        "without an actuator the command is applied"
    );
}

#[test]
fn the_identity_sorts_channels_by_identifier_in_utf16_order() {
    let channels = [
        channel(
            "feed-flow",
            "reactor",
            AuxiliaryRole::Disturbance,
            "%",
            Some(32.0),
        ),
        channel(
            "actuator",
            "primary",
            AuxiliaryRole::Actuator,
            "%",
            Some(31.5),
        ),
    ];
    // The values do not take part in the identity: only identifier, channel, role and unit do.
    assert_eq!(
        recorded_channel_identity(Some(&channels)),
        r#"[["actuator","primary","actuator","%"],["feed-flow","reactor","disturbance","%"]]"#
    );
    assert_eq!(recorded_channel_identity(None), "[]");
    // U+1F600 is the surrogate pair D83D DE00 in UTF-16, which sorts before U+FFFD, as JavaScript sorts it.
    let astral = [
        channel("\u{FFFD}", "x", AuxiliaryRole::State, "u", None),
        channel("\u{1F600}", "x", AuxiliaryRole::State, "u", None),
    ];
    assert_eq!(
        recorded_channel_identity(Some(&astral)),
        format!(
            "[[\"{}\",\"x\",\"state\",\"u\"],[\"{}\",\"x\",\"state\",\"u\"]]",
            "\u{1F600}", "\u{FFFD}"
        )
    );
}

#[test]
fn a_list_of_channels_is_valid_up_to_32_distinct_named_channels_with_finite_values() {
    let named = |index: usize| {
        channel(
            &format!("signal-{index}"),
            "primary",
            AuxiliaryRole::State,
            "",
            Some(1.0),
        )
    };
    let thirty_two: Vec<RecordedChannel> = (0..32).map(named).collect();
    assert!(
        valid_recorded_channels(&thirty_two),
        "32 channels are the most one recording keeps"
    );
    let thirty_three: Vec<RecordedChannel> = (0..33).map(named).collect();
    assert!(!valid_recorded_channels(&thirty_three));
    let duplicate = [named(1), named(1)];
    assert!(
        !valid_recorded_channels(&duplicate),
        "identifiers are unique"
    );
    let unnamed = [channel("", "primary", AuxiliaryRole::State, "%", None)];
    assert!(
        !valid_recorded_channels(&unnamed),
        "an identifier is not empty"
    );
    let no_channel = [channel("a", "", AuxiliaryRole::State, "%", None)];
    assert!(
        !valid_recorded_channels(&no_channel),
        "a channel is not empty"
    );
    let not_finite = [channel(
        "a",
        "primary",
        AuxiliaryRole::State,
        "%",
        Some(f64::NAN),
    )];
    assert!(
        !valid_recorded_channels(&not_finite),
        "a value is missing or finite"
    );
    let unitless = [channel("a", "primary", AuxiliaryRole::State, "", None)];
    assert!(
        valid_recorded_channels(&unitless),
        "an empty unit is a unit"
    );
}

/// The loop of `control-studio-runtime-recording.test.ts`: the tank with the quality flag `valid` at `%MX16.0`.
fn recording_loop() -> LoopDefinition {
    let mut loop_definition = loop_definition();
    loop_definition.signals.additional = Some(vec![auxiliary_signal(
        "valid", "primary", "quality", "P1.Valid", "%MX16.0", "read", "%",
    )]);
    loop_definition
}

/// The bindings of that test's recording: the three signals of the loop and the flag.
fn recording_bindings() -> Vec<SignalBinding> {
    let mut bindings = three_signals();
    bindings.push(binding("valid", "%MX16.0", "BOOL"));
    bindings
}

/// A scan of that recording at `time_ns`: a setpoint of 1, a measurement of 0.5, a request of 2 and the flag `valid`, all
/// of good quality.
fn scan_at(scan: u64, time_ns: i64, valid: f64) -> RecordedSample {
    sample(
        scan,
        Some(time_ns),
        &[Some(1.0), Some(0.5), Some(2.0), Some(valid)],
        &[Quality::Good; 4],
    )
}

fn assert_followed(followed: &FollowSample, seconds: f64, good: bool, output: Option<f64>) {
    assert!(
        (followed.elapsed_seconds - seconds).abs() <= 1e-12,
        "{followed:?} is not at {seconds} s"
    );
    assert_eq!(followed.good, good, "{followed:?}");
    assert_eq!(followed.output, output, "{followed:?}");
}

#[test]
fn a_status_page_gives_each_sample_its_process_time_its_quality_and_its_measurement() {
    // The page of the TypeScript follow test: scans 1 and 2, 10 ms apart, with a setpoint of 1, a measurement of 0.5, a
    // request of 2 and a raised flag.
    let loop_definition = recording_loop();
    let bindings = recording_bindings();
    let page = [scan_at(1, 0, 1.0), scan_at(2, 10_000_000, 1.0)];
    let mut first_time = None;
    let followed = follow_samples(&loop_definition, &bindings, &page, &mut first_time)
        .expect("a page of well-formed samples");
    assert_eq!(followed.len(), 2);
    assert_followed(&followed[0], 0.0, true, Some(0.5));
    assert_followed(&followed[1], 0.01, true, Some(0.5));
    assert_eq!(
        first_time,
        Some(0),
        "the first sample's runtime time is the origin"
    );
    // The next page continues after the samples already read: the runtime has none to send, and the origin stays.
    let empty = follow_samples(&loop_definition, &bindings, &[], &mut first_time)
        .expect("an empty page is a page");
    assert_eq!(empty, Vec::new());
    assert_eq!(first_time, Some(0));
}

#[test]
fn a_lowered_flag_a_forced_value_or_a_missing_value_makes_a_sample_not_good() {
    let loop_definition = recording_loop();
    let bindings = recording_bindings();
    let follow_one = |scan: RecordedSample| -> FollowSample {
        follow_samples(&loop_definition, &bindings, &[scan], &mut None)
            .expect("a well-formed sample")[0]
    };
    // A sensor-quality signal that reads 0 is bad identification data, though the acquisition is complete.
    assert!(follow_one(scan_at(1, 0, 1.0)).good);
    assert!(!follow_one(scan_at(1, 0, 0.0)).good);
    let mut forced = scan_at(1, 0, 1.0);
    forced.forced[1] = true;
    assert!(!follow_one(forced).good);
    let missing = follow_one(sample(
        1,
        Some(0),
        &[Some(1.0), None, Some(2.0), Some(1.0)],
        &[
            Quality::Good,
            Quality::Missing,
            Quality::Good,
            Quality::Good,
        ],
    ));
    assert!(!missing.good);
    assert_eq!(missing.output, None, "a missing measurement is no output");
}

#[test]
fn a_channel_that_is_not_good_makes_only_its_own_sample_not_good() {
    // The unaddressed mode channel `P1.Manual` is the fifth binding. The second sample reports it with bad quality.
    let mut bindings = recording_bindings();
    bindings.push(SignalBinding {
        id: "controllerMode".to_string(),
        address: None,
        symbol: Some("P1.Manual".to_string()),
        iec_type: "BOOL".to_string(),
    });
    let values = [Some(1.0), Some(0.5), Some(2.0), Some(1.0), Some(1.0)];
    let mut second = sample(2, Some(10_000_000), &values, &[Quality::Good; 5]);
    second.quality[4] = Quality::Bad;
    let page = [sample(1, Some(0), &values, &[Quality::Good; 5]), second];
    let followed = follow_samples(&recording_loop(), &bindings, &page, &mut None)
        .expect("a page of well-formed samples");
    assert_followed(&followed[0], 0.0, true, Some(0.5));
    assert_followed(&followed[1], 0.01, false, Some(0.5));
}

#[test]
fn a_second_page_counts_from_the_origin_of_the_first() {
    // The recording's first sample is at 5 s of runtime time, so the origin is not zero. The runtime has no sample for the
    // first request.
    let loop_definition = recording_loop();
    let bindings = recording_bindings();
    let mut first_time = None;
    let nothing = follow_samples(&loop_definition, &bindings, &[], &mut first_time)
        .expect("an empty page is a page");
    assert_eq!(nothing, Vec::new());
    assert_eq!(
        first_time, None,
        "no sample has been followed, so there is no origin"
    );

    let first = [
        scan_at(1, 5_000_000_000, 1.0),
        scan_at(2, 5_010_000_000, 1.0),
    ];
    let page = follow_samples(&loop_definition, &bindings, &first, &mut first_time)
        .expect("the first page");
    assert_followed(&page[0], 0.0, true, Some(0.5));
    assert_followed(&page[1], 0.01, true, Some(0.5));
    assert_eq!(first_time, Some(5_000_000_000));

    let second = [
        scan_at(3, 5_020_000_000, 1.0),
        scan_at(4, 5_030_000_000, 1.0),
    ];
    let page = follow_samples(&loop_definition, &bindings, &second, &mut first_time)
        .expect("the second page");
    assert_followed(&page[0], 0.02, true, Some(0.5));
    assert_followed(&page[1], 0.03, true, Some(0.5));
    assert_eq!(
        first_time,
        Some(5_000_000_000),
        "a later page keeps the origin of the first"
    );
}

#[test]
fn the_measurement_is_the_process_variable_binding_wherever_it_sits() {
    // A loop without a setpoint binding records the measurement first.
    let bindings = [
        binding("processVariable", "%IL0", "LREAL"),
        binding("manipulatedVariable", "%QL0", "LREAL"),
    ];
    let page = [sample(
        1,
        Some(0),
        &[Some(48.0), Some(32.0)],
        &[Quality::Good; 2],
    )];
    let followed = follow_samples(&loop_definition(), &bindings, &page, &mut None)
        .expect("a well-formed sample");
    assert_eq!(followed[0].output, Some(48.0));
}

#[test]
fn a_sample_without_its_runtime_time_has_no_process_time_and_is_refused() {
    let loop_definition = recording_loop();
    let bindings = recording_bindings();
    let untimed = sample(
        1,
        None,
        &[Some(1.0), Some(0.5), Some(2.0), Some(1.0)],
        &[Quality::Good; 4],
    );
    let mut first_time = None;
    let failure = follow_samples(&loop_definition, &bindings, &[untimed], &mut first_time)
        .expect_err("no runtime time, no process time");
    assert_eq!(
        failure.message,
        "The runtime recording contains invalid samples."
    );
    assert_eq!(first_time, None);
}
