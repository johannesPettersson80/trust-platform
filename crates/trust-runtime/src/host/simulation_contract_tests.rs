use super::*;

use std::fmt::Debug;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);
const MAX_DURATION_MILLIS: u64 = (i64::MAX / 1_000_000) as u64;

#[test]
fn simulation_contract_defaults_and_programmatic_scale_floor_are_stable() {
    let config = SimulationConfig::default();
    assert!(!config.enabled);
    assert_eq!(config.seed, 0);
    assert_eq!(config.time_scale, 1);
    assert!(config.couplings.is_empty());
    assert!(config.disturbances.is_empty());
    assert!(config.physics.is_none());

    let default_controller = SimulationController::new(config);
    assert!(!default_controller.enabled());
    assert_eq!(default_controller.time_scale(), 1);

    let zero_scale = SimulationController::new(SimulationConfig {
        enabled: true,
        time_scale: 0,
        ..SimulationConfig::default()
    });
    assert!(zero_scale.enabled());
    assert_eq!(zero_scale.time_scale(), 1);
}

#[test]
fn simulation_contract_optional_and_required_file_loading_are_path_qualified() {
    let root = TempTree::new("trust-simulation-load");
    let missing = root.path().join("missing.toml");
    assert!(SimulationConfig::load_optional(&missing).unwrap().is_none());

    let error = SimulationConfig::load(&missing).unwrap_err();
    assert!(error
        .to_string()
        .contains(missing.to_string_lossy().as_ref()));
    assert!(error
        .to_string()
        .contains("failed to read simulation config"));

    let directory = root.path().join("directory");
    fs::create_dir_all(&directory).unwrap();
    assert!(SimulationConfig::load_optional(&directory)
        .unwrap()
        .is_none());
}

#[test]
fn simulation_contract_malformed_toml_is_rejected_with_path() {
    let root = TempTree::new("trust-simulation-malformed");
    let path = root.path().join("simulation.toml");
    fs::write(&path, "[simulation\nenabled = true").unwrap();

    let error = SimulationConfig::load(&path).unwrap_err();
    assert!(error.to_string().contains(path.to_string_lossy().as_ref()));
    assert!(error.to_string().contains("invalid simulation config"));
}

#[test]
fn simulation_contract_file_activation_inference_and_explicit_disable_are_stable() {
    assert!(!load_text("").enabled);
    assert!(
        load_text(
            r#"
[[couplings]]
source = "%QX0.0"
target = "%IX0.0"
"#
        )
        .enabled
    );
    assert!(
        load_text(
            r#"
[[disturbances]]
at_ms = 1
target = "%IX0.0"
value = "TRUE"
"#
        )
        .enabled
    );
    assert!(
        !load_text(
            r#"
[physics]
enabled = true
"#
        )
        .enabled
    );
    assert!(
        load_text(
            r#"
[physics]
enabled = true
[[physics.joints]]
id = "axis"
kind = "revolute"
enable_source = "%QX0.0"
feedback_target = "%IW0"
"#
        )
        .enabled
    );
    assert!(
        !load_text(
            r#"
[simulation]
enabled = false
[[couplings]]
source = "%QX0.0"
target = "%IX0.0"
"#
        )
        .enabled
    );
}

#[test]
fn simulation_contract_seed_and_positive_time_scale_round_trip() {
    let config = load_text(
        r#"
[simulation]
enabled = true
seed = 18446744073709551615
time_scale = 4294967295
"#,
    );
    assert!(config.enabled);
    assert_eq!(config.seed, u64::MAX);
    assert_eq!(config.time_scale, u32::MAX);

    assert_error_contains(
        load_text_result(
            r#"
[simulation]
time_scale = 0
"#,
        ),
        "simulation.time_scale must be >= 1",
    );
}

#[test]
fn simulation_contract_coupling_requires_output_source_and_input_target() {
    for (source, target, expected) in [
        ("%IX0.0", "%IX0.1", "source must be %Q"),
        ("%MX0.0", "%IX0.1", "source must be %Q"),
        ("%QX0.0", "%QX0.1", "target must be %I"),
        ("%QX0.0", "%MX0.1", "target must be %I"),
    ] {
        assert_error_contains(
            load_text_result(&format!(
                r#"
[[couplings]]
source = "{source}"
target = "{target}"
"#
            )),
            expected,
        );
    }
}

#[test]
fn simulation_contract_threshold_must_be_finite_and_owns_branch_values() {
    for threshold in ["nan", "+inf", "-inf"] {
        assert_error_contains(
            load_text_result(&format!(
                r#"
[[couplings]]
source = "%QW0"
target = "%IW0"
threshold = {threshold}
"#
            )),
            "couplings.threshold must be finite",
        );
    }
    for field in ["on_true", "on_false"] {
        assert_error_contains(
            load_text_result(&format!(
                r#"
[[couplings]]
source = "%QX0.0"
target = "%IX0.0"
{field} = "TRUE"
"#
            )),
            "require threshold",
        );
    }
}

#[test]
fn simulation_contract_coupling_delay_fits_signed_runtime_duration() {
    let accepted = load_text(&format!(
        r#"
[[couplings]]
source = "%QX0.0"
target = "%IX0.0"
delay_ms = {MAX_DURATION_MILLIS}
"#,
    ));
    assert_eq!(
        accepted.couplings[0].delay,
        Duration::from_millis(MAX_DURATION_MILLIS as i64)
    );

    assert_error_contains(
        load_text_result(&format!(
            r#"
[[couplings]]
source = "%QX0.0"
target = "%IX0.0"
delay_ms = {}
"#,
            MAX_DURATION_MILLIS + 1
        )),
        "couplings.delay_ms",
    );
}

#[test]
fn simulation_contract_io_literal_parsing_is_width_checked() {
    for (size, text, expected) in [
        (IoSize::Bit, " true ", Value::Bool(true)),
        (IoSize::Bit, "0", Value::Bool(false)),
        (IoSize::Byte, "0xFF", Value::Byte(u8::MAX)),
        (IoSize::Word, "65535", Value::Word(u16::MAX)),
        (IoSize::DWord, "0xFFFFFFFF", Value::DWord(u32::MAX)),
        (
            IoSize::LWord,
            "18446744073709551615",
            Value::LWord(u64::MAX),
        ),
        (IoSize::Bytes(4), "'test'", Value::String("test".into())),
    ] {
        assert_eq!(parse_io_value(text, size).unwrap(), expected);
    }

    for (size, text) in [
        (IoSize::Byte, "256"),
        (IoSize::Word, "65536"),
        (IoSize::DWord, "4294967296"),
        (IoSize::LWord, "18446744073709551616"),
        (IoSize::Bytes(3), "'four'"),
    ] {
        assert_error_contains(parse_io_value(text, size), "overflow");
    }
    assert_error_contains(parse_io_value("yes", IoSize::Bit), "invalid BOOL");
}

#[test]
fn simulation_contract_disturbances_are_stably_sorted_with_defaults() {
    let config = load_text(
        r#"
[[disturbances]]
at_ms = 20
target = "%IX0.0"
value = "TRUE"

[[disturbances]]
at_ms = 10
kind = "FAULT"

[[disturbances]]
at_ms = 20
kind = "fault"
message = "second-at-20"
"#,
    );
    assert_eq!(
        config
            .disturbances
            .iter()
            .map(|entry| entry.at)
            .collect::<Vec<_>>(),
        vec![
            Duration::from_millis(10),
            Duration::from_millis(20),
            Duration::from_millis(20),
        ]
    );
    assert!(matches!(
        &config.disturbances[0].kind,
        SimulationDisturbanceKind::Fault { message }
            if message == "simulated fault injection"
    ));
    assert!(matches!(
        &config.disturbances[1].kind,
        SimulationDisturbanceKind::SetInput {
            target,
            value: Value::Bool(true)
        } if target == &IoAddress::parse("%IX0.0").unwrap()
    ));
    assert!(matches!(
        &config.disturbances[2].kind,
        SimulationDisturbanceKind::Fault { message } if message == "second-at-20"
    ));
}

#[test]
fn simulation_contract_disturbance_time_fits_signed_runtime_duration() {
    let accepted = load_text(&format!(
        r#"
[[disturbances]]
at_ms = {MAX_DURATION_MILLIS}
kind = "fault"
"#,
    ));
    assert_eq!(
        accepted.disturbances[0].at,
        Duration::from_millis(MAX_DURATION_MILLIS as i64)
    );

    assert_error_contains(
        load_text_result(&format!(
            r#"
[[disturbances]]
at_ms = {}
kind = "fault"
"#,
            MAX_DURATION_MILLIS + 1
        )),
        "disturbances.at_ms",
    );
}

#[test]
fn simulation_contract_disturbance_kind_and_set_fields_are_closed() {
    for (text, expected) in [
        (
            r#"[[disturbances]]
at_ms = 1
kind = "pulse""#,
            "unsupported disturbance kind",
        ),
        (
            r#"[[disturbances]]
at_ms = 1
kind = "set"
value = "TRUE""#,
            "target required",
        ),
        (
            r#"[[disturbances]]
at_ms = 1
kind = "set"
target = "%IX0.0""#,
            "value required",
        ),
        (
            r#"[[disturbances]]
at_ms = 1
target = "%QX0.0"
value = "TRUE""#,
            "target must be %I",
        ),
    ] {
        assert_error_contains(load_text_result(text), expected);
    }
}

#[test]
fn simulation_contract_physics_defaults_are_explicit() {
    let physics = load_text(
        r#"
[physics]
[[physics.joints]]
id = "axis"
kind = "REVOLUTE"
enable_source = "%QX0.0"
feedback_target = "%IW0"
"#,
    )
    .physics
    .unwrap();
    assert!(physics.enabled);
    assert_eq!(physics.backend, PhysicsBackend::InTreeRapier);
    assert_eq!(physics.step, Duration::from_millis(10));
    assert_eq!(physics.encoder_counts_per_radian, 1000.0);
    assert_eq!(physics.joints[0].velocity_rad_per_s, 0.0);
    assert_eq!(physics.joints[0].lower_rad, 0.0);
    assert_eq!(physics.joints[0].upper_rad, std::f64::consts::FRAC_PI_2);
    assert_eq!(physics.joints[0].encoder_counts_per_radian, 1000.0);
}

#[test]
fn simulation_contract_physics_backend_and_joint_kind_are_closed() {
    assert_error_contains(
        load_text_result(
            r#"
[physics]
backend = "external"
"#,
        ),
        "unsupported physics backend",
    );
    assert_error_contains(
        load_text_result(
            r#"
[physics]
[[physics.joints]]
id = "axis"
kind = "prismatic"
enable_source = "%QX0.0"
feedback_target = "%IW0"
"#,
        ),
        "unsupported physics joint kind",
    );
}

#[test]
fn simulation_contract_physics_step_is_positive_signed_duration() {
    for step in ["0".to_string(), (MAX_DURATION_MILLIS + 1).to_string()] {
        assert_error_contains(
            load_text_result(&format!(
                r#"
[physics]
step_ms = {step}
"#
            )),
            "physics.step_ms",
        );
    }
    let accepted = load_text(&format!(
        r#"
[physics]
step_ms = {MAX_DURATION_MILLIS}
"#,
    ));
    assert_eq!(
        accepted.physics.unwrap().step,
        Duration::from_millis(MAX_DURATION_MILLIS as i64)
    );
}

#[test]
fn simulation_contract_encoder_scales_are_positive_and_finite() {
    for value in ["0.0", "-1.0", "nan", "inf", "-inf"] {
        assert_error_contains(
            load_text_result(&format!(
                r#"
[physics]
encoder_counts_per_radian = {value}
"#
            )),
            "physics.encoder_counts_per_radian",
        );
    }
    for value in ["0.0", "-1.0", "nan", "inf", "-inf"] {
        assert_error_contains(
            load_text_result(&format!(
                r#"
[physics]
[[physics.joints]]
id = "axis"
kind = "revolute"
enable_source = "%QX0.0"
feedback_target = "%IW0"
encoder_counts_per_radian = {value}
"#
            )),
            "physics.joints.encoder_counts_per_radian",
        );
    }
}

#[test]
fn simulation_contract_physics_joint_addresses_have_exact_areas_and_widths() {
    for (enable, feedback, expected) in [
        ("%IX0.0", "%IW0", "enable_source"),
        ("%QW0", "%IW0", "enable_source"),
        ("%QX0.0", "%QW0", "feedback_target"),
        ("%QX0.0", "%IX0.0", "feedback_target"),
    ] {
        assert_error_contains(
            load_text_result(&format!(
                r#"
[physics]
[[physics.joints]]
id = "axis"
kind = "revolute"
enable_source = "{enable}"
feedback_target = "{feedback}"
"#
            )),
            expected,
        );
    }
}

#[test]
fn simulation_contract_physics_joint_numbers_are_finite_and_ordered() {
    for (field, value, expected) in [
        ("velocity_rad_per_s", "nan", "velocity_rad_per_s"),
        ("lower_rad", "-inf", "lower_rad"),
        ("upper_rad", "inf", "upper_rad"),
    ] {
        assert_error_contains(
            load_text_result(&format!(
                r#"
[physics]
[[physics.joints]]
id = "axis"
kind = "revolute"
enable_source = "%QX0.0"
feedback_target = "%IW0"
{field} = {value}
"#
            )),
            expected,
        );
    }
    assert_error_contains(
        load_text_result(
            r#"
[physics]
[[physics.joints]]
id = "axis"
kind = "revolute"
enable_source = "%QX0.0"
feedback_target = "%IW0"
lower_rad = 2.0
upper_rad = 1.0
"#,
        ),
        "lower_rad must be <=",
    );
}

#[test]
fn simulation_contract_physics_joint_ids_are_unique() {
    assert_error_contains(
        load_text_result(
            r#"
[physics]
[[physics.joints]]
id = "axis"
kind = "revolute"
enable_source = "%QX0.0"
feedback_target = "%IW0"

[[physics.joints]]
id = "axis"
kind = "revolute"
enable_source = "%QX0.1"
feedback_target = "%IW2"
"#,
        ),
        "duplicate physics joint id",
    );
}

#[test]
fn simulation_contract_feedback_targets_are_unique_across_physics_and_couplings() {
    assert_error_contains(
        load_text_result(
            r#"
[[couplings]]
source = "%QW0"
target = "%IW0"

[physics]
[[physics.joints]]
id = "axis"
kind = "revolute"
enable_source = "%QX2.0"
feedback_target = "%IW0"
"#,
        ),
        "conflicts with coupling target",
    );
    assert_error_contains(
        load_text_result(
            r#"
[physics]
[[physics.joints]]
id = "axis-a"
kind = "revolute"
enable_source = "%QX0.0"
feedback_target = "%IW0"

[[physics.joints]]
id = "axis-b"
kind = "revolute"
enable_source = "%QX0.1"
feedback_target = "%IW0"
"#,
        ),
        "another physics joint target",
    );
}

#[test]
fn simulation_contract_threshold_derivation_honors_defaults_and_overrides() {
    let target = IoAddress::parse("%IW0").unwrap();
    let default_rule = SignalCouplingRule {
        source: IoAddress::parse("%QW0").unwrap(),
        target: target.clone(),
        threshold: Some(10.0),
        delay: Duration::ZERO,
        on_true: None,
        on_false: None,
    };
    assert_eq!(
        derive_coupling_value(&default_rule, &Value::Word(10)).unwrap(),
        Value::Word(1)
    );
    assert_eq!(
        derive_coupling_value(&default_rule, &Value::Word(9)).unwrap(),
        Value::Word(0)
    );

    let overridden = SignalCouplingRule {
        on_true: Some(Value::Word(42)),
        on_false: Some(Value::Word(7)),
        ..default_rule
    };
    assert_eq!(
        derive_coupling_value(&overridden, &Value::Word(10)).unwrap(),
        Value::Word(42)
    );
    assert_eq!(
        derive_coupling_value(&overridden, &Value::Word(9)).unwrap(),
        Value::Word(7)
    );
}

#[test]
fn simulation_contract_numeric_coupling_coercion_is_width_checked() {
    for (size, source, expected) in [
        (IoSize::Bit, Value::Word(0), Value::Bool(false)),
        (IoSize::Bit, Value::Word(2), Value::Bool(true)),
        (IoSize::Byte, Value::Word(255), Value::Byte(255)),
        (IoSize::Word, Value::DWord(65535), Value::Word(65535)),
        (
            IoSize::DWord,
            Value::LWord(u64::from(u32::MAX)),
            Value::DWord(u32::MAX),
        ),
        (
            IoSize::LWord,
            Value::ULInt(u64::MAX),
            Value::LWord(u64::MAX),
        ),
    ] {
        assert_eq!(coerce_to_size(&source, size).unwrap(), expected);
    }

    for (size, source) in [
        (IoSize::Byte, Value::Word(256)),
        (IoSize::Word, Value::DWord(65536)),
        (IoSize::DWord, Value::LWord(u64::from(u32::MAX) + 1)),
    ] {
        assert_error_contains(coerce_to_size(&source, size), "overflow");
    }
    assert_error_contains(
        coerce_to_size(&Value::Int(-1), IoSize::Word),
        "type mismatch",
    );
}

#[test]
fn simulation_contract_text_coupling_coercion_is_typed_and_bounded() {
    assert_eq!(
        coerce_to_size(&Value::String("abc".into()), IoSize::Bytes(3)).unwrap(),
        Value::String("abc".into())
    );
    assert_eq!(
        coerce_to_size(&Value::WString("å".to_string()), IoSize::Bytes(2)).unwrap(),
        Value::String("å".into())
    );
    assert_eq!(
        coerce_to_size(&Value::Char(b'Z'), IoSize::Bytes(1)).unwrap(),
        Value::String("Z".into())
    );
    assert_error_contains(
        coerce_to_size(&Value::String("four".into()), IoSize::Bytes(3)),
        "overflow",
    );
    assert_error_contains(
        coerce_to_size(&Value::Bool(true), IoSize::Bytes(4)),
        "type mismatch",
    );
}

#[test]
fn simulation_contract_default_values_and_io_formatting_cover_every_width() {
    for (size, inactive, active) in [
        (IoSize::Bit, Value::Bool(false), Value::Bool(true)),
        (IoSize::Byte, Value::Byte(0), Value::Byte(1)),
        (IoSize::Word, Value::Word(0), Value::Word(1)),
        (IoSize::DWord, Value::DWord(0), Value::DWord(1)),
        (IoSize::LWord, Value::LWord(0), Value::LWord(1)),
        (
            IoSize::Bytes(4),
            Value::String("FALSE".into()),
            Value::String("TRUE".into()),
        ),
    ] {
        assert_eq!(default_value_for_size(size, false), inactive);
        assert_eq!(default_value_for_size(size, true), active);
    }
    for (text, expected) in [
        ("%IX1.2", "%IX1.2"),
        ("%QB3", "%QB3"),
        ("%IW4", "%IW4"),
        ("%QD8", "%QD8"),
        ("%IL16", "%IL16"),
    ] {
        assert_eq!(format_io(&IoAddress::parse(text).unwrap()), expected);
    }
}

#[test]
fn simulation_contract_disabled_controller_performs_no_io_or_fault() {
    let target = IoAddress::parse("%IX0.0").unwrap();
    let mut runtime = Runtime::new();
    let mut controller = SimulationController::new(SimulationConfig {
        enabled: false,
        disturbances: vec![SimulationDisturbance {
            at: Duration::ZERO,
            kind: SimulationDisturbanceKind::Fault {
                message: "must not fire".into(),
            },
        }],
        couplings: vec![SignalCouplingRule {
            source: IoAddress::parse("%QX0.0").unwrap(),
            target: target.clone(),
            threshold: None,
            delay: Duration::ZERO,
            on_true: None,
            on_false: None,
        }],
        ..SimulationConfig::default()
    });

    controller
        .apply_pre_cycle(Duration::from_millis(100), &mut runtime)
        .unwrap();
    controller
        .apply_post_cycle(Duration::from_millis(100), &runtime)
        .unwrap();
    assert!(!runtime.faulted());
    assert_eq!(runtime.io().read(&target).unwrap(), Value::Bool(false));
    assert!(controller.pending_effects.is_empty());
}

#[test]
fn simulation_contract_equal_time_disturbances_preserve_file_order() {
    let target = IoAddress::parse("%IX0.0").unwrap();
    let config = load_text(
        r#"
[[disturbances]]
at_ms = 10
target = "%IX0.0"
value = "TRUE"

[[disturbances]]
at_ms = 10
target = "%IX0.0"
value = "FALSE"
"#,
    );
    let mut controller = SimulationController::new(config);
    let mut runtime = Runtime::new();

    controller
        .apply_pre_cycle(Duration::from_millis(10), &mut runtime)
        .unwrap();

    assert_eq!(runtime.io().read(&target).unwrap(), Value::Bool(false));
}

#[test]
fn simulation_contract_equal_due_effects_preserve_coupling_order() {
    let source_a = IoAddress::parse("%QX0.0").unwrap();
    let source_b = IoAddress::parse("%QX0.1").unwrap();
    let target = IoAddress::parse("%IX0.0").unwrap();
    let mut runtime = Runtime::new();
    runtime
        .io_mut()
        .write(&source_a, Value::Bool(true))
        .unwrap();
    runtime
        .io_mut()
        .write(&source_b, Value::Bool(false))
        .unwrap();
    let mut controller = SimulationController::new(SimulationConfig {
        enabled: true,
        couplings: vec![
            SignalCouplingRule {
                source: source_a,
                target: target.clone(),
                threshold: None,
                delay: Duration::from_millis(5),
                on_true: None,
                on_false: None,
            },
            SignalCouplingRule {
                source: source_b,
                target: target.clone(),
                threshold: None,
                delay: Duration::from_millis(5),
                on_true: None,
                on_false: None,
            },
        ],
        ..SimulationConfig::default()
    });

    controller
        .apply_post_cycle(Duration::ZERO, &runtime)
        .unwrap();
    assert_eq!(controller.pending_effects.len(), 2);
    controller
        .apply_pre_cycle(Duration::from_millis(5), &mut runtime)
        .unwrap();
    assert_eq!(runtime.io().read(&target).unwrap(), Value::Bool(false));
}

#[test]
fn simulation_contract_unchanged_coupling_does_not_enqueue_again() {
    let source = IoAddress::parse("%QW0").unwrap();
    let target = IoAddress::parse("%IW0").unwrap();
    let mut runtime = Runtime::new();
    runtime.io_mut().write(&source, Value::Word(7)).unwrap();
    let mut controller = SimulationController::new(SimulationConfig {
        enabled: true,
        couplings: vec![SignalCouplingRule {
            source,
            target,
            threshold: None,
            delay: Duration::from_millis(10),
            on_true: None,
            on_false: None,
        }],
        ..SimulationConfig::default()
    });

    controller
        .apply_post_cycle(Duration::ZERO, &runtime)
        .unwrap();
    controller
        .apply_post_cycle(Duration::from_millis(1), &runtime)
        .unwrap();
    assert_eq!(controller.pending_effects.len(), 1);
    assert_eq!(controller.next_sequence, 1);
}

#[test]
fn simulation_contract_delayed_effect_is_not_visible_until_exact_due_boundary() {
    let source = IoAddress::parse("%QX0.0").unwrap();
    let target = IoAddress::parse("%IX0.0").unwrap();
    let mut runtime = Runtime::new();
    runtime.io_mut().write(&source, Value::Bool(true)).unwrap();
    let mut controller = SimulationController::new(SimulationConfig {
        enabled: true,
        couplings: vec![SignalCouplingRule {
            source,
            target: target.clone(),
            threshold: None,
            delay: Duration::from_millis(10),
            on_true: None,
            on_false: None,
        }],
        ..SimulationConfig::default()
    });

    controller
        .apply_post_cycle(Duration::from_millis(5), &runtime)
        .unwrap();
    controller
        .apply_pre_cycle(Duration::from_millis(14), &mut runtime)
        .unwrap();
    assert_eq!(runtime.io().read(&target).unwrap(), Value::Bool(false));
    controller
        .apply_pre_cycle(Duration::from_millis(15), &mut runtime)
        .unwrap();
    assert_eq!(runtime.io().read(&target).unwrap(), Value::Bool(true));
}

#[test]
fn simulation_contract_coupling_source_read_error_remains_an_io_error() {
    let mut source = IoAddress::parse("%QW0").unwrap();
    source.wildcard = true;
    let target = IoAddress::parse("%IW0").unwrap();
    let runtime = Runtime::new();
    let mut controller = SimulationController::new(SimulationConfig {
        enabled: true,
        couplings: vec![SignalCouplingRule {
            source,
            target,
            threshold: None,
            delay: Duration::ZERO,
            on_true: None,
            on_false: None,
        }],
        ..SimulationConfig::default()
    });

    let error = controller
        .apply_post_cycle(Duration::ZERO, &runtime)
        .unwrap_err();
    assert!(!matches!(error, RuntimeError::SimulationFault(_)));
}

#[test]
fn simulation_contract_failed_disturbance_write_becomes_runtime_fault() {
    let mut runtime = Runtime::new();
    let mut controller = SimulationController::new(SimulationConfig {
        enabled: true,
        disturbances: vec![SimulationDisturbance {
            at: Duration::ZERO,
            kind: SimulationDisturbanceKind::SetInput {
                target: IoAddress::parse("%IW99999999").unwrap(),
                value: Value::Word(7),
            },
        }],
        ..SimulationConfig::default()
    });

    let error = controller
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap_err();
    assert!(matches!(error, RuntimeError::SimulationFault(_)));
    assert!(error.to_string().contains("simulation disturbance failed"));
    assert!(runtime.faulted());
}

#[test]
fn simulation_contract_failed_delayed_write_becomes_runtime_fault() {
    let mut runtime = Runtime::new();
    let mut controller = SimulationController::new(SimulationConfig {
        enabled: true,
        ..SimulationConfig::default()
    });
    controller.enqueue_effect(PendingEffect {
        due: Duration::ZERO,
        sequence: 0,
        target: IoAddress::parse("%IW99999999").unwrap(),
        value: Value::Word(1),
    });

    let error = controller
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap_err();
    assert!(matches!(error, RuntimeError::SimulationFault(_)));
    assert!(error
        .to_string()
        .contains("simulation delayed effect failed"));
    assert!(runtime.faulted());
}

#[test]
fn simulation_contract_physics_enable_read_failure_is_a_simulation_fault() {
    let runtime = Runtime::new();
    let mut invalid_enable = IoAddress::parse("%QX0.0").unwrap();
    invalid_enable.wildcard = true;
    let physics = PhysicsController::new(&PhysicsConfig {
        enabled: true,
        backend: PhysicsBackend::InTreeRapier,
        step: Duration::from_millis(10),
        encoder_counts_per_radian: 1000.0,
        joints: vec![PhysicsJointConfig {
            id: "axis".into(),
            kind: PhysicsJointKind::Revolute,
            enable_source: invalid_enable,
            feedback_target: IoAddress::parse("%IW0").unwrap(),
            velocity_rad_per_s: 1.0,
            lower_rad: 0.0,
            upper_rad: 1.0,
            encoder_counts_per_radian: 1000.0,
        }],
    });
    let joint = &physics.joints[0];

    let error = joint.enabled(&runtime).unwrap_err();
    assert!(matches!(error, RuntimeError::SimulationFault(_)));
    assert!(error.to_string().contains("physics enable read failed"));
}

#[test]
fn simulation_contract_disabled_or_empty_physics_creates_no_controller() {
    for physics in [
        PhysicsConfig {
            enabled: false,
            backend: PhysicsBackend::InTreeRapier,
            step: Duration::from_millis(10),
            encoder_counts_per_radian: 1000.0,
            joints: vec![physics_joint("axis")],
        },
        PhysicsConfig {
            enabled: true,
            backend: PhysicsBackend::InTreeRapier,
            step: Duration::from_millis(10),
            encoder_counts_per_radian: 1000.0,
            joints: Vec::new(),
        },
    ] {
        let controller = SimulationController::new(SimulationConfig {
            enabled: true,
            physics: Some(physics),
            ..SimulationConfig::default()
        });
        assert!(controller.physics.is_none());
    }
}

fn physics_joint(id: &str) -> PhysicsJointConfig {
    PhysicsJointConfig {
        id: id.into(),
        kind: PhysicsJointKind::Revolute,
        enable_source: IoAddress::parse("%QX0.0").unwrap(),
        feedback_target: IoAddress::parse("%IW0").unwrap(),
        velocity_rad_per_s: 1.0,
        lower_rad: 0.0,
        upper_rad: 1.0,
        encoder_counts_per_radian: 1000.0,
    }
}

fn load_text(text: &str) -> SimulationConfig {
    load_text_result(text).expect("valid simulation config")
}

fn load_text_result(text: &str) -> Result<SimulationConfig, RuntimeError> {
    let root = TempTree::new("trust-simulation-config");
    let path = root.path().join("simulation.toml");
    fs::write(&path, text).expect("write simulation config");
    SimulationConfig::load(path)
}

fn assert_error_contains<T: Debug>(result: Result<T, RuntimeError>, expected: &str) {
    let error = result.expect_err("expected simulation error");
    assert!(
        error
            .to_string()
            .to_ascii_lowercase()
            .contains(expected.to_ascii_lowercase().as_str()),
        "expected error containing {expected:?}, got {error}"
    );
}

struct TempTree {
    path: PathBuf,
}

impl TempTree {
    fn new(prefix: &str) -> Self {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("{prefix}-{}-{sequence}", std::process::id()));
        fs::create_dir_all(&path).expect("create temp tree");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
