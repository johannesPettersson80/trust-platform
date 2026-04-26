use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

const OSCAT_COMPONENT_EXAMPLES: &[&str] = &[
    "oscat_components_tank_level_pid_classic",
    "oscat_components_tank_level_pid_components",
    "oscat_components_greenhouse_temperature_classic",
    "oscat_components_greenhouse_temperature_components",
    "oscat_components_conveyor_pulse_classic",
    "oscat_components_conveyor_pulse_components",
    "oscat_components_production_queue_classic",
    "oscat_components_production_queue_components",
    "oscat_components_maintenance_stack_classic",
    "oscat_components_maintenance_stack_components",
    "oscat_components_ventilation_filter_classic",
    "oscat_components_ventilation_filter_components",
    "oscat_components_solar_lighting_clock_classic",
    "oscat_components_solar_lighting_clock_components",
    "oscat_components_pump_pressure_classic",
    "oscat_components_pump_pressure_components",
    "oscat_components_energy_normalization_classic",
    "oscat_components_energy_normalization_components",
    "oscat_components_wind_speed_alarm_classic",
    "oscat_components_wind_speed_alarm_components",
    "oscat_components_cold_storage_alarm_classic",
    "oscat_components_cold_storage_alarm_components",
    "oscat_components_wastewater_aeration_classic",
    "oscat_components_wastewater_aeration_components",
    "oscat_components_packaging_reject_pulse_classic",
    "oscat_components_packaging_reject_pulse_components",
    "oscat_components_recipe_batch_stack_classic",
    "oscat_components_recipe_batch_stack_components",
    "oscat_components_shift_order_queue_classic",
    "oscat_components_shift_order_queue_components",
    "oscat_components_irrigation_sun_clock_classic",
    "oscat_components_irrigation_sun_clock_components",
    "oscat_components_compressor_pressure_filter_classic",
    "oscat_components_compressor_pressure_filter_components",
    "oscat_components_chiller_temperature_pid_classic",
    "oscat_components_chiller_temperature_pid_components",
    "oscat_components_boiler_feedwater_alarm_classic",
    "oscat_components_boiler_feedwater_alarm_components",
    "oscat_components_weather_station_conversion_classic",
    "oscat_components_weather_station_conversion_components",
];

fn example_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join(name)
}

fn run_example_st_tests(name: &str) -> Result<(), String> {
    let project = example_path(name);
    let output = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(["test", "--project"])
        .arg(&project)
        .output()
        .expect("run trust-runtime test");

    if output.status.success() {
        return Ok(());
    }

    Err(format!(
        "expected ST example tests to pass for {name} at {}\nstdout:\n{}\nstderr:\n{}",
        project.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
}

#[test]
fn oscat_components_example_st_unit_tests_pass() {
    let next = Arc::new(AtomicUsize::new(0));
    let failures = Arc::new(Mutex::new(Vec::new()));
    let worker_count = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(4)
        .clamp(1, 6)
        .min(OSCAT_COMPONENT_EXAMPLES.len());

    thread::scope(|scope| {
        for _ in 0..worker_count {
            let next = Arc::clone(&next);
            let failures = Arc::clone(&failures);
            scope.spawn(move || loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(example) = OSCAT_COMPONENT_EXAMPLES.get(index) else {
                    break;
                };
                if let Err(message) = run_example_st_tests(example) {
                    failures.lock().expect("failure lock").push(message);
                }
            });
        }
    });

    let failures = failures.lock().expect("failure lock");
    assert!(
        failures.is_empty(),
        "{} OSCAT Components example project(s) failed:\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}
