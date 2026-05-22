use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use trust_runtime::world::{
    assert_world_actuator_smoke_trace, assert_world_smoke_trace, run_world_actuator_smoke,
    run_world_smoke, ActuatorState, WorldActuatorSmokeConfig, WorldSmokeConfig, WorldSmokeTrace,
};

#[test]
fn cube_floor_world_smoke_trace_proves_physics_and_handoff() -> anyhow::Result<()> {
    let trace = run_smoke_trace(WorldSmokeConfig::default())?;
    assert!(
        trace.assertions.cube_above_floor.ok,
        "cube_above_floor failed: min_cube_y={} floor_y={}",
        trace.assertions.cube_above_floor.min_cube_y, trace.assertions.cube_above_floor.floor_y
    );
    assert!(
        trace.assertions.gravity_applied.ok,
        "gravity_applied failed: max_downward_velocity_before_contact={}",
        trace
            .assertions
            .gravity_applied
            .max_downward_velocity_before_contact
    );
    assert!(
        trace.assertions.contact_fired.ok,
        "contact_fired failed: first_contact_tick={:?}",
        trace.assertions.contact_fired.first_contact_tick
    );
    assert_eq!(trace.world_abstraction.type_name, "World");
    assert_eq!(trace.world_abstraction.solvers_registered, ["rapier3d"]);
    assert_eq!(
        trace.transform_handoff.function,
        "trust_runtime::world::apply_rapier_body_pose_to_scena_node"
    );
    assert_eq!(
        trace.transform_handoff.read_source,
        "rapier3d::dynamics::RigidBody::position"
    );

    let repeat = run_smoke_trace(WorldSmokeConfig::default())?;
    assert_eq!(trace.determinism_trace_hash, repeat.determinism_trace_hash);

    Ok(())
}

#[test]
fn workpiece_fixture_actuator_smoke_trace_proves_joint_driven_carry() -> anyhow::Result<()> {
    let trace = run_actuator_trace(WorldActuatorSmokeConfig::default())?;
    write_trace_artifact(&trace)?;

    assert_p1_positive_assertions(&trace);
    assert_eq!(trace.world_abstraction.type_name, "World");
    assert_eq!(trace.world_abstraction.solvers_registered, ["rapier3d"]);
    assert_eq!(
        trace.transform_handoff.function,
        "trust_runtime::world::apply_rapier_body_pose_to_scena_node"
    );
    let actuator = trace.actuator.as_ref().expect("P1 actuator trace exists");
    assert!(
        actuator
            .state_transitions
            .iter()
            .any(|transition| transition.to == ActuatorState::Carrying),
        "actuator must enter Carrying"
    );
    assert!(
        actuator
            .state_transitions
            .iter()
            .any(|transition| transition.to == ActuatorState::Releasing),
        "actuator must enter Releasing"
    );
    let joints = trace.joints.as_ref().expect("P1 joint trace exists");
    assert_eq!(joints.joint_create_ticks.len(), 1);
    assert_eq!(joints.joint_destroy_ticks.len(), 1);

    let repeat = run_actuator_trace(WorldActuatorSmokeConfig::default())?;
    assert_eq!(trace.determinism_trace_hash, repeat.determinism_trace_hash);

    Ok(())
}

#[test]
fn cube_floor_world_smoke_without_floor_triggers_above_floor_assertion() -> anyhow::Result<()> {
    let trace = run_smoke_trace(WorldSmokeConfig {
        include_floor: false,
        ..WorldSmokeConfig::default()
    })?;
    let assertions = assert_world_smoke_trace(&trace.per_tick_trace);
    assert!(!assertions.cube_above_floor.ok);
    assert!(assertions.gravity_applied.ok);
    assert!(!assertions.contact_fired.ok);
    assert!(
        assertions.cube_above_floor.min_cube_y < 0.0,
        "floor-removed variant must fall below y=0"
    );
    Ok(())
}

#[test]
fn workpiece_fixture_no_joint_variant_fails_carry_assertion() -> anyhow::Result<()> {
    let trace = run_actuator_trace(WorldActuatorSmokeConfig {
        create_joint: false,
        ..WorldActuatorSmokeConfig::default()
    })?;
    let assertions = assert_world_actuator_smoke_trace(&trace.per_tick_trace);
    assert!(
        assertions
            .grip_event_has_contact
            .expect("grip assertion exists")
            .ok,
        "no-joint variant should still grip only from contact"
    );
    assert!(
        !assertions
            .carry_constraint_driven
            .expect("carry assertion exists")
            .ok,
        "no-joint variant must fail the joint-driven carry assertion"
    );
    Ok(())
}

#[test]
fn workpiece_fixture_floor_removed_triggers_above_floor_assertions() -> anyhow::Result<()> {
    let trace = run_actuator_trace(WorldActuatorSmokeConfig {
        include_floor: false,
        drive_carrier: false,
        ..WorldActuatorSmokeConfig::default()
    })?;
    let assertions = assert_world_actuator_smoke_trace(&trace.per_tick_trace);
    assert!(
        !assertions
            .workpiece_above_floor
            .expect("workpiece assertion exists")
            .ok,
        "floor-removed variant must let the workpiece fall below y=0"
    );
    assert!(
        !assertions
            .carrier_above_floor
            .expect("carrier assertion exists")
            .ok,
        "floor-removed variant with disabled carrier motor must let the carrier fall below y=0"
    );
    Ok(())
}

#[test]
fn cube_floor_world_smoke_bypass_fixture_is_rejected_by_lint() {
    assert_fixture_rejected(
        "crates/trust-runtime/tests/fixtures/world_smoke_transform_bypass.rs",
        "forbidden dynamic-body transform write",
    );
}

#[test]
fn workpiece_fixture_pose_copy_bypass_fixture_is_rejected_by_lint() {
    assert_fixture_rejected(
        "crates/trust-runtime/tests/fixtures/world_smoke_pose_copy_bypass.rs",
        "forbidden carrier-to-workpiece pose copy",
    );
}

#[test]
fn workpiece_fixture_teleport_bypass_fixture_is_rejected_by_lint() {
    assert_fixture_rejected(
        "crates/trust-runtime/tests/fixtures/world_smoke_workpiece_teleport_bypass.rs",
        "forbidden workpiece rigid-body teleport",
    );
}

#[test]
fn cube_floor_world_smoke_handoff_lint_accepts_repo_boundary() {
    let root = repo_root();
    let output = Command::new("node")
        .current_dir(&root)
        .arg("scripts/check_world_smoke_transform_handoff.mjs")
        .arg("--repo")
        .output()
        .expect("node lint command starts");

    assert!(
        output.status.success(),
        "repo handoff lint failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_smoke_trace(config: WorldSmokeConfig) -> anyhow::Result<WorldSmokeTrace> {
    let mut scene = scena::Scene::new();
    let cube_node = scene.add_empty(scene.root(), scena::Transform::IDENTITY)?;
    run_world_smoke(config, &mut scene, cube_node)
}

fn run_actuator_trace(config: WorldActuatorSmokeConfig) -> anyhow::Result<WorldSmokeTrace> {
    let mut scene = scena::Scene::new();
    let carrier_node = scene.add_empty(scene.root(), scena::Transform::IDENTITY)?;
    let workpiece_node = scene.add_empty(scene.root(), scena::Transform::IDENTITY)?;
    run_world_actuator_smoke(config, &mut scene, carrier_node, workpiece_node)
}

fn write_trace_artifact(trace: &WorldSmokeTrace) -> anyhow::Result<()> {
    let artifact_path = repo_root().join("target/gate-artifacts/world_smoke_trace.json");
    fs::create_dir_all(artifact_path.parent().expect("artifact path has parent"))?;
    fs::write(&artifact_path, serde_json::to_vec_pretty(trace)?)?;
    Ok(())
}

fn assert_p1_positive_assertions(trace: &WorldSmokeTrace) {
    let assertions = &trace.assertions;
    assert!(
        assertions
            .workpiece_above_floor
            .as_ref()
            .expect("workpiece assertion exists")
            .ok,
        "workpiece must stay above the floor"
    );
    assert!(
        assertions
            .carrier_above_floor
            .as_ref()
            .expect("carrier assertion exists")
            .ok,
        "carrier must stay above the floor"
    );
    assert!(
        assertions
            .no_fixture_interpenetration
            .as_ref()
            .expect("fixture assertion exists")
            .ok,
        "fixture interpenetration must stay within tolerance"
    );
    assert!(
        assertions
            .grip_event_has_contact
            .as_ref()
            .expect("grip assertion exists")
            .ok,
        "grip must happen from a Rapier contact pair"
    );
    assert!(
        assertions
            .carry_constraint_driven
            .as_ref()
            .expect("carry assertion exists")
            .ok,
        "carry must be driven by an active fixed joint"
    );
    assert!(
        assertions
            .release_destroyed_joint
            .as_ref()
            .expect("release assertion exists")
            .ok,
        "release must destroy the fixed joint"
    );
    assert!(
        assertions
            .workpiece_settled_on_fixture
            .as_ref()
            .expect("settle assertion exists")
            .ok,
        "workpiece must settle on the fixture"
    );
}

fn assert_fixture_rejected(fixture: &str, expected: &str) {
    let root = repo_root();
    let output = Command::new("node")
        .current_dir(&root)
        .arg("scripts/check_world_smoke_transform_handoff.mjs")
        .arg("--fixture")
        .arg(fixture)
        .output()
        .expect("node lint command starts");

    assert!(
        !output.status.success(),
        "bypass fixture must be rejected by the lint"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(expected),
        "lint stderr should contain '{expected}', got: {stderr}"
    );
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate has workspace parent")
        .parent()
        .expect("workspace root exists")
        .to_path_buf()
}
