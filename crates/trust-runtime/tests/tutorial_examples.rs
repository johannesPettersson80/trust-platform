use std::path::{Path, PathBuf};

use trust_runtime::harness::{
    bytecode_module_from_source_with_path, bytecode_module_from_sources_with_paths, TestHarness,
};
use trust_runtime::value::{Duration, Value};

const HELLO_COUNTER: &str = include_str!("../../../examples/tutorials/01_hello_counter.st");
const BLINKER: &str = include_str!("../../../examples/tutorials/02_blinker.st");
const TRAFFIC_LIGHT: &str = include_str!("../../../examples/tutorials/03_traffic_light.st");
const TANK_LEVEL: &str = include_str!("../../../examples/tutorials/04_tank_level.st");
const MOTOR_STARTER: &str = include_str!("../../../examples/tutorials/05_motor_starter.st");
const RECIPE_MANAGER: &str = include_str!("../../../examples/tutorials/06_recipe_manager.st");
const PID_LOOP: &str = include_str!("../../../examples/tutorials/07_pid_loop.st");
const CONVEYOR_SYSTEM: &str = include_str!("../../../examples/tutorials/08_conveyor_system.st");
const SIMULATION_COUPLING: &str =
    include_str!("../../../examples/tutorials/09_simulation_coupling.st");
const TUTORIALS: [(&str, &str); 9] = [
    ("01_hello_counter.st", HELLO_COUNTER),
    ("02_blinker.st", BLINKER),
    ("03_traffic_light.st", TRAFFIC_LIGHT),
    ("04_tank_level.st", TANK_LEVEL),
    ("05_motor_starter.st", MOTOR_STARTER),
    ("06_recipe_manager.st", RECIPE_MANAGER),
    ("07_pid_loop.st", PID_LOOP),
    ("08_conveyor_system.st", CONVEYOR_SYSTEM),
    ("09_simulation_coupling.st", SIMULATION_COUPLING),
];

fn load_example_sources(example: &str) -> (Vec<String>, Vec<String>) {
    let src_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(example)
        .join("src");

    let mut files = std::fs::read_dir(&src_root)
        .unwrap_or_else(|err| {
            panic!(
                "failed to read example directory {}: {err}",
                src_root.display()
            )
        })
        .map(|entry| entry.expect("directory entry").path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("st"))
        .collect::<Vec<_>>();
    files.sort();

    let mut sources = Vec::with_capacity(files.len());
    let mut paths = Vec::with_capacity(files.len());
    for file in files {
        let source = std::fs::read_to_string(&file)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", file.display()));
        let file_name = file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_else(|| panic!("invalid example file name: {}", file.display()));
        sources.push(source);
        paths.push(format!("{example}/{file_name}"));
    }
    (sources, paths)
}

fn assert_example_compiles(example: &str, label: &str) {
    let (sources, paths) = load_example_sources(example);
    let source_refs = sources
        .iter()
        .map(std::string::String::as_str)
        .collect::<Vec<_>>();
    let path_refs = paths
        .iter()
        .map(std::string::String::as_str)
        .collect::<Vec<_>>();

    TestHarness::from_sources(&source_refs)
        .unwrap_or_else(|err| panic!("runtime compile failed for {label}: {err}"));
    bytecode_module_from_sources_with_paths(&source_refs, &path_refs)
        .unwrap_or_else(|err| panic!("bytecode compile failed for {label}: {err}"));
}

fn visual_example_roots() -> [(&'static str, &'static str); 3] {
    [
        ("ladder", ".ladder.json"),
        ("blockly", ".blockly.json"),
        ("statecharts", ".statechart.json"),
    ]
}

fn read_visual_companion_pair(
    visual_source_path: &Path,
    visual_suffix: &str,
) -> Option<(String, String, PathBuf, PathBuf)> {
    let file_name = visual_source_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_else(|| {
            panic!(
                "invalid visual source name: {}",
                visual_source_path.display()
            )
        });
    let base_name = file_name.strip_suffix(visual_suffix).unwrap_or_else(|| {
        panic!(
            "unexpected visual suffix for {}",
            visual_source_path.display()
        )
    });

    let parent = visual_source_path.parent().unwrap_or_else(|| {
        panic!(
            "visual source has no parent directory: {}",
            visual_source_path.display()
        )
    });
    let companion_path = parent.join(format!("{base_name}.st"));
    let runtime_path = parent.join(format!("{base_name}.visual.runtime.st"));

    if !companion_path.exists() || !runtime_path.exists() {
        return None;
    }

    let companion = std::fs::read_to_string(&companion_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", companion_path.display()));
    let runtime = std::fs::read_to_string(&runtime_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", runtime_path.display()));
    Some((companion, runtime, companion_path, runtime_path))
}

#[test]
fn tutorial_examples_parse_typecheck_and_compile_to_bytecode() {
    for (name, source) in TUTORIALS {
        TestHarness::from_source(source)
            .unwrap_or_else(|err| panic!("runtime compile failed for {name}: {err}"));
        bytecode_module_from_source_with_path(source, name)
            .unwrap_or_else(|err| panic!("bytecode compile failed for {name}: {err}"));
    }
}

#[test]
fn siemens_scl_v1_example_parse_typecheck_and_compile_to_bytecode() {
    assert_example_compiles("siemens_scl_v1", "Siemens SCL v1 example");
}

#[test]
fn mitsubishi_gxworks3_v1_example_parse_typecheck_and_compile_to_bytecode() {
    assert_example_compiles("mitsubishi_gxworks3_v1", "Mitsubishi GX Works3 v1 example");
}

#[test]
fn ethercat_ek1100_elx008_v1_example_parse_typecheck_and_compile_to_bytecode() {
    assert_example_compiles(
        "ethercat_ek1100_elx008_v1",
        "EtherCAT EK1100/ELx008 v1 example",
    );
}

#[test]
fn plcopen_xml_st_complete_example_parse_typecheck_and_compile_to_bytecode() {
    assert_example_compiles("plcopen_xml_st_complete", "PLCopen XML ST-complete example");
}

#[test]
fn visual_examples_compile_generated_companion_and_runtime_entry() {
    let examples_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut compiled_pairs = 0usize;
    for (subdir, suffix) in visual_example_roots() {
        let visual_root = examples_root.join(subdir);
        let entries = std::fs::read_dir(&visual_root).unwrap_or_else(|err| {
            panic!(
                "failed to read visual example directory {}: {err}",
                visual_root.display()
            )
        });
        for entry in entries {
            let entry = entry.expect("visual example directory entry");
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            if !file_name.ends_with(suffix) {
                continue;
            }

            let Some((companion, runtime, companion_path, runtime_path)) =
                read_visual_companion_pair(&path, suffix)
            else {
                continue;
            };
            let sources = [companion.as_str(), runtime.as_str()];
            compiled_pairs += 1;
            TestHarness::from_sources(&sources).unwrap_or_else(|err| {
                panic!(
                    "runtime compile failed for visual artifacts {} + {}: {err}",
                    companion_path.display(),
                    runtime_path.display()
                )
            });
            let companion_path_text = companion_path.to_string_lossy().to_string();
            let runtime_path_text = runtime_path.to_string_lossy().to_string();
            let path_refs = [companion_path_text.as_str(), runtime_path_text.as_str()];
            bytecode_module_from_sources_with_paths(&sources, &path_refs).unwrap_or_else(|err| {
                panic!(
                    "bytecode compile failed for visual artifacts {} + {}: {err}",
                    companion_path.display(),
                    runtime_path.display()
                )
            });
        }
    }
    assert!(
        compiled_pairs > 0,
        "expected at least one visual companion/runtime pair to compile"
    );
}

#[test]
fn tutorial_blinker_ton_timing_behavior() {
    let mut harness = TestHarness::from_source(BLINKER).expect("compile blinker tutorial");

    harness.cycle();
    harness.assert_eq("Lamp", false);

    harness.advance_time(Duration::from_millis(250));
    harness.cycle();
    harness.assert_eq("Lamp", true);

    harness.advance_time(Duration::from_millis(1));
    harness.cycle();
    harness.assert_eq("Lamp", true);

    harness.advance_time(Duration::from_millis(250));
    harness.cycle();
    harness.assert_eq("Lamp", false);
}

fn advance_traffic_phase(harness: &mut TestHarness) {
    harness.advance_time(Duration::from_millis(500));
    harness.cycle();
    harness.advance_time(Duration::from_millis(1));
    harness.cycle();
}

fn traffic_state(harness: &TestHarness) -> (Option<Value>, Option<Value>, Option<Value>) {
    (
        harness.get_output("Red"),
        harness.get_output("Yellow"),
        harness.get_output("Green"),
    )
}

#[test]
fn tutorial_traffic_light_state_sequence() {
    let mut harness = TestHarness::from_source(TRAFFIC_LIGHT).expect("compile traffic tutorial");

    harness.cycle();
    let s0 = traffic_state(&harness);

    advance_traffic_phase(&mut harness);
    let s1 = traffic_state(&harness);

    advance_traffic_phase(&mut harness);
    let s2 = traffic_state(&harness);

    advance_traffic_phase(&mut harness);
    let s3 = traffic_state(&harness);

    advance_traffic_phase(&mut harness);
    let s4 = traffic_state(&harness);

    assert_eq!(
        [s0, s1, s2, s3, s4],
        [
            (
                Some(Value::Bool(true)),
                Some(Value::Bool(false)),
                Some(Value::Bool(false))
            ),
            (
                Some(Value::Bool(true)),
                Some(Value::Bool(true)),
                Some(Value::Bool(false))
            ),
            (
                Some(Value::Bool(false)),
                Some(Value::Bool(false)),
                Some(Value::Bool(true))
            ),
            (
                Some(Value::Bool(false)),
                Some(Value::Bool(true)),
                Some(Value::Bool(false))
            ),
            (
                Some(Value::Bool(true)),
                Some(Value::Bool(false)),
                Some(Value::Bool(false))
            )
        ]
    );
}

#[test]
fn tutorial_motor_starter_latch_and_unlatch() {
    let mut harness = TestHarness::from_source(MOTOR_STARTER).expect("compile motor tutorial");

    harness.cycle();
    harness.assert_eq("MotorRun", false);

    harness.set_input("StartPb", true);
    harness.cycle();
    harness.assert_eq("MotorRun", true);
    harness.assert_eq("SealInContact", true);

    harness.set_input("StartPb", false);
    harness.cycle();
    harness.assert_eq("MotorRun", true);
    harness.assert_eq("SealInContact", true);

    harness.set_input("StopPb", true);
    harness.cycle();
    harness.assert_eq("MotorRun", false);
    harness.assert_eq("SealInContact", false);

    harness.set_input("StopPb", false);
    harness.set_input("StartPb", true);
    harness.cycle();
    harness.assert_eq("MotorRun", true);

    harness.set_input("StartPb", false);
    harness.set_input("OverloadTrip", true);
    harness.cycle();
    harness.assert_eq("MotorRun", false);
    harness.assert_eq("SealInContact", false);
}
