use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use trust_twin_compiler::{
    generate_robot_function_block_from_manifest_toml, GeneratedRobotFunctionBlock,
};

#[test]
fn robot_manifest_generates_checked_in_function_block_deterministically() {
    let root = workspace_root();
    let manifest_path = root.join("examples/trust-twin/robot-cell/robot/p3-minimal-arm.robot.toml");
    let expected_path = root.join("examples/trust-twin/robot-cell/src/Robot_P3MinimalArm.fb.st");
    let manifest = fs::read_to_string(&manifest_path).expect("read robot FB manifest");
    let expected = normalize_line_endings(
        &fs::read_to_string(&expected_path).expect("read checked-in generated robot FB"),
    );

    let first = generate(&manifest);
    let second = generate(&manifest);

    assert_eq!(first, second, "robot FB generation must be deterministic");
    assert_eq!(first.function_block_name, "Robot_P3MinimalArm");
    assert_eq!(
        first.source_urdf,
        "crates/trust-runtime/tests/fixtures/p3_minimal_arm.urdf"
    );
    assert_eq!(normalize_line_endings(&first.source), expected);
}

#[test]
fn generated_robot_function_block_declares_native_world_arm_bridge_only() {
    let root = workspace_root();
    let manifest = fs::read_to_string(
        root.join("examples/trust-twin/robot-cell/robot/p3-minimal-arm.robot.toml"),
    )
    .expect("read robot FB manifest");

    let generated = generate(&manifest);
    assert!(
        generated
            .source
            .contains("trust_runtime::world::arm::step_robot_p3_minimal_arm_bridge"),
        "generated FB must name the runtime world::arm bridge"
    );
    assert!(
        generated
            .source
            .contains("FUNCTION_BLOCK Robot_P3MinimalArm"),
        "generated FB must expose the Robot_<Model> type"
    );

    for forbidden in [
        "set_transform",
        "set_position",
        "set_translation",
        "transform.position",
        "FK_TO_TRANSFORM",
    ] {
        assert!(
            !generated.source.contains(forbidden),
            "generated FB must not contain forbidden transform bypass marker '{forbidden}'"
        );
    }
}

#[test]
fn compiler_cli_writes_generated_robot_function_block() {
    let root = workspace_root();
    let temp = std::env::temp_dir().join(format!(
        "trust-twin-robot-fb-cli-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir_all(&temp).expect("create temp root");
    let output_path = temp.join("Robot_P3MinimalArm.fb.st");
    let manifest_path = root.join("examples/trust-twin/robot-cell/robot/p3-minimal-arm.robot.toml");

    let compiler_exe =
        std::env::var("CARGO_BIN_EXE_trust-twin-compiler").expect("compiler binary path");
    let output = Command::new(compiler_exe)
        .arg("--robot-fb")
        .arg("--input")
        .arg(&manifest_path)
        .arg("--output")
        .arg(&output_path)
        .arg("--json")
        .output()
        .expect("run robot FB generator CLI");

    assert!(
        output.status.success(),
        "robot FB CLI failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("CLI JSON");
    assert_eq!(json["ok"], true);
    assert_eq!(json["mode"], "robot-fb-write");
    assert_eq!(json["function_block_name"], "Robot_P3MinimalArm");
    assert_eq!(
        json["source_urdf"],
        "crates/trust-runtime/tests/fixtures/p3_minimal_arm.urdf"
    );

    let expected = fs::read_to_string(
        root.join("examples/trust-twin/robot-cell/src/Robot_P3MinimalArm.fb.st"),
    )
    .expect("read checked-in FB");
    let actual = fs::read_to_string(output_path).expect("read generated output");
    assert_eq!(
        normalize_line_endings(&actual),
        normalize_line_endings(&expected)
    );
}

fn generate(manifest: &str) -> GeneratedRobotFunctionBlock {
    generate_robot_function_block_from_manifest_toml(manifest).expect("generate robot FB")
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn normalize_line_endings(source: &str) -> String {
    source.replace("\r\n", "\n")
}
