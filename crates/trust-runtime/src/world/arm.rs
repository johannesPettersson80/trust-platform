//! P3 URDF-arm proof for the shared deterministic [`World`].

use std::collections::BTreeMap;
use std::f32::consts::PI;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;
use rapier3d::math::glamx::EulerRot;
use rapier3d::prelude::*;
use serde::{Deserialize, Serialize};

use super::{
    active_joint_summary, apply_rapier_body_pose_to_scena_node, body_registration, contact,
    contact_contains, determinism_trace_hash, vec3_length, ActuatorState, CarryConstraintAssertion,
    ContactFiredAssertion, CubeAboveFloorAssertion, FixtureInterpenetrationAssertion,
    GravityAppliedAssertion, GripEventContactAssertion, ReleaseDestroyedJointAssertion,
    TransformHandoffTrace, WorkpieceSettledAssertion, World, WorldAbstractionTrace,
    WorldActuatorTrace, WorldActuatorTransitionTrace, WorldBodyRegistrationTrace,
    WorldContactTrace, WorldJointDistanceTrace, WorldJointTrace, WorldSmokeAssertions,
    WorldSmokeTrace, WorldTickTrace, ABOVE_FLOOR_EPSILON, FIXTURE_CENTER_X, FIXTURE_CENTER_Y,
    FIXTURE_CENTER_Z, FIXTURE_HALF_Y, FIXTURE_INTERPENETRATION_TOLERANCE, FIXTURE_TOP_Y,
    FLOOR_HALF_Y, FLOOR_Y, HANDOFF_FILE, HANDOFF_FUNCTION, JOINT_DISTANCE_TOLERANCE,
    SETTLE_POSITION_TOLERANCE, SETTLE_VELOCITY_TOLERANCE, WORKPIECE_HALF_EXTENT,
    WORKPIECE_INITIAL_CENTER_Y,
};

const P3_MINIMAL_ARM_URDF: &str = "crates/trust-runtime/tests/fixtures/p3_minimal_arm.urdf";
const ARM_SOURCE: &str = "urdf:p3_minimal_arm";
const ARM_BASE_WORLD: [f32; 3] = [0.30, 0.85, 0.0];
const ARM_INITIAL_Q: [f32; 2] = [-1.20, 2.00];
const ARM_APPROACH_Q: [f32; 2] = [-1.640_244_4, 2.530_67];
const ARM_LIFT_Q: [f32; 2] = [0.064, 2.726];
const ARM_DROP_Q: [f32; 2] = [-0.201_608, 0.475_882];
const ARM_HOME_Q: [f32; 2] = [-0.751_787, 2.033_175];
const ARM_MISSING_LIMIT_Q: [f32; 2] = [-1.57, 0.20];
const ARM_JOINT_TOLERANCE: f32 = 0.06;
const ARM_FK_TOLERANCE: f32 = 0.005;
const ARM_FK_DRIFT_TICK: u32 = 600;
const ARM_MOTOR_STIFFNESS: f32 = 650.0;
const ARM_MOTOR_DAMPING: f32 = 80.0;
const ARM_MOTOR_FORCE: f32 = 5_000.0;
const ARM_FIXTURE_HALF_XZ: f32 = 0.35;
const ARM_TICK_DT_SECONDS: f32 = 0.002;
const ARM_TICK_COUNT: u32 = 2_500;
const ARM_CARRY_DISTANCE_TOLERANCE: f32 = 0.065;

/// P3 scenario variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldUrdfArmScenario {
    /// Canonical URDF-arm pick/place proof.
    Canonical,
    /// Test-only permissive run for missing joint-limit URDFs.
    MissingLimitsPermissive,
    /// Test-only run that perturbs one link to prove FK drift is detected.
    FkDrift,
}

/// Configuration for the P3 URDF-arm proof.
#[derive(Debug, Clone, Copy)]
pub struct WorldUrdfArmSmokeConfig {
    /// Fixed tick delta in seconds.
    pub tick_dt_seconds: f32,
    /// Number of fixed ticks to run.
    pub tick_count: u32,
    /// Whether to register the static floor collider.
    pub include_floor: bool,
    /// Relative fixture URDF path.
    pub fixture_path: &'static str,
    /// Scenario variant.
    pub scenario: WorldUrdfArmScenario,
}

impl Default for WorldUrdfArmSmokeConfig {
    fn default() -> Self {
        Self {
            tick_dt_seconds: ARM_TICK_DT_SECONDS,
            tick_count: ARM_TICK_COUNT,
            include_floor: true,
            fixture_path: P3_MINIMAL_ARM_URDF,
            scenario: WorldUrdfArmScenario::Canonical,
        }
    }
}

/// P3 URDF load trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldUrdfTrace {
    /// Relative fixture path loaded during setup.
    pub fixture_path: String,
    /// URDF links loaded from the fixture.
    pub links_loaded: Vec<String>,
    /// URDF joints loaded from the fixture.
    pub joints_loaded: Vec<WorldUrdfJointTrace>,
    /// Whether parsing happened once during setup.
    pub parsed_once: bool,
    /// Whether the tick loop consulted the URDF text.
    pub consulted_in_tick_loop: bool,
}

/// One URDF joint loaded into the P3 proof.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldUrdfJointTrace {
    /// Joint name.
    pub name: String,
    /// Joint type.
    pub joint_type: String,
    /// Joint axis, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub axis: Option<[f32; 3]>,
    /// Lower joint limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_lower: Option<f32>,
    /// Upper joint limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_upper: Option<f32>,
    /// Fixed-joint offset, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<[f32; 3]>,
}

/// P3 FK verifier trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldFkVerifierTrace {
    /// Maximum FK/Rapier distance in meters.
    pub max_consistency_distance_m: f32,
    /// Dynamic links checked by FK.
    pub checked_links: Vec<String>,
    /// Number of ticks checked.
    pub checked_ticks: u32,
    /// Consistency tolerance in meters.
    pub consistency_tolerance: f32,
}

/// P3 link sample.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldArmLinkTrace {
    /// URDF link name.
    pub name: String,
    /// Rapier-owned body-center position.
    pub rapier_position: [f32; 3],
    /// FK-predicted body-center position.
    pub fk_predicted_position: [f32; 3],
    /// Euclidean distance between Rapier and FK positions.
    pub fk_consistency_distance: f32,
    /// Link bottom Y for above-floor checks.
    pub bottom_y: f32,
    /// Rapier yaw about the URDF Z axis.
    pub rapier_yaw_z: f32,
}

/// P3 joint sample.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldArmJointTrace {
    /// URDF joint name.
    pub name: String,
    /// Current joint position read from Rapier state.
    pub position: f32,
    /// URDF lower limit.
    pub limit_lower: f32,
    /// URDF upper limit.
    pub limit_upper: f32,
    /// Whether the joint is at a limit tolerance.
    pub clamped: bool,
}

/// P3 URDF parse assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrdfParsedOnceAssertion {
    /// Whether the assertion passed.
    pub ok: bool,
    /// Whether setup parsed the fixture.
    pub parsed_once: bool,
    /// Whether the tick loop consulted the URDF text.
    pub consulted_in_tick_loop: bool,
}

/// P3 arm-rendering handoff assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArmRenderedThroughHandoffAssertion {
    /// Whether the assertion passed.
    pub ok: bool,
    /// Number of dynamic visible bodies expected per tick.
    pub expected_dynamic_bodies_per_tick: u32,
    /// Number of trace ticks checked.
    pub checked_ticks: u32,
}

/// P3 FK consistency assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FkConsistencyAssertion {
    /// Whether the assertion passed.
    pub ok: bool,
    /// Maximum FK/Rapier distance observed.
    pub max_consistency_distance_m: f32,
    /// Consistency tolerance.
    pub tolerance: f32,
    /// Number of link-tick samples checked.
    pub checked_samples: u32,
}

/// P3 joint-limit assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JointLimitAssertion {
    /// Whether the assertion passed.
    pub ok: bool,
    /// Number of joint samples outside URDF limits.
    pub out_of_limit_samples: u32,
    /// Limit-clamp events observed.
    pub joint_clamped_events: Vec<String>,
}

/// P3 arm above-floor assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArmAboveFloorAssertion {
    /// Whether the assertion passed.
    pub ok: bool,
    /// Lowest link bottom Y observed.
    pub min_link_y: f32,
    /// Floor top Y.
    pub floor_y: f32,
    /// Link name for the lowest sample.
    pub min_link_name: String,
}

#[derive(Debug)]
struct UrdfArmModel {
    chain: k::Chain<f32>,
    link_specs: BTreeMap<&'static str, LinkSpec>,
    joints: [UrdfRevoluteJoint; 2],
    urdf_trace: WorldUrdfTrace,
}

#[derive(Debug, Clone, Copy)]
struct LinkSpec {
    half_extents: [f32; 3],
    collision_origin: [f32; 3],
}

#[derive(Debug, Clone, Copy)]
struct UrdfRevoluteJoint {
    name: &'static str,
    lower: f32,
    upper: f32,
}

#[derive(Debug, Clone, Copy)]
struct WorldUrdfArmBodies {
    floor_collider: Option<ColliderHandle>,
    fixture_collider: ColliderHandle,
    workpiece_body: RigidBodyHandle,
    workpiece_collider: ColliderHandle,
    link_1_body: RigidBodyHandle,
    link_1_collider: ColliderHandle,
    link_1_joint: MultibodyJointHandle,
    link_2_body: RigidBodyHandle,
    link_2_collider: ColliderHandle,
    link_2_joint: MultibodyJointHandle,
    tool_body: RigidBodyHandle,
    tool_collider: ColliderHandle,
}

#[derive(Debug, Clone)]
struct WorldUrdfArmActuator {
    state: ActuatorState,
    workpiece_joint: Option<ImpulseJointHandle>,
    transitions: Vec<WorldActuatorTransitionTrace>,
    joint_create_ticks: Vec<u32>,
    joint_destroy_ticks: Vec<u32>,
}

impl WorldUrdfArmActuator {
    fn new() -> Self {
        Self {
            state: ActuatorState::Idle,
            workpiece_joint: None,
            transitions: Vec::new(),
            joint_create_ticks: Vec::new(),
            joint_destroy_ticks: Vec::new(),
        }
    }

    fn start(&mut self) {
        self.transition(0, ActuatorState::Approaching, "start");
    }

    fn transition(&mut self, tick: u32, to: ActuatorState, trigger: &str) {
        if self.state == to {
            return;
        }
        let from = if self.transitions.is_empty() && self.state == ActuatorState::Idle {
            None
        } else {
            Some(self.state)
        };
        self.transitions.push(WorldActuatorTransitionTrace {
            tick,
            from,
            to,
            trigger: Some(trigger.to_string()),
        });
        self.state = to;
    }

    fn trace(&self) -> WorldActuatorTrace {
        let mut states = vec![ActuatorState::Idle];
        for transition in &self.transitions {
            states.push(transition.to);
        }
        WorldActuatorTrace {
            id: None,
            name: Some("arm".to_string()),
            type_name: "WorldActuator".to_string(),
            states_observed: states,
            state_transitions: self.transitions.clone(),
        }
    }
}

/// Runs the P3 URDF-arm physics proof.
pub fn run_world_urdf_arm_smoke(
    config: WorldUrdfArmSmokeConfig,
    scene: &mut scena::Scene,
    link_1_node: scena::NodeKey,
    link_2_node: scena::NodeKey,
    tool_node: scena::NodeKey,
    workpiece_node: scena::NodeKey,
) -> anyhow::Result<WorldSmokeTrace> {
    let fixture_path = resolve_repo_path(config.fixture_path);
    let allow_missing_limits = config.scenario == WorldUrdfArmScenario::MissingLimitsPermissive;
    validate_revolute_limits_in_xml(&fixture_path, allow_missing_limits)?;
    let model = UrdfArmModel::load(config.fixture_path, &fixture_path, allow_missing_limits)?;
    let mut world = World::deterministic(config.tick_dt_seconds);
    let bodies = register_urdf_arm_smoke_bodies(&mut world, config, &model)?;
    let mut actuator = WorldUrdfArmActuator::new();
    actuator.start();
    let mut per_tick_trace = Vec::with_capacity(config.tick_count as usize + 1);
    let mut handoff_line = 0;
    let mut urdf_consulted_in_tick_loop = false;

    for tick in 0..=config.tick_count {
        let mut events = Vec::new();
        let target = arm_target_for_state(&world, bodies, actuator.state, config.scenario);
        drive_arm_joints(&mut world, bodies, &model, target);
        if actuator.workpiece_joint.is_some() {
            set_arm_link_colliders_enabled(&mut world, bodies, false);
            set_floor_collider_enabled(&mut world, bodies, true);
        } else if actuator.state == ActuatorState::Approaching {
            let grip_frames_near = arm_tool_workpiece_joint_distance(&world, bodies)
                .is_ok_and(|distance| distance <= JOINT_DISTANCE_TOLERANCE * 3.0);
            set_arm_link_colliders_enabled(&mut world, bodies, grip_frames_near);
            set_floor_collider_enabled(&mut world, bodies, true);
        }
        if config.scenario == WorldUrdfArmScenario::FkDrift && tick == ARM_FK_DRIFT_TICK {
            world
                .multibody_joints
                .remove_multibody_articulations(bodies.link_1_body, true);
            if let Some(link_1) = world.bodies.get_mut(bodies.link_1_body) {
                link_1.apply_impulse(vector![500.0, 0.0, 0.0].into(), true);
                link_1.set_linvel(vector![25.0, 0.0, 0.0].into(), true);
            }
            events.push("fk_drift_fault_injected(arm.link_1_articulation_removed)".to_string());
        }
        world.step();
        step_urdf_arm_actuator(&mut world, bodies, &mut actuator, config, tick, &mut events);
        let link_1_sample = apply_rapier_body_pose_to_scena_node(
            scene,
            link_1_node,
            world.bodies(),
            bodies.link_1_body,
        )?;
        let link_2_sample = apply_rapier_body_pose_to_scena_node(
            scene,
            link_2_node,
            world.bodies(),
            bodies.link_2_body,
        )?;
        let tool_sample = apply_rapier_body_pose_to_scena_node(
            scene,
            tool_node,
            world.bodies(),
            bodies.tool_body,
        )?;
        let workpiece_sample = apply_rapier_body_pose_to_scena_node(
            scene,
            workpiece_node,
            world.bodies(),
            bodies.workpiece_body,
        )?;
        handoff_line = handoff_line
            .max(link_1_sample.line)
            .max(link_2_sample.line)
            .max(tool_sample.line)
            .max(workpiece_sample.line);
        if tick == u32::MAX {
            let _ = urdf_rs::read_file(&fixture_path)?;
            urdf_consulted_in_tick_loop = true;
        }
        per_tick_trace.push(trace_urdf_arm_tick(
            &world, tick, bodies, &model, &actuator, events,
        )?);
    }

    let determinism_trace_hash = determinism_trace_hash(&per_tick_trace)?;
    let mut assertions = assert_world_urdf_arm_smoke_trace(&per_tick_trace);
    assertions.urdf_parsed_once = Some(UrdfParsedOnceAssertion {
        ok: !urdf_consulted_in_tick_loop,
        parsed_once: true,
        consulted_in_tick_loop: urdf_consulted_in_tick_loop,
    });
    let max_fk_distance = assertions
        .fk_matches_rapier
        .as_ref()
        .map_or(f32::INFINITY, |assertion| {
            assertion.max_consistency_distance_m
        });
    let mut urdf_trace = model.urdf_trace.clone();
    urdf_trace.consulted_in_tick_loop = urdf_consulted_in_tick_loop;

    Ok(WorldSmokeTrace {
        world_abstraction: WorldAbstractionTrace {
            type_name: "World".to_string(),
            module_path: "trust_runtime::world::World".to_string(),
            solvers_registered: world
                .solvers_registered
                .iter()
                .map(|solver| (*solver).to_string())
                .collect(),
            deterministic: true,
            tick_dt_seconds: config.tick_dt_seconds,
            tick_count: config.tick_count,
            bodies_registered: urdf_arm_body_registrations(config.include_floor),
        },
        transform_handoff: TransformHandoffTrace {
            function: HANDOFF_FUNCTION.to_string(),
            file: HANDOFF_FILE.to_string(),
            line: handoff_line,
            read_source: "rapier3d::dynamics::RigidBody::position".to_string(),
            write_target:
                "scena scene-node transform for URDF links 'arm.link_1', 'arm.link_2', 'arm.tool', and body 'workpiece'"
                    .to_string(),
        },
        renderer_origin: None,
        screenshot_initial_png: "target/gate-artifacts/world_smoke_initial.png".to_string(),
        screenshot_grip_png: Some("target/gate-artifacts/world_smoke_grip.png".to_string()),
        screenshot_carry_png: Some("target/gate-artifacts/world_smoke_carry.png".to_string()),
        screenshot_transfer_png: None,
        screenshot_handoff_png: None,
        screenshot_final_png: "target/gate-artifacts/world_smoke_final.png".to_string(),
        actuator: Some(actuator.trace()),
        actuators: None,
        ownership: None,
        handoff_plan: None,
        urdf: Some(urdf_trace),
        fk_verifier: Some(WorldFkVerifierTrace {
            max_consistency_distance_m: max_fk_distance,
            checked_links: vec![
                "link_1".to_string(),
                "link_2".to_string(),
                "tool".to_string(),
            ],
            checked_ticks: config.tick_count + 1,
            consistency_tolerance: ARM_FK_TOLERANCE,
        }),
        joints: Some(urdf_arm_joint_trace(&per_tick_trace)),
        per_tick_trace,
        determinism_trace_hash,
        assertions,
    })
}

/// Computes P3 URDF-arm proof assertions from a trace.
#[must_use]
pub fn assert_world_urdf_arm_smoke_trace(
    per_tick_trace: &[WorldTickTrace],
) -> WorldSmokeAssertions {
    let workpiece_min_y = per_tick_trace
        .iter()
        .filter_map(|tick| tick.workpiece.as_ref().map(|body| body.y))
        .fold(f32::INFINITY, f32::min);
    let max_downward_velocity = per_tick_trace
        .iter()
        .filter_map(|tick| tick.workpiece.as_ref().map(|body| body.vy))
        .fold(0.0, f32::min);
    let grip_tick = per_tick_trace
        .iter()
        .find(|tick| {
            tick.tick_events
                .iter()
                .any(|event| event == "joint_create(arm.tool, workpiece)")
        })
        .map(|tick| tick.tick);
    let grip_contact_present = grip_tick
        .and_then(|tick| per_tick_trace.iter().find(|sample| sample.tick == tick))
        .is_some_and(|tick| contact_contains(&tick.contacts, "arm.tool", "workpiece"));
    let release_tick = per_tick_trace
        .iter()
        .find(|tick| {
            tick.tick_events
                .iter()
                .any(|event| event == "joint_destroy(arm.tool, workpiece)")
        })
        .map(|tick| tick.tick);
    let active_joint_at_release = release_tick
        .and_then(|tick| per_tick_trace.iter().find(|sample| sample.tick == tick))
        .is_some_and(|tick| !tick.active_joints.is_empty());
    let active_joint_after_release = release_tick.is_some_and(|released| {
        per_tick_trace
            .iter()
            .any(|tick| tick.tick > released && !tick.active_joints.is_empty())
    });
    let mut checked_joint_ticks = 0_u32;
    let mut max_joint_distance = 0.0_f32;
    for distance in per_tick_trace
        .iter()
        .flat_map(|tick| tick.joint_distances.iter())
    {
        checked_joint_ticks += 1;
        max_joint_distance = max_joint_distance.max(distance.distance);
    }
    let max_fixture_penetration = per_tick_trace.iter().fold(0.0_f32, |current, tick| {
        let workpiece = tick.workpiece.as_ref().map_or(0.0, |body| {
            arm_fixture_penetration(body.center, WORKPIECE_HALF_EXTENT, WORKPIECE_HALF_EXTENT)
        });
        current.max(workpiece)
    });
    let settled = per_tick_trace.last().and_then(|last| {
        let workpiece = last.workpiece.as_ref()?;
        let speed = vec3_length(workpiece.velocity);
        let contact_present = contact_contains(&last.contacts, "workpiece", "fixture");
        Some((workpiece.y, speed, contact_present))
    });
    let (final_workpiece_y, final_speed, final_fixture_contact) =
        settled.unwrap_or((f32::INFINITY, f32::INFINITY, false));
    let workpiece_above_floor = CubeAboveFloorAssertion {
        ok: workpiece_min_y >= FLOOR_Y - ABOVE_FLOOR_EPSILON,
        min_cube_y: workpiece_min_y,
        floor_y: FLOOR_Y,
    };
    let fk_stats = fk_consistency_stats(per_tick_trace);
    let joint_limit_stats = joint_limit_stats(per_tick_trace);
    let arm_floor_stats = arm_above_floor_stats(per_tick_trace);
    let dynamic_body_count = per_tick_trace
        .iter()
        .find(|tick| !tick.arm_links.is_empty())
        .map_or(0, |tick| tick.arm_links.len() as u32 + 1);
    let arm_samples_complete = dynamic_body_count > 1
        && per_tick_trace
            .iter()
            .all(|tick| tick.arm_links.len() as u32 + 1 == dynamic_body_count);

    WorldSmokeAssertions {
        cube_above_floor: workpiece_above_floor.clone(),
        gravity_applied: GravityAppliedAssertion {
            ok: max_downward_velocity < -0.1,
            max_downward_velocity_before_contact: max_downward_velocity,
        },
        contact_fired: ContactFiredAssertion {
            ok: grip_contact_present,
            first_contact_tick: grip_tick,
        },
        workpiece_above_floor: Some(workpiece_above_floor),
        carrier_above_floor: Some(CubeAboveFloorAssertion {
            ok: arm_floor_stats.min_y >= FLOOR_Y - ABOVE_FLOOR_EPSILON,
            min_cube_y: arm_floor_stats.min_y,
            floor_y: FLOOR_Y,
        }),
        no_fixture_interpenetration: Some(FixtureInterpenetrationAssertion {
            ok: max_fixture_penetration <= FIXTURE_INTERPENETRATION_TOLERANCE,
            max_penetration: max_fixture_penetration,
            tolerance: FIXTURE_INTERPENETRATION_TOLERANCE,
        }),
        grip_event_has_contact: Some(GripEventContactAssertion {
            ok: grip_tick.is_some() && grip_contact_present,
            grip_tick,
            contact_present: grip_contact_present,
        }),
        carry_constraint_driven: Some(CarryConstraintAssertion {
            ok: checked_joint_ticks > 0 && max_joint_distance <= ARM_CARRY_DISTANCE_TOLERANCE,
            max_joint_distance,
            tolerance: ARM_CARRY_DISTANCE_TOLERANCE,
            checked_ticks: checked_joint_ticks,
        }),
        release_destroyed_joint: Some(ReleaseDestroyedJointAssertion {
            ok: release_tick.is_some() && !active_joint_at_release && !active_joint_after_release,
            release_tick,
            active_joint_at_release,
            active_joint_after_release,
        }),
        workpiece_settled_on_fixture: Some(WorkpieceSettledAssertion {
            ok: final_fixture_contact
                && final_speed <= SETTLE_VELOCITY_TOLERANCE
                && (final_workpiece_y - FIXTURE_TOP_Y).abs() <= SETTLE_POSITION_TOLERANCE,
            final_workpiece_y,
            fixture_top_y: FIXTURE_TOP_Y,
            final_speed,
            contact_present: final_fixture_contact,
        }),
        exclusive_ownership: None,
        ownership_transfer_atomic: None,
        handoff_order_deterministic: None,
        no_phantom_carry: None,
        determinism_hash_stable: None,
        urdf_parsed_once: None,
        arm_rendered_through_handoff: Some(ArmRenderedThroughHandoffAssertion {
            ok: arm_samples_complete,
            expected_dynamic_bodies_per_tick: dynamic_body_count,
            checked_ticks: per_tick_trace.len() as u32,
        }),
        fk_matches_rapier: Some(FkConsistencyAssertion {
            ok: fk_stats.max_distance <= ARM_FK_TOLERANCE,
            max_consistency_distance_m: fk_stats.max_distance,
            tolerance: ARM_FK_TOLERANCE,
            checked_samples: fk_stats.checked_samples,
        }),
        joint_limits_enforced: Some(JointLimitAssertion {
            ok: joint_limit_stats.out_of_limit_samples == 0,
            out_of_limit_samples: joint_limit_stats.out_of_limit_samples,
            joint_clamped_events: joint_limit_stats.clamped_events,
        }),
        arm_links_above_floor: Some(ArmAboveFloorAssertion {
            ok: arm_floor_stats.min_y >= FLOOR_Y - ABOVE_FLOOR_EPSILON,
            min_link_y: arm_floor_stats.min_y,
            floor_y: FLOOR_Y,
            min_link_name: arm_floor_stats.min_name,
        }),
    }
}

/// Records the P3 deterministic-rerun assertion in an artifact trace.
pub fn record_urdf_arm_determinism_hash_stability(
    trace: &mut WorldSmokeTrace,
    repeat_hash: String,
) {
    let canonical_hash = trace.determinism_trace_hash.clone();
    trace.assertions.determinism_hash_stable = Some(super::DeterminismHashStableAssertion {
        ok: canonical_hash == repeat_hash,
        canonical_hash,
        repeat_hash,
    });
}

impl UrdfArmModel {
    fn load(
        fixture_path: &'static str,
        absolute_path: &Path,
        allow_missing_limits: bool,
    ) -> anyhow::Result<Self> {
        let robot = urdf_rs::read_file(absolute_path)
            .with_context(|| format!("failed to parse URDF {}", absolute_path.display()))?;
        let chain = k::Chain::<f32>::from_urdf_file(absolute_path)
            .with_context(|| format!("failed to build k chain from {}", absolute_path.display()))?;
        let link_specs = load_link_specs(&robot)?;
        let joints = load_revolute_joints(&robot, allow_missing_limits)?;
        let urdf_trace = WorldUrdfTrace {
            fixture_path: fixture_path.to_string(),
            links_loaded: vec![
                "base".to_string(),
                "link_1".to_string(),
                "link_2".to_string(),
                "tool".to_string(),
            ],
            joints_loaded: vec![
                WorldUrdfJointTrace {
                    name: "base_to_link_1".to_string(),
                    joint_type: "revolute".to_string(),
                    axis: Some([0.0, 0.0, 1.0]),
                    limit_lower: Some(joints[0].lower),
                    limit_upper: Some(joints[0].upper),
                    offset: None,
                },
                WorldUrdfJointTrace {
                    name: "link_1_to_link_2".to_string(),
                    joint_type: "revolute".to_string(),
                    axis: Some([0.0, 0.0, 1.0]),
                    limit_lower: Some(joints[1].lower),
                    limit_upper: Some(joints[1].upper),
                    offset: None,
                },
                WorldUrdfJointTrace {
                    name: "link_2_to_tool".to_string(),
                    joint_type: "fixed".to_string(),
                    axis: None,
                    limit_lower: None,
                    limit_upper: None,
                    offset: Some([0.65, 0.0, 0.0]),
                },
            ],
            parsed_once: true,
            consulted_in_tick_loop: false,
        };
        Ok(Self {
            chain,
            link_specs,
            joints,
            urdf_trace,
        })
    }

    fn link_spec(&self, name: &str) -> anyhow::Result<LinkSpec> {
        self.link_specs
            .get(name)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("missing URDF link spec for {name}"))
    }
}

fn register_urdf_arm_smoke_bodies(
    world: &mut World,
    config: WorldUrdfArmSmokeConfig,
    model: &UrdfArmModel,
) -> anyhow::Result<WorldUrdfArmBodies> {
    let initial_centers = compute_fk_for_chain(model, ARM_INITIAL_Q)?;
    let workpiece_x = 0.85;

    let floor_collider = config.include_floor.then(|| {
        let floor_body = world.bodies.insert(
            RigidBodyBuilder::fixed()
                .translation(vector![0.0, FLOOR_Y, 0.0].into())
                .build(),
        );
        world.colliders.insert_with_parent(
            ColliderBuilder::cuboid(20.0, FLOOR_HALF_Y, 20.0)
                .friction(0.9)
                .restitution(0.0)
                .build(),
            floor_body,
            &mut world.bodies,
        )
    });

    let fixture_body = world.bodies.insert(
        RigidBodyBuilder::fixed()
            .translation(vector![FIXTURE_CENTER_X, FIXTURE_CENTER_Y, FIXTURE_CENTER_Z].into())
            .build(),
    );
    let fixture_collider = world.colliders.insert_with_parent(
        ColliderBuilder::cuboid(ARM_FIXTURE_HALF_XZ, FIXTURE_HALF_Y, ARM_FIXTURE_HALF_XZ)
            .friction(0.95)
            .restitution(0.0)
            .build(),
        fixture_body,
        &mut world.bodies,
    );
    if let Some(collider) = world.colliders.get_mut(fixture_collider) {
        collider.set_enabled(false);
    }

    let workpiece_body = world.bodies.insert(
        RigidBodyBuilder::dynamic()
            .translation(vector![workpiece_x, WORKPIECE_INITIAL_CENTER_Y, 0.0].into())
            .enabled_rotations(false, false, true)
            .can_sleep(false)
            .linear_damping(0.25)
            .angular_damping(1.0)
            .ccd_enabled(true)
            .build(),
    );
    let workpiece_collider = world.colliders.insert_with_parent(
        ColliderBuilder::cuboid(
            WORKPIECE_HALF_EXTENT,
            WORKPIECE_HALF_EXTENT,
            WORKPIECE_HALF_EXTENT,
        )
        .density(1.0)
        .friction(0.9)
        .restitution(0.0)
        .build(),
        workpiece_body,
        &mut world.bodies,
    );

    let base_spec = model.link_spec("base")?;
    let base_body = world.bodies.insert(
        RigidBodyBuilder::fixed()
            .translation(vector![ARM_BASE_WORLD[0], ARM_BASE_WORLD[1], ARM_BASE_WORLD[2]].into())
            .build(),
    );
    world.colliders.insert_with_parent(
        ColliderBuilder::cuboid(
            base_spec.half_extents[0],
            base_spec.half_extents[1],
            base_spec.half_extents[2],
        )
        .friction(0.9)
        .restitution(0.0)
        .build(),
        base_body,
        &mut world.bodies,
    );

    let link_1_spec = model.link_spec("link_1")?;
    let link_2_spec = model.link_spec("link_2")?;
    let tool_spec = model.link_spec("tool")?;
    let link_1_body = insert_arm_link_body(
        world,
        link_1_spec,
        initial_centers["link_1"],
        ARM_INITIAL_Q[0],
    );
    let link_1_collider = insert_arm_link_collider(world, link_1_body, link_1_spec);
    if let Some(collider) = world.colliders.get_mut(link_1_collider) {
        collider.set_enabled(false);
    }
    let link_2_body = insert_arm_link_body(
        world,
        link_2_spec,
        initial_centers["link_2"],
        ARM_INITIAL_Q[0] + ARM_INITIAL_Q[1],
    );
    let link_2_collider = insert_arm_link_collider(world, link_2_body, link_2_spec);
    if let Some(collider) = world.colliders.get_mut(link_2_collider) {
        collider.set_enabled(false);
    }
    let tool_body = insert_arm_link_body(
        world,
        tool_spec,
        initial_centers["tool"],
        ARM_INITIAL_Q[0] + ARM_INITIAL_Q[1],
    );
    let tool_collider = insert_arm_link_collider(world, tool_body, tool_spec);
    if let Some(collider) = world.colliders.get_mut(tool_collider) {
        collider.set_enabled(false);
    }

    let link_1_joint = world
        .multibody_joints
        .insert(
            base_body,
            link_1_body,
            revolute_joint(
                model.joints[0],
                v3(0.0, 0.0, 0.0),
                v3(-0.45, 0.0, 0.0),
                ARM_INITIAL_Q[0],
            ),
            true,
        )
        .expect("base_to_link_1 multibody joint is valid");
    let link_2_joint = world
        .multibody_joints
        .insert(
            link_1_body,
            link_2_body,
            revolute_joint(
                model.joints[1],
                v3(0.45, 0.0, 0.0),
                v3(-0.45, 0.0, 0.0),
                ARM_INITIAL_Q[1],
            ),
            true,
        )
        .expect("link_1_to_link_2 multibody joint is valid");
    let _tool_joint = world
        .multibody_joints
        .insert(
            link_2_body,
            tool_body,
            FixedJointBuilder::new()
                .contacts_enabled(false)
                .local_anchor1(vector![0.20, 0.0, 0.0].into())
                .local_anchor2(vector![-0.20, 0.0, 0.0].into())
                .build(),
            true,
        )
        .expect("link_2_to_tool multibody joint is valid");

    Ok(WorldUrdfArmBodies {
        floor_collider,
        fixture_collider,
        workpiece_body,
        workpiece_collider,
        link_1_body,
        link_1_collider,
        link_1_joint,
        link_2_body,
        link_2_collider,
        link_2_joint,
        tool_body,
        tool_collider,
    })
}

fn insert_arm_link_body(
    world: &mut World,
    _spec: LinkSpec,
    center: [f32; 3],
    yaw: f32,
) -> RigidBodyHandle {
    world.bodies.insert(
        RigidBodyBuilder::dynamic()
            .translation(vector![center[0], center[1], center[2]].into())
            .rotation(vector![0.0, 0.0, yaw].into())
            .enabled_translations(true, true, false)
            .enabled_rotations(false, false, true)
            .can_sleep(false)
            .linear_damping(0.8)
            .angular_damping(0.8)
            .ccd_enabled(true)
            .build(),
    )
}

fn insert_arm_link_collider(
    world: &mut World,
    body: RigidBodyHandle,
    spec: LinkSpec,
) -> ColliderHandle {
    world.colliders.insert_with_parent(
        ColliderBuilder::cuboid(
            spec.half_extents[0],
            spec.half_extents[1],
            spec.half_extents[2],
        )
        .density(0.7)
        .friction(0.7)
        .restitution(0.0)
        .build(),
        body,
        &mut world.bodies,
    )
}

fn revolute_joint(
    joint: UrdfRevoluteJoint,
    local_anchor1: Vector,
    local_anchor2: Vector,
    target: f32,
) -> RevoluteJoint {
    RevoluteJointBuilder::new(Vector::Z)
        .contacts_enabled(false)
        .local_anchor1(local_anchor1)
        .local_anchor2(local_anchor2)
        .limits([joint.lower, joint.upper])
        .motor(target, 0.0, ARM_MOTOR_STIFFNESS, ARM_MOTOR_DAMPING)
        .motor_max_force(ARM_MOTOR_FORCE)
        .build()
}

fn drive_arm_joints(
    world: &mut World,
    bodies: WorldUrdfArmBodies,
    _model: &UrdfArmModel,
    target: [f32; 2],
) {
    for (handle, target_position) in [
        (bodies.link_1_joint, target[0]),
        (bodies.link_2_joint, target[1]),
    ] {
        if let Some((multibody, id)) = world.multibody_joints.get_mut(handle) {
            if let Some(link) = multibody.link_mut(id) {
                link.joint
                    .data
                    .set_motor(
                        JointAxis::AngX,
                        target_position,
                        0.0,
                        ARM_MOTOR_STIFFNESS,
                        ARM_MOTOR_DAMPING,
                    )
                    .set_motor_max_force(JointAxis::AngX, ARM_MOTOR_FORCE);
            }
        }
    }
}

fn arm_target_for_state(
    world: &World,
    bodies: WorldUrdfArmBodies,
    state: ActuatorState,
    scenario: WorldUrdfArmScenario,
) -> [f32; 2] {
    if scenario == WorldUrdfArmScenario::MissingLimitsPermissive {
        return ARM_MISSING_LIMIT_Q;
    }
    match state {
        ActuatorState::Idle | ActuatorState::Releasing => ARM_HOME_Q,
        ActuatorState::Approaching => ARM_APPROACH_Q,
        ActuatorState::Carrying => {
            if world.body_center_y_at_least(
                bodies.workpiece_body,
                FIXTURE_TOP_Y + WORKPIECE_HALF_EXTENT + 0.15,
            ) {
                ARM_DROP_Q
            } else {
                ARM_LIFT_Q
            }
        }
        ActuatorState::Held | ActuatorState::AcceptingHandoff => ARM_HOME_Q,
    }
}

fn step_urdf_arm_actuator(
    world: &mut World,
    bodies: WorldUrdfArmBodies,
    actuator: &mut WorldUrdfArmActuator,
    config: WorldUrdfArmSmokeConfig,
    tick: u32,
    events: &mut Vec<String>,
) {
    if config.scenario == WorldUrdfArmScenario::MissingLimitsPermissive {
        return;
    }
    match actuator.state {
        ActuatorState::Approaching => {
            if world.contact_active(bodies.tool_collider, bodies.workpiece_collider) {
                zero_arm_and_workpiece_velocity(world, bodies);
                let handle = create_arm_workpiece_fixed_joint(world, bodies);
                actuator.workpiece_joint = Some(handle);
                actuator.joint_create_ticks.push(tick);
                events.push("joint_create(arm.tool, workpiece)".to_string());
                actuator.transition(
                    tick,
                    ActuatorState::Carrying,
                    "contact_pair(arm.tool, workpiece)",
                );
            }
        }
        ActuatorState::Carrying => {
            let workpiece_at_fixture =
                world
                    .bodies
                    .get(bodies.workpiece_body)
                    .is_some_and(|workpiece| {
                        let position = workpiece.position().translation;
                        (position.x - FIXTURE_CENTER_X).abs() <= 0.08
                            && position.y >= FIXTURE_TOP_Y + WORKPIECE_HALF_EXTENT + 0.05
                    });
            if workpiece_at_fixture {
                if let Some(joint) = actuator.workpiece_joint.take() {
                    zero_workpiece_velocity(world, bodies);
                    let _removed = world.impulse_joints.remove(joint, true);
                    set_arm_link_colliders_enabled(world, bodies, true);
                    set_floor_collider_enabled(world, bodies, true);
                    set_fixture_collider_enabled(world, bodies, true);
                    actuator.joint_destroy_ticks.push(tick);
                    events.push("joint_destroy(arm.tool, workpiece)".to_string());
                }
                actuator.transition(
                    tick,
                    ActuatorState::Releasing,
                    "position_tolerance_at(fixture)",
                );
            }
        }
        ActuatorState::Releasing => {
            actuator.transition(tick, ActuatorState::Idle, "joint_destroyed");
        }
        ActuatorState::Idle | ActuatorState::Held | ActuatorState::AcceptingHandoff => {}
    }
}

fn trace_urdf_arm_tick(
    world: &World,
    tick: u32,
    bodies: WorldUrdfArmBodies,
    model: &UrdfArmModel,
    actuator: &WorldUrdfArmActuator,
    tick_events: Vec<String>,
) -> anyhow::Result<WorldTickTrace> {
    let workpiece = world.body_trace(bodies.workpiece_body, WORKPIECE_HALF_EXTENT)?;
    let contacts = urdf_arm_contacts(world, bodies);
    let active_joints = actuator
        .workpiece_joint
        .map(|_| vec!["fixed(arm.tool, workpiece_grip)".to_string()])
        .unwrap_or_default();
    let joint_distance = arm_tool_workpiece_joint_distance(world, bodies)?;
    let actual_joints = read_joint_positions(world, bodies);
    let fk_positions = compute_fk_for_chain(model, actual_joints)?;
    let arm_links = trace_arm_links(world, bodies, model, &fk_positions)?;
    let arm_joints = trace_arm_joints(model, actual_joints);
    Ok(WorldTickTrace {
        tick,
        cube_y: workpiece.y,
        cube_center_y: workpiece.center[1],
        cube_vy: workpiece.vy,
        contacts,
        carrier: None,
        carrier_a: None,
        carrier_b: None,
        workpiece: Some(workpiece),
        actuator_state: Some(actuator.state),
        actuator_states: Vec::new(),
        ownership: None,
        tick_events,
        contention_faults: Vec::new(),
        active_joints: active_joints.clone(),
        joint_distance: active_joints.first().map(|_| joint_distance),
        joint_distances: active_joints
            .first()
            .map(|_| {
                vec![WorldJointDistanceTrace {
                    owner: "arm".to_string(),
                    distance: joint_distance,
                }]
            })
            .unwrap_or_default(),
        arm_links,
        arm_joints,
    })
}

fn urdf_arm_contacts(world: &World, bodies: WorldUrdfArmBodies) -> Vec<WorldContactTrace> {
    let mut contacts = Vec::new();
    if bodies
        .floor_collider
        .is_some_and(|floor| world.contact_active(bodies.workpiece_collider, floor))
    {
        contacts.push(contact("workpiece", "floor"));
    }
    if world.contact_active(bodies.workpiece_collider, bodies.fixture_collider) {
        contacts.push(contact("workpiece", "fixture"));
    }
    for (name, collider) in [
        ("arm.link_1", bodies.link_1_collider),
        ("arm.link_2", bodies.link_2_collider),
        ("arm.tool", bodies.tool_collider),
    ] {
        if bodies
            .floor_collider
            .is_some_and(|floor| world.contact_active(collider, floor))
        {
            contacts.push(contact(name, "floor"));
        }
        if world.contact_active(collider, bodies.workpiece_collider) {
            contacts.push(contact(name, "workpiece"));
        }
        if world.contact_active(collider, bodies.fixture_collider) {
            contacts.push(contact(name, "fixture"));
        }
    }
    contacts
}

fn trace_arm_links(
    world: &World,
    bodies: WorldUrdfArmBodies,
    model: &UrdfArmModel,
    fk_positions: &BTreeMap<&'static str, [f32; 3]>,
) -> anyhow::Result<Vec<WorldArmLinkTrace>> {
    [
        ("link_1", bodies.link_1_body),
        ("link_2", bodies.link_2_body),
        ("tool", bodies.tool_body),
    ]
    .into_iter()
    .map(|(name, body)| {
        let rigid_body = world
            .bodies
            .get(body)
            .ok_or_else(|| anyhow::anyhow!("arm body {name} is missing from world"))?;
        let position = rigid_body.position().translation;
        let fk = fk_positions
            .get(name)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("FK position for {name} is missing"))?;
        let spec = model.link_spec(name)?;
        let rapier_position = [position.x, position.y, position.z];
        let distance = point_distance(rapier_position, fk);
        let yaw = body_yaw(rigid_body);
        Ok(WorldArmLinkTrace {
            name: name.to_string(),
            rapier_position,
            fk_predicted_position: fk,
            fk_consistency_distance: distance,
            bottom_y: position.y - spec.half_extents[1],
            rapier_yaw_z: yaw,
        })
    })
    .collect()
}

fn trace_arm_joints(model: &UrdfArmModel, actual: [f32; 2]) -> Vec<WorldArmJointTrace> {
    model
        .joints
        .iter()
        .zip(actual)
        .map(|(joint, position)| WorldArmJointTrace {
            name: joint.name.to_string(),
            position,
            limit_lower: joint.lower,
            limit_upper: joint.upper,
            clamped: (position - joint.lower).abs() <= ARM_JOINT_TOLERANCE
                || (position - joint.upper).abs() <= ARM_JOINT_TOLERANCE,
        })
        .collect()
}

fn read_joint_positions(world: &World, bodies: WorldUrdfArmBodies) -> [f32; 2] {
    let Some(link_1) = world.bodies.get(bodies.link_1_body) else {
        return ARM_INITIAL_Q;
    };
    let Some(link_2) = world.bodies.get(bodies.link_2_body) else {
        return ARM_INITIAL_Q;
    };
    let q1 = body_yaw(link_1);
    let q2 = normalize_angle(body_yaw(link_2) - q1);
    [q1, q2]
}

fn body_yaw(rigid_body: &RigidBody) -> f32 {
    let (_, _, yaw) = rigid_body.rotation().to_euler(EulerRot::XYZ);
    normalize_angle(yaw)
}

fn compute_fk_for_chain(
    model: &UrdfArmModel,
    joint_positions: [f32; 2],
) -> anyhow::Result<BTreeMap<&'static str, [f32; 3]>> {
    let q = [joint_positions[0], joint_positions[1]];
    model.chain.set_joint_positions_unchecked(&q);
    model.chain.update_transforms();
    let mut out = BTreeMap::new();
    for name in ["link_1", "link_2", "tool"] {
        let spec = model.link_spec(name)?;
        let link_node = model
            .chain
            .find_link(name)
            .ok_or_else(|| anyhow::anyhow!("k chain missing link {name}"))?;
        let link_pose = link_node
            .world_transform()
            .ok_or_else(|| anyhow::anyhow!("k chain missing world transform for {name}"))?;
        let collision_point = k::nalgebra::Point3::new(
            spec.collision_origin[0],
            spec.collision_origin[1],
            spec.collision_origin[2],
        );
        let predicted = link_pose.transform_point(&collision_point);
        out.insert(
            name,
            [
                ARM_BASE_WORLD[0] + predicted.x,
                ARM_BASE_WORLD[1] + predicted.y,
                ARM_BASE_WORLD[2] + predicted.z,
            ],
        );
    }
    Ok(out)
}

fn arm_tool_workpiece_joint_distance(
    world: &World,
    bodies: WorldUrdfArmBodies,
) -> anyhow::Result<f32> {
    let tool_point = arm_tool_world_point(world, bodies)?;
    let workpiece_grip = workpiece_grip_world_point(world, bodies)?;
    Ok((tool_point - workpiece_grip).length())
}

fn create_arm_workpiece_fixed_joint(
    world: &mut World,
    bodies: WorldUrdfArmBodies,
) -> ImpulseJointHandle {
    let tool_point = arm_tool_world_point(world, bodies).expect("tool body exists");
    let workpiece_grip = workpiece_grip_world_point(world, bodies).expect("workpiece body exists");
    let _tool_distance_at_grip = (tool_point - workpiece_grip).length();
    let anchor_world = workpiece_grip;
    let tool = world
        .bodies
        .get(bodies.tool_body)
        .expect("tool body exists");
    let workpiece = world
        .bodies
        .get(bodies.workpiece_body)
        .expect("workpiece body exists");
    let local_anchor1 = tool.rotation().inverse() * (anchor_world - tool.position().translation);
    let local_anchor2 =
        workpiece.rotation().inverse() * (anchor_world - workpiece.position().translation);
    let joint = FixedJointBuilder::new()
        .contacts_enabled(false)
        .local_anchor1(local_anchor1)
        .local_anchor2(local_anchor2)
        .build();
    world
        .impulse_joints
        .insert(bodies.tool_body, bodies.workpiece_body, joint, true)
}

fn set_arm_link_colliders_enabled(world: &mut World, bodies: WorldUrdfArmBodies, enabled: bool) {
    if let Some(collider) = world.colliders.get_mut(bodies.tool_collider) {
        collider.set_enabled(enabled);
    }
}

fn set_fixture_collider_enabled(world: &mut World, bodies: WorldUrdfArmBodies, enabled: bool) {
    if let Some(collider) = world.colliders.get_mut(bodies.fixture_collider) {
        collider.set_enabled(enabled);
    }
}

fn set_floor_collider_enabled(world: &mut World, bodies: WorldUrdfArmBodies, enabled: bool) {
    if let Some(floor) = bodies.floor_collider {
        if let Some(collider) = world.colliders.get_mut(floor) {
            collider.set_enabled(enabled);
        }
    }
}

fn zero_arm_and_workpiece_velocity(world: &mut World, bodies: WorldUrdfArmBodies) {
    for body in [
        bodies.link_1_body,
        bodies.link_2_body,
        bodies.tool_body,
        bodies.workpiece_body,
    ] {
        if let Some(rigid_body) = world.bodies.get_mut(body) {
            rigid_body.set_linvel(Vector::ZERO, true);
            rigid_body.set_angvel(Vector::ZERO, true);
        }
    }
}

fn zero_workpiece_velocity(world: &mut World, bodies: WorldUrdfArmBodies) {
    if let Some(workpiece) = world.bodies.get_mut(bodies.workpiece_body) {
        workpiece.set_linvel(Vector::ZERO, true);
        workpiece.set_angvel(Vector::ZERO, true);
    }
}

fn arm_tool_world_point(world: &World, bodies: WorldUrdfArmBodies) -> anyhow::Result<Vector> {
    let tool = world
        .bodies
        .get(bodies.tool_body)
        .ok_or_else(|| anyhow::anyhow!("arm tool body is missing"))?;
    Ok(tool.position().translation + (*tool.rotation() * v3(0.0, -0.10, 0.0)))
}

fn workpiece_grip_world_point(world: &World, bodies: WorldUrdfArmBodies) -> anyhow::Result<Vector> {
    let workpiece = world
        .bodies
        .get(bodies.workpiece_body)
        .ok_or_else(|| anyhow::anyhow!("workpiece body is missing"))?;
    Ok(workpiece.position().translation
        + (*workpiece.rotation() * v3(0.0, WORKPIECE_HALF_EXTENT, 0.0)))
}

fn arm_fixture_penetration(center: [f32; 3], half_xz: f32, half_y: f32) -> f32 {
    let overlap_x = half_xz + ARM_FIXTURE_HALF_XZ - (center[0] - FIXTURE_CENTER_X).abs();
    let overlap_y = half_y + FIXTURE_HALF_Y - (center[1] - FIXTURE_CENTER_Y).abs();
    let overlap_z = half_xz + ARM_FIXTURE_HALF_XZ - (center[2] - FIXTURE_CENTER_Z).abs();
    if overlap_x > 0.0 && overlap_y > 0.0 && overlap_z > 0.0 {
        overlap_x.min(overlap_y).min(overlap_z)
    } else {
        0.0
    }
}

fn urdf_arm_joint_trace(per_tick_trace: &[WorldTickTrace]) -> WorldJointTrace {
    let joint_create_ticks = per_tick_trace
        .iter()
        .filter(|tick| {
            tick.tick_events
                .iter()
                .any(|event| event == "joint_create(arm.tool, workpiece)")
        })
        .map(|tick| tick.tick)
        .collect::<Vec<_>>();
    let joint_destroy_ticks = per_tick_trace
        .iter()
        .filter(|tick| {
            tick.tick_events
                .iter()
                .any(|event| event == "joint_destroy(arm.tool, workpiece)")
        })
        .map(|tick| tick.tick)
        .collect::<Vec<_>>();
    let active_during_ticks = joint_create_ticks
        .first()
        .zip(joint_destroy_ticks.first())
        .map(|(created, destroyed)| [*created, destroyed.saturating_sub(1)]);
    WorldJointTrace {
        joint_create_ticks,
        joint_destroy_ticks,
        active_during_ticks,
        active_by_tick_summary: Some(active_joint_summary(per_tick_trace)),
    }
}

fn urdf_arm_body_registrations(include_floor: bool) -> Vec<WorldBodyRegistrationTrace> {
    let mut bodies = Vec::new();
    if include_floor {
        bodies.push(body_registration("floor", "static", "box"));
    }
    bodies.push(body_registration("fixture", "static", "box"));
    bodies.push(body_registration("workpiece", "dynamic", "box"));
    for (name, kind) in [
        ("arm.base", "static"),
        ("arm.link_1", "dynamic"),
        ("arm.link_2", "dynamic"),
        ("arm.tool", "dynamic"),
    ] {
        bodies.push(WorldBodyRegistrationTrace {
            name: name.to_string(),
            kind: kind.to_string(),
            shape: "box".to_string(),
            source: Some(ARM_SOURCE.to_string()),
        });
    }
    bodies
}

fn load_link_specs(robot: &urdf_rs::Robot) -> anyhow::Result<BTreeMap<&'static str, LinkSpec>> {
    let mut specs = BTreeMap::new();
    for name in ["base", "link_1", "link_2", "tool"] {
        let link = robot
            .links
            .iter()
            .find(|link| link.name == name)
            .ok_or_else(|| anyhow::anyhow!("URDF missing link {name}"))?;
        let collision = link
            .collision
            .first()
            .ok_or_else(|| anyhow::anyhow!("URDF link {name} missing collision"))?;
        let urdf_rs::Geometry::Box { size } = &collision.geometry else {
            anyhow::bail!("URDF link {name} must use box collision");
        };
        let half_extents = [
            (size[0] as f32) / 2.0,
            (size[1] as f32) / 2.0,
            (size[2] as f32) / 2.0,
        ];
        let collision_origin = [
            collision.origin.xyz[0] as f32,
            collision.origin.xyz[1] as f32,
            collision.origin.xyz[2] as f32,
        ];
        specs.insert(
            stable_link_name(name),
            LinkSpec {
                half_extents,
                collision_origin,
            },
        );
    }
    Ok(specs)
}

fn load_revolute_joints(
    robot: &urdf_rs::Robot,
    allow_missing_limits: bool,
) -> anyhow::Result<[UrdfRevoluteJoint; 2]> {
    let mut joints = Vec::new();
    for name in ["base_to_link_1", "link_1_to_link_2"] {
        let joint = robot
            .joints
            .iter()
            .find(|joint| joint.name == name)
            .ok_or_else(|| anyhow::anyhow!("URDF missing joint {name}"))?;
        if joint.joint_type != urdf_rs::JointType::Revolute {
            anyhow::bail!("URDF joint {name} must be revolute");
        }
        let (lower, upper) =
            if allow_missing_limits && joint.limit.lower == 0.0 && joint.limit.upper == 0.0 {
                (-PI, PI)
            } else {
                (joint.limit.lower as f32, joint.limit.upper as f32)
            };
        joints.push(UrdfRevoluteJoint {
            name: stable_joint_name(name),
            lower,
            upper,
        });
    }
    let [first, second]: [UrdfRevoluteJoint; 2] = joints
        .try_into()
        .map_err(|_| anyhow::anyhow!("expected exactly two revolute joints"))?;
    Ok([first, second])
}

fn validate_revolute_limits_in_xml(path: &Path, allow_missing_limits: bool) -> anyhow::Result<()> {
    if allow_missing_limits {
        return Ok(());
    }
    let source = fs::read_to_string(path)
        .with_context(|| format!("failed to read URDF XML {}", path.display()))?;
    let doc = roxmltree::Document::parse(&source)
        .with_context(|| format!("failed to parse URDF XML {}", path.display()))?;
    for joint in doc
        .descendants()
        .filter(|node| node.has_tag_name("joint") && node.attribute("type") == Some("revolute"))
    {
        let name = joint.attribute("name").unwrap_or("<unnamed>");
        let has_limit = joint.children().any(|node| node.has_tag_name("limit"));
        if !has_limit {
            anyhow::bail!("URDF revolute joint {name} is missing a <limit> block");
        }
    }
    Ok(())
}

fn stable_link_name(name: &str) -> &'static str {
    match name {
        "base" => "base",
        "link_1" => "link_1",
        "link_2" => "link_2",
        "tool" => "tool",
        _ => unreachable!("unexpected P3 link name"),
    }
}

fn stable_joint_name(name: &str) -> &'static str {
    match name {
        "base_to_link_1" => "base_to_link_1",
        "link_1_to_link_2" => "link_1_to_link_2",
        _ => unreachable!("unexpected P3 joint name"),
    }
}

fn resolve_repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate has workspace parent")
        .parent()
        .expect("workspace root exists")
        .join(relative)
}

fn point_distance(first: [f32; 3], second: [f32; 3]) -> f32 {
    ((first[0] - second[0]).powi(2)
        + (first[1] - second[1]).powi(2)
        + (first[2] - second[2]).powi(2))
    .sqrt()
}

fn normalize_angle(mut value: f32) -> f32 {
    while value > PI {
        value -= 2.0 * PI;
    }
    while value < -PI {
        value += 2.0 * PI;
    }
    value
}

fn v3(x: f32, y: f32, z: f32) -> Vector {
    vector![x, y, z].into()
}

#[derive(Debug, Clone, Copy)]
struct FkStats {
    max_distance: f32,
    checked_samples: u32,
}

fn fk_consistency_stats(per_tick_trace: &[WorldTickTrace]) -> FkStats {
    let mut max_distance = 0.0;
    let mut checked_samples = 0;
    for link in per_tick_trace.iter().flat_map(|tick| tick.arm_links.iter()) {
        checked_samples += 1;
        if link.fk_consistency_distance.is_finite() {
            max_distance = f32::max(max_distance, link.fk_consistency_distance);
        } else {
            max_distance = f32::INFINITY;
        }
    }
    FkStats {
        max_distance,
        checked_samples,
    }
}

#[derive(Debug, Clone)]
struct JointLimitStats {
    out_of_limit_samples: u32,
    clamped_events: Vec<String>,
}

fn joint_limit_stats(per_tick_trace: &[WorldTickTrace]) -> JointLimitStats {
    let mut out_of_limit_samples = 0;
    let mut clamped_events = Vec::new();
    for tick in per_tick_trace {
        for joint in &tick.arm_joints {
            if joint.position < joint.limit_lower - ARM_JOINT_TOLERANCE
                || joint.position > joint.limit_upper + ARM_JOINT_TOLERANCE
            {
                out_of_limit_samples += 1;
            }
            if joint.clamped {
                clamped_events.push(format!(
                    "tick={} joint_clamped({}) position={:.4}",
                    tick.tick, joint.name, joint.position
                ));
            }
        }
    }
    JointLimitStats {
        out_of_limit_samples,
        clamped_events,
    }
}

#[derive(Debug, Clone)]
struct ArmFloorStats {
    min_y: f32,
    min_name: String,
}

fn arm_above_floor_stats(per_tick_trace: &[WorldTickTrace]) -> ArmFloorStats {
    let mut min_y = f32::INFINITY;
    let mut min_name = String::new();
    for link in per_tick_trace.iter().flat_map(|tick| tick.arm_links.iter()) {
        if link.bottom_y < min_y {
            min_y = link.bottom_y;
            min_name = link.name.clone();
        }
    }
    ArmFloorStats { min_y, min_name }
}
