use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use trust_runtime::world::{
    assert_world_smoke_trace, run_world_smoke, WorldSmokeConfig, WorldSmokeTrace,
};

#[test]
fn cube_floor_world_smoke_trace_proves_physics_and_handoff() -> anyhow::Result<()> {
    let trace = run_smoke_trace(WorldSmokeConfig::default())?;
    let artifact_path = repo_root().join("target/gate-artifacts/world_smoke_trace.json");
    fs::create_dir_all(artifact_path.parent().expect("artifact path has parent"))?;
    fs::write(&artifact_path, serde_json::to_vec_pretty(&trace)?)?;

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
fn cube_floor_world_smoke_bypass_fixture_is_rejected_by_lint() {
    let root = repo_root();
    let output = Command::new("node")
        .current_dir(&root)
        .arg("scripts/check_world_smoke_transform_handoff.mjs")
        .arg("--fixture")
        .arg("crates/trust-runtime/tests/fixtures/world_smoke_transform_bypass.rs")
        .output()
        .expect("node lint command starts");

    assert!(
        !output.status.success(),
        "bypass fixture must be rejected by the lint"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("forbidden dynamic-body transform write"),
        "lint stderr should name the forbidden write path, got: {stderr}"
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

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate has workspace parent")
        .parent()
        .expect("workspace root exists")
        .to_path_buf()
}
