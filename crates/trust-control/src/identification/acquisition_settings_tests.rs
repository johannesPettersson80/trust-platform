#![allow(clippy::float_cmp)] // Declared limits are compared exactly: they are typed, not computed.

use serde_json::{json, Value};

use super::*;
use crate::failure::NextAction;
use crate::loop_definition::{Direction, Topology};

/// The fixture loop of the acquisition-limits test: a manipulated variable in 0 to 100 %, a measured output in 20 to
/// 80 degC, output limits of 0 to 100, and a 20 s observation.
fn limits_loop() -> LoopDefinition {
    serde_json::from_value(json!({
        "schemaVersion": 2,
        "id": "limits",
        "name": "Limits",
        "revision": "sha256:limits",
        "projectId": "test",
        "runtimeId": "test",
        "controller": {
            "instance": "P1.Controller",
            "type": "Control.PID",
            "revision": "sha256:controller",
            "action": "direct",
            "taskPeriodMs": 100.0
        },
        "observedSampleTiming": { "nominalPeriodMs": 100.0 },
        "signals": {
            "manipulatedVariable": {
                "symbol": "P1.MV", "address": "%QW0", "iecType": "REAL", "direction": "write",
                "unit": "%", "range": { "minimum": 0.0, "maximum": 100.0 }
            },
            "processVariable": {
                "symbol": "P1.PV", "address": "%IW0", "iecType": "REAL", "direction": "read",
                "unit": "degC", "range": { "minimum": 20.0, "maximum": 80.0 }
            }
        },
        "limits": { "outputMinimum": 0.0, "outputMaximum": 100.0 },
        "experiment": {
            "execution": "control-studio", "kind": "observation", "amplitude": 10.0,
            "durationSeconds": 20.0, "seed": 1
        }
    }))
    .expect("the fixture loop decodes")
}

#[allow(clippy::needless_pass_by_value)] // json! only borrows its arguments.
fn settings_with(method: &str, ranges: Value) -> Value {
    let mut settings = json!({
        "method": method,
        "durationSeconds": 12.0,
        "amplitude": 0.0,
        "reuseRecordings": true,
    });
    if let (Some(object), Some(extra)) = (settings.as_object_mut(), ranges.as_object()) {
        for (key, value) in extra {
            object.insert(key.clone(), value.clone());
        }
    }
    settings
}

#[allow(clippy::needless_pass_by_value)] // json! only borrows its arguments.
fn automatic_settings(input: Value, output: Value) -> Value {
    settings_with(
        "automatic",
        json!({ "inputRange": input, "outputRange": output }),
    )
}

#[test]
fn the_native_experiment_is_constrained_to_the_reviewed_ranges_and_the_source_loop_is_not_changed()
{
    let source = limits_loop();
    let settings = decode_acquisition_settings(&automatic_settings(
        json!({ "minimum": 10.0, "maximum": 70.0 }),
        json!({ "minimum": 25.0, "maximum": 60.0 }),
    ))
    .expect("the settings decode");
    let bounded =
        configured_acquisition(&source, Some(&settings)).expect("the ranges are within the limits");
    assert_eq!(
        bounded.signals.manipulated_variable.range,
        Range {
            minimum: 10.0,
            maximum: 70.0
        }
    );
    assert_eq!(
        bounded.signals.process_variable.range,
        Range {
            minimum: 25.0,
            maximum: 60.0
        }
    );
    assert_eq!(bounded.limits.output_minimum, Some(10.0));
    assert_eq!(bounded.limits.output_maximum, Some(70.0));
    assert_eq!(
        source.signals.manipulated_variable.range,
        Range {
            minimum: 0.0,
            maximum: 100.0
        },
        "a study limit must not change the project's limits"
    );
}

#[test]
fn a_plant_input_limit_the_loop_does_not_set_is_not_filled_in_from_the_input_range() {
    let mut source = limits_loop();
    source.limits.output_maximum = None;
    let settings = decode_acquisition_settings(&automatic_settings(
        json!({ "minimum": 10.0, "maximum": 70.0 }),
        json!({ "minimum": 25.0, "maximum": 60.0 }),
    ))
    .expect("the settings decode");
    let bounded = configured_acquisition(&source, Some(&settings))
        .expect("the ranges are within the declared ones");
    assert_eq!(bounded.limits.output_minimum, Some(10.0));
    assert_eq!(
        bounded.limits.output_maximum, None,
        "the missing limit stays missing, so Identify names it (B3.4 step 1)"
    );
}

#[test]
fn a_range_outside_the_configured_plant_limits_is_refused_and_named() {
    let below = decode_acquisition_settings(&automatic_settings(
        json!({ "minimum": -1.0, "maximum": 100.0 }),
        json!({ "minimum": 25.0, "maximum": 60.0 }),
    ))
    .expect("the settings decode");
    let error = configured_acquisition(&limits_loop(), Some(&below))
        .expect_err("the input is below its limit");
    assert!(
        matches!(error, AcquisitionError::RangeOutsideLimits { ref signal, .. } if signal == "P1.MV")
    );

    let above = decode_acquisition_settings(&automatic_settings(
        json!({ "minimum": 10.0, "maximum": 70.0 }),
        json!({ "minimum": 20.0, "maximum": 90.0 }),
    ))
    .expect("the settings decode");
    let error = configured_acquisition(&limits_loop(), Some(&above))
        .expect_err("the output is above its limit");
    assert!(
        matches!(error, AcquisitionError::RangeOutsideLimits { ref signal, .. } if signal == "P1.PV")
    );
    assert_eq!(
        error.failure().next_action,
        NextAction::OpenLimits,
        "a range outside its limits is fixed in Limits"
    );
}

#[test]
fn a_reversed_range_or_a_missing_bound_is_refused_before_planning() {
    assert!(
        decode_acquisition_settings(&automatic_settings(
            json!({ "minimum": 50.0, "maximum": 40.0 }),
            Value::Null,
        ))
        .is_none(),
        "a reversed range"
    );
    assert!(
        decode_acquisition_settings(&automatic_settings(
            json!({ "minimum": 0.0, "maximum": Value::Null }),
            Value::Null,
        ))
        .is_none(),
        "a bound that is not a number: JSON cannot carry an infinite bound"
    );
}

#[test]
fn the_duration_must_be_between_one_second_and_twenty_four_hours() {
    let with_duration = |seconds: f64| {
        decode_acquisition_settings(&json!({
            "method": "automatic", "durationSeconds": seconds, "amplitude": 0.0, "reuseRecordings": true
        }))
    };
    assert!(with_duration(0.5).is_none());
    assert!(with_duration(86_401.0).is_none());
    assert!(with_duration(86_400.0).is_some());
    assert!(with_duration(1.0).is_some());
}

#[test]
fn an_experiment_without_a_declared_duration_is_left_to_the_precondition_that_names_it() {
    let mut source = limits_loop();
    source.experiment = None;
    assert_eq!(
        configured_acquisition(&source, None).expect("the campaign's precondition names the field"),
        source,
        "a missing duration is not refused here (B3.4 step 1)"
    );
    let mut zero = limits_loop();
    if let Some(experiment) = zero.experiment.as_mut() {
        experiment.duration_seconds = Some(0.0);
    }
    let error = configured_acquisition(&zero, None).expect_err("a zero duration is not a duration");
    assert_eq!(error, AcquisitionError::LimitsRequired);
    let failure = error.failure();
    assert_eq!(
        failure.message,
        "Set the maximum experiment duration in Limits before identification."
    );
    assert_eq!(failure.next_action, NextAction::OpenLimits);
    assert_eq!(
        failure.field.as_deref(),
        Some("durationSeconds"),
        "the missing limit is the duration, so the Limits editor marks that field (B3.2, B8)"
    );
    assert!(
        configured_acquisition(&limits_loop(), None).is_ok(),
        "a declared duration needs no settings"
    );
}

#[test]
fn a_relay_needs_its_first_direction() {
    let relay_without_direction = settings_with("relay", json!({}));
    assert!(decode_acquisition_settings(&relay_without_direction).is_none());
    let relay = settings_with("relay", json!({ "relayDirection": "positive" }));
    assert!(decode_acquisition_settings(&relay).is_some());
}

#[test]
fn a_refused_setting_names_the_first_rule_it_breaks() {
    // A relay without its direction names the sentence that the method's applicability gives (B3.2).
    let relay = settings_with("relay", json!({}));
    assert_eq!(
        check_acquisition_settings(&relay)
            .expect_err("a relay needs its direction")
            .message,
        method_refusal_text(MethodRefusal::RelayDirection)
    );
    // One other rule names its own sentence.
    let short = settings_with("automatic", json!({ "durationSeconds": 0.5 }));
    assert_eq!(
        check_acquisition_settings(&short)
            .expect_err("a short duration")
            .message,
        DURATION_RULE
    );
    // The rules are checked in order: the duration comes before the relay's direction.
    let both = settings_with("relay", json!({ "durationSeconds": 0.5 }));
    assert_eq!(
        check_acquisition_settings(&both)
            .expect_err("two rules")
            .message,
        DURATION_RULE
    );
    let reversed = automatic_settings(json!({ "minimum": 50.0, "maximum": 40.0 }), Value::Null);
    assert_eq!(
        check_acquisition_settings(&reversed)
            .expect_err("a reversed range")
            .message,
        "The input range must be finite, with its minimum below its maximum."
    );
    // JSON that does not decode is not Identify's settings at all.
    let malformed = json!({ "method": "nonsense" });
    assert_eq!(
        check_acquisition_settings(&malformed)
            .expect_err("not Identify's settings")
            .message,
        NOT_IDENTIFY_SETTINGS
    );
}

#[test]
fn a_typed_duration_reads_as_the_seconds_it_names() {
    let typed = duration_of(json!("1 h 30 min")).expect("a typed duration is read");
    let number = duration_of(json!(5400.0)).expect("a number of seconds is read");
    assert_eq!(typed.duration_seconds, 5400.0);
    assert_eq!(
        typed, number,
        "a typed duration is the settings of its seconds"
    );
}

#[test]
fn a_time_that_no_duration_reads_is_refused_under_the_duration_field() {
    for unreadable in [
        json!("12 m"),
        json!("soon"),
        json!(""),
        json!(null),
        json!(true),
    ] {
        assert_eq!(
            duration_of(unreadable)
                .expect_err("no duration reads")
                .message,
            DURATION_TEXT
        );
    }
    let refusal = unreadable_duration_failure();
    assert_eq!(refusal.message, DURATION_TEXT);
    assert_eq!(refusal.field.as_deref(), Some("durationSeconds"));
    assert_eq!(refusal.next_action, NextAction::OpenLimits);
}

/// The settings of an automatic experiment whose duration is `duration`, checked as a page's request is.
fn duration_of(duration: Value) -> Result<AcquisitionSettings, Failure> {
    let mut settings = settings_with("automatic", json!({}));
    settings["durationSeconds"] = duration;
    check_acquisition_settings(&settings)
}

/// B3.2: a request that names no reuse flag asks for no reuse, and the record the study keeps omits the flag: the page encodes no
/// `reuseRecordings`. Catches a decoder that requires the flag, and a record that keeps `reuseRecordings: false` as a second form. A
/// record of the earlier form, which names the flag, still reads.
#[test]
fn a_request_without_the_reuse_flag_asks_for_no_reuse_and_the_record_omits_it() {
    let settings = typed_settings(&json!({
        "method": "automatic", "durationSeconds": 60, "amplitude": 0
    }));
    assert!(!settings.reuse_recordings);
    let value = serde_json::to_value(&settings).expect("the settings serialize");
    assert!(value.get("reuseRecordings").is_none(), "{value}");
    let earlier = typed_settings(&json!({
        "method": "automatic", "durationSeconds": 60, "amplitude": 0, "reuseRecordings": true
    }));
    assert!(earlier.reuse_recordings);
}

/// The settings a page's request decodes to, without the checks of `check_acquisition_settings`: a range may break the loop's declared
/// limits, and the loop's own check (`range_refusal`) names it.
fn typed_settings(value: &Value) -> AcquisitionSettings {
    crate::json::decode_value(value.clone()).expect("the settings decode")
}

#[test]
fn a_range_beyond_the_declared_limits_names_the_field_of_the_bound_it_breaks() {
    let field = |input: Value, output: Value| {
        configured_acquisition(
            &limits_loop(),
            Some(&typed_settings(&automatic_settings(input, output))),
        )
        .expect_err("the range is beyond the declared limits")
        .failure()
        .field
    };
    // The manipulated variable is declared 0 to 100 %: a maximum above it, or a minimum below it, names the bound it breaks.
    assert_eq!(
        field(json!({ "minimum": 0.0, "maximum": 150.0 }), Value::Null).as_deref(),
        Some("inputMaximum")
    );
    assert_eq!(
        field(json!({ "minimum": -1.0, "maximum": 50.0 }), Value::Null).as_deref(),
        Some("inputMinimum")
    );
    // The measured output is declared 20 to 80 degC.
    assert_eq!(
        field(Value::Null, json!({ "minimum": 25.0, "maximum": 90.0 })).as_deref(),
        Some("outputMaximum")
    );
    assert_eq!(
        field(Value::Null, json!({ "minimum": 10.0, "maximum": 60.0 })).as_deref(),
        Some("outputMinimum")
    );
    // A reversed range breaks its maximum, the field the Limits editor marks for it, and its sentence says that it is reversed.
    assert_eq!(
        field(json!({ "minimum": 50.0, "maximum": 40.0 }), Value::Null).as_deref(),
        Some("inputMaximum")
    );
    let reversed = configured_acquisition(
        &limits_loop(),
        Some(&typed_settings(&automatic_settings(
            json!({ "minimum": 50.0, "maximum": 40.0 }),
            Value::Null,
        ))),
    )
    .expect_err("a reversed range");
    assert_eq!(
        reversed.failure().message,
        "The range of P1.MV must be finite, with its minimum below its maximum."
    );
}

#[test]
fn the_loop_check_refuses_a_range_beyond_its_limits_and_leaves_one_within_them() {
    let refusal = range_refusal(
        &limits_loop(),
        &typed_settings(&automatic_settings(
            json!({ "minimum": 0.0, "maximum": 150.0 }),
            Value::Null,
        )),
    )
    .expect("the range is beyond the declared limits");
    assert_eq!(refusal.field.as_deref(), Some("inputMaximum"));
    assert_eq!(refusal.next_action, NextAction::OpenLimits);
    let within = typed_settings(&automatic_settings(
        json!({ "minimum": 10.0, "maximum": 70.0 }),
        Value::Null,
    ));
    assert!(
        range_refusal(&limits_loop(), &within).is_none(),
        "a range within the declared limits is not refused"
    );
}

#[test]
fn the_settings_check_names_the_field_of_a_refused_duration_or_range() {
    let refusal = |settings: &Value| {
        check_acquisition_settings(settings).expect_err("the settings are refused")
    };
    let duration = refusal(&settings_with(
        "automatic",
        json!({ "durationSeconds": 0.5 }),
    ));
    assert_eq!(duration.field.as_deref(), Some("durationSeconds"));
    assert_eq!(duration.next_action, NextAction::OpenLimits);
    let input = refusal(&automatic_settings(
        json!({ "minimum": 50.0, "maximum": 40.0 }),
        Value::Null,
    ));
    assert_eq!(input.field.as_deref(), Some("inputMaximum"));
    assert_eq!(input.next_action, NextAction::OpenLimits);
    let output = refusal(&automatic_settings(
        Value::Null,
        json!({ "minimum": 50.0, "maximum": 40.0 }),
    ));
    assert_eq!(output.field.as_deref(), Some("outputMaximum"));
    // A map of ranges comes from a loop with several channels, so its field names the signal.
    let mapped = refusal(&settings_with(
        "automatic",
        json!({ "inputRanges": { "P1.MV": { "minimum": 50.0, "maximum": 40.0 } } }),
    ));
    assert_eq!(mapped.field.as_deref(), Some("inputMaximum:P1.MV"));
    assert_eq!(mapped.next_action, NextAction::OpenLimits);
}

// Specification 35, B3.2: the Limits editor sends each bound as the text the engineer typed. The engine reads it as the number it names,
// the study keeps the number, and a bound that no number reads (blank text included) is refused under the field of the range's maximum,
// as a duration that no time reads is refused under its own field.
#[test]
fn a_range_bound_typed_as_text_reads_as_a_number_and_an_unreadable_one_is_refused_under_its_field()
{
    let settings = check_acquisition_settings(&automatic_settings(
        json!({ "minimum": " 10 ", "maximum": "70.5" }),
        Value::Null,
    ))
    .expect("the typed bounds read");
    assert_eq!(
        settings.input_range,
        Some(RangeSetting {
            minimum: 10.0,
            maximum: 70.5
        })
    );
    assert_eq!(
        serde_json::to_value(settings).expect("the settings serialize")["inputRange"],
        json!({ "minimum": 10.0, "maximum": 70.5 }),
        "the study keeps the numbers"
    );
    for unreadable in [
        json!(""),
        json!("   "),
        json!("abc"),
        json!("1e999"),
        Value::Null,
        json!([1]),
    ] {
        let refusal = check_acquisition_settings(&automatic_settings(
            json!({ "minimum": unreadable, "maximum": 70.0 }),
            Value::Null,
        ))
        .expect_err("a bound that no number reads");
        assert_eq!(
            refusal.field.as_deref(),
            Some("inputMaximum"),
            "{unreadable}"
        );
        assert_eq!(refusal.next_action, NextAction::OpenLimits);
    }
    // The ranges of a loop with several channels read the same way, and their field names the signal.
    let mapped = check_acquisition_settings(&settings_with(
        "automatic",
        json!({ "outputRanges": { "P1.PV": { "minimum": "20", "maximum": "" } } }),
    ))
    .expect_err("a blank maximum");
    assert_eq!(mapped.field.as_deref(), Some("outputMaximum:P1.PV"));
    // The loop's own check names an unreadable bound as it names any range that is not valid.
    let blank = range_refusal(
        &limits_loop(),
        &typed_settings(&automatic_settings(
            json!({ "minimum": "", "maximum": 50.0 }),
            Value::Null,
        )),
    )
    .expect("a blank minimum is not a valid range");
    assert_eq!(blank.field.as_deref(), Some("inputMaximum"));
}

#[test]
fn a_range_field_carries_its_signal_only_on_a_loop_with_several_channels() {
    assert_eq!(
        limit_field(ChannelDirection::Input, Bound::Minimum, None),
        "inputMinimum"
    );
    assert_eq!(
        limit_field(ChannelDirection::Output, Bound::Maximum, None),
        "outputMaximum"
    );
    assert_eq!(
        limit_field(ChannelDirection::Input, Bound::Maximum, Some("P1.MV")),
        "inputMaximum:P1.MV"
    );
}

/// The open loop of the setpoint test: no setpoint of its own, and a declared further signal that is a setpoint in
/// the measured output's unit.
fn open_loop() -> LoopDefinition {
    let mut loop_definition = limits_loop();
    loop_definition.id = "limits-reference-selection".to_string();
    loop_definition.topology = Some(Topology::OpenLoop);
    loop_definition.signals.setpoint = None;
    loop_definition.signals.additional = Some(vec![AuxiliarySignal {
        id: "additional-reference".to_string(),
        channel: "primary".to_string(),
        role: AuxiliaryRole::Setpoint,
        symbol: "Main.Reference".to_string(),
        address: Some("%ML64".to_string()),
        iec_type: "LREAL".to_string(),
        direction: Direction::Read,
        unit: "degC".to_string(),
        range: Range {
            minimum: 20.0,
            maximum: 80.0,
        },
    }]);
    loop_definition
}

fn setpoint_settings(id: &str) -> AcquisitionSettings {
    decode_acquisition_settings(&json!({
        "method": "setpoint_step",
        "durationSeconds": 10.0,
        "amplitude": 1.0,
        "reuseRecordings": true,
        "setpointSignalId": id,
    }))
    .expect("the settings decode")
}

#[test]
fn only_a_declared_reference_is_used_and_the_controller_is_not_changed() {
    let loop_definition = open_loop();
    let configured = configured_acquisition(
        &loop_definition,
        Some(&setpoint_settings("additional-reference")),
    )
    .expect("the declared reference is used");
    assert_eq!(
        configured
            .signals
            .setpoint
            .as_ref()
            .map(|signal| signal.symbol.as_str()),
        Some("Main.Reference")
    );
    assert!(
        loop_definition.signals.setpoint.is_none(),
        "the source loop keeps no setpoint"
    );
    assert_eq!(configured.controller, loop_definition.controller);

    for id in ["unknown", "manipulatedVariable"] {
        assert_eq!(
            configured_acquisition(&loop_definition, Some(&setpoint_settings(id))),
            Err(AcquisitionError::SetpointReference),
            "{id} is not a declared setpoint"
        );
    }
    let mut wrong_unit = open_loop();
    if let Some(signals) = wrong_unit.signals.additional.as_mut() {
        signals[0].unit = "wrong-unit".to_string();
    }
    assert_eq!(
        configured_acquisition(
            &wrong_unit,
            Some(&setpoint_settings("additional-reference"))
        ),
        Err(AcquisitionError::SetpointReference),
        "the reference must be in the measured output's unit"
    );
}

/// B3.3, B3.6: the Setpoint selector belongs to the setpoint step, and the page sends the signal the engineer chose whatever the method
/// (a hidden selector keeps its value). The engine uses it for the setpoint step alone: any other method runs as if none were named, so a
/// choice made for a setpoint step never gives Automatic a setpoint to step. Catches an engine that applies the signal for every method,
/// which the page once had to prevent by sending it for the setpoint step only.
#[test]
fn a_setpoint_signal_is_used_only_by_the_setpoint_step() {
    let loop_definition = open_loop();
    for method in ["configured", "automatic", "mv_step", "mv_pulse", "prbs"] {
        let settings = decode_acquisition_settings(&settings_with(
            method,
            json!({ "setpointSignalId": "additional-reference" }),
        ))
        .expect("the settings decode");
        let configured = configured_acquisition(&loop_definition, Some(&settings))
            .unwrap_or_else(|error| panic!("{method} names a setpoint it does not use: {error:?}"));
        assert!(
            configured.signals.setpoint.is_none(),
            "{method} runs as if no setpoint were named"
        );
    }
    // The setpoint step uses it, and an id that names no declared setpoint is still refused for that method.
    let step = configured_acquisition(
        &loop_definition,
        Some(&setpoint_settings("additional-reference")),
    )
    .expect("the setpoint step uses the declared reference");
    assert_eq!(
        step.signals
            .setpoint
            .as_ref()
            .map(|signal| signal.symbol.as_str()),
        Some("Main.Reference")
    );
    assert_eq!(
        configured_acquisition(&loop_definition, Some(&setpoint_settings("unknown"))),
        Err(AcquisitionError::SetpointReference)
    );
}

#[test]
fn a_configured_method_keeps_the_loop_kind_and_an_automatic_one_observes() {
    let mut loop_definition = limits_loop();
    if let Some(experiment) = loop_definition.experiment.as_mut() {
        experiment.kind = ExperimentKind::MvStep;
    }
    let configured = decode_acquisition_settings(&settings_with("configured", json!({})))
        .expect("the settings decode");
    let kept = configured_acquisition(&loop_definition, Some(&configured)).expect("configured");
    assert_eq!(
        kept.experiment.as_ref().map(|experiment| experiment.kind),
        Some(ExperimentKind::MvStep)
    );
    let automatic = decode_acquisition_settings(&settings_with("automatic", json!({})))
        .expect("the settings decode");
    let observed = configured_acquisition(&loop_definition, Some(&automatic)).expect("automatic");
    assert_eq!(
        observed
            .experiment
            .as_ref()
            .map(|experiment| experiment.kind),
        Some(ExperimentKind::Observation)
    );
}

#[test]
fn a_further_signal_takes_its_requested_range_within_its_declared_one() {
    // The further-signal range of the measured-output companion, as `control-studio-acquisition-limits.test.ts` (its
    // multichannel test) asserts it. That test's manipulated-variable companion is checked through its channel actuator,
    // which belongs to stage V7, because channels are parked.
    let mut source = limits_loop();
    source.signals.additional = Some(vec![
        AuxiliarySignal {
            id: "manipulatedVariable2".to_string(),
            channel: "input-2".to_string(),
            role: AuxiliaryRole::ManipulatedVariable,
            symbol: "P1.MV2".to_string(),
            address: Some("%QW1".to_string()),
            iec_type: "REAL".to_string(),
            direction: Direction::Write,
            unit: "%".to_string(),
            range: Range {
                minimum: 10.0,
                maximum: 90.0,
            },
        },
        AuxiliarySignal {
            id: "processVariable2".to_string(),
            channel: "output-2".to_string(),
            role: AuxiliaryRole::ProcessVariable,
            symbol: "P1.PV2".to_string(),
            address: Some("%IW1".to_string()),
            iec_type: "REAL".to_string(),
            direction: Direction::Read,
            unit: "degC".to_string(),
            range: Range {
                minimum: 25.0,
                maximum: 75.0,
            },
        },
    ]);
    let settings = decode_acquisition_settings(&settings_with(
        "automatic",
        json!({
            "inputRange": { "minimum": 20.0, "maximum": 70.0 },
            "outputRange": { "minimum": 35.0, "maximum": 65.0 },
            "inputRanges": {
                "P1.MV": { "minimum": 20.0, "maximum": 70.0 },
                "P1.MV2": { "minimum": 25.0, "maximum": 65.0 }
            },
            "outputRanges": {
                "P1.PV": { "minimum": 35.0, "maximum": 65.0 },
                "P1.PV2": { "minimum": 40.0, "maximum": 60.0 }
            }
        }),
    ))
    .expect("the settings decode");
    let bounded = configured_acquisition(&source, Some(&settings))
        .expect("the further ranges are within their declared ranges");
    let further = bounded.signals.additional.as_deref().unwrap_or_default();
    assert_eq!(
        further[1].range,
        Range {
            minimum: 40.0,
            maximum: 60.0
        }
    );
}

// Specification 35, §0.1b and B3.2: the command line starts from the loop's declared limits.
#[test]
fn the_declared_settings_use_the_loops_maximum_duration_and_the_declared_amplitude() {
    let example = LoopDefinition::decode(include_str!(
        "../../../../examples/control-pid-commissioning/control-loop.json"
    ))
    .expect("the example loop decodes");
    let settings = declared_settings(&example, AcquisitionMethod::MvStep);
    assert_eq!(
        settings,
        AcquisitionSettings {
            method: AcquisitionMethod::MvStep,
            duration_seconds: 16.0,
            amplitude: 0.0,
            setpoint_signal_id: None,
            relay_direction: None,
            reuse_recordings: false,
            input_range: None,
            output_range: None,
            input_ranges: None,
            output_ranges: None,
        },
        "the example declares a 16 s experiment"
    );
    assert_eq!(
        configured_acquisition(&example, Some(&settings))
            .expect("the declared settings configure the loop")
            .experiment
            .map(|experiment| (experiment.kind, experiment.duration_seconds)),
        Some((ExperimentKind::MvStep, Some(16.0)))
    );
    let mut undeclared = example;
    undeclared.experiment = None;
    assert_eq!(
        declared_settings(&undeclared, AcquisitionMethod::Automatic).duration_seconds,
        0.0,
        "no declared duration"
    );

    // The declared amplitude and relay direction are kept, so a relay passes the settings' checks.
    let mut relay_loop = limits_loop();
    if let Some(experiment) = relay_loop.experiment.as_mut() {
        experiment.relay_direction = Some(RelayDirection::Positive);
    }
    let relay = declared_settings(&relay_loop, AcquisitionMethod::Relay);
    assert_eq!(
        (
            relay.duration_seconds,
            relay.amplitude,
            relay.relay_direction
        ),
        (20.0, 10.0, Some(RelayDirection::Positive))
    );
    let value = serde_json::to_value(&relay).expect("the settings serialize");
    assert_eq!(decode_acquisition_settings(&value), Some(relay));
}

#[test]
fn the_duration_of_the_settings_is_declared_as_a_number_or_the_text_the_page_types() {
    // The page sends the time it typed (`15 min`), which `duration_of_setting` reads; the generated type of the field is `number | string`.
    let schema = schemars::SchemaGenerator::default().into_root_schema_for::<AcquisitionSettings>();
    let schema = serde_json::to_value(&schema).expect("the schema is JSON");
    assert_eq!(
        schema["properties"]["durationSeconds"]["anyOf"],
        json!([{ "type": "number" }, { "type": "string" }])
    );
    let typed: AcquisitionSettings = serde_json::from_value(json!({
        "method": "automatic", "durationSeconds": "15 min", "amplitude": 0.0, "reuseRecordings": false
    }))
    .expect("the typed time decodes");
    assert_eq!(typed.duration_seconds, 900.0);
}
