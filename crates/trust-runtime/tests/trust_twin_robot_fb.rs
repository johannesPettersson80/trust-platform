use std::fs;
use std::path::{Path, PathBuf};

use trust_runtime::harness::TestHarness;
use trust_runtime::stdlib::fbs::{builtin_kind, BuiltinFbKind};
use trust_runtime::value::Value;

#[test]
fn generated_robot_fb_executes_native_world_arm_bridge() {
    let fb_source = generated_robot_fb_source();
    let program = r#"
PROGRAM Main
VAR
    Robot : Robot_P3MinimalArm;
    RobotEnabled : BOOL := TRUE;
    RobotCommand : INT := 0;
    RobotMotionStep : INT := 0;
    RobotShoulderAngle : REAL := 0.0;
    RobotElbowAngle : REAL := 0.0;
    RobotWristAngle : REAL := 0.0;
    RobotGripperOpen : BOOL := TRUE;
    RobotGripperX : REAL := 0.0;
    RobotGripperY : REAL := 0.0;
    RobotGripperZ : REAL := 0.0;
    RobotBoxX : REAL := 0.0;
    RobotBoxY : REAL := 0.0;
    RobotBoxZ : REAL := 0.0;
    RobotEnabledOut : BOOL := FALSE;
    RobotHasWorkpiece : BOOL := FALSE;
    RobotStatusLight : BOOL := FALSE;
END_VAR

Robot(Enable := RobotEnabled, Command := RobotCommand);
RobotMotionStep := Robot.State;
RobotShoulderAngle := Robot.Joint1;
RobotElbowAngle := Robot.Joint2;
RobotWristAngle := Robot.ToolYaw;
RobotGripperOpen := Robot.GripperOpen;
RobotGripperX := Robot.ToolX;
RobotGripperY := Robot.ToolY;
RobotGripperZ := Robot.ToolZ;
RobotBoxX := Robot.WorkpieceX;
RobotBoxY := Robot.WorkpieceY;
RobotBoxZ := Robot.WorkpieceZ;
RobotEnabledOut := Robot.EnabledOut;
RobotHasWorkpiece := Robot.HasWorkpiece;
RobotStatusLight := Robot.StatusLight;
END_PROGRAM
"#;
    let mut harness =
        TestHarness::from_sources(&[fb_source.as_str(), program]).expect("compile robot FB");

    harness.set_input("RobotCommand", Value::Int(1));
    harness.cycle();
    harness.assert_eq("RobotMotionStep", Value::Int(1));
    harness.assert_eq("RobotGripperOpen", Value::Bool(true));
    harness.assert_eq("RobotHasWorkpiece", Value::Bool(false));
    assert_real_output(&harness, "RobotBoxX", 0.0);

    harness.set_input("RobotCommand", Value::Int(3));
    harness.cycle();
    harness.assert_eq("RobotMotionStep", Value::Int(3));
    harness.assert_eq("RobotGripperOpen", Value::Bool(false));
    harness.assert_eq("RobotHasWorkpiece", Value::Bool(true));
    assert_real_output(&harness, "RobotBoxY", 1.15);

    harness.set_input("RobotCommand", Value::Int(6));
    harness.cycle();
    harness.assert_eq("RobotMotionStep", Value::Int(6));
    harness.assert_eq("RobotGripperOpen", Value::Bool(true));
    harness.assert_eq("RobotHasWorkpiece", Value::Bool(false));
    harness.assert_eq("RobotStatusLight", Value::Bool(false));
    assert_real_output(&harness, "RobotBoxX", 4.0);
    assert_real_output(&harness, "RobotBoxY", 0.35);
}

#[test]
fn robot_p3_minimal_arm_is_registered_as_native_function_block() {
    assert_eq!(
        builtin_kind("Robot_P3MinimalArm"),
        Some(BuiltinFbKind::RobotP3MinimalArm)
    );
}

fn generated_robot_fb_source() -> String {
    fs::read_to_string(
        workspace_root().join("examples/trust-twin/robot-cell/src/Robot_P3MinimalArm.fb.st"),
    )
    .expect("read generated robot FB")
}

fn assert_real_output(harness: &TestHarness, name: &str, expected: f32) {
    let value = harness
        .get_output(name)
        .unwrap_or_else(|| panic!("missing output '{name}'"));
    let actual = match value {
        Value::Real(value) => value,
        other => panic!("expected REAL for {name}, got {other:?}"),
    };
    assert!(
        (actual - expected).abs() < 0.000_01,
        "{name}: expected {expected}, got {actual}"
    );
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}
