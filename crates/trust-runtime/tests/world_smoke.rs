use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use trust_runtime::world::{
    assert_world_actuator_smoke_trace, assert_world_multi_actuator_smoke_trace,
    assert_world_multi_urdf_arm_smoke_trace, assert_world_smoke_trace,
    assert_world_urdf_arm_smoke_trace, record_determinism_hash_stability,
    record_multi_urdf_arm_determinism_hash_stability, record_urdf_arm_determinism_hash_stability,
    run_world_actuator_smoke, run_world_multi_actuator_smoke, run_world_multi_urdf_arm_smoke,
    run_world_smoke, run_world_urdf_arm_smoke, ActuatorState, WorldActuatorSmokeConfig,
    WorldMultiActuatorScenario, WorldMultiActuatorSmokeConfig, WorldMultiUrdfArmScenario,
    WorldMultiUrdfArmSmokeConfig, WorldSmokeConfig, WorldSmokeTrace, WorldUrdfArmScenario,
    WorldUrdfArmSmokeConfig,
};

#[test]
fn cube_floor_world_smoke_trace_proves_physics_and_handoff() -> anyhow::Result<()> {
    let trace = run_smoke_trace(WorldSmokeConfig::default())?;
    assert!(
        trace.assertions.workpiece_above_floor.ok,
        "workpiece_above_floor failed: min_y={} floor_y={}",
        trace.assertions.workpiece_above_floor.min_y,
        trace.assertions.workpiece_above_floor.floor_y
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
fn multi_actuator_handoff_smoke_trace_proves_atomic_transfer() -> anyhow::Result<()> {
    let mut trace = run_multi_trace(WorldMultiActuatorSmokeConfig::default())?;
    let repeat = run_multi_trace(WorldMultiActuatorSmokeConfig::default())?;
    assert_eq!(trace.determinism_trace_hash, repeat.determinism_trace_hash);
    record_determinism_hash_stability(&mut trace, repeat.determinism_trace_hash);

    assert_p2_positive_assertions(&trace);
    let handoff_plan = trace.handoff_plan.as_ref().expect("P2 handoff plan exists");
    let handoff_tick = handoff_plan.atomic_tick.expect("handoff tick recorded");
    let handoff_sample = trace
        .per_tick_trace
        .iter()
        .find(|sample| sample.tick == handoff_tick)
        .expect("handoff tick sample exists");
    let observed = handoff_sample
        .tick_events
        .iter()
        .filter(|event| event.starts_with("joint_") || event.starts_with("state_transition("))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(observed, handoff_plan.atomic_event_order);

    Ok(())
}

#[test]
fn urdf_arm_smoke_trace_proves_fk_is_verifier_not_writer() -> anyhow::Result<()> {
    let mut trace = run_urdf_arm_trace(WorldUrdfArmSmokeConfig::default())?;
    let repeat = run_urdf_arm_trace(WorldUrdfArmSmokeConfig::default())?;
    assert_eq!(trace.determinism_trace_hash, repeat.determinism_trace_hash);
    record_urdf_arm_determinism_hash_stability(&mut trace, repeat.determinism_trace_hash);
    write_trace_artifact(&trace)?;

    assert_p3_positive_assertions(&trace);
    let urdf = trace.urdf.as_ref().expect("P3 URDF trace exists");
    assert_eq!(
        urdf.fixture_path,
        "crates/trust-runtime/tests/fixtures/p3_minimal_arm.urdf"
    );
    assert_eq!(urdf.links_loaded, ["base", "link_1", "link_2", "tool"]);
    assert_eq!(urdf.joints_loaded.len(), 3);
    assert!(urdf.parsed_once);
    assert!(!urdf.consulted_in_tick_loop);
    let fk = trace.fk_verifier.as_ref().expect("P3 FK verifier exists");
    assert!(fk.max_consistency_distance_m <= fk.consistency_tolerance);
    assert_eq!(fk.checked_links, ["link_1", "link_2", "tool"]);

    Ok(())
}

#[test]
fn multi_urdf_arm_handoff_smoke_trace_proves_composition() -> anyhow::Result<()> {
    let mut trace = run_multi_urdf_arm_trace(WorldMultiUrdfArmSmokeConfig::default())?;
    let repeat = run_multi_urdf_arm_trace(WorldMultiUrdfArmSmokeConfig::default())?;
    assert_eq!(trace.determinism_trace_hash, repeat.determinism_trace_hash);
    record_multi_urdf_arm_determinism_hash_stability(&mut trace, repeat.determinism_trace_hash);
    write_trace_artifact(&trace)?;

    assert_p4_positive_assertions(&trace);
    let urdf = trace.urdf.as_ref().expect("P4 URDF trace exists");
    let instances = urdf
        .instances
        .iter()
        .map(|instance| instance.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(instances, ["arm_a", "arm_b"]);

    let handoff_plan = trace.handoff_plan.as_ref().expect("P4 handoff plan exists");
    let handoff_tick = handoff_plan.atomic_tick.expect("handoff tick recorded");
    let handoff_sample = trace
        .per_tick_trace
        .iter()
        .find(|sample| sample.tick == handoff_tick)
        .expect("handoff tick sample exists");
    let observed = handoff_sample
        .tick_events
        .iter()
        .filter(|event| event.starts_with("joint_") || event.starts_with("state_transition("))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(observed, handoff_plan.atomic_event_order);

    Ok(())
}

#[test]
fn cube_floor_world_smoke_without_floor_triggers_above_floor_assertion() -> anyhow::Result<()> {
    let trace = run_smoke_trace(WorldSmokeConfig {
        include_floor: false,
        ..WorldSmokeConfig::default()
    })?;
    let assertions = assert_world_smoke_trace(&trace.per_tick_trace);
    assert!(!assertions.workpiece_above_floor.ok);
    assert!(assertions.gravity_applied.ok);
    assert!(!assertions.contact_fired.ok);
    assert!(
        assertions.workpiece_above_floor.min_y < 0.0,
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
        !assertions.workpiece_above_floor.ok,
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
fn multi_actuator_simultaneous_grip_without_plan_is_deterministic() -> anyhow::Result<()> {
    let config = WorldMultiActuatorSmokeConfig {
        scenario: WorldMultiActuatorScenario::SimultaneousGripNoHandoff,
        tick_count: 200,
        ..WorldMultiActuatorSmokeConfig::default()
    };
    let trace = run_multi_trace(config)?;
    let repeat = run_multi_trace(config)?;
    assert_eq!(trace.determinism_trace_hash, repeat.determinism_trace_hash);
    assert!(
        trace.per_tick_trace.iter().any(|tick| {
            tick.ownership
                .as_ref()
                .and_then(|ownership| ownership.owner.as_deref())
                == Some("carrier_a")
        }),
        "lower actuator id carrier_a must win simultaneous grip"
    );
    assert!(
        trace.per_tick_trace.iter().any(|tick| {
            tick.contention_faults.iter().any(|fault| {
                fault.actuator == "carrier_b"
                    && fault.code == "grip_denied_workpiece_owned_by(carrier_a)"
            })
        }),
        "carrier_b must receive contention fault"
    );
    let assertions = assert_world_multi_actuator_smoke_trace(&trace.per_tick_trace);
    assert_eq!(
        assertions
            .exclusive_ownership
            .expect("exclusive assertion exists")
            .ticks_with_two_joints,
        0
    );
    Ok(())
}

#[test]
fn multi_actuator_second_joint_while_owned_is_rejected() -> anyhow::Result<()> {
    let config = WorldMultiActuatorSmokeConfig {
        scenario: WorldMultiActuatorScenario::SecondGripWhileOwned,
        ..WorldMultiActuatorSmokeConfig::default()
    };
    let trace = run_multi_trace(config)?;
    assert!(
        trace.per_tick_trace.iter().any(|tick| {
            tick.contention_faults.iter().any(|fault| {
                fault.actuator == "carrier_b"
                    && fault.code == "grip_denied_workpiece_owned_by(carrier_a)"
            })
        }),
        "carrier_b grip must be denied while carrier_a owns the workpiece"
    );
    let assertions = assert_world_multi_actuator_smoke_trace(&trace.per_tick_trace);
    let exclusive = assertions
        .exclusive_ownership
        .expect("exclusive ownership assertion exists");
    assert!(exclusive.ok);
    assert_eq!(exclusive.ticks_with_two_joints, 0);
    Ok(())
}

#[test]
fn multi_actuator_reversed_registration_keeps_trace_hash_stable() -> anyhow::Result<()> {
    let canonical = run_multi_trace(WorldMultiActuatorSmokeConfig::default())?;
    let reversed = run_multi_trace(WorldMultiActuatorSmokeConfig {
        reverse_actuator_registration: true,
        ..WorldMultiActuatorSmokeConfig::default()
    })?;
    assert_eq!(
        canonical.determinism_trace_hash,
        reversed.determinism_trace_hash
    );
    Ok(())
}

#[test]
fn multi_actuator_floor_removed_triggers_above_floor_assertions() -> anyhow::Result<()> {
    let trace = run_multi_trace(WorldMultiActuatorSmokeConfig {
        include_floor: false,
        drive_actuators: false,
        ..WorldMultiActuatorSmokeConfig::default()
    })?;
    let assertions = assert_world_multi_actuator_smoke_trace(&trace.per_tick_trace);
    assert!(
        !assertions.workpiece_above_floor.ok,
        "floor-removed variant must let the workpiece fall below y=0"
    );
    assert!(
        !assertions
            .carrier_above_floor
            .expect("carrier assertion exists")
            .ok,
        "floor-removed variant must let carriers fall below y=0"
    );
    Ok(())
}

#[test]
fn urdf_arm_missing_joint_limits_fails_closed_and_can_trigger_floor_assertion() -> anyhow::Result<()>
{
    let rejected = run_urdf_arm_trace(WorldUrdfArmSmokeConfig {
        fixture_path: "crates/trust-runtime/tests/fixtures/p3_no_joint_limits.urdf",
        ..WorldUrdfArmSmokeConfig::default()
    });
    assert!(
        rejected.is_err(),
        "normal URDF loader must reject revolute joints without limits"
    );

    let trace = run_urdf_arm_trace(WorldUrdfArmSmokeConfig {
        fixture_path: "crates/trust-runtime/tests/fixtures/p3_no_joint_limits.urdf",
        scenario: WorldUrdfArmScenario::MissingLimitsPermissive,
        tick_count: 900,
        ..WorldUrdfArmSmokeConfig::default()
    })?;
    let assertions = assert_world_urdf_arm_smoke_trace(&trace.per_tick_trace);
    let floor = assertions
        .arm_links_above_floor
        .expect("arm above-floor assertion exists");
    assert!(
        !floor.ok,
        "missing-limits permissive variant must drive a link below floor"
    );
    assert!(
        floor.min_link_y < 0.0,
        "floor assertion must fail with min_link_y < 0, got {}",
        floor.min_link_y
    );
    Ok(())
}

#[test]
fn urdf_arm_fk_drift_variant_triggers_fk_consistency_assertion() -> anyhow::Result<()> {
    let trace = run_urdf_arm_trace(WorldUrdfArmSmokeConfig {
        scenario: WorldUrdfArmScenario::FkDrift,
        tick_count: 1200,
        ..WorldUrdfArmSmokeConfig::default()
    })?;
    let assertions = assert_world_urdf_arm_smoke_trace(&trace.per_tick_trace);
    let fk = assertions
        .fk_matches_rapier
        .expect("FK consistency assertion exists");
    assert!(
        !fk.ok,
        "FK drift variant must fail FK/Rapier consistency assertion; max={} tolerance={}",
        fk.max_consistency_distance_m, fk.tolerance
    );
    assert!(
        fk.max_consistency_distance_m > fk.tolerance,
        "FK drift max distance must exceed tolerance"
    );
    Ok(())
}

#[test]
fn multi_urdf_simultaneous_grip_without_plan_is_deterministic() -> anyhow::Result<()> {
    let config = WorldMultiUrdfArmSmokeConfig {
        scenario: WorldMultiUrdfArmScenario::SimultaneousGripNoHandoff,
        tick_count: 500,
        ..WorldMultiUrdfArmSmokeConfig::default()
    };
    let trace = run_multi_urdf_arm_trace(config)?;
    let repeat = run_multi_urdf_arm_trace(config)?;
    assert_eq!(trace.determinism_trace_hash, repeat.determinism_trace_hash);
    assert!(
        trace.per_tick_trace.iter().any(|tick| {
            tick.ownership
                .as_ref()
                .and_then(|ownership| ownership.owner.as_deref())
                == Some("arm_a")
        }),
        "lower-id URDF arm_a must win simultaneous grip"
    );
    assert!(
        trace.per_tick_trace.iter().any(|tick| {
            tick.contention_faults.iter().any(|fault| {
                fault.actuator == "arm_b" && fault.code == "grip_denied_workpiece_owned_by(arm_a)"
            })
        }),
        "URDF arm_b must receive a contention fault"
    );
    let assertions = assert_world_multi_urdf_arm_smoke_trace(&trace.per_tick_trace);
    assert_eq!(
        assertions
            .exclusive_ownership
            .expect("exclusive assertion exists")
            .ticks_with_two_joints,
        0
    );
    Ok(())
}

#[test]
fn multi_urdf_second_arm_grip_while_owned_is_rejected() -> anyhow::Result<()> {
    let config = WorldMultiUrdfArmSmokeConfig {
        scenario: WorldMultiUrdfArmScenario::SecondGripWhileOwned,
        ..WorldMultiUrdfArmSmokeConfig::default()
    };
    let trace = run_multi_urdf_arm_trace(config)?;
    assert!(
        trace.per_tick_trace.iter().any(|tick| {
            tick.contention_faults.iter().any(|fault| {
                fault.actuator == "arm_b" && fault.code == "grip_denied_workpiece_owned_by(arm_a)"
            })
        }),
        "URDF arm_b grip must be denied while arm_a owns the workpiece"
    );
    let assertions = assert_world_multi_urdf_arm_smoke_trace(&trace.per_tick_trace);
    let exclusive = assertions
        .exclusive_ownership
        .expect("exclusive ownership assertion exists");
    assert!(exclusive.ok);
    assert_eq!(exclusive.ticks_with_two_joints, 0);
    Ok(())
}

#[test]
fn multi_urdf_reversed_registration_keeps_trace_hash_stable() -> anyhow::Result<()> {
    let canonical = run_multi_urdf_arm_trace(WorldMultiUrdfArmSmokeConfig::default())?;
    let reversed = run_multi_urdf_arm_trace(WorldMultiUrdfArmSmokeConfig {
        reverse_arm_registration: true,
        ..WorldMultiUrdfArmSmokeConfig::default()
    })?;
    assert_eq!(
        canonical.determinism_trace_hash,
        reversed.determinism_trace_hash
    );
    Ok(())
}

#[test]
fn multi_urdf_receiver_fk_drift_is_isolated() -> anyhow::Result<()> {
    let trace = run_multi_urdf_arm_trace(WorldMultiUrdfArmSmokeConfig {
        scenario: WorldMultiUrdfArmScenario::FkDriftReceiver,
        tick_count: 1700,
        ..WorldMultiUrdfArmSmokeConfig::default()
    })?;
    let assertions = assert_world_multi_urdf_arm_smoke_trace(&trace.per_tick_trace);
    let per_arm = assertions
        .per_arm_fk_consistency
        .expect("per-arm FK consistency assertion exists");
    assert!(
        !per_arm.ok,
        "receiver FK drift variant must fail the per-arm FK assertion"
    );
    let arm_a = per_arm
        .max_consistency_distance_by_arm
        .get("arm_a")
        .copied()
        .expect("arm_a FK stat exists");
    let arm_b = per_arm
        .max_consistency_distance_by_arm
        .get("arm_b")
        .copied()
        .expect("arm_b FK stat exists");
    assert!(
        arm_a <= per_arm.tolerance,
        "arm_a must remain within FK tolerance; arm_a={arm_a} tolerance={}",
        per_arm.tolerance
    );
    assert!(
        arm_b > per_arm.tolerance,
        "arm_b must exceed FK tolerance after drift; arm_b={arm_b} tolerance={}",
        per_arm.tolerance
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
fn urdf_arm_fk_to_transform_bypass_fixture_is_rejected_by_lint() {
    assert_fixture_rejected(
        "crates/trust-runtime/tests/fixtures/world_smoke_fk_to_transform_bypass.rs",
        "forbidden FK-to-transform write",
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

fn run_multi_trace(config: WorldMultiActuatorSmokeConfig) -> anyhow::Result<WorldSmokeTrace> {
    let mut scene = scena::Scene::new();
    let carrier_a_node = scene.add_empty(scene.root(), scena::Transform::IDENTITY)?;
    let carrier_b_node = scene.add_empty(scene.root(), scena::Transform::IDENTITY)?;
    let workpiece_node = scene.add_empty(scene.root(), scena::Transform::IDENTITY)?;
    run_world_multi_actuator_smoke(
        config,
        &mut scene,
        carrier_a_node,
        carrier_b_node,
        workpiece_node,
    )
}

fn run_urdf_arm_trace(config: WorldUrdfArmSmokeConfig) -> anyhow::Result<WorldSmokeTrace> {
    let mut scene = scena::Scene::new();
    let link_1_node = scene.add_empty(scene.root(), scena::Transform::IDENTITY)?;
    let link_2_node = scene.add_empty(scene.root(), scena::Transform::IDENTITY)?;
    let tool_node = scene.add_empty(scene.root(), scena::Transform::IDENTITY)?;
    let workpiece_node = scene.add_empty(scene.root(), scena::Transform::IDENTITY)?;
    run_world_urdf_arm_smoke(
        config,
        &mut scene,
        link_1_node,
        link_2_node,
        tool_node,
        workpiece_node,
    )
}

fn run_multi_urdf_arm_trace(
    config: WorldMultiUrdfArmSmokeConfig,
) -> anyhow::Result<WorldSmokeTrace> {
    let mut scene = scena::Scene::new();
    let arm_a_link_1_node = scene.add_empty(scene.root(), scena::Transform::IDENTITY)?;
    let arm_a_link_2_node = scene.add_empty(scene.root(), scena::Transform::IDENTITY)?;
    let arm_a_tool_node = scene.add_empty(scene.root(), scena::Transform::IDENTITY)?;
    let arm_b_link_1_node = scene.add_empty(scene.root(), scena::Transform::IDENTITY)?;
    let arm_b_link_2_node = scene.add_empty(scene.root(), scena::Transform::IDENTITY)?;
    let arm_b_tool_node = scene.add_empty(scene.root(), scena::Transform::IDENTITY)?;
    let workpiece_node = scene.add_empty(scene.root(), scena::Transform::IDENTITY)?;
    run_world_multi_urdf_arm_smoke(
        config,
        &mut scene,
        arm_a_link_1_node,
        arm_a_link_2_node,
        arm_a_tool_node,
        arm_b_link_1_node,
        arm_b_link_2_node,
        arm_b_tool_node,
        workpiece_node,
    )
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
        assertions.workpiece_above_floor.ok,
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

fn assert_p2_positive_assertions(trace: &WorldSmokeTrace) {
    assert_p1_positive_assertions(trace);
    let assertions = &trace.assertions;
    assert!(
        assertions
            .exclusive_ownership
            .as_ref()
            .expect("exclusive assertion exists")
            .ok,
        "workpiece ownership must be exclusive"
    );
    assert!(
        assertions
            .ownership_transfer_atomic
            .as_ref()
            .expect("atomic transfer assertion exists")
            .ok,
        "ownership transfer must be atomic"
    );
    assert!(
        assertions
            .handoff_order_deterministic
            .as_ref()
            .expect("handoff order assertion exists")
            .ok,
        "handoff event order must match the contract"
    );
    assert!(
        assertions
            .no_phantom_carry
            .as_ref()
            .expect("phantom carry assertion exists")
            .ok,
        "actuator carry states must agree with active joints"
    );
    assert!(
        assertions
            .determinism_hash_stable
            .as_ref()
            .expect("determinism assertion exists")
            .ok,
        "repeated P2 run must produce the same trace hash"
    );
}

fn assert_p3_positive_assertions(trace: &WorldSmokeTrace) {
    assert_p1_positive_assertions(trace);
    let assertions = &trace.assertions;
    assert!(
        assertions
            .urdf_parsed_once
            .as_ref()
            .expect("URDF parsed-once assertion exists")
            .ok,
        "URDF must be parsed once at setup and not consulted in the tick loop"
    );
    assert!(
        assertions
            .arm_rendered_through_handoff
            .as_ref()
            .expect("arm handoff assertion exists")
            .ok,
        "all arm links must be traced through the audited handoff"
    );
    assert!(
        assertions
            .fk_matches_rapier
            .as_ref()
            .expect("FK consistency assertion exists")
            .ok,
        "FK verifier must match Rapier-owned link positions"
    );
    assert!(
        assertions
            .joint_limits_enforced
            .as_ref()
            .expect("joint-limit assertion exists")
            .ok,
        "URDF joint limits must be enforced"
    );
    assert!(
        assertions
            .arm_links_above_floor
            .as_ref()
            .expect("arm above-floor assertion exists")
            .ok,
        "arm links must stay above the floor"
    );
    assert!(
        assertions
            .determinism_hash_stable
            .as_ref()
            .expect("P3 determinism assertion exists")
            .ok,
        "repeated P3 run must produce the same trace hash"
    );
}

fn assert_p4_positive_assertions(trace: &WorldSmokeTrace) {
    assert_p2_positive_assertions(trace);
    assert_p3_positive_assertions(trace);
    let assertions = &trace.assertions;
    assert!(
        assertions
            .multi_urdf_arms_loaded
            .as_ref()
            .expect("multi-URDF loaded assertion exists")
            .ok,
        "both URDF arm instances must be loaded"
    );
    assert!(
        assertions
            .per_arm_fk_consistency
            .as_ref()
            .expect("per-arm FK assertion exists")
            .ok,
        "each URDF arm must pass FK consistency independently"
    );
    let rendered = assertions
        .arm_rendered_through_handoff
        .as_ref()
        .expect("arm handoff assertion exists");
    assert_eq!(
        rendered.expected_dynamic_bodies_per_tick, 7,
        "P4 must hand off 3 links per arm plus the workpiece"
    );
    let joints = trace.joints.as_ref().expect("P4 joint trace exists");
    assert_eq!(
        joints
            .active_by_tick_summary
            .as_ref()
            .expect("active joint summary exists")
            .ticks_with_two_joints,
        0,
        "P4 must never have two active joints against the workpiece"
    );
    let fk = trace
        .fk_verifier
        .as_ref()
        .expect("P4 FK verifier trace exists");
    assert_eq!(fk.per_arm.len(), 2);
    assert!(
        fk.per_arm
            .values()
            .all(|arm| arm.max_consistency_distance_m <= fk.consistency_tolerance),
        "both per-arm FK distributions must stay within tolerance"
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
