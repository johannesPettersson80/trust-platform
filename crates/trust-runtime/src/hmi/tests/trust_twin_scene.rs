use std::collections::BTreeMap;

use crate::value::Value;

#[test]
fn trust_twin_scene_bridge_applies_rotation_binding_to_scena_node() {
    let view: HmiSceneViewPayload = toml::from_str(
        r##"
[[node]]
id = "motor-1.shaft"
primitive = "box"
[node.transform]
position = [0.0, 0.0, 0.0]
rotation = [0.0, 0.0, 0.0]
scale = [1.0, 0.35, 0.35]
[node.material]
base_color = "#3b82f6"

[[camera]]
id = "main"
position = [0.0, 0.0, 4.0]
target = [0.0, 0.0, 0.0]

[[light]]
id = "key"
kind = "directional"
intensity = 1.0

[[bind3d]]
node = "motor-1.shaft"
property = "transform.rotation.y"
source = "Program.motor1.shaft_angle"
scale = { min = 0.0, max = 10.0, output_min = 0.0, output_max = 3.14159265 }
"##,
    )
    .expect("view fixture parses");

    let mut scene = build_trust_twin_scene(&view).expect("build trust-twin scena bridge");
    assert!(scene.scena_scene().active_camera().is_some());
    assert!(scene.node_key("motor-1.shaft").is_some());

    let mut values = BTreeMap::new();
    values.insert("Program.motor1.shaft_angle".to_string(), Value::Real(5.0));
    let report = scene
        .apply_bindings(&view.bindings3d, &values)
        .expect("apply bind3d values");

    assert_eq!(report.applied.len(), 1);
    assert!(report.missing_sources.is_empty());
    assert!(report.errors.is_empty());
    let state = scene
        .node_state("motor-1.shaft")
        .expect("motor node state remains addressable");
    assert!(
        (state.rotation[1] - std::f32::consts::FRAC_PI_2).abs() < 0.000_01,
        "rotation.y should be scaled into radians, got {}",
        state.rotation[1]
    );
}

#[test]
fn trust_twin_scene_bridge_propagates_parented_joint_rotation_to_descendants() {
    let view: HmiSceneViewPayload = toml::from_str(
        r##"
[[node]]
id = "ROBOT-1"
primitive = "box"
parent = ""
local_position = [0.0, 0.0, 0.0]
pivot = [0.0, 0.0, 0.0]
[node.transform]
scale = [0.5, 0.5, 0.5]

[[node]]
id = "ROBOT-1.shoulder"
primitive = "box"
parent = "ROBOT-1"
local_position = [0.0, 1.0, 0.0]
pivot = [0.0, 0.0, 0.0]
[node.transform]
scale = [1.0, 0.2, 0.2]

[[node]]
id = "ROBOT-1.elbow"
primitive = "box"
parent = "ROBOT-1.shoulder"
local_position = [1.0, 0.0, 0.0]
pivot = [0.0, 0.0, 0.0]
[node.transform]
scale = [0.8, 0.18, 0.18]

[[node]]
id = "ROBOT-1.wrist"
primitive = "box"
parent = "ROBOT-1.elbow"
local_position = [0.8, 0.0, 0.0]
pivot = [0.0, 0.0, 0.0]
[node.transform]
scale = [0.4, 0.16, 0.16]

[[node]]
id = "GRIPPER-1"
primitive = "box"
parent = "ROBOT-1.wrist"
local_position = [1.0, 0.0, 0.0]
pivot = [0.0, 0.0, 0.0]
[node.transform]
scale = [0.2, 0.2, 0.2]

[[bind3d]]
node = "ROBOT-1.shoulder"
property = "transform.rotation.z"
source = "Main.RobotShoulderAngle"

[[bind3d]]
node = "ROBOT-1.elbow"
property = "transform.rotation.z"
source = "Main.RobotElbowAngle"

[[bind3d]]
node = "ROBOT-1.wrist"
property = "transform.rotation.z"
source = "Main.RobotWristAngle"
"##,
    )
    .expect("view fixture parses");

    let mut scene = build_trust_twin_scene(&view).expect("build parented scene");
    assert_eq!(scene.node_parent_id("ROBOT-1.elbow"), Some("ROBOT-1.shoulder"));
    assert_eq!(scene.node_parent_id("ROBOT-1.wrist"), Some("ROBOT-1.elbow"));
    assert_eq!(scene.node_parent_id("GRIPPER-1"), Some("ROBOT-1.wrist"));

    let initial_wrist = scene
        .node_world_position("ROBOT-1.wrist")
        .expect("initial wrist world position");
    let initial_gripper = scene
        .node_world_position("GRIPPER-1")
        .expect("initial gripper world position");

    let mut values = BTreeMap::new();
    values.insert(
        "Main.RobotShoulderAngle".to_string(),
        Value::Real(std::f32::consts::FRAC_PI_4),
    );
    values.insert("Main.RobotElbowAngle".to_string(), Value::Real(0.0));
    values.insert("Main.RobotWristAngle".to_string(), Value::Real(0.0));
    scene
        .apply_bindings(&view.bindings3d, &values)
        .expect("apply shoulder value");
    let shoulder_wrist = scene
        .node_world_position("ROBOT-1.wrist")
        .expect("shoulder-rotated wrist world position");
    assert!(
        distance(initial_wrist, shoulder_wrist) > 0.1,
        "rotating shoulder must move wrist through parented scene graph"
    );

    values.insert("Main.RobotShoulderAngle".to_string(), Value::Real(0.0));
    values.insert(
        "Main.RobotElbowAngle".to_string(),
        Value::Real(std::f32::consts::FRAC_PI_4),
    );
    scene
        .apply_bindings(&view.bindings3d, &values)
        .expect("apply elbow value");
    let elbow_wrist = scene
        .node_world_position("ROBOT-1.wrist")
        .expect("elbow-rotated wrist world position");
    assert!(
        distance(initial_wrist, elbow_wrist) > 0.1,
        "rotating elbow must move wrist through parented scene graph"
    );

    values.insert("Main.RobotElbowAngle".to_string(), Value::Real(0.0));
    values.insert(
        "Main.RobotWristAngle".to_string(),
        Value::Real(std::f32::consts::FRAC_PI_4),
    );
    scene
        .apply_bindings(&view.bindings3d, &values)
        .expect("apply wrist value");
    let wrist_gripper = scene
        .node_world_position("GRIPPER-1")
        .expect("wrist-rotated gripper world position");
    assert!(
        distance(initial_gripper, wrist_gripper) > 0.1,
        "rotating wrist must move gripper through parented scene graph"
    );
}

#[test]
fn trust_twin_scene_bridge_reparents_workpiece_from_pickup_to_gripper_to_drop() {
    let view: HmiSceneViewPayload = toml::from_str(
        r##"
[[node]]
id = "PICKUP-1"
primitive = "box"
parent = ""
local_position = [0.0, 0.0, 0.0]
pivot = [0.0, 0.0, 0.0]
[node.transform]
scale = [1.4, 0.3, 1.0]

[[node]]
id = "GRIPPER-1"
primitive = "box"
parent = ""
local_position = [1.0, 1.0, 0.0]
pivot = [0.0, 0.0, 0.0]
[node.transform]
scale = [0.2, 0.2, 0.2]

[[node]]
id = "DROP-1"
primitive = "box"
parent = ""
local_position = [4.0, 0.0, 0.0]
pivot = [0.0, 0.0, 0.0]
[node.transform]
scale = [1.4, 0.3, 1.0]

[[node]]
id = "BOX-1"
primitive = "box"
parent = "PICKUP-1"
local_position = [0.0, 0.35, 0.0]
pivot = [0.0, 0.0, 0.0]
[node.transform]
scale = [0.4, 0.4, 0.4]

[[node.parent_pose]]
parent = "PICKUP-1"
local_position = [0.0, 0.35, 0.0]

[[node.parent_pose]]
parent = "GRIPPER-1"
local_position = [0.0, -0.28, 0.0]

[[node.parent_pose]]
parent = "DROP-1"
local_position = [0.0, 0.35, 0.0]

[[bind3d]]
node = "BOX-1"
property = "parent"
source = "Main.RobotBoxParentState"
map = { "0" = "PICKUP-1", "1" = "GRIPPER-1", "2" = "DROP-1" }
"##,
    )
    .expect("view fixture parses");

    let mut scene = build_trust_twin_scene(&view).expect("build reparent scene");
    assert_eq!(scene.node_parent_id("BOX-1"), Some("PICKUP-1"));

    let mut values = BTreeMap::new();
    values.insert("Main.RobotBoxParentState".to_string(), Value::DInt(1));
    let report = scene
        .apply_bindings(&view.bindings3d, &values)
        .expect("apply gripper parent state");
    assert!(report.errors.is_empty(), "parent binding should apply cleanly");
    assert_eq!(scene.node_parent_id("BOX-1"), Some("GRIPPER-1"));
    assert_eq!(
        scene.node_state("BOX-1").expect("box state").position,
        [0.0, -0.28, 0.0],
        "BOX-1 local pose must switch to the gripper frame"
    );

    values.insert("Main.RobotBoxParentState".to_string(), Value::DInt(2));
    scene
        .apply_bindings(&view.bindings3d, &values)
        .expect("apply drop parent state");
    assert_eq!(scene.node_parent_id("BOX-1"), Some("DROP-1"));
    assert_eq!(
        scene.node_state("BOX-1").expect("box state").position,
        [0.0, 0.35, 0.0],
        "BOX-1 local pose must switch to the drop surface frame"
    );
}

#[test]
fn trust_twin_static_view_proof_writes_gate_artifact_with_capabilities() {
    let view: HmiSceneViewPayload = toml::from_str(
        r##"
[[node]]
id = "motor-1.shaft"
primitive = "box"
[node.transform]
scale = [1.0, 0.35, 0.35]
[node.material]
base_color = "#22c55e"

[[camera]]
id = "main"
position = [0.0, 0.0, 4.0]
target = [0.0, 0.0, 0.0]

[[light]]
id = "key"
kind = "directional"
intensity = 1.0

[[bind3d]]
node = "motor-1.shaft"
property = "transform.rotation.y"
source = "Program.motor1.shaft_angle"
"##,
    )
    .expect("view fixture parses");
    let mut values = BTreeMap::new();
    values.insert("Program.motor1.shaft_angle".to_string(), Value::Real(0.75));
    let artifact_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("target/gate-artifacts");

    let artifact = write_trust_twin_static_view_proof(&view, &values, &artifact_dir)
        .expect("write trust-twin P1 proof artifact");

    assert_eq!(artifact.driver, "scena");
    assert_eq!(artifact.binding.applied_count, 1);
    assert!(artifact.binding.errors.is_empty());
    assert!(artifact.visual.non_background_pixels > 0);
    assert!(artifact.visual.frame_sha256.len() == 64);
    assert!(artifact.proof_image.ends_with("trust-twin-p1-static-view.ppm"));
    assert!(
        artifact
            .baseline
            .tolerance
            .contains_key("non_background_pixels_min")
    );
    assert_eq!(artifact.capability_report.backend, "Headless");
    assert!(artifact.evidence_blockers.is_empty());

    let artifact_path = artifact_dir.join("trust-twin-p1-static-view.json");
    assert!(artifact_path.is_file(), "gate artifact must be written");
    let image_path = artifact_dir.join("trust-twin-p1-static-view.ppm");
    assert!(image_path.is_file(), "render proof image must be written");
}

fn distance(left: [f32; 3], right: [f32; 3]) -> f32 {
    ((left[0] - right[0]).powi(2)
        + (left[1] - right[1]).powi(2)
        + (left[2] - right[2]).powi(2))
    .sqrt()
}
