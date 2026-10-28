//! Specification 46 §3.2.1: the existing actuator publishes through ordinary I/O.
use std::sync::atomic::{AtomicU64, Ordering};
use trust_hir::TypeId;
use trust_runtime::error::RuntimeError;
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::CompiledRuntime;

static NEXT_CONFIG: AtomicU64 = AtomicU64::new(0);

fn load(text: &str) -> Result<SimulationConfig, RuntimeError> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "trust-actuator-feedback-{}-{stamp}-{}.toml",
        std::process::id(),
        NEXT_CONFIG.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, text).unwrap();
    let result = SimulationConfig::load(&path);
    std::fs::remove_file(path).unwrap();
    result
}

fn heater(targets: Option<&str>, options: &str, actuator_options: &str) -> String {
    let targets = targets
        .map(|value| format!("actuator_feedback_targets = {value}"))
        .unwrap_or_default();
    format!(
        r#"
[simulation]
enabled = true
plant_step_ms = 100
plant_trace_capacity = 32
[[plants]]
id = "heater"
model = "first_order_dead_time"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
initial_state = [35.0]
{targets}
{options}
[plants.parameters]
gain = 0.2
time_constant_s = 5.0
dead_time_s = 1.0
initial_delayed_input = 75.0
output_bias = 20.0
[plants.actuator]
minimum = 0.0
maximum = 100.0
initial_applied = 75.0
{actuator_options}
"#
    )
}

fn runtime(bindings: &[(&str, &str, TypeId)]) -> CompiledRuntime {
    let typed: Vec<(IoAddress, TypeId)> = bindings
        .iter()
        .map(|(_, address, value_type)| (IoAddress::parse(address).unwrap(), *value_type))
        .collect();
    trust_runtime::harness::runtime_with_bindings(&typed).expect("compile the plant's bindings")
}

fn heater_runtime() -> CompiledRuntime {
    runtime(&[
        ("command", "%QL0", TypeId::LREAL),
        ("temperature", "%IL0", TypeId::LREAL),
        ("applied", "%IL8", TypeId::LREAL),
    ])
}

fn write(runtime: &mut CompiledRuntime, address: &str, value: f64) {
    runtime
        .write_io(
            &IoAddress::parse(address).unwrap(),
            Value::LWord(value.to_bits()),
        )
        .unwrap();
}

fn read(runtime: &CompiledRuntime, address: &str) -> f64 {
    match runtime
        .read_io(&IoAddress::parse(address).unwrap())
        .unwrap()
    {
        Value::LWord(bits) => f64::from_bits(bits),
        value => panic!("expected LREAL at {address}, got {value:?}"),
    }
}

fn step(simulation: &mut SimulationController, runtime: &mut CompiledRuntime, millis: i64) {
    simulation
        .apply_post_cycle(Duration::from_millis(millis), runtime)
        .unwrap();
    simulation
        .apply_pre_cycle(Duration::from_millis(millis), runtime)
        .unwrap();
}

#[test]
fn applied_feedback_initial_value_and_scan_boundary_precede_heat_delivery() {
    let mut simulation =
        SimulationController::new(load(&heater(Some("[{ address = '%IL8' }]"), "", "")).unwrap());
    let mut runtime = heater_runtime();
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    assert_eq!(read(&runtime, "%IL0"), 35.0);
    assert_eq!(
        read(&runtime, "%IL8"),
        75.0,
        "initial applied actuator state is available before the first PLC command"
    );

    write(&mut runtime, "%QL0", 125.0);
    simulation
        .apply_post_cycle(Duration::from_millis(100), &runtime)
        .unwrap();
    assert_eq!(
        read(&runtime, "%IL8"),
        75.0,
        "post-cycle advancement must not publish inputs early"
    );
    assert_eq!(simulation.status().plants[0].applied_input, vec![100.0]);
    simulation
        .apply_pre_cycle(Duration::from_millis(100), &mut runtime)
        .unwrap();
    assert_eq!(
        read(&runtime, "%QL0"),
        125.0,
        "feedback publication cannot rewrite the PLC request"
    );
    assert_eq!(
        read(&runtime, "%IL8"),
        100.0,
        "native actuator saturation owns the applied value"
    );
    assert_eq!(
        read(&runtime, "%IL0"),
        35.0,
        "actuator feedback is before the heater's one-second transport delay"
    );
}

#[test]
fn applied_feedback_uses_inverse_command_scaling_not_sensor_scaling() {
    for (scale, command, initial, applied) in [(2.0, 60.0, 32.5, 45.0), (-2.0, -60.0, -32.5, -45.0)]
    {
        let options = format!("input_scale = [{scale}]\ninput_offset = [10.0]\noutput_scale = [3.0]\noutput_offset = [-2.0]");
        let mut simulation = SimulationController::new(
            load(&heater(Some("[{ address = '%IL8' }]"), &options, "")).unwrap(),
        );
        let mut runtime = heater_runtime();
        simulation
            .apply_pre_cycle(Duration::ZERO, &mut runtime)
            .unwrap();
        assert_eq!(read(&runtime, "%IL8"), initial);
        write(&mut runtime, "%QL0", command);
        step(&mut simulation, &mut runtime, 100);
        assert_eq!(
            simulation.status().plants[0].transformed_command,
            vec![130.0]
        );
        assert_eq!(simulation.status().plants[0].applied_input, vec![100.0]);
        assert_eq!(read(&runtime, "%IL8"), applied);
        assert_eq!(
            read(&runtime, "%IL0"),
            103.0,
            "sensor conversion remains independently owned"
        );
    }
}

#[test]
fn reset_restores_actuator_state_before_advancing_the_same_held_command() {
    let base = heater(Some("[{ address = '%IL8' }]"), "", "rate_up_per_s = 10.0");
    let reset =
        format!("{base}\n[[plant_events]]\nat_ms = 200\nplant = 'heater'\nkind = 'reset_plant'\n");
    let mut simulation = SimulationController::new(load(&reset).unwrap());
    let mut reference = SimulationController::new(load(&base).unwrap());
    let mut actual = heater_runtime();
    let mut uninterrupted = heater_runtime();
    for (controller, runtime) in [
        (&mut simulation, &mut actual),
        (&mut reference, &mut uninterrupted),
    ] {
        controller.apply_pre_cycle(Duration::ZERO, runtime).unwrap();
        write(runtime, "%QL0", 100.0);
        step(controller, runtime, 100);
        assert_eq!(read(runtime, "%IL8"), 76.0);
        step(controller, runtime, 200);
    }
    assert_eq!(
        read(&actual, "%IL8"),
        76.0,
        "reset restores 75 then this substep advances one point"
    );
    assert_eq!(read(&uninterrupted, "%IL8"), 77.0);
    step(&mut simulation, &mut actual, 300);
    assert_eq!(
        read(&actual, "%IL8"),
        77.0,
        "the reset event does not repeat"
    );
}

#[test]
fn omitted_feedback_and_explicit_empty_channel_preserve_existing_behavior() {
    let mut temperatures = Vec::new();
    for targets in [None, Some("[{}]")] {
        let mut simulation = SimulationController::new(load(&heater(targets, "", "")).unwrap());
        let mut runtime = heater_runtime();
        write(&mut runtime, "%IL8", -123.0);
        simulation
            .apply_pre_cycle(Duration::ZERO, &mut runtime)
            .unwrap();
        write(&mut runtime, "%QL0", 125.0);
        let mut samples = Vec::new();
        for sample in 1..=20 {
            step(&mut simulation, &mut runtime, sample * 100);
            assert_eq!(
                read(&runtime, "%IL8"),
                -123.0,
                "an omitted feedback channel has no publication owner"
            );
            samples.push(read(&runtime, "%IL0"));
        }
        temperatures.push(samples);
    }
    assert_eq!(temperatures[0], temperatures[1]);
}

#[test]
fn optional_feedback_records_follow_command_channel_order() {
    let config = r#"
[simulation]
enabled = true
plant_step_ms = 100
[[plants]]
id = "two-input"
model = "state_space"
command_sources = ["%QL0", "%QL8"]
feedback_targets = ["%IL0"]
actuator_feedback_targets = [{}, { address = "%IL16" }]
initial_state = [0.0]
input_scale = [2.0, 10.0]
input_offset = [10.0, -10.0]
actuators = [{ minimum = 0.0, maximum = 80.0, initial_applied = 60.0 }, { minimum = 0.0, maximum = 100.0, initial_applied = 30.0 }]
[plants.parameters]
a = [[0.0]]
b = [[0.0, 0.0]]
c = [[0.0]]
d = [[0.0, 0.0]]
"#;
    let mut simulation = SimulationController::new(load(config).unwrap());
    let mut runtime = runtime(&[
        ("first", "%QL0", TypeId::LREAL),
        ("second", "%QL8", TypeId::LREAL),
        ("process", "%IL0", TypeId::LREAL),
        ("unused", "%IL8", TypeId::LREAL),
        ("second_applied", "%IL16", TypeId::LREAL),
    ]);
    write(&mut runtime, "%IL8", -123.0);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    assert_eq!(read(&runtime, "%IL16"), 4.0);
    write(&mut runtime, "%QL0", 100.0);
    write(&mut runtime, "%QL8", 20.0);
    step(&mut simulation, &mut runtime, 100);
    assert_eq!(read(&runtime, "%IL16"), 11.0);
    assert_eq!(read(&runtime, "%IL8"), -123.0);
}

#[test]
fn configuration_rejects_dimensions_invalid_addresses_and_plant_overlap() {
    for (targets, expected) in [
        ("[]", "match ordered command channels"),
        ("[{}, {}]", "match ordered command channels"),
        ("[{ address = '%QL8' }]", "actuator feedback"),
        ("[{ address = '%IW4' }]", "overlap"),
    ] {
        let error = load(&heater(Some(targets), "", "")).unwrap_err();
        assert!(error.to_string().contains(expected), "{targets}: {error}");
    }
    let zero = heater(Some("[{ address = '%IL8' }]"), "input_scale = [0.0]", "");
    assert!(load(&zero)
        .unwrap_err()
        .to_string()
        .contains("zero command scale"));
    assert!(load(&heater(Some("[{}]"), "input_scale = [0.0]", "")).is_ok());
    let other = r#"
[[plants]]
id = "other"
model = "static_gain"
command_sources = ["%QL16"]
feedback_targets = ["%IW12"]
[plants.parameters]
gain = 1.0
"#;
    let error = load(&format!(
        "{}{other}",
        heater(Some("[{ address = '%IL8' }]"), "", "")
    ))
    .unwrap_err();
    assert!(
        error.to_string().contains("already owned by another plant"),
        "{error}"
    );
}

#[test]
fn actuator_feedback_rejects_coupling_byte_range_ownership_conflict() {
    let config = format!(
        "[[couplings]]\nsource = '%QL24'\ntarget = '%IW12'\n{}",
        heater(Some("[{ address = '%IL8' }]"), "", "")
    );
    let result = load(&config);
    assert!(matches!(&result, Err(RuntimeError::InvalidConfig(message)) if message.contains("conflicts with coupling target")),
        "actuator feedback owns IL8 bytes 8..15; an overlapping coupling must be rejected, got {result:?}");
}

#[test]
fn actuator_feedback_rejects_physics_byte_range_ownership_conflict() {
    let config = format!("[physics]\nenabled = true\n[[physics.joints]]\nid = 'axis'\nkind = 'revolute'\nenable_source = '%QX24.0'\nfeedback_target = '%IW14'\n{}", heater(Some("[{ address = '%IL8' }]"), "", ""));
    let result = load(&config);
    assert!(matches!(&result, Err(RuntimeError::InvalidConfig(message)) if message.contains("conflicts with physics feedback target")),
        "actuator feedback owns IL8 bytes 8..15; overlapping physics must be rejected, got {result:?}");
}

#[test]
fn failed_applied_conversion_leaves_process_and_actuator_inputs_unchanged() {
    let config = r#"
[simulation]
enabled = true
plant_step_ms = 100
[[plants]]
id = "atomic"
model = "static_gain"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
actuator_feedback_targets = [{ address = "%IB8" }]
[plants.parameters]
gain = 1.0
"#;
    let mut simulation = SimulationController::new(load(config).unwrap());
    let mut runtime = runtime(&[
        ("command", "%QL0", TypeId::LREAL),
        ("process", "%IL0", TypeId::LREAL),
        ("applied", "%IB8", TypeId::USINT),
    ]);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    write(&mut runtime, "%QL0", 50.0);
    step(&mut simulation, &mut runtime, 100);
    write(&mut runtime, "%QL0", 300.0);
    simulation
        .apply_post_cycle(Duration::from_millis(200), &runtime)
        .unwrap();
    for _ in 0..2 {
        let error = simulation
            .apply_pre_cycle(Duration::from_millis(200), &mut runtime)
            .unwrap_err();
        assert!(
            error.to_string().contains("atomic") && error.to_string().contains("feedback publish"),
            "{error}"
        );
        assert_eq!(
            read(&runtime, "%IL0"),
            50.0,
            "valid process feedback cannot publish before the applied conversion succeeds"
        );
        assert_eq!(
            runtime.read_io(&IoAddress::parse("%IB8").unwrap()).unwrap(),
            Value::Byte(50)
        );
    }
}
