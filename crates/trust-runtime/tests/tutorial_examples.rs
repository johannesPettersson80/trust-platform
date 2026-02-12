use trust_runtime::harness::{bytecode_module_from_source_with_path, TestHarness};
use trust_runtime::value::Duration;

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
const SIEMENS_SCL_V1_MAIN: &str = include_str!("../../../examples/siemens_scl_v1/src/Main.st");
const MITSUBISHI_GXWORKS3_V1_MAIN: &str =
    include_str!("../../../examples/mitsubishi_gxworks3_v1/src/Main.st");

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
    TestHarness::from_source(SIEMENS_SCL_V1_MAIN)
        .expect("runtime compile failed for Siemens SCL v1 example");
    bytecode_module_from_source_with_path(SIEMENS_SCL_V1_MAIN, "siemens_scl_v1/Main.st")
        .expect("bytecode compile failed for Siemens SCL v1 example");
}

#[test]
fn mitsubishi_gxworks3_v1_example_parse_typecheck_and_compile_to_bytecode() {
    TestHarness::from_source(MITSUBISHI_GXWORKS3_V1_MAIN)
        .expect("runtime compile failed for Mitsubishi GX Works3 v1 example");
    bytecode_module_from_source_with_path(
        MITSUBISHI_GXWORKS3_V1_MAIN,
        "mitsubishi_gxworks3_v1/Main.st",
    )
    .expect("bytecode compile failed for Mitsubishi GX Works3 v1 example");
}

#[test]
fn tutorial_blinker_ton_timing_behavior() {
    let mut harness = TestHarness::from_source(BLINKER).expect("compile blinker tutorial");

    harness.cycle();
    harness.assert_eq("lamp", false);

    harness.advance_time(Duration::from_millis(250));
    harness.cycle();
    harness.assert_eq("lamp", true);

    harness.advance_time(Duration::from_millis(1));
    harness.cycle();
    harness.assert_eq("lamp", true);

    harness.advance_time(Duration::from_millis(250));
    harness.cycle();
    harness.assert_eq("lamp", false);
}

fn advance_traffic_phase(harness: &mut TestHarness) {
    harness.advance_time(Duration::from_millis(500));
    harness.cycle();
    harness.advance_time(Duration::from_millis(1));
    harness.cycle();
}

#[test]
fn tutorial_traffic_light_state_sequence() {
    let mut harness = TestHarness::from_source(TRAFFIC_LIGHT).expect("compile traffic tutorial");

    harness.cycle();
    harness.assert_eq("red", true);
    harness.assert_eq("yellow", false);
    harness.assert_eq("green", false);

    advance_traffic_phase(&mut harness);
    harness.assert_eq("red", true);
    harness.assert_eq("yellow", true);
    harness.assert_eq("green", false);

    advance_traffic_phase(&mut harness);
    harness.assert_eq("red", false);
    harness.assert_eq("yellow", false);
    harness.assert_eq("green", true);

    advance_traffic_phase(&mut harness);
    harness.assert_eq("red", false);
    harness.assert_eq("yellow", true);
    harness.assert_eq("green", false);

    advance_traffic_phase(&mut harness);
    harness.assert_eq("red", true);
    harness.assert_eq("yellow", false);
    harness.assert_eq("green", false);
}

#[test]
fn tutorial_motor_starter_latch_and_unlatch() {
    let mut harness = TestHarness::from_source(MOTOR_STARTER).expect("compile motor tutorial");

    harness.cycle();
    harness.assert_eq("motor_run", false);

    harness.set_input("start_pb", true);
    harness.cycle();
    harness.assert_eq("motor_run", true);
    harness.assert_eq("seal_in_contact", true);

    harness.set_input("start_pb", false);
    harness.cycle();
    harness.assert_eq("motor_run", true);
    harness.assert_eq("seal_in_contact", true);

    harness.set_input("stop_pb", true);
    harness.cycle();
    harness.assert_eq("motor_run", false);
    harness.assert_eq("seal_in_contact", false);

    harness.set_input("stop_pb", false);
    harness.set_input("start_pb", true);
    harness.cycle();
    harness.assert_eq("motor_run", true);

    harness.set_input("start_pb", false);
    harness.set_input("overload_trip", true);
    harness.cycle();
    harness.assert_eq("motor_run", false);
    harness.assert_eq("seal_in_contact", false);
}
