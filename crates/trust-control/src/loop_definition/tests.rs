//! Specification 35, §17.1 and A5: the loop contract, decoded and checked before runtime access.

use serde_json::{json, Value};

use super::checks::{addresses_overlap, parse_address};
use super::*;

fn decode(value: &Value) -> Result<LoopDefinition, LoopRejection> {
    LoopDefinition::decode(&value.to_string())
}

fn scope(value: &Value) -> LoopScope {
    LoopScope {
        project_id: value["projectId"].as_str().unwrap_or_default().into(),
        runtime_id: value["runtimeId"].as_str().unwrap_or_default().into(),
    }
}

fn validate(value: &Value) -> Result<(), LoopRejection> {
    decode(value)?.validate(&scope(value))
}

/// A complete SISO Control.PID loop with its parameter record.
fn complete_loop() -> Value {
    json!({
        "schemaVersion": 2,
        "id": "loop:tank",
        "name": "Tank level",
        "revision": "sha256:loop-1",
        "projectId": "project:sha256:abc",
        "runtimeId": "runtime:local",
        "controller": {
            "instance": "Main.TankController", "type": "Control.PID", "revision": "sha256:controller-1",
            "action": "direct", "taskPeriodMs": 10, "modeSignal": "%MX0.0", "trackingSignal": "%ML24",
            "law": { "kind": "pid", "parameterRecordSymbol": "Main.Parameters", "proportionalGainSymbol": "Main.Parameters.Kp",
                "integralGainSymbol": "Main.Parameters.Ki", "derivativeGainSymbol": "Main.Parameters.Kd" }
        },
        "parameterBinding": {
            "loop_id": "loop:tank", "task_name": "MainTask",
            "parameters": [
                { "symbol": "Main.Parameters.Kp", "iec_type": "LREAL", "unit": "%/%", "convention": "proportional gain", "minimum": 0, "maximum": 100 },
                { "symbol": "Main.Parameters.Ki", "iec_type": "LREAL", "unit": "%/(%*s)", "convention": "integral gain", "minimum": 0, "maximum": 100 },
                { "symbol": "Main.Parameters.Kd", "iec_type": "LREAL", "unit": "%*s/%", "convention": "derivative gain", "minimum": 0, "maximum": 100 }
            ],
            "mode": { "symbol": "Main.Auto", "manual_value": false }
        },
        "signals": {
            "setpoint": { "symbol": "LevelSP", "address": "%ML0", "iecType": "LREAL", "direction": "read", "unit": "%", "range": { "minimum": 0, "maximum": 100 } },
            "processVariable": { "symbol": "LevelPV", "address": "%IL0", "iecType": "LREAL", "direction": "read", "unit": "%", "range": { "minimum": 0, "maximum": 100 } },
            "manipulatedVariable": { "symbol": "ValveMV", "address": "%QL0", "iecType": "LREAL", "direction": "write", "unit": "%", "range": { "minimum": 0, "maximum": 100 } },
            "actuator": { "symbol": "ValvePosition", "address": "%IL8", "iecType": "LREAL", "direction": "read", "unit": "%", "range": { "minimum": 0, "maximum": 100 } }
        },
        "limits": { "outputMinimum": 0, "outputMaximum": 100, "rateUpPerS": 20, "rateDownPerS": 25 },
        "observedSampleTiming": { "nominalPeriodMs": 10, "maximumJitterMs": 1 },
        "plantId": "tank",
        "operatingPoint": "Normal production"
    })
}

fn with(mut value: Value, path: &[&str], replacement: Value) -> Value {
    let mut target = &mut value;
    for key in &path[..path.len() - 1] {
        target = &mut target[*key];
    }
    target[path[path.len() - 1]] = replacement;
    value
}

fn auxiliary(
    id: &str,
    channel: &str,
    role: &str,
    address: &str,
    unit: &str,
    direction: &str,
) -> Value {
    json!({ "id": id, "channel": channel, "role": role, "symbol": id, "address": address, "iecType": "LREAL",
        "direction": direction, "unit": unit, "range": { "minimum": -1000, "maximum": 1000 } })
}

#[test]
fn accepts_the_complete_siso_control_pid_loop_without_losing_its_engineering_identity() {
    let value = complete_loop();
    let loop_definition = decode(&value).expect("accepted");
    assert_eq!(loop_definition.controller.instance, "Main.TankController");
    assert_eq!(
        loop_definition.signals.manipulated_variable.direction,
        Direction::Write
    );
    assert_eq!(loop_definition.limits.rate_down_per_s, Some(25.0));
    assert_eq!(
        loop_definition.operating_point.as_deref(),
        Some("Normal production")
    );
    let written = serde_json::to_string(&loop_definition).expect("json");
    assert_eq!(
        LoopDefinition::decode(&written).expect("read back"),
        loop_definition,
        "it writes back what it read"
    );
    loop_definition.validate(&scope(&value)).expect("valid");
    loop_definition
        .rust_engine_support()
        .expect("the Rust engine runs it");
}

#[test]
fn only_the_current_schema_version_is_read_and_unknown_keys_are_refused() {
    let earlier = with(complete_loop(), &["schemaVersion"], json!(1));
    assert_eq!(decode(&earlier), Err(LoopRejection::UnsupportedVersion(1)));
    let legacy = with(
        complete_loop(),
        &["controller", "saturationEvidence"],
        json!({}),
    );
    assert!(
        matches!(decode(&legacy), Err(LoopRejection::Malformed(_))),
        "a removed key is not accepted"
    );
}

#[test]
fn validates_the_runtime_scope_directions_numeric_types_and_units() {
    let value = complete_loop();
    let loop_definition = decode(&value).expect("accepted");
    let other = LoopScope {
        runtime_id: "runtime:other".into(),
        ..scope(&value)
    };
    assert_eq!(
        loop_definition.validate(&other),
        Err(LoopRejection::CrossRuntime)
    );
    let written_setpoint = with(
        complete_loop(),
        &["signals", "setpoint", "direction"],
        json!("write"),
    );
    assert_eq!(
        validate(&written_setpoint),
        Err(LoopRejection::InvalidSignalDirection("setpoint".into()))
    );
    let boolean_measurement = with(
        complete_loop(),
        &["signals", "processVariable", "iecType"],
        json!("BOOL"),
    );
    assert_eq!(
        validate(&boolean_measurement),
        Err(LoopRejection::IncompatibleSignalType(
            "processVariable".into()
        ))
    );
    let other_unit = with(
        complete_loop(),
        &["signals", "processVariable", "unit"],
        json!("degC"),
    );
    assert_eq!(validate(&other_unit), Err(LoopRejection::IncompatibleUnits));
}

#[test]
fn a_pid_designed_loop_needs_control_pid_and_an_open_loop_a_project_design() {
    let other_type = with(
        complete_loop(),
        &["controller", "type"],
        json!("Vendor.PID"),
    );
    assert_eq!(
        validate(&other_type),
        Err(LoopRejection::IncompatibleControllerDesign)
    );
    let mut open = with(complete_loop(), &["topology"], json!("open-loop"));
    open["signals"]
        .as_object_mut()
        .expect("signals")
        .remove("setpoint");
    assert_eq!(validate(&open), Err(LoopRejection::IncompatibleTopology));
}

#[test]
fn a_closed_loop_needs_a_setpoint_and_an_open_loop_has_none() {
    let mut without = complete_loop();
    without["signals"]
        .as_object_mut()
        .expect("signals")
        .remove("setpoint");
    assert!(matches!(decode(&without), Err(LoopRejection::Malformed(_))));
    let open_with_setpoint = with(complete_loop(), &["topology"], json!("open-loop"));
    assert!(matches!(
        decode(&open_with_setpoint),
        Err(LoopRejection::Malformed(_))
    ));
}

#[test]
fn accepts_bounded_experiment_defaults_and_rejects_ambiguous_scripted_excitation() {
    let configured = with(
        complete_loop(),
        &["experiment"],
        json!({
        "execution": "control-studio", "kind": "setpoint_step", "amplitude": 5, "durationSeconds": 120, "seed": 20_260_824 }),
    );
    decode(&configured).expect("accepted");
    let scripted = with(
        complete_loop(),
        &["experiment"],
        json!({
        "execution": "project-scripted", "kind": "load_step", "amplitude": 5, "durationSeconds": 120, "seed": 20_260_824 }),
    );
    assert!(matches!(
        decode(&scripted),
        Err(LoopRejection::Malformed(_))
    ));
    let observation_with_amplitude = with(
        complete_loop(),
        &["experiment"],
        json!({
        "execution": "control-studio", "kind": "observation", "amplitude": 1, "durationSeconds": 120, "seed": 1 }),
    );
    assert!(matches!(
        decode(&observation_with_amplitude),
        Err(LoopRejection::Malformed(_))
    ));
    let too_long = with(
        complete_loop(),
        &["experiment"],
        json!({
        "execution": "control-studio", "kind": "mv_step", "amplitude": 5, "durationSeconds": 86_401, "seed": 1 }),
    );
    assert!(
        matches!(decode(&too_long), Err(LoopRejection::Malformed(_))),
        "an experiment lasts at most 24 h (A7)"
    );
}

#[test]
fn a_project_experiment_reads_every_signal_and_control_studio_writes_the_output() {
    let scripted = |value: Value| {
        with(
            value,
            &["experiment"],
            json!({
        "execution": "project-scripted", "kind": "observation", "amplitude": 0, "durationSeconds": 60, "seed": 1 }),
        )
    };
    let reading = scripted(with(
        complete_loop(),
        &["signals", "manipulatedVariable", "direction"],
        json!("read"),
    ));
    validate(&reading).expect("an observation-only project experiment needs no activation");
    let writing = scripted(complete_loop());
    assert_eq!(
        validate(&writing),
        Err(LoopRejection::InvalidSignalDirection(
            "manipulatedVariable".into()
        ))
    );
    let auxiliary_write = with(
        reading.clone(),
        &["signals", "additional"],
        json!([auxiliary(
            "load",
            "zone",
            "disturbance",
            "%QL32",
            "%",
            "write"
        )]),
    );
    assert_eq!(
        validate(&auxiliary_write),
        Err(LoopRejection::InvalidSignalDirection("additional".into()))
    );
    let read_output = with(
        complete_loop(),
        &["signals", "manipulatedVariable", "direction"],
        json!("read"),
    );
    assert_eq!(
        validate(&read_output),
        Err(LoopRejection::InvalidSignalDirection(
            "manipulatedVariable".into()
        )),
        "a Control Studio experiment keeps the output writable"
    );
}

#[test]
fn a_signal_without_a_unit_is_dimensionless() {
    let value = with(
        with(complete_loop(), &["signals", "setpoint", "unit"], json!("")),
        &["signals", "processVariable", "unit"],
        json!(" "),
    );
    let loop_definition = decode(&value).expect("accepted");
    assert_eq!(loop_definition.signals.process_variable.unit, "1");
    assert_eq!(
        loop_definition
            .signals
            .setpoint
            .as_ref()
            .map(|signal| signal.unit.as_str()),
        Some("1")
    );
}

#[test]
fn rejects_overlapping_addresses_and_duplicate_identities_inside_one_channel() {
    let overlapping = with(
        complete_loop(),
        &["signals", "additional"],
        json!([auxiliary(
            "corrupt-pv",
            "zone-b",
            "PV",
            "%IL1",
            "degC",
            "read"
        )]),
    );
    assert_eq!(
        validate(&overlapping),
        Err(LoopRejection::AmbiguousSignalAddress(
            "processVariable".into(),
            "corrupt-pv".into()
        ))
    );
    let duplicate_role = with(
        complete_loop(),
        &["signals", "additional"],
        json!([
            auxiliary("zone-b-pv-1", "zone-b", "PV", "%IL32", "degC", "read"),
            auxiliary("zone-b-pv-2", "zone-b", "PV", "%IL40", "degC", "read")
        ]),
    );
    assert_eq!(
        validate(&duplicate_role),
        Err(LoopRejection::DuplicateSignalRole("zone-b-pv-2".into()))
    );
    let reserved = with(
        complete_loop(),
        &["signals", "additional"],
        json!([auxiliary(
            "processVariable",
            "zone-b",
            "PV",
            "%IL32",
            "degC",
            "read"
        )]),
    );
    assert_eq!(
        validate(&reserved),
        Err(LoopRejection::DuplicateSignalId("processVariable".into()))
    );
    let channel_units = with(
        complete_loop(),
        &["signals", "additional"],
        json!([
            auxiliary("b-sp", "zone-b", "SP", "%ML64", "degC", "read"),
            auxiliary("b-pv", "zone-b", "PV", "%IL64", "K", "read")
        ]),
    );
    assert_eq!(
        validate(&channel_units),
        Err(LoopRejection::IncompatibleAuxiliaryUnits("zone-b".into()))
    );
}

#[test]
fn a_bit_address_overlaps_only_its_byte_and_a_bit_number_on_a_wide_address_is_ignored() {
    assert!(addresses_overlap("%IX0.3", "%IB0"));
    assert!(!addresses_overlap("%IX1.0", "%IB0"));
    assert!(
        !addresses_overlap("%IL0", "%QL0"),
        "different areas never overlap"
    );
    assert!(addresses_overlap("%ML0", "%MW6"));
    assert_eq!(parse_address("%IW4.1"), parse_address("%IW4"));
    assert_eq!(parse_address("%IX0.8"), None);
    assert_eq!(parse_address("LevelPV"), None);
    assert_eq!(
        parse_address("%IL2305843009213693951"),
        None,
        "an address past the last bit is refused, never wrapped"
    );
}

#[test]
fn rejects_a_non_address_binding_and_unknown_sample_timing_before_monitoring() {
    let named = with(
        complete_loop(),
        &["signals", "processVariable", "address"],
        json!("LevelPV"),
    );
    assert!(matches!(decode(&named), Err(LoopRejection::Malformed(_))));
    let no_period = with(
        complete_loop(),
        &["observedSampleTiming"],
        json!({ "nominalPeriodMs": 0 }),
    );
    assert!(matches!(
        decode(&no_period),
        Err(LoopRejection::Malformed(_))
    ));
}

#[test]
fn rejects_malformed_parameter_bindings_and_laws_before_runtime_access() {
    let other_loop = with(
        complete_loop(),
        &["parameterBinding", "loop_id"],
        json!("loop:other"),
    );
    assert!(matches!(
        decode(&other_loop),
        Err(LoopRejection::Malformed(_))
    ));
    let empty = with(
        complete_loop(),
        &["parameterBinding", "parameters"],
        json!([]),
    );
    assert!(matches!(decode(&empty), Err(LoopRejection::Malformed(_))));
    let unbound_gain = with(
        complete_loop(),
        &["controller", "law", "derivativeGainSymbol"],
        json!("Main.Parameters.Td"),
    );
    assert!(matches!(
        decode(&unbound_gain),
        Err(LoopRejection::Malformed(_))
    ));
    let same_field = with(
        complete_loop(),
        &["controller", "law", "integralGainSymbol"],
        json!("Main.Parameters.Kp"),
    );
    assert!(matches!(
        decode(&same_field),
        Err(LoopRejection::Malformed(_))
    ));
    let tracking = with(
        complete_loop(),
        &["controller", "law", "antiWindup"],
        json!({ "method": "back-calculation", "trackingGainSymbol": "Main.Parameters.Kd" }),
    );
    assert!(
        matches!(decode(&tracking), Err(LoopRejection::Malformed(_))),
        "a tracking gain is in 1/s and distinct"
    );
}

/// The seven retained states of the shipped Control.PID at `Main.TankController`.
fn library_states(output_unit: &str, input_unit: &str) -> Value {
    let members = [
        (
            "Setpoint",
            output_unit.to_string(),
            json!({ "source": "model-output", "index": 0 }),
        ),
        ("SetpointRate", format!("{output_unit}/s"), json!(0)),
        (
            "ProcessValue",
            output_unit.to_string(),
            json!({ "source": "model-output", "index": 0 }),
        ),
        ("ProcessValueRate", format!("{output_unit}/s"), json!(0)),
        ("FeedForward", input_unit.to_string(), json!(0)),
        (
            "NominalOutput",
            input_unit.to_string(),
            json!({ "source": "model-input", "index": 0 }),
        ),
        (
            "SaturatedOutput",
            input_unit.to_string(),
            json!({ "source": "model-input", "index": 0 }),
        ),
    ];
    Value::Array(members.into_iter().map(|(member, unit, initial)| json!({
        "symbol": format!("Main.TankController.StateValue.{member}"), "iec_type": "LREAL", "unit": unit,
        "minimum": -10_000, "maximum": 10_000, "initial": initial, "postScan": "bounded" })).collect())
}

#[test]
fn a_control_pid_initialization_binds_its_seven_states_to_their_operating_point_coordinates() {
    let initialized = with(
        complete_loop(),
        &["controller", "pidInitialization"],
        json!({
        "kind": "control-pid-v1", "maximumSnapshotAgeMs": 100, "states": library_states("%", "%") }),
    );
    validate(&initialized).expect("valid");
    let wrong_unit = with(
        complete_loop(),
        &["controller", "pidInitialization"],
        json!({
        "kind": "control-pid-v1", "maximumSnapshotAgeMs": 100, "states": library_states("degC", "%") }),
    );
    assert!(matches!(
        validate(&wrong_unit),
        Err(LoopRejection::IncompatiblePidInitialization(_))
    ));
    let too_old = with(
        complete_loop(),
        &["controller", "pidInitialization"],
        json!({
        "kind": "control-pid-v1", "maximumSnapshotAgeMs": 20_000, "states": library_states("%", "%") }),
    );
    assert!(matches!(
        validate(&too_old),
        Err(LoopRejection::IncompatiblePidInitialization(_))
    ));
}

#[test]
fn the_rust_engine_refuses_a_parked_feature_and_a_loop_without_a_law() {
    let grouped = with(
        complete_loop(),
        &["controller", "group"],
        json!({ "members": [] }),
    );
    let failure = decode(&grouped)
        .expect("decoded")
        .rust_engine_support()
        .expect_err("refused");
    assert!(
        failure.message.contains("a PID group"),
        "{}",
        failure.message
    );
    let mut lawless = complete_loop();
    lawless["controller"]
        .as_object_mut()
        .expect("controller")
        .remove("law");
    let failure = decode(&lawless)
        .expect("decoded")
        .rust_engine_support()
        .expect_err("refused");
    assert_eq!(failure.next_action, NextAction::OpenLoopSetup);
}

#[test]
fn the_reference_examples_loops_decode_validate_and_run_on_the_rust_engine() {
    for text in [
        include_str!("../../../../examples/room_heating/control-loops/tank-temperature.json"),
        include_str!("../../../../examples/control-pid-commissioning/control-loop.json"),
    ] {
        let value: Value = serde_json::from_str(text).expect("json");
        let loop_definition =
            LoopDefinition::decode(text).unwrap_or_else(|error| panic!("{}: {error}", value["id"]));
        loop_definition
            .validate(&scope(&value))
            .unwrap_or_else(|error| panic!("{}: {error}", value["id"]));
        loop_definition
            .rust_engine_support()
            .unwrap_or_else(|error| panic!("{}: {error}", value["id"]));
    }
}

/// Every `control-loop.json` and `control-loops/*.json` below `folder`.
fn shipped_loops(folder: &std::path::Path, found: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for path in entries.filter_map(Result::ok).map(|entry| entry.path()) {
        let in_loops_folder = folder
            .file_name()
            .is_some_and(|name| name == "control-loops");
        if path.is_dir() {
            shipped_loops(&path, found);
        } else if path
            .file_name()
            .is_some_and(|name| name == "control-loop.json")
            || (in_loops_folder
                && path
                    .extension()
                    .is_some_and(|extension| extension == "json"))
        {
            found.push(path);
        }
    }
}

#[test]
fn every_shipped_loop_of_the_current_version_decodes_and_validates() {
    let mut files = Vec::new();
    shipped_loops(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples"),
        &mut files,
    );
    assert!(
        files.len() >= 80,
        "the examples hold the course's loops; found {}",
        files.len()
    );
    let mut problems = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(&file).expect("readable");
        let value: Value = serde_json::from_str(&text).expect("json");
        if value["schemaVersion"] == 1 {
            // The four cold-storage ON/OFF lessons; they are regenerated with the ON/OFF port (V7).
            assert_eq!(
                LoopDefinition::decode(&text),
                Err(LoopRejection::UnsupportedVersion(1)),
                "{}",
                file.display()
            );
            continue;
        }
        if let Err(error) = LoopDefinition::decode(&text)
            .and_then(|loop_definition| loop_definition.validate(&scope(&value)))
        {
            problems.push(format!("{}: {error}", file.display()));
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
}
