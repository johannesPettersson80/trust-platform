use crate::io::IoAddress;
use crate::simulation::{SimulationConfig, SimulationController};
use crate::value::{Duration, Value};
use crate::Runtime;
use std::sync::atomic::{AtomicU64, Ordering};
use trust_hir::TypeId;

static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

fn advance(model: &str, initial: &[f64], input: f64, step_ms: i64) -> Vec<f64> {
    let path = std::env::temp_dir().join(format!(
        "trust-gravity-cascade-{}-{}.toml",
        std::process::id(),
        NEXT_FILE.fetch_add(1, Ordering::Relaxed)
    ));
    let outputs = if initial.len() == 2 {
        "[\"%IL0\", \"%IL8\"]"
    } else {
        "[\"%IL0\"]"
    };
    std::fs::write(
        &path,
        format!(
            r#"
[simulation]
enabled = true
plant_step_ms = {step_ms}
[[plants]]
id = "cascade"
model = "{model}"
command_sources = ["%QL0"]
feedback_targets = {outputs}
initial_state = {initial:?}
[plants.parameters]
area_m2 = 0.5
outflow_coefficient_m2_sqrt_m_per_s = 0.020
"#
        ),
    )
    .unwrap();
    let loaded = SimulationConfig::load(&path);
    let _ = std::fs::remove_file(path);
    assert!(
        loaded.is_ok(),
        "the specified coupled tank model must load: {loaded:?}"
    );
    let mut simulation = SimulationController::new(loaded.unwrap());
    let mut runtime = Runtime::new();
    for (name, address) in [
        ("flow", "%QL0"),
        ("upstream", "%IL0"),
        ("downstream", "%IL8"),
    ] {
        runtime
            .io_mut()
            .bind_typed(name, IoAddress::parse(address).unwrap(), TypeId::LREAL);
    }
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    runtime
        .io_mut()
        .write(
            &IoAddress::parse("%QL0").unwrap(),
            Value::LWord(input.to_bits()),
        )
        .unwrap();
    simulation
        .apply_post_cycle(Duration::from_millis(step_ms), &runtime)
        .unwrap();
    simulation
        .apply_pre_cycle(Duration::from_millis(step_ms), &mut runtime)
        .unwrap();
    ["%IL0", "%IL8"]
        .iter()
        .take(initial.len())
        .map(|address| {
            match runtime
                .io()
                .read(&IoAddress::parse(address).unwrap())
                .unwrap()
            {
                Value::LWord(bits) => f64::from_bits(bits),
                other => panic!("expected LREAL, got {other:?}"),
            }
        })
        .collect()
}

#[test]
fn cascade_coupled_mass_balance_and_equilibrium() {
    let equilibrium = advance("gravity_tank_cascade", &[0.25, 0.25], 0.010, 100);
    assert_eq!(equilibrium, vec![0.25, 0.25]);
    let response = advance("gravity_tank_cascade", &[0.27, 0.25], 0.010, 1);
    let expected_rate = (0.010 - 0.020 * 0.27_f64.sqrt()) / 0.5;
    assert!(((response[0] - 0.27) / 0.001 - expected_rate).abs() < 1e-7);
    assert!(((response[1] - 0.25) / 0.001 + expected_rate).abs() < 1e-7);
    assert!(
        (0.5 * (response[0] + response[1] - 0.52)).abs() < 1e-10,
        "internal transfer must cancel rather than appear as external volume"
    );
}

#[test]
fn cascade_addition_preserves_single_gravity_tank_drainage() {
    let response = advance("gravity_tank", &[0.25], 0.0, 100);
    let expected = (0.5_f64 - 0.020 * 0.1 / (2.0 * 0.5)).powi(2);
    assert!((response[0] - expected).abs() < 1e-10);
}
