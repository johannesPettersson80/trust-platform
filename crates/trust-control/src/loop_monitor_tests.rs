//! Specification 35, B2 and A3.4: the live monitor's sample, its freshness, its reasons and the wire fields the page reads.

// The values are the numbers the scan gave the rule, which passes them on unchanged, so they are compared exactly.
#![allow(clippy::float_cmp)]

use serde_json::{json, Value};

use super::*;
use crate::loop_definition::AuxiliaryRole;
use crate::signals::{loop_inspection_bindings, loop_inspection_mode_id, InspectionSample};

const STALE_AFTER_MS: u64 = 5_000;

fn signal(symbol: &str, address: &str, direction: &str) -> Value {
    json!({
        "symbol": symbol, "address": address, "iecType": "LREAL", "direction": direction, "unit": "%",
        "range": { "minimum": 0.0, "maximum": 100.0 }
    })
}

fn tank_json() -> Value {
    json!({
        "schemaVersion": 2, "id": "loop:tank", "name": "Tank level", "revision": "sha256:loop-1",
        "projectId": "project:fixture", "runtimeId": "runtime:local",
        "controller": {
            "instance": "TankController", "type": "Control.PID", "revision": "sha256:controller-1",
            "action": "direct", "taskPeriodMs": 10.0
        },
        "signals": {
            "setpoint": signal("LevelSP", "%ML0", "read"),
            "processVariable": signal("LevelPV", "%IL0", "read"),
            "manipulatedVariable": signal("ValveMV", "%QL0", "write")
        },
        "limits": { "outputMinimum": 0.0, "outputMaximum": 100.0, "rateUpPerS": 20.0, "rateDownPerS": 25.0 },
        "observedSampleTiming": { "nominalPeriodMs": 10.0 }
    })
}

fn decode(value: &Value) -> LoopDefinition {
    LoopDefinition::decode(&value.to_string()).expect("the fixture loop decodes")
}

fn tank() -> LoopDefinition {
    decode(&tank_json())
}

fn reading(symbol: &str, address: &str, value: f64) -> MonitorSignal {
    MonitorSignal {
        symbol: symbol.into(),
        address: Some(address.into()),
        value,
        unit: "%".into(),
        quality: MonitorQuality::Good,
        plotted: true,
    }
}

fn readings(scan: u64, process_variable: f64, manipulated_variable: f64) -> CoherentReadings {
    CoherentReadings {
        scan,
        controller_mode: ControllerMode::Auto,
        setpoint: Some(reading("LevelSP", "%ML0", 50.0)),
        process_variable: Some(reading("LevelPV", "%IL0", process_variable)),
        manipulated_variable: Some(reading("ValveMV", "%QL0", manipulated_variable)),
        additional: Vec::new(),
    }
}

fn observe(
    monitor: &mut LoopMonitor,
    loop_definition: &LoopDefinition,
    readings: CoherentReadings,
    observed_at_ms: u64,
) -> LoopMonitorSample {
    monitor.observe(
        loop_definition,
        MonitorSource::Simulation,
        Some(readings),
        observed_at_ms,
        STALE_AFTER_MS,
    )
}

fn available(sample: LoopMonitorSample) -> AvailableSample {
    match sample {
        LoopMonitorSample::Available(sample) => sample,
        LoopMonitorSample::Unavailable(sample) => {
            panic!("the sample is unavailable: {}", sample.reason)
        }
    }
}

fn reason(sample: &LoopMonitorSample) -> &str {
    match sample {
        LoopMonitorSample::Unavailable(sample) => &sample.reason,
        LoopMonitorSample::Available(_) => panic!("the sample is available"),
    }
}

/// One scan of `loop_definition` in which each named point has the value, forced flag and quality given; every other binding
/// reads 0 with good quality.
fn scan_of(
    loop_definition: &LoopDefinition,
    scan: u64,
    points: &[(&str, Option<f64>, bool, Quality)],
) -> InspectionSample {
    let bindings = loop_inspection_bindings(loop_definition);
    let mut values = Vec::new();
    let mut forced = Vec::new();
    let mut quality = Vec::new();
    for binding in &bindings {
        let (value, is_forced, point_quality) =
            points.iter().find(|(id, ..)| *id == binding.id).map_or(
                (Some(0.0), false, Quality::Good),
                |(_, value, is_forced, point_quality)| (*value, *is_forced, *point_quality),
            );
        values.push(value);
        forced.push(is_forced);
        quality.push(point_quality);
    }
    InspectionSample {
        program_revision: "sha256:program".into(),
        loop_revision: loop_definition.revision.clone(),
        bindings,
        scan,
        runtime_time_ns: scan * 100_000_000,
        captured_at_ms: 1_000,
        values,
        forced,
        quality,
    }
}

#[test]
fn a_stale_scan_is_reported_only_after_the_threshold_has_passed() {
    let loop_definition = tank();
    let mut monitor = LoopMonitor::default();
    assert!(matches!(
        observe(
            &mut monitor,
            &loop_definition,
            readings(10, 21.5, 36.0),
            1_000
        ),
        LoopMonitorSample::Available(_)
    ));
    assert!(
        matches!(
            observe(
                &mut monitor,
                &loop_definition,
                readings(10, 21.5, 36.0),
                6_000
            ),
            LoopMonitorSample::Available(_)
        ),
        "5 s without a newer scan is not stale"
    );
    let stale = observe(
        &mut monitor,
        &loop_definition,
        readings(10, 21.5, 36.0),
        6_001,
    );
    assert_eq!(reason(&stale), "Runtime scan is stale");
    assert!(
        matches!(
            observe(
                &mut monitor,
                &loop_definition,
                readings(11, 21.5, 36.0),
                6_010
            ),
            LoopMonitorSample::Available(_)
        ),
        "a newer scan resumes the observation"
    );
}

#[test]
fn a_scan_that_went_backwards_is_refused_until_a_newer_scan_arrives() {
    let loop_definition = tank();
    let mut monitor = LoopMonitor::default();
    observe(
        &mut monitor,
        &loop_definition,
        readings(10, 48.0, 32.0),
        1_000,
    );
    let backwards = observe(
        &mut monitor,
        &loop_definition,
        readings(9, 47.0, 31.0),
        1_020,
    );
    assert_eq!(
        reason(&backwards),
        "Runtime scan moved backwards from 10 to 9"
    );
    assert!(matches!(
        observe(
            &mut monitor,
            &loop_definition,
            readings(11, 49.0, 33.0),
            1_040
        ),
        LoopMonitorSample::Available(_)
    ));
}

#[test]
fn a_missing_signal_names_its_address_and_the_scan() {
    let loop_definition = tank();
    let mut no_process_value = readings(12, 20.5, 31.0);
    no_process_value.process_variable = None;
    let sample = observe(
        &mut LoopMonitor::default(),
        &loop_definition,
        no_process_value,
        1_000,
    );
    assert_eq!(
        reason(&sample),
        "Process variable %IL0 is missing from runtime scan 12"
    );

    let mut no_output = readings(12, 20.5, 31.0);
    no_output.manipulated_variable = None;
    let sample = observe(
        &mut LoopMonitor::default(),
        &loop_definition,
        no_output,
        1_000,
    );
    assert_eq!(
        reason(&sample),
        "Manipulated variable %QL0 is missing from runtime scan 12"
    );

    let mut without_sp = readings(12, 20.5, 31.0);
    without_sp.setpoint = None;
    let sample = observe(
        &mut LoopMonitor::default(),
        &loop_definition,
        without_sp,
        1_000,
    );
    assert_eq!(
        reason(&sample),
        "Setpoint %ML0 is missing from runtime scan 12"
    );
}

#[test]
fn a_bad_point_is_missing_and_never_promoted_to_a_reading() {
    let loop_definition = tank();
    let scan = scan_of(
        &loop_definition,
        8,
        &[("processVariable", Some(48.0), false, Quality::Bad)],
    );
    let readings = coherent_readings(&loop_definition, &scan, None);
    assert!(
        readings.process_variable.is_none(),
        "a bad value is not a reading"
    );
    let sample = observe(
        &mut LoopMonitor::default(),
        &loop_definition,
        readings,
        1_000,
    );
    assert_eq!(
        reason(&sample),
        "Process variable %IL0 is missing from runtime scan 8"
    );
}

#[test]
fn a_forced_point_is_forced_and_a_good_point_is_good() {
    let loop_definition = tank();
    let scan = scan_of(
        &loop_definition,
        3,
        &[
            ("processVariable", Some(21.5), true, Quality::Good),
            ("manipulatedVariable", Some(36.0), false, Quality::Forced),
            ("setpoint", Some(50.0), false, Quality::Good),
        ],
    );
    let readings = coherent_readings(&loop_definition, &scan, None);
    let sample = available(observe(
        &mut LoopMonitor::default(),
        &loop_definition,
        readings,
        1_000,
    ));
    assert_eq!(
        sample.process_variable.quality,
        MonitorQuality::Forced,
        "the forced flag"
    );
    assert_eq!(
        sample.manipulated_variable.quality,
        MonitorQuality::Forced,
        "the forced quality"
    );
    assert_eq!(
        sample.setpoint.map(|signal| signal.quality),
        Some(MonitorQuality::Good)
    );
}

#[test]
fn a_forced_value_is_a_gap_in_the_plot_and_a_good_value_is_drawn() {
    // Wrong behaviour caught: the page deciding from the quality which live values it draws. B9.6: a forced value is a gap in its
    // own trace and removes no good value of another signal.
    let loop_definition = tank();
    let scan = scan_of(
        &loop_definition,
        3,
        &[
            ("processVariable", Some(21.5), true, Quality::Good),
            ("manipulatedVariable", Some(36.0), false, Quality::Forced),
            ("setpoint", Some(50.0), false, Quality::Good),
        ],
    );
    let sample = available(observe(
        &mut LoopMonitor::default(),
        &loop_definition,
        coherent_readings(&loop_definition, &scan, None),
        1_000,
    ));
    assert!(!sample.process_variable.plotted, "the forced measurement");
    assert!(!sample.manipulated_variable.plotted, "the forced output");
    assert_eq!(
        sample.setpoint.map(|signal| signal.plotted),
        Some(true),
        "the good setpoint is drawn"
    );
}

#[test]
fn the_controller_mode_follows_the_mode_signal_and_its_meaning_when_true() {
    let mut value = tank_json();
    value["controller"]["modeSignal"] = json!("%MX0.0");
    value["controller"]["modeWhenTrue"] = json!("Manual");
    let loop_definition = decode(&value);
    let bindings = loop_inspection_bindings(&loop_definition);
    let mode_id = loop_inspection_mode_id(&loop_definition, &bindings, "%MX0.0")
        .expect("the mode is read")
        .to_string();
    for (bit, expected) in [(1.0, ControllerMode::Manual), (0.0, ControllerMode::Auto)] {
        let scan = scan_of(
            &loop_definition,
            4,
            &[(mode_id.as_str(), Some(bit), false, Quality::Good)],
        );
        let readings = coherent_readings(&loop_definition, &scan, Some(mode_id.as_str()));
        assert_eq!(readings.controller_mode, expected, "mode bit {bit}");
    }
    let unusable = scan_of(
        &loop_definition,
        5,
        &[(mode_id.as_str(), Some(1.0), false, Quality::Bad)],
    );
    assert_eq!(
        coherent_readings(&loop_definition, &unusable, Some(mode_id.as_str())).controller_mode,
        ControllerMode::Unavailable,
        "a bad mode value is not a mode"
    );
    assert_eq!(
        coherent_readings(&loop_definition, &unusable, None).controller_mode,
        ControllerMode::Unavailable,
        "a loop with no mode signal has no mode"
    );
}

#[test]
fn saturation_is_minimum_maximum_or_none() {
    let loop_definition = tank();
    for (output, expected) in [
        (0.0, Saturation::Minimum),
        (100.0, Saturation::Maximum),
        (36.0, Saturation::NotSaturated),
    ] {
        let sample = available(observe(
            &mut LoopMonitor::default(),
            &loop_definition,
            readings(1, 21.5, output),
            1_000,
        ));
        assert_eq!(sample.saturation, expected, "output {output}");
    }
}

#[test]
fn a_runtime_that_is_not_running_is_not_producing_scans_and_forgets_the_scan() {
    let loop_definition = tank();
    let mut monitor = LoopMonitor::default();
    observe(
        &mut monitor,
        &loop_definition,
        readings(10, 21.5, 36.0),
        1_000,
    );
    let stopped = monitor.observe(
        &loop_definition,
        MonitorSource::Physical,
        None,
        1_100,
        STALE_AFTER_MS,
    );
    assert_eq!(reason(&stopped), "Runtime is not producing scans");
    assert!(
        matches!(&stopped, LoopMonitorSample::Unavailable(sample) if sample.source == MonitorSource::Physical),
        "a stopped runtime is physical"
    );
    assert!(
        matches!(
            observe(
                &mut monitor,
                &loop_definition,
                readings(1, 21.5, 36.0),
                1_200
            ),
            LoopMonitorSample::Available(_)
        ),
        "a restarted runtime starts a new observation, so its lower scan is not backwards"
    );
}

#[test]
fn signal_identifiers_are_humanized_for_display() {
    assert_eq!(
        humanize_signal_identifier("door-open-indication"),
        "Door open indication"
    );
    assert_eq!(
        humanize_signal_identifier("effective-surrounding-temperature"),
        "Effective surrounding temperature"
    );
    assert_eq!(
        humanize_signal_identifier("feed_flow_rate"),
        "Feed flow rate"
    );
    assert_eq!(humanize_signal_identifier("SETPOINT"), "Setpoint");
    assert_eq!(humanize_signal_identifier("gust"), "Gust");
    assert_eq!(humanize_signal_identifier(""), "");
    assert_eq!(
        humanize_signal_identifier("--"),
        "--",
        "an identifier with no word is kept as it is"
    );
}

#[test]
fn the_recording_flag_is_carried_by_each_sample() {
    let loop_definition = tank();
    let mut monitor = LoopMonitor::default();
    assert!(
        !available(observe(
            &mut monitor,
            &loop_definition,
            readings(1, 21.5, 36.0),
            1_000
        ))
        .recording
    );
    monitor.set_runtime_recording(true);
    assert!(
        available(observe(
            &mut monitor,
            &loop_definition,
            readings(2, 21.5, 36.0),
            1_010
        ))
        .recording
    );
}

#[test]
fn a_symbol_only_signal_is_shown_by_its_symbol_without_an_address() {
    let mut value = tank_json();
    value["signals"]["processVariable"] = json!({
        "symbol": "Main.Measurement", "iecType": "REAL", "direction": "read", "unit": "%",
        "range": { "minimum": 0.0, "maximum": 100.0 }
    });
    let loop_definition = decode(&value);
    let scan = scan_of(
        &loop_definition,
        7,
        &[("processVariable", Some(42.5), false, Quality::Good)],
    );
    let readings = coherent_readings(&loop_definition, &scan, None);
    let sample = available(observe(
        &mut LoopMonitor::default(),
        &loop_definition,
        readings,
        1_000,
    ));
    assert_eq!(sample.process_variable.symbol, "Main.Measurement");
    assert_eq!(sample.process_variable.address, None);
    assert_eq!(sample.process_variable.value, 42.5);
    let wire =
        serde_json::to_value(LoopMonitorSample::Available(sample)).expect("the sample encodes");
    assert!(wire["processVariable"].get("address").is_none(), "{wire}");
}

#[test]
fn an_unavailable_sample_has_only_the_fields_the_page_reads() {
    let wire = serde_json::to_value(unavailable_sample(
        MonitorSource::Simulation,
        "loop:tank",
        STALE_SCAN,
    ))
    .expect("the sample encodes");
    assert_eq!(
        wire,
        json!({
            "availability": "unavailable",
            "source": "Simulation",
            "loopId": "loop:tank",
            "reason": "Runtime scan is stale"
        })
    );
}

#[test]
fn an_available_sample_has_the_wire_fields_the_page_reads() {
    let sample = available(observe(
        &mut LoopMonitor::default(),
        &tank(),
        readings(41, 21.5, 36.0),
        1_000,
    ));
    let wire =
        serde_json::to_value(LoopMonitorSample::Available(sample)).expect("the sample encodes");
    assert_eq!(
        wire,
        json!({
            "availability": "available",
            "source": "Simulation",
            "loopId": "loop:tank",
            "loopRevision": "sha256:loop-1",
            "controller": { "instance": "TankController", "revision": "sha256:controller-1", "mode": "Auto" },
            "scan": 41,
            "observedAtMs": 1000,
            "setpoint": { "symbol": "LevelSP", "address": "%ML0", "value": 50.0, "unit": "%", "quality": "good", "plotted": true },
            "processVariable": { "symbol": "LevelPV", "address": "%IL0", "value": 21.5, "unit": "%", "quality": "good", "plotted": true },
            "manipulatedVariable": { "symbol": "ValveMV", "address": "%QL0", "value": 36.0, "unit": "%", "quality": "good", "plotted": true },
            "outputLimits": { "minimum": 0.0, "maximum": 100.0 },
            "rateLimits": { "upPerSecond": 20.0, "downPerSecond": 25.0 },
            "saturation": "none",
            "recording": false
        })
    );
}

#[test]
fn a_loop_without_output_limits_is_unavailable_with_its_reason() {
    let mut value = tank_json();
    value["limits"] = json!({});
    let sample = observe(
        &mut LoopMonitor::default(),
        &decode(&value),
        readings(1, 21.5, 36.0),
        1_000,
    );
    assert_eq!(reason(&sample), NO_OUTPUT_LIMITS);
}

#[test]
fn auxiliary_signals_carry_their_role_channel_and_display_name_from_the_same_scan() {
    let mut value = tank_json();
    value["signals"]["additional"] = json!([{
        "id": "feed-flow", "channel": "reactor", "role": "disturbance", "symbol": "FeedFlow", "address": "%QL8",
        "iecType": "LREAL", "direction": "write", "unit": "%", "range": { "minimum": 0.0, "maximum": 100.0 }
    }]);
    let loop_definition = decode(&value);
    let scan = scan_of(
        &loop_definition,
        9,
        &[("feed-flow", Some(27.5), false, Quality::Good)],
    );
    let readings = coherent_readings(&loop_definition, &scan, None);
    let sample = available(observe(
        &mut LoopMonitor::default(),
        &loop_definition,
        readings,
        1_000,
    ));
    let additional = sample.additional.expect("the loop has one further signal");
    assert_eq!(additional.len(), 1);
    assert_eq!(additional[0].id, "feed-flow");
    assert_eq!(additional[0].channel, "reactor");
    assert_eq!(additional[0].role, AuxiliaryRole::Disturbance);
    assert_eq!(additional[0].display_name, "Feed flow");
    assert_eq!(additional[0].address.as_deref(), Some("%QL8"));
    assert_eq!(additional[0].value, 27.5);

    let missing = scan_of(
        &loop_definition,
        10,
        &[("feed-flow", None, false, Quality::Missing)],
    );
    let readings = coherent_readings(&loop_definition, &missing, None);
    let sample = observe(
        &mut LoopMonitor::default(),
        &loop_definition,
        readings,
        1_000,
    );
    assert_eq!(
        reason(&sample),
        "FeedFlow %QL8 is missing from runtime scan 10"
    );
}

#[test]
fn an_open_loop_without_a_setpoint_is_available_with_no_setpoint() {
    let mut value = tank_json();
    value["signals"]
        .as_object_mut()
        .expect("the signals are an object")
        .remove("setpoint");
    value["topology"] = json!("open-loop");
    let loop_definition = decode(&value);
    let scan = scan_of(
        &loop_definition,
        6,
        &[
            ("processVariable", Some(21.5), false, Quality::Good),
            ("manipulatedVariable", Some(36.0), false, Quality::Good),
        ],
    );
    let readings = coherent_readings(&loop_definition, &scan, None);
    let sample = available(observe(
        &mut LoopMonitor::default(),
        &loop_definition,
        readings,
        1_000,
    ));
    assert_eq!(
        sample.setpoint, None,
        "a loop without a setpoint shows none"
    );
    assert_eq!(sample.process_variable.value, 21.5);
    assert_eq!(sample.manipulated_variable.value, 36.0);
}

#[test]
fn a_mode_symbol_declared_as_an_auxiliary_bool_signal_reads_that_signal() {
    let mut value = tank_json();
    value["controller"]["modeSignal"] = json!("P1.Manual");
    value["controller"]["modeWhenTrue"] = json!("Manual");
    value["signals"]["additional"] = json!([{
        "id": "manual", "channel": "manual", "role": "state", "symbol": "P1.Manual", "address": "%MX0.0",
        "iecType": "BOOL", "direction": "read", "unit": "", "range": { "minimum": 0.0, "maximum": 1.0 }
    }]);
    let loop_definition = decode(&value);
    let bindings = loop_inspection_bindings(&loop_definition);
    let mode_id = loop_inspection_mode_id(&loop_definition, &bindings, "P1.Manual")
        .expect("the mode is read")
        .to_string();
    assert_eq!(
        mode_id, "manual",
        "the auxiliary BOOL that declares the mode is the mode's binding"
    );
    let scan = scan_of(
        &loop_definition,
        2,
        &[("manual", Some(1.0), false, Quality::Good)],
    );
    let readings = coherent_readings(&loop_definition, &scan, Some(mode_id.as_str()));
    assert_eq!(readings.controller_mode, ControllerMode::Manual);
}
