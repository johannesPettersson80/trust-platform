use trust_hir::TypeId;
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController, SimulationTraceRow};
use trust_runtime::value::{Duration, Value};
use trust_runtime::CompiledRuntime;

fn write_lreal(runtime: &mut CompiledRuntime, address: &IoAddress, value: f64) {
    runtime
        .write_io(address, Value::LWord(value.to_bits()))
        .expect("write LREAL process-image bits");
}

fn trace_for(
    plant_step_ms: i64,
    scan_ms: i64,
    uncertainty: &str,
    reset_at_ms: Option<i64>,
) -> Vec<SimulationTraceRow> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "trust-held-command-{}-{}-{stamp}.toml",
        std::process::id(),
        plant_step_ms
    ));
    let event = reset_at_ms.map_or_else(String::new, |at_ms| {
        format!(
            "\n[[plant_events]]\nat_ms = {at_ms}\nplant = \"process\"\nkind = \"reset_plant\"\n"
        )
    });
    std::fs::write(
        &path,
        format!(
            r#"
[simulation]
enabled = true
plant_step_ms = {plant_step_ms}
plant_trace_capacity = 32

[[plants]]
id = "process"
model = "static_gain"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]

[plants.parameters]
gain = 1.0

{uncertainty}
{event}
"#
        ),
    )
    .unwrap();
    let mut simulation = SimulationController::new(SimulationConfig::load(&path).unwrap());
    let command = IoAddress::parse("%QL0").unwrap();
    let mut runtime = trust_runtime::harness::runtime_with_bindings(&[
        (command.clone(), TypeId::LREAL),
        (IoAddress::parse("%IL0").unwrap(), TypeId::LREAL),
    ])
    .expect("compile the plant's bindings");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    write_lreal(&mut runtime, &command, 10.0);
    simulation
        .apply_post_cycle(Duration::from_millis(scan_ms), &runtime)
        .unwrap();
    simulation
        .apply_pre_cycle(Duration::from_millis(scan_ms), &mut runtime)
        .unwrap();
    let rows = simulation.trace_snapshot().rows;
    let _ = std::fs::remove_file(path);
    rows
}

#[test]
fn held_command_uncertainty_is_applied_once_per_substep_and_replays() {
    let additive = r#"[[plants.uncertainties]]
id = "constant-offset"
target = "command"
channel = 0
mode = "add"
generator = "constant"
value = 1.0"#;
    let additive_rows = trace_for(10, 100, additive, Some(50));
    assert_eq!(additive_rows.len(), 10);
    assert!(additive_rows
        .iter()
        .all(|row| { row.transformed_command == vec![11.0] && row.applied_input == vec![11.0] }));
    assert!(additive_rows
        .iter()
        .all(|row| row.ideal_output == vec![11.0]));

    let multiplicative = r#"[[plants.uncertainties]]
id = "constant-scale"
target = "command"
channel = 0
mode = "multiply"
generator = "constant"
value = 1.5"#;
    let multiplicative_rows = trace_for(25, 100, multiplicative, None);
    assert_eq!(multiplicative_rows.len(), 4);
    assert!(multiplicative_rows
        .iter()
        .all(|row| { row.transformed_command == vec![15.0] && row.applied_input == vec![15.0] }));

    let composed = format!(
        "{additive}\n\n[[plants.uncertainties]]\nid = \"constant-scale\"\ntarget = \"command\"\nchannel = 0\nmode = \"multiply\"\ngenerator = \"constant\"\nvalue = 1.5"
    );
    let first = trace_for(10, 100, &composed, None);
    let replay = trace_for(10, 100, &composed, None);
    assert_eq!(
        first, replay,
        "same held command and uncertainty must replay"
    );
    assert!(first
        .iter()
        .all(|row| { row.transformed_command == vec![16.5] && row.applied_input == vec![16.5] }));
}
