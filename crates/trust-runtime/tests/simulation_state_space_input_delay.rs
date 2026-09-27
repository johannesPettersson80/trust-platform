//! Specification 33 §7.6.1: independently delayed native state-space inputs.
use std::sync::atomic::{AtomicU64, Ordering};
use trust_hir::TypeId;
use trust_runtime::error::RuntimeError;
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

// Reuse the existing scalar delay assertions in this focused validation binary.
#[path = "simulation_plant_delay_defaults.rs"]
mod existing_scalar_defaults;
#[path = "simulation_plant_temperature_example.rs"]
mod existing_temperature;

static NEXT_CONFIG: AtomicU64 = AtomicU64::new(0);

fn scalar_history(model: &str, delay: f64) -> String {
    let time_constant = if model == "first_order_dead_time" {
        "time_constant_s = 5.0"
    } else {
        ""
    };
    format!(
        r#"
[simulation]
enabled = true
plant_step_ms = 100
[[plants]]
id = "scalar"
model = "{model}"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
initial_state = [0.0]
[plants.parameters]
gain = 1.0
dead_time_s = {delay}
initial_delayed_input = 0.0
{time_constant}
"#
    )
}

#[test]
fn history_budget_rejects_oversized_configuration_before_allocation() {
    let mut cases = vec![
        heater(Some("[1000000000.0, 0.0]"), 0.0, "[0.0, 0.0]", ""),
        heater(Some("[5000.0, 5000.0]"), 0.0, "[0.0, 0.0]", ""),
        heater(Some("[9999.5, 0.0]"), 0.0, "[0.0, 0.0]", ""),
        heater(Some("[100.0, 0.0]"), 0.0, "[0.0, 0.0]", "step_ms = 1"),
    ];
    for model in ["first_order_dead_time", "integrator_dead_time"] {
        cases.push(scalar_history(model, 1000000000.0));
        cases.push(scalar_history(model, 9999.8));
    }
    for text in cases {
        // Never construct an accepted controller here: the old implementation
        // would attempt an enormous allocation. This red is configuration-only.
        let result = load(&text);
        assert!(
            matches!(&result, Err(RuntimeError::InvalidConfig(message))
                if message.contains("transport history") && message.contains("100000")),
            "oversized transport history must be rejected before controller construction; got {result:?}"
        );
    }
}

#[test]
fn history_budget_accepts_exact_capacity_before_controller_construction() {
    let mut cases = vec![
        heater(Some("[4999.7, 4999.7]"), 0.0, "[0.0, 0.0]", ""),
        heater(Some("[9999.4, 0.0]"), 0.0, "[0.0, 0.0]", ""),
        heater(Some("[0.0, 0.0]"), 0.0, "[0.0, 0.0]", "step_ms = 1"),
    ];
    for model in ["first_order_dead_time", "integrator_dead_time"] {
        cases.push(scalar_history(model, 9999.7));
    }
    for text in cases {
        let config = load(&text).expect("supported history capacity must be accepted");
        // Every constructed case is explicitly at or below 100000 live slots.
        let controller = SimulationController::new(config);
        assert_eq!(controller.status().plants[0].delay_buffer_depth, 0);
    }
}

fn load(text: &str) -> Result<SimulationConfig, RuntimeError> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "trust-state-space-input-delay-{}-{stamp}-{}.toml",
        std::process::id(),
        NEXT_CONFIG.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, text).unwrap();
    let result = SimulationConfig::load(&path);
    std::fs::remove_file(path).unwrap();
    result
}

fn heater(delay: Option<&str>, state: f64, initial_input: &str, options: &str) -> String {
    let delay = delay
        .map(|value| format!("input_delays_s = {value}"))
        .unwrap_or_default();
    format!(
        r#"
[simulation]
enabled = true
seed = 17
plant_step_ms = 100
plant_trace_capacity = 64
[[plants]]
id = "heater"
model = "state_space"
command_sources = ["%QL0", "%QL8"]
feedback_targets = ["%IL0"]
initial_state = [{state}]
initial_input = {initial_input}
output_offset = [20.0]
{options}
[plants.parameters]
a = [[-0.2]]
b = [[0.04, -0.02]]
c = [[1.0]]
d = [[0.0, 0.0]]
{delay}
"#
    )
}

fn rig(text: &str) -> (SimulationController, Runtime) {
    let loaded = load(text);
    assert!(
        loaded.is_ok(),
        "the declared native state-space plant must load: {loaded:?}"
    );
    let mut simulation = SimulationController::new(loaded.unwrap());
    let mut runtime = Runtime::new();
    for (name, address) in [
        ("heater", "%QL0"),
        ("load", "%QL8"),
        ("temperature", "%IL0"),
    ] {
        runtime
            .io_mut()
            .bind_typed(name, IoAddress::parse(address).unwrap(), TypeId::LREAL);
    }
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    (simulation, runtime)
}

fn command(runtime: &mut Runtime, heater: f64, load: f64) {
    for (address, value) in [("%QL0", heater), ("%QL8", load)] {
        runtime
            .io_mut()
            .write(
                &IoAddress::parse(address).unwrap(),
                Value::LWord(value.to_bits()),
            )
            .unwrap();
    }
}

fn temperature(runtime: &Runtime) -> f64 {
    let Value::LWord(bits) = runtime
        .io()
        .read(&IoAddress::parse("%IL0").unwrap())
        .unwrap()
    else {
        panic!("LREAL temperature process image");
    };
    f64::from_bits(bits)
}

fn step(simulation: &mut SimulationController, runtime: &mut Runtime, millis: i64) -> f64 {
    let previous = temperature(runtime);
    simulation
        .apply_post_cycle(Duration::from_millis(millis), runtime)
        .unwrap();
    assert_eq!(
        temperature(runtime),
        previous,
        "post-cycle integration cannot publish PLC input early"
    );
    simulation
        .apply_pre_cycle(Duration::from_millis(millis), runtime)
        .unwrap();
    temperature(runtime)
}

#[test]
fn configuration_accepts_one_delay_per_continuous_input() {
    let result = load(&heater(Some("[1.0, 0.0]"), 0.0, "[0.0, 0.0]", ""));
    assert!(
        result.is_ok(),
        "the continuous state-space input-delay contract must accept its valid vector: {result:?}"
    );
}

#[test]
fn configuration_rejects_invalid_delay_dimensions_values_and_model_scope() {
    for (vector, expected) in [
        ("[1.0]", "each of the 2 channels"),
        ("[1.0, 0.0, 0.0]", "each of the 2 channels"),
        ("[-1.0, 0.0]", "nonnegative"),
        ("[nan, 0.0]", "finite"),
        ("[inf, 0.0]", "finite"),
        ("[1.0e20, 0.0]", "nanosecond"),
    ] {
        let error = load(&heater(Some(vector), 0.0, "[0.0, 0.0]", "")).unwrap_err();
        assert!(error.to_string().contains(expected), "{vector}: {error}");
    }
    let discrete = heater(Some("[1.0, 0.0]"), 0.0, "[0.0, 0.0]", "").replace(
        "model = \"state_space\"",
        "model = \"discrete_state_space\"",
    );
    assert!(
        load(&discrete).is_err(),
        "nonzero continuous transport delay cannot be silently ignored by discrete state space"
    );
}

#[test]
fn zero_delay_existing_process_image_behavior_lock() {
    let (mut simulation, mut runtime) = rig(&heater(None, 0.0, "[0.0, 0.0]", ""));
    assert_eq!(temperature(&runtime), 20.0);
    command(&mut runtime, 50.0, 30.0);
    for sample in 1..=20 {
        let time = sample as f64 * 0.1;
        let expected = 20.0 + 7.0 * (1.0 - (-time / 5.0).exp());
        assert!((step(&mut simulation, &mut runtime, sample * 100) - expected).abs() < 1e-7);
    }
    assert_eq!(simulation.status().plants[0].delay_buffer_depth, 0);
}

#[test]
fn delayed_heater_does_not_delay_the_independent_load() {
    let (mut simulation, mut runtime) = rig(&heater(Some("[1.0, 0.0]"), 0.0, "[0.0, 0.0]", ""));
    command(&mut runtime, 50.0, 30.0);
    for sample in 1..=30 {
        let time = sample as f64 * 0.1;
        let expected = 20.0 - 3.0 * (1.0 - (-time / 5.0).exp())
            + 10.0 * (1.0 - (-(time - 1.0).max(0.0) / 5.0).exp());
        let actual = step(&mut simulation, &mut runtime, sample * 100);
        assert!((actual - expected).abs() < 1e-7, "at {time}s heater delivery must wait1s while load enters immediately: expected {expected}, got {actual}");
    }
}

#[test]
fn fractional_input_delay_splits_the_rk4_step() {
    let (mut simulation, mut runtime) = rig(&heater(Some("[0.15, 0.0]"), 0.0, "[0.0, 0.0]", ""));
    command(&mut runtime, 50.0, 30.0);
    step(&mut simulation, &mut runtime, 100);
    let actual = step(&mut simulation, &mut runtime, 200);
    let expected =
        20.0 - 3.0 * (1.0 - (-0.2_f64 / 5.0).exp()) + 10.0 * (1.0 - (-0.05_f64 / 5.0).exp());
    assert!((actual - expected).abs() < 1e-8, "150ms delivery must integrate only the last50ms of the second step: expected {expected}, got {actual}");
}

#[test]
fn initial_input_is_engineering_prehistory_and_history_records_applied_input() {
    let options = "input_scale = [2.0, 10.0]\ninput_offset = [5.0, -10.0]\nactuators = [{ minimum = 0.0, maximum = 80.0 }, {}]";
    let (mut simulation, mut runtime) =
        rig(&heater(Some("[1.0, 0.0]"), 15.0, "[75.0, 0.0]", options));
    assert_eq!(temperature(&runtime), 35.0);
    command(&mut runtime, 40.0, 4.0); // transformed85/30, then heater limited80.
    for sample in 1..=20 {
        let time = sample as f64 * 0.1;
        let expected =
            35.0 - 3.0 * (1.0 - (-time / 5.0).exp()) + (1.0 - (-(time - 1.0).max(0.0) / 5.0).exp());
        let actual = step(&mut simulation, &mut runtime, sample * 100);
        assert!((actual - expected).abs() < 1e-7, "initial75 must not be rescaled, and delayed history must contain applied80, at {time}s: {actual} vs {expected}");
    }
    let status = &simulation.status().plants[0];
    assert_eq!(status.transformed_command, vec![85.0, 30.0]);
    assert_eq!(status.applied_input, vec![80.0, 30.0]);
}

#[test]
fn explicit_zero_delay_vector_matches_omitted_trace_bit_for_bit() {
    let (mut original, mut left) = rig(&heater(None, 0.0, "[0.0, 0.0]", ""));
    let (mut explicit, mut right) = rig(&heater(Some("[0.0, 0.0]"), 0.0, "[0.0, 0.0]", ""));
    for sample in 1..=30 {
        let heat = if sample % 3 == 0 { 75.0 } else { 50.0 };
        let load = if sample > 10 { 30.0 } else { 0.0 };
        command(&mut left, heat, load);
        command(&mut right, heat, load);
        assert_eq!(
            step(&mut original, &mut left, sample * 100),
            step(&mut explicit, &mut right, sample * 100)
        );
    }
    assert_eq!(explicit.status().plants[0].delay_buffer_depth, 0);
}

#[test]
fn delayed_input_feedthrough_uses_the_completed_boundary_value() {
    let text = heater(Some("[0.1, 0.0]"), 0.0, "[0.0, 0.0]", "")
        .replace("a = [[-0.2]]", "a = [[0.0]]")
        .replace("b = [[0.04, -0.02]]", "b = [[0.0, 0.0]]")
        .replace("d = [[0.0, 0.0]]", "d = [[2.0, -1.0]]");
    let (mut simulation, mut runtime) = rig(&text);
    command(&mut runtime, 3.0, 4.0);
    assert_eq!(
        step(&mut simulation, &mut runtime, 100),
        22.0,
        "D must see the heater transition exactly at the completed100ms boundary"
    );
    let (mut simulation, mut runtime) = rig(&text.replace("[0.1, 0.0]", "[0.15, 0.0]"));
    command(&mut runtime, 3.0, 4.0);
    assert_eq!(
        step(&mut simulation, &mut runtime, 100),
        16.0,
        "D must not see the heater input before its own150ms delay"
    );
    assert_eq!(step(&mut simulation, &mut runtime, 200), 22.0);
}

#[test]
fn independent_histories_stay_bounded_and_reset_replays_initial_history() {
    let base = heater(Some("[0.15, 0.35]"), 15.0, "[75.0, 0.0]", "");
    let (mut bounded, mut runtime) = rig(&base);
    command(&mut runtime, 85.0, 30.0);
    for sample in 1..=1000 {
        step(&mut bounded, &mut runtime, sample * 100);
    }
    let depth = bounded.status().plants[0].delay_buffer_depth;
    assert!(
        (2..=12).contains(&depth),
        "two histories require nonzero storage bounded by both horizons, got {depth}"
    );

    let reset = format!(
        "{base}\n[[plant_events]]\nat_ms = 200\nplant = \"heater\"\nkind = \"reset_plant\"\n"
    );
    let (mut actual, mut left) = rig(&reset);
    let (mut reference, mut right) = rig(&base);
    command(&mut left, 85.0, 30.0);
    command(&mut right, 85.0, 30.0);
    step(&mut actual, &mut left, 100);
    for sample in 1..=12 {
        assert_eq!(
            step(&mut actual, &mut left, (sample + 1) * 100),
            step(&mut reference, &mut right, sample * 100),
            "reset must replay both initial histories at relative sample{sample}"
        );
    }
}

#[test]
fn failed_substep_restores_all_input_histories_and_published_state() {
    let text = format!("{}\n[[plants.uncertainties]]\nid = \"failed-sensor\"\ntarget = \"measurement\"\nchannel = 0\nmode = \"replace\"\ngenerator = \"dropout\"\nstart_s = 0.15\nrate_per_s = 1000000000.0\nduration_s = 1.0\ndropout_policy = \"fault\"\n", heater(Some("[0.15, 0.35]"), 15.0, "[75.0, 0.0]", ""));
    let (mut simulation, mut runtime) = rig(&text);
    command(&mut runtime, 85.0, 30.0);
    step(&mut simulation, &mut runtime, 100);
    let before = simulation.status().plants[0].clone();
    let before_pv = temperature(&runtime);
    assert!(before.delay_buffer_depth > 0);
    command(&mut runtime, 90.0, 40.0);
    for time in [200, 300] {
        let error = simulation
            .apply_post_cycle(Duration::from_millis(time), &runtime)
            .unwrap_err();
        assert!(error.to_string().contains("failed-sensor"));
        let after = &simulation.status().plants[0];
        assert_eq!(after.simulated_time, before.simulated_time);
        assert_eq!(after.state, before.state);
        assert_eq!(after.delay_buffer_depth, before.delay_buffer_depth);
        assert_eq!(after.applied_input, before.applied_input);
        assert_eq!(temperature(&runtime), before_pv);
        assert_eq!(simulation.trace_snapshot().rows.len(), 1);
    }
}
