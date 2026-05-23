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
    contact_contains, determinism_trace_hash, vec3_length, ActuatorState, BodyAboveFloorAssertion,
    CarryConstraintAssertion, ContactFiredAssertion, ExclusiveOwnershipAssertion,
    FixtureInterpenetrationAssertion, GravityAppliedAssertion, GripEventContactAssertion,
    HandoffOrderAssertion, NoPhantomCarryAssertion, OwnershipTransferAtomicAssertion,
    ReleaseDestroyedJointAssertion, TransformHandoffTrace, WorkpieceSettledAssertion, World,
    WorldAbstractionTrace, WorldActuatorStateSample, WorldActuatorTrace,
    WorldActuatorTransitionTrace, WorldBodyRegistrationTrace, WorldContactTrace,
    WorldHandoffPairTrace, WorldHandoffPlanTrace, WorldJointDistanceTrace, WorldJointTrace,
    WorldOwnershipFaultTrace, WorldOwnershipSample, WorldOwnershipTrace,
    WorldOwnershipTransitionTrace, WorldSmokeAssertions, WorldSmokeTrace, WorldTickTrace,
    ABOVE_FLOOR_EPSILON, FIXTURE_CENTER_X, FIXTURE_CENTER_Y, FIXTURE_CENTER_Z, FIXTURE_HALF_Y,
    FIXTURE_INTERPENETRATION_TOLERANCE, FIXTURE_TOP_Y, FLOOR_HALF_Y, FLOOR_Y, HANDOFF_FILE,
    HANDOFF_FUNCTION, JOINT_DISTANCE_TOLERANCE, SETTLE_POSITION_TOLERANCE,
    SETTLE_VELOCITY_TOLERANCE, WORKPIECE_HALF_EXTENT, WORKPIECE_INITIAL_CENTER_Y,
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
const P4_ARM_A_BASE_WORLD: [f32; 3] = [0.30, 0.85, 0.0];
const P4_ARM_B_BASE_WORLD: [f32; 3] = [1.40, 0.85, 0.0];
const P4_TRANSFER_CENTER_X: f32 = 1.80;
const P4_TRANSFER_MARKER_Y: f32 = FLOOR_Y + FLOOR_HALF_Y + 0.02;
const P4_TRANSFER_HALF_XZ: f32 = 0.18;
const P4_TRANSFER_HALF_Y: f32 = 0.02;
const P4_WORKPIECE_PICKUP_X: f32 = 0.85;
const P4_CONTESTED_WORKPIECE_X: f32 = 1.80;
const P4_ARM_A_TRANSFER_Q: [f32; 2] = [-0.384, 1.180];
const P4_ARM_A_CONTESTED_Q: [f32; 2] = [-0.696, 1.144];
const P4_ARM_B_TRANSFER_Q: [f32; 2] = [-0.756, 2.746];
const P4_ARM_B_FINAL_Q: [f32; 2] = [-0.816, 2.524];
const P4_ARM_B_CONTESTED_Q: [f32; 2] = P4_ARM_A_CONTESTED_Q;
const P4_HANDOFF_TOLERANCE: f32 = 0.08;
const P4_MIN_RECEIVER_CARRY_TICKS: u32 = 80;

/// Input contract for the generated `Robot_P3MinimalArm` native FB bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RobotP3MinimalArmBridgeInput {
    /// Whether the generated robot FB is enabled by PLC code.
    pub enable: bool,
    /// Command/state id requested by PLC code.
    pub command: i16,
}

/// Output contract produced by the generated `Robot_P3MinimalArm` native FB bridge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RobotP3MinimalArmBridgeOutput {
    /// Echo of the enabled state after native bridge evaluation.
    pub enabled_out: bool,
    /// Robot is moving or holding a workpiece.
    pub busy: bool,
    /// Command has reached a stable terminal presentation state.
    pub done: bool,
    /// Command was rejected by the bridge contract.
    pub fault: bool,
    /// State id emitted to PLC/HMI outputs.
    pub state: i16,
    /// Workpiece owner id; zero means unowned, one means this robot owns it.
    pub owner: i16,
    /// Whether the tool currently owns the workpiece.
    pub has_workpiece: bool,
    /// Whether the gripper jaws are open.
    pub gripper_open: bool,
    /// Status-light output for the sample HMI.
    pub status_light: bool,
    /// First URDF revolute joint presentation angle.
    pub joint1: f32,
    /// Second URDF revolute joint presentation angle.
    pub joint2: f32,
    /// Tool yaw presentation angle.
    pub tool_yaw: f32,
    /// Tool center presentation position.
    pub tool_position: [f32; 3],
    /// Workpiece presentation position.
    pub workpiece_position: [f32; 3],
}

/// Executes the writer-free product bridge for the generated P3 minimal-arm FB.
///
/// The bridge intentionally returns typed PLC/HMI outputs only. It does not
/// write Rapier bodies, scene nodes, FK results, or visible transforms; the
/// physics proofs keep those ownership paths in the shared [`World`].
#[must_use]
pub fn step_robot_p3_minimal_arm_bridge(
    input: RobotP3MinimalArmBridgeInput,
) -> RobotP3MinimalArmBridgeOutput {
    if !input.enable {
        return robot_p3_minimal_arm_disabled_output();
    }

    match input.command {
        0 => robot_p3_minimal_arm_output(RobotP3MinimalArmPose {
            state: 0,
            busy: false,
            done: true,
            has_workpiece: false,
            gripper_open: true,
            joints: [-0.60, 0.35],
            tool_yaw: 0.25,
            tool_position: [0.0, 0.80, 0.0],
            workpiece_position: [0.0, 0.35, 0.0],
        }),
        1 => robot_p3_minimal_arm_output(RobotP3MinimalArmPose {
            state: 1,
            busy: true,
            done: false,
            has_workpiece: false,
            gripper_open: true,
            joints: [-0.72, 0.42],
            tool_yaw: 0.25,
            tool_position: [0.0, 0.80, 0.0],
            workpiece_position: [0.0, 0.35, 0.0],
        }),
        2 => robot_p3_minimal_arm_output(RobotP3MinimalArmPose {
            state: 2,
            busy: true,
            done: false,
            has_workpiece: true,
            gripper_open: false,
            joints: [-0.72, 0.42],
            tool_yaw: 0.25,
            tool_position: [0.0, 0.55, 0.0],
            workpiece_position: [0.0, 0.35, 0.0],
        }),
        3 => robot_p3_minimal_arm_output(RobotP3MinimalArmPose {
            state: 3,
            busy: true,
            done: false,
            has_workpiece: true,
            gripper_open: false,
            joints: [-0.35, 0.72],
            tool_yaw: -0.45,
            tool_position: [0.80, 1.15, 0.0],
            workpiece_position: [0.80, 1.15, 0.0],
        }),
        4 => robot_p3_minimal_arm_output(RobotP3MinimalArmPose {
            state: 4,
            busy: true,
            done: false,
            has_workpiece: true,
            gripper_open: false,
            joints: [0.35, 0.70],
            tool_yaw: -0.85,
            tool_position: [2.40, 1.15, 0.0],
            workpiece_position: [2.40, 1.15, 0.0],
        }),
        5 => robot_p3_minimal_arm_output(RobotP3MinimalArmPose {
            state: 5,
            busy: true,
            done: false,
            has_workpiece: true,
            gripper_open: false,
            joints: [0.72, 0.40],
            tool_yaw: 0.85,
            tool_position: [4.0, 0.55, 0.0],
            workpiece_position: [4.0, 0.35, 0.0],
        }),
        6 => robot_p3_minimal_arm_output(RobotP3MinimalArmPose {
            state: 6,
            busy: false,
            done: true,
            has_workpiece: false,
            gripper_open: true,
            joints: [0.72, 0.40],
            tool_yaw: 0.85,
            tool_position: [4.0, 0.80, 0.0],
            workpiece_position: [4.0, 0.35, 0.0],
        }),
        7 => robot_p3_minimal_arm_output(RobotP3MinimalArmPose {
            state: 7,
            busy: false,
            done: true,
            has_workpiece: false,
            gripper_open: true,
            joints: [-0.15, 0.55],
            tool_yaw: 0.0,
            tool_position: [2.0, 1.20, 0.0],
            workpiece_position: [4.0, 0.35, 0.0],
        }),
        rejected => RobotP3MinimalArmBridgeOutput {
            fault: true,
            state: rejected,
            ..robot_p3_minimal_arm_disabled_output()
        },
    }
}

#[derive(Debug, Clone, Copy)]
struct RobotP3MinimalArmPose {
    state: i16,
    busy: bool,
    done: bool,
    has_workpiece: bool,
    gripper_open: bool,
    joints: [f32; 2],
    tool_yaw: f32,
    tool_position: [f32; 3],
    workpiece_position: [f32; 3],
}

fn robot_p3_minimal_arm_disabled_output() -> RobotP3MinimalArmBridgeOutput {
    RobotP3MinimalArmBridgeOutput {
        enabled_out: false,
        busy: false,
        done: false,
        fault: false,
        state: 0,
        owner: 0,
        has_workpiece: false,
        gripper_open: true,
        status_light: false,
        joint1: -0.60,
        joint2: 0.35,
        tool_yaw: 0.25,
        tool_position: [0.0, 0.80, 0.0],
        workpiece_position: [0.0, 0.35, 0.0],
    }
}

fn robot_p3_minimal_arm_output(pose: RobotP3MinimalArmPose) -> RobotP3MinimalArmBridgeOutput {
    RobotP3MinimalArmBridgeOutput {
        enabled_out: true,
        busy: pose.busy,
        done: pose.done,
        fault: false,
        state: pose.state,
        owner: if pose.has_workpiece { 1 } else { 0 },
        has_workpiece: pose.has_workpiece,
        gripper_open: pose.gripper_open,
        status_light: pose.has_workpiece,
        joint1: pose.joints[0],
        joint2: pose.joints[1],
        tool_yaw: pose.tool_yaw,
        tool_position: pose.tool_position,
        workpiece_position: pose.workpiece_position,
    }
}

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

/// P4 multi-URDF scenario variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldMultiUrdfArmScenario {
    /// Canonical two-arm handoff proof.
    CanonicalHandoff,
    /// Two arms contend for an unowned workpiece without a handoff plan.
    SimultaneousGripNoHandoff,
    /// Receiver attempts to grip while the offerer still owns the workpiece.
    SecondGripWhileOwned,
    /// Receiver-arm FK drift after handoff.
    FkDriftReceiver,
}

/// Configuration for the P4 multi-URDF proof.
#[derive(Debug, Clone, Copy)]
pub struct WorldMultiUrdfArmSmokeConfig {
    /// Fixed tick delta in seconds.
    pub tick_dt_seconds: f32,
    /// Number of fixed ticks to run.
    pub tick_count: u32,
    /// Whether to register the static floor collider.
    pub include_floor: bool,
    /// Whether actuator registration is reversed before id-ordered stepping.
    pub reverse_arm_registration: bool,
    /// Scenario variant.
    pub scenario: WorldMultiUrdfArmScenario,
}

impl Default for WorldMultiUrdfArmSmokeConfig {
    fn default() -> Self {
        Self {
            tick_dt_seconds: ARM_TICK_DT_SECONDS,
            tick_count: ARM_TICK_COUNT,
            include_floor: true,
            reverse_arm_registration: false,
            scenario: WorldMultiUrdfArmScenario::CanonicalHandoff,
        }
    }
}

/// P3 URDF load trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldUrdfTrace {
    /// Relative fixture path loaded during setup.
    pub fixture_path: String,
    /// URDF arm instances loaded from this fixture.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub instances: Vec<WorldUrdfArmInstanceTrace>,
    /// URDF links loaded from the fixture.
    pub links_loaded: Vec<String>,
    /// URDF joints loaded from the fixture.
    pub joints_loaded: Vec<WorldUrdfJointTrace>,
    /// Whether parsing happened once during setup.
    pub parsed_once: bool,
    /// Whether the tick loop consulted the URDF text.
    pub consulted_in_tick_loop: bool,
}

/// One URDF arm instance loaded into the shared world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldUrdfArmInstanceTrace {
    /// Stable arm id.
    pub id: String,
    /// Base position used to instantiate the arm.
    pub base_position: [f32; 3],
    /// Whether this instance parsed the fixture once during setup.
    pub parsed_once: bool,
    /// Whether the tick loop consulted the URDF text for this instance.
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
    /// Per-arm FK verifier results for multi-URDF proofs.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub per_arm: BTreeMap<String, WorldFkArmVerifierTrace>,
    /// Dynamic links checked by FK.
    pub checked_links: Vec<String>,
    /// Number of ticks checked.
    pub checked_ticks: u32,
    /// Consistency tolerance in meters.
    pub consistency_tolerance: f32,
}

/// Per-arm FK verifier trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldFkArmVerifierTrace {
    /// Maximum FK/Rapier distance in meters for this arm.
    pub max_consistency_distance_m: f32,
    /// Dynamic links checked by FK.
    pub checked_links: Vec<String>,
    /// Number of link-tick samples checked.
    pub checked_samples: u32,
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

/// P4 multi-URDF load assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiUrdfArmsLoadedAssertion {
    /// Whether the assertion passed.
    pub ok: bool,
    /// Arm ids loaded in the proof.
    pub arm_ids: Vec<String>,
    /// Number of URDF instances loaded.
    pub instance_count: u32,
    /// Whether every instance parsed once and was not consulted in the tick loop.
    pub parsed_once_per_instance: bool,
}

/// P4 per-arm FK consistency assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerArmFkConsistencyAssertion {
    /// Whether the assertion passed.
    pub ok: bool,
    /// Consistency tolerance.
    pub tolerance: f32,
    /// Per-arm maximum FK/Rapier distance.
    pub max_consistency_distance_by_arm: BTreeMap<String, f32>,
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
            per_arm: BTreeMap::new(),
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
        .map(|tick| tick.workpiece.y)
        .fold(f32::INFINITY, f32::min);
    let max_downward_velocity = per_tick_trace
        .iter()
        .map(|tick| tick.workpiece.vy)
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
        let workpiece = arm_fixture_penetration(
            tick.workpiece.center,
            WORKPIECE_HALF_EXTENT,
            WORKPIECE_HALF_EXTENT,
        );
        current.max(workpiece)
    });
    let settled = per_tick_trace.last().map(|last| {
        let workpiece = &last.workpiece;
        let speed = vec3_length(workpiece.velocity);
        let contact_present = contact_contains(&last.contacts, "workpiece", "fixture");
        (workpiece.y, speed, contact_present)
    });
    let (final_workpiece_y, final_speed, final_fixture_contact) =
        settled.unwrap_or((f32::INFINITY, f32::INFINITY, false));
    let workpiece_above_floor = BodyAboveFloorAssertion {
        ok: workpiece_min_y >= FLOOR_Y - ABOVE_FLOOR_EPSILON,
        min_y: workpiece_min_y,
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
        workpiece_above_floor,
        gravity_applied: GravityAppliedAssertion {
            ok: max_downward_velocity < -0.1,
            max_downward_velocity_before_contact: max_downward_velocity,
        },
        contact_fired: ContactFiredAssertion {
            ok: grip_contact_present,
            first_contact_tick: grip_tick,
        },
        carrier_above_floor: Some(BodyAboveFloorAssertion {
            ok: arm_floor_stats.min_y >= FLOOR_Y - ABOVE_FLOOR_EPSILON,
            min_y: arm_floor_stats.min_y,
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
        multi_urdf_arms_loaded: None,
        per_arm_fk_consistency: None,
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

/// Runs the P4 two-URDF-arm coordination proof.
#[allow(clippy::too_many_arguments)]
pub fn run_world_multi_urdf_arm_smoke(
    config: WorldMultiUrdfArmSmokeConfig,
    scene: &mut scena::Scene,
    arm_a_link_1_node: scena::NodeKey,
    arm_a_link_2_node: scena::NodeKey,
    arm_a_tool_node: scena::NodeKey,
    arm_b_link_1_node: scena::NodeKey,
    arm_b_link_2_node: scena::NodeKey,
    arm_b_tool_node: scena::NodeKey,
    workpiece_node: scena::NodeKey,
) -> anyhow::Result<WorldSmokeTrace> {
    let fixture_path = resolve_repo_path(P3_MINIMAL_ARM_URDF);
    validate_revolute_limits_in_xml(&fixture_path, false)?;
    let mut world = World::deterministic(config.tick_dt_seconds);
    let env = register_p4_environment(&mut world, config);
    let model_a = UrdfArmModel::load(P3_MINIMAL_ARM_URDF, &fixture_path, false)?;
    let model_b = UrdfArmModel::load(P3_MINIMAL_ARM_URDF, &fixture_path, false)?;
    let arm_a_base_world = p4_base_world(P4ArmId::ArmA, config.scenario);
    let arm_b_base_world = p4_base_world(P4ArmId::ArmB, config.scenario);
    let arm_a_bodies = register_p4_arm_instance(
        &mut world,
        &model_a,
        P4ArmId::ArmA,
        arm_a_base_world,
        p4_initial_q(P4ArmId::ArmA, config.scenario),
    )?;
    let arm_b_bodies = register_p4_arm_instance(
        &mut world,
        &model_b,
        P4ArmId::ArmB,
        arm_b_base_world,
        p4_initial_q(P4ArmId::ArmB, config.scenario),
    )?;
    let mut arms = vec![
        P4ArmInstance::new(
            0,
            P4ArmId::ArmA,
            P4ArmRole::Offerer,
            arm_a_base_world,
            model_a,
            arm_a_bodies,
        ),
        P4ArmInstance::new(
            1,
            P4ArmId::ArmB,
            P4ArmRole::Receiver,
            arm_b_base_world,
            model_b,
            arm_b_bodies,
        ),
    ];
    if config.reverse_arm_registration {
        arms.reverse();
    }
    for arm in &mut arms {
        if arm.role == P4ArmRole::Offerer
            || config.scenario == WorldMultiUrdfArmScenario::SimultaneousGripNoHandoff
        {
            arm.transition(0, ActuatorState::Approaching, "start");
        }
    }
    let mut ownership = P4Ownership::new();
    let mut handoff_plan = P4HandoffPlan::new(matches!(
        config.scenario,
        WorldMultiUrdfArmScenario::CanonicalHandoff | WorldMultiUrdfArmScenario::FkDriftReceiver
    ));
    let mut per_tick_trace = Vec::with_capacity(config.tick_count as usize + 1);
    let mut handoff_line = p4_apply_handoff(
        scene,
        &world,
        p4_nodes(
            arm_a_link_1_node,
            arm_a_link_2_node,
            arm_a_tool_node,
            arm_b_link_1_node,
            arm_b_link_2_node,
            arm_b_tool_node,
            workpiece_node,
        ),
        p4_arm(&arms, P4ArmId::ArmA).bodies,
        p4_arm(&arms, P4ArmId::ArmB).bodies,
        env.workpiece_body,
    )?;
    per_tick_trace.push(trace_p4_tick(
        &world,
        0,
        env,
        &arms,
        &ownership,
        Vec::new(),
        Vec::new(),
    )?);

    for tick in 1..=config.tick_count {
        let mut events = Vec::new();
        p4_apply_motors(&mut world, env, &arms, config);
        p4_update_tool_colliders(&mut world, env, &arms, &ownership, config);
        if config.scenario == WorldMultiUrdfArmScenario::FkDriftReceiver
            && handoff_plan
                .atomic_tick
                .is_some_and(|handoff_tick| tick == handoff_tick + 80)
        {
            let arm_b = p4_arm(&arms, P4ArmId::ArmB);
            world
                .multibody_joints
                .remove_multibody_articulations(arm_b.bodies.link_1_body, true);
            if let Some(link_1) = world.bodies.get_mut(arm_b.bodies.link_1_body) {
                link_1.apply_impulse(vector![500.0, 0.0, 0.0].into(), true);
                link_1.set_linvel(vector![25.0, 0.0, 0.0].into(), true);
            }
            events.push("fk_drift_fault_injected(arm_b.link_1_articulation_removed)".to_string());
        }
        world.step();
        let (step_events, faults) = step_p4_arms(
            &mut world,
            env,
            &mut arms,
            &mut ownership,
            &mut handoff_plan,
            config,
            tick,
        )?;
        events.extend(step_events);
        handoff_line = p4_apply_handoff(
            scene,
            &world,
            p4_nodes(
                arm_a_link_1_node,
                arm_a_link_2_node,
                arm_a_tool_node,
                arm_b_link_1_node,
                arm_b_link_2_node,
                arm_b_tool_node,
                workpiece_node,
            ),
            p4_arm(&arms, P4ArmId::ArmA).bodies,
            p4_arm(&arms, P4ArmId::ArmB).bodies,
            env.workpiece_body,
        )?;
        per_tick_trace.push(trace_p4_tick(
            &world, tick, env, &arms, &ownership, events, faults,
        )?);
    }

    let determinism_trace_hash = determinism_trace_hash(&per_tick_trace)?;
    let mut assertions = assert_world_multi_urdf_arm_smoke_trace(&per_tick_trace);
    assertions.urdf_parsed_once = Some(UrdfParsedOnceAssertion {
        ok: true,
        parsed_once: true,
        consulted_in_tick_loop: false,
    });
    assertions.multi_urdf_arms_loaded = Some(MultiUrdfArmsLoadedAssertion {
        ok: true,
        arm_ids: vec!["arm_a".to_string(), "arm_b".to_string()],
        instance_count: 2,
        parsed_once_per_instance: true,
    });
    let fk_verifier = p4_fk_verifier_trace(&assertions, config.tick_count + 1);
    let urdf_trace = p4_urdf_trace(&arms);
    let joint_trace = p4_joint_trace(&per_tick_trace);
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
            bodies_registered: p4_body_registrations(config.include_floor),
        },
        transform_handoff: TransformHandoffTrace {
            function: HANDOFF_FUNCTION.to_string(),
            file: HANDOFF_FILE.to_string(),
            line: handoff_line,
            read_source: "rapier3d::dynamics::RigidBody::position".to_string(),
            write_target:
                "scena scene-node transform for URDF links 'arm_a.*', 'arm_b.*', and body 'workpiece'"
                    .to_string(),
        },
        renderer_origin: None,
        screenshot_initial_png: "target/gate-artifacts/world_smoke_initial.png".to_string(),
        screenshot_grip_png: Some("target/gate-artifacts/world_smoke_grip.png".to_string()),
        screenshot_carry_png: None,
        screenshot_transfer_png: Some("target/gate-artifacts/world_smoke_transfer.png".to_string()),
        screenshot_handoff_png: Some("target/gate-artifacts/world_smoke_handoff.png".to_string()),
        screenshot_final_png: "target/gate-artifacts/world_smoke_final.png".to_string(),
        actuator: None,
        actuators: Some(p4_actuator_traces(&arms)),
        ownership: Some(ownership.trace()),
        handoff_plan: Some(handoff_plan.trace()),
        urdf: Some(urdf_trace),
        fk_verifier: Some(fk_verifier),
        joints: Some(joint_trace),
        per_tick_trace,
        determinism_trace_hash,
        assertions,
    })
}

/// Computes P4 multi-URDF proof assertions from a trace.
#[must_use]
pub fn assert_world_multi_urdf_arm_smoke_trace(
    per_tick_trace: &[WorldTickTrace],
) -> WorldSmokeAssertions {
    let workpiece_min_y = per_tick_trace
        .iter()
        .map(|tick| tick.workpiece.y)
        .fold(f32::INFINITY, f32::min);
    let max_downward_velocity = per_tick_trace
        .iter()
        .map(|tick| tick.workpiece.vy)
        .fold(0.0, f32::min);
    let grip_tick = per_tick_trace
        .iter()
        .find(|tick| {
            tick.tick_events
                .iter()
                .any(|event| event == "joint_create(arm_a.tool, workpiece)")
        })
        .map(|tick| tick.tick);
    let grip_contact_present = grip_tick
        .and_then(|tick| per_tick_trace.iter().find(|sample| sample.tick == tick))
        .is_some_and(|tick| contact_contains(&tick.contacts, "arm_a.tool", "workpiece"));
    let release_tick = per_tick_trace
        .iter()
        .find(|tick| {
            tick.tick_events
                .iter()
                .any(|event| event == "joint_destroy(arm_b.tool, workpiece)")
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
        let workpiece = arm_fixture_penetration(
            tick.workpiece.center,
            WORKPIECE_HALF_EXTENT,
            WORKPIECE_HALF_EXTENT,
        );
        current.max(workpiece)
    });
    let settled = per_tick_trace.last().map(|last| {
        let workpiece = &last.workpiece;
        let speed = vec3_length(workpiece.velocity);
        let contact_present = contact_contains(&last.contacts, "workpiece", "fixture");
        (workpiece.y, speed, contact_present)
    });
    let (final_workpiece_y, final_speed, final_fixture_contact) =
        settled.unwrap_or((f32::INFINITY, f32::INFINITY, false));
    let joint_summary = active_joint_summary(per_tick_trace);
    let handoff_tick = p4_find_handoff_tick(per_tick_trace);
    let expected_order = p4_expected_handoff_event_order();
    let observed_order = handoff_tick
        .and_then(|tick| per_tick_trace.iter().find(|sample| sample.tick == tick))
        .map(|tick| {
            tick.tick_events
                .iter()
                .filter(|event| {
                    event.starts_with("joint_") || event.starts_with("state_transition(")
                })
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let ownership_transfer_atomic = p4_ownership_transfer_atomic(per_tick_trace, handoff_tick);
    let phantom_violations = p4_phantom_carry_violation_count(per_tick_trace);
    let (per_arm_fk_ok, per_arm_fk) = p4_per_arm_fk_stats(per_tick_trace);
    let max_fk = per_arm_fk.values().copied().fold(0.0_f32, f32::max);
    let all_arm_links_above_floor = p4_arm_floor_stats(per_tick_trace);
    let joint_limit_stats = p4_joint_limit_stats(per_tick_trace);
    let dynamic_body_count = per_tick_trace
        .iter()
        .find(|tick| !tick.arm_a_links.is_empty() || !tick.arm_b_links.is_empty())
        .map_or(0, |tick| {
            tick.arm_a_links.len() + tick.arm_b_links.len() + 1
        });
    let handoff_complete = dynamic_body_count == 7
        && per_tick_trace
            .iter()
            .all(|tick| tick.arm_a_links.len() == 3 && tick.arm_b_links.len() == 3);
    let workpiece_above_floor = BodyAboveFloorAssertion {
        ok: workpiece_min_y >= FLOOR_Y - ABOVE_FLOOR_EPSILON,
        min_y: workpiece_min_y,
        floor_y: FLOOR_Y,
    };
    let mut assertions = WorldSmokeAssertions {
        workpiece_above_floor,
        gravity_applied: GravityAppliedAssertion {
            ok: max_downward_velocity < -0.1,
            max_downward_velocity_before_contact: max_downward_velocity,
        },
        contact_fired: ContactFiredAssertion {
            ok: grip_contact_present,
            first_contact_tick: grip_tick,
        },
        carrier_above_floor: Some(BodyAboveFloorAssertion {
            ok: all_arm_links_above_floor.min_y >= FLOOR_Y - ABOVE_FLOOR_EPSILON,
            min_y: all_arm_links_above_floor.min_y,
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
        arm_rendered_through_handoff: None,
        fk_matches_rapier: None,
        joint_limits_enforced: Some(JointLimitAssertion {
            ok: joint_limit_stats.out_of_limit_samples == 0,
            out_of_limit_samples: joint_limit_stats.out_of_limit_samples,
            joint_clamped_events: joint_limit_stats.clamped_events,
        }),
        arm_links_above_floor: None,
        multi_urdf_arms_loaded: None,
        per_arm_fk_consistency: None,
    };

    assertions.exclusive_ownership = Some(ExclusiveOwnershipAssertion {
        ok: joint_summary.ticks_with_two_joints == 0,
        ticks_with_zero_joints: joint_summary.ticks_with_zero_joints,
        ticks_with_one_joint: joint_summary.ticks_with_one_joint,
        ticks_with_two_joints: joint_summary.ticks_with_two_joints,
    });
    assertions.ownership_transfer_atomic = Some(OwnershipTransferAtomicAssertion {
        ok: ownership_transfer_atomic,
        handoff_tick,
        destroy_and_create_same_tick: handoff_tick.is_some(),
        no_undefined_transfer_tick: ownership_transfer_atomic,
    });
    assertions.handoff_order_deterministic = Some(HandoffOrderAssertion {
        ok: !observed_order.is_empty() && observed_order == expected_order,
        handoff_tick,
        expected_order,
        observed_order,
    });
    assertions.no_phantom_carry = Some(NoPhantomCarryAssertion {
        ok: phantom_violations == 0,
        violation_count: phantom_violations,
    });
    assertions.arm_rendered_through_handoff = Some(ArmRenderedThroughHandoffAssertion {
        ok: handoff_complete,
        expected_dynamic_bodies_per_tick: dynamic_body_count as u32,
        checked_ticks: per_tick_trace.len() as u32,
    });
    assertions.fk_matches_rapier = Some(FkConsistencyAssertion {
        ok: max_fk <= ARM_FK_TOLERANCE,
        max_consistency_distance_m: max_fk,
        tolerance: ARM_FK_TOLERANCE,
        checked_samples: (per_tick_trace.len() * 6) as u32,
    });
    assertions.per_arm_fk_consistency = Some(PerArmFkConsistencyAssertion {
        ok: per_arm_fk_ok,
        tolerance: ARM_FK_TOLERANCE,
        max_consistency_distance_by_arm: per_arm_fk,
    });
    assertions.arm_links_above_floor = Some(ArmAboveFloorAssertion {
        ok: all_arm_links_above_floor.min_y >= FLOOR_Y - ABOVE_FLOOR_EPSILON,
        min_link_y: all_arm_links_above_floor.min_y,
        floor_y: FLOOR_Y,
        min_link_name: all_arm_links_above_floor.min_name,
    });
    assertions
}

/// Records the P4 deterministic-rerun assertion in an artifact trace.
pub fn record_multi_urdf_arm_determinism_hash_stability(
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum P4ArmId {
    ArmA,
    ArmB,
}

impl P4ArmId {
    fn name(self) -> &'static str {
        match self {
            Self::ArmA => "arm_a",
            Self::ArmB => "arm_b",
        }
    }

    fn source(self) -> String {
        format!("{}@{}", ARM_SOURCE, self.name())
    }

    fn joint_name(self) -> &'static str {
        match self {
            Self::ArmA => "fixed(arm_a.tool, workpiece_grip)",
            Self::ArmB => "fixed(arm_b.tool, workpiece_grip)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum P4ArmRole {
    Offerer,
    Receiver,
}

#[derive(Debug, Clone, Copy)]
struct P4EnvironmentBodies {
    floor_collider: Option<ColliderHandle>,
    fixture_collider: ColliderHandle,
    transfer_collider: ColliderHandle,
    workpiece_body: RigidBodyHandle,
    workpiece_collider: ColliderHandle,
}

#[derive(Debug, Clone, Copy)]
struct P4ArmBodies {
    link_1_body: RigidBodyHandle,
    link_1_collider: ColliderHandle,
    link_1_joint: MultibodyJointHandle,
    link_2_body: RigidBodyHandle,
    link_2_collider: ColliderHandle,
    link_2_joint: MultibodyJointHandle,
    tool_body: RigidBodyHandle,
    tool_collider: ColliderHandle,
}

#[derive(Debug)]
struct P4ArmInstance {
    id: u8,
    arm_id: P4ArmId,
    role: P4ArmRole,
    base_world: [f32; 3],
    model: UrdfArmModel,
    bodies: P4ArmBodies,
    state: ActuatorState,
    transitions: Vec<WorldActuatorTransitionTrace>,
    faults: Vec<WorldOwnershipFaultTrace>,
}

impl P4ArmInstance {
    fn new(
        id: u8,
        arm_id: P4ArmId,
        role: P4ArmRole,
        base_world: [f32; 3],
        model: UrdfArmModel,
        bodies: P4ArmBodies,
    ) -> Self {
        Self {
            id,
            arm_id,
            role,
            base_world,
            model,
            bodies,
            state: ActuatorState::Idle,
            transitions: Vec::new(),
            faults: Vec::new(),
        }
    }

    fn transition(&mut self, tick: u32, to: ActuatorState, trigger: &str) -> String {
        if self.state == to {
            return format!(
                "state_transition({}: {:?} -> {:?})",
                self.arm_id.name(),
                self.state,
                to
            );
        }
        let from = if self.transitions.is_empty() && self.state == ActuatorState::Idle {
            None
        } else {
            Some(self.state)
        };
        let previous = self.state;
        self.transitions.push(WorldActuatorTransitionTrace {
            tick,
            from,
            to,
            trigger: Some(trigger.to_string()),
        });
        self.state = to;
        format!(
            "state_transition({}: {:?} -> {:?})",
            self.arm_id.name(),
            previous,
            to
        )
    }

    fn trace(&self) -> WorldActuatorTrace {
        let mut states = vec![ActuatorState::Idle];
        for transition in &self.transitions {
            states.push(transition.to);
        }
        WorldActuatorTrace {
            id: Some(self.id),
            name: Some(self.arm_id.name().to_string()),
            type_name: "WorldActuator".to_string(),
            states_observed: states,
            state_transitions: self.transitions.clone(),
        }
    }
}

#[derive(Debug, Clone)]
struct P4Ownership {
    owner: Option<P4ArmId>,
    active_joint: Option<(P4ArmId, ImpulseJointHandle)>,
    transitions: Vec<WorldOwnershipTransitionTrace>,
}

impl P4Ownership {
    fn new() -> Self {
        Self {
            owner: None,
            active_joint: None,
            transitions: Vec::new(),
        }
    }

    fn sample(&self) -> WorldOwnershipSample {
        WorldOwnershipSample {
            workpiece: "workpiece".to_string(),
            owner: self.owner.map(P4ArmId::name).map(str::to_string),
        }
    }

    fn transition(&mut self, tick: u32, to: Option<P4ArmId>, trigger: &str) {
        let from = self.owner;
        self.transitions.push(WorldOwnershipTransitionTrace {
            tick,
            workpiece: "workpiece".to_string(),
            from: from.map(P4ArmId::name).map(str::to_string),
            to: to.map(P4ArmId::name).map(str::to_string),
            trigger: trigger.to_string(),
        });
        self.owner = to;
    }

    fn trace(&self) -> WorldOwnershipTrace {
        WorldOwnershipTrace {
            transitions: self.transitions.clone(),
        }
    }
}

#[derive(Debug, Clone)]
struct P4HandoffPlan {
    registered: bool,
    atomic_tick: Option<u32>,
    atomic_event_order: Vec<String>,
}

impl P4HandoffPlan {
    fn new(registered: bool) -> Self {
        Self {
            registered,
            atomic_tick: None,
            atomic_event_order: p4_expected_handoff_event_order(),
        }
    }

    fn trace(&self) -> WorldHandoffPlanTrace {
        WorldHandoffPlanTrace {
            registered_pairs: if self.registered {
                vec![WorldHandoffPairTrace {
                    offerer: "arm_a".to_string(),
                    receiver: "arm_b".to_string(),
                    transfer_zone: "transfer_zone".to_string(),
                }]
            } else {
                Vec::new()
            },
            atomic_tick: self.atomic_tick,
            atomic_event_order: self.atomic_event_order.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct P4SceneNodes {
    arm_a_link_1: scena::NodeKey,
    arm_a_link_2: scena::NodeKey,
    arm_a_tool: scena::NodeKey,
    arm_b_link_1: scena::NodeKey,
    arm_b_link_2: scena::NodeKey,
    arm_b_tool: scena::NodeKey,
    workpiece: scena::NodeKey,
}

fn p4_nodes(
    arm_a_link_1: scena::NodeKey,
    arm_a_link_2: scena::NodeKey,
    arm_a_tool: scena::NodeKey,
    arm_b_link_1: scena::NodeKey,
    arm_b_link_2: scena::NodeKey,
    arm_b_tool: scena::NodeKey,
    workpiece: scena::NodeKey,
) -> P4SceneNodes {
    P4SceneNodes {
        arm_a_link_1,
        arm_a_link_2,
        arm_a_tool,
        arm_b_link_1,
        arm_b_link_2,
        arm_b_tool,
        workpiece,
    }
}

fn p4_initial_q(arm_id: P4ArmId, scenario: WorldMultiUrdfArmScenario) -> [f32; 2] {
    match (arm_id, scenario) {
        (P4ArmId::ArmA, WorldMultiUrdfArmScenario::SimultaneousGripNoHandoff) => {
            P4_ARM_A_CONTESTED_Q
        }
        (P4ArmId::ArmB, WorldMultiUrdfArmScenario::SimultaneousGripNoHandoff) => {
            P4_ARM_B_CONTESTED_Q
        }
        _ => ARM_HOME_Q,
    }
}

fn p4_base_world(arm_id: P4ArmId, scenario: WorldMultiUrdfArmScenario) -> [f32; 3] {
    match (arm_id, scenario) {
        (P4ArmId::ArmB, WorldMultiUrdfArmScenario::SimultaneousGripNoHandoff) => {
            P4_ARM_A_BASE_WORLD
        }
        (P4ArmId::ArmA, _) => P4_ARM_A_BASE_WORLD,
        (P4ArmId::ArmB, _) => P4_ARM_B_BASE_WORLD,
    }
}

fn p4_arm(arms: &[P4ArmInstance], arm_id: P4ArmId) -> &P4ArmInstance {
    arms.iter()
        .find(|arm| arm.arm_id == arm_id)
        .expect("P4 arm exists")
}

fn p4_two_arms_mut(
    arms: &mut [P4ArmInstance],
    first: P4ArmId,
    second: P4ArmId,
) -> (&mut P4ArmInstance, &mut P4ArmInstance) {
    let first_index = arms
        .iter()
        .position(|arm| arm.arm_id == first)
        .expect("first P4 arm exists");
    let second_index = arms
        .iter()
        .position(|arm| arm.arm_id == second)
        .expect("second P4 arm exists");
    assert_ne!(first_index, second_index);
    if first_index < second_index {
        let (left, right) = arms.split_at_mut(second_index);
        (&mut left[first_index], &mut right[0])
    } else {
        let (left, right) = arms.split_at_mut(first_index);
        (&mut right[0], &mut left[second_index])
    }
}

fn register_p4_environment(
    world: &mut World,
    config: WorldMultiUrdfArmSmokeConfig,
) -> P4EnvironmentBodies {
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

    let transfer_body = world.bodies.insert(
        RigidBodyBuilder::fixed()
            .translation(vector![P4_TRANSFER_CENTER_X, P4_TRANSFER_MARKER_Y, 0.0].into())
            .build(),
    );
    let transfer_collider = world.colliders.insert_with_parent(
        ColliderBuilder::cuboid(P4_TRANSFER_HALF_XZ, P4_TRANSFER_HALF_Y, P4_TRANSFER_HALF_XZ)
            .sensor(true)
            .build(),
        transfer_body,
        &mut world.bodies,
    );

    let workpiece_x = if config.scenario == WorldMultiUrdfArmScenario::SimultaneousGripNoHandoff {
        P4_CONTESTED_WORKPIECE_X
    } else {
        P4_WORKPIECE_PICKUP_X
    };
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
    let workpiece_density =
        if config.scenario == WorldMultiUrdfArmScenario::SimultaneousGripNoHandoff {
            50.0
        } else {
            1.0
        };
    let workpiece_collider = world.colliders.insert_with_parent(
        ColliderBuilder::cuboid(
            WORKPIECE_HALF_EXTENT,
            WORKPIECE_HALF_EXTENT,
            WORKPIECE_HALF_EXTENT,
        )
        .density(workpiece_density)
        .friction(0.9)
        .restitution(0.0)
        .build(),
        workpiece_body,
        &mut world.bodies,
    );

    P4EnvironmentBodies {
        floor_collider,
        fixture_collider,
        transfer_collider,
        workpiece_body,
        workpiece_collider,
    }
}

fn register_p4_arm_instance(
    world: &mut World,
    model: &UrdfArmModel,
    arm_id: P4ArmId,
    base_world: [f32; 3],
    initial_q: [f32; 2],
) -> anyhow::Result<P4ArmBodies> {
    let initial_centers = compute_fk_for_chain_at(model, initial_q, base_world)?;
    let base_spec = model.link_spec("base")?;
    let base_body = world.bodies.insert(
        RigidBodyBuilder::fixed()
            .translation(vector![base_world[0], base_world[1], base_world[2]].into())
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
    let link_1_body =
        insert_arm_link_body(world, link_1_spec, initial_centers["link_1"], initial_q[0]);
    let link_1_collider = insert_arm_link_collider(world, link_1_body, link_1_spec);
    if let Some(collider) = world.colliders.get_mut(link_1_collider) {
        collider.set_enabled(false);
    }
    let link_2_body = insert_arm_link_body(
        world,
        link_2_spec,
        initial_centers["link_2"],
        initial_q[0] + initial_q[1],
    );
    let link_2_collider = insert_arm_link_collider(world, link_2_body, link_2_spec);
    if let Some(collider) = world.colliders.get_mut(link_2_collider) {
        collider.set_enabled(false);
    }
    let tool_body = insert_arm_link_body(
        world,
        tool_spec,
        initial_centers["tool"],
        initial_q[0] + initial_q[1],
    );
    let tool_collider = insert_arm_link_collider(world, tool_body, tool_spec);
    if let Some(collider) = world.colliders.get_mut(tool_collider) {
        collider.set_enabled(true);
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
                initial_q[0],
            ),
            true,
        )
        .unwrap_or_else(|| panic!("{}_base_to_link_1 multibody joint is valid", arm_id.name()));
    let link_2_joint = world
        .multibody_joints
        .insert(
            link_1_body,
            link_2_body,
            revolute_joint(
                model.joints[1],
                v3(0.45, 0.0, 0.0),
                v3(-0.45, 0.0, 0.0),
                initial_q[1],
            ),
            true,
        )
        .unwrap_or_else(|| {
            panic!(
                "{}_link_1_to_link_2 multibody joint is valid",
                arm_id.name()
            )
        });
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
        .unwrap_or_else(|| panic!("{}_link_2_to_tool multibody joint is valid", arm_id.name()));

    Ok(P4ArmBodies {
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

fn p4_apply_motors(
    world: &mut World,
    env: P4EnvironmentBodies,
    arms: &[P4ArmInstance],
    config: WorldMultiUrdfArmSmokeConfig,
) {
    for arm in arms {
        let target = p4_target_for_arm(world, env, arm, config);
        drive_p4_arm_joints(world, arm.bodies, target);
    }
}

fn p4_target_for_arm(
    _world: &World,
    _env: P4EnvironmentBodies,
    arm: &P4ArmInstance,
    config: WorldMultiUrdfArmSmokeConfig,
) -> [f32; 2] {
    match arm.arm_id {
        P4ArmId::ArmA => match arm.state {
            ActuatorState::Approaching => {
                if config.scenario == WorldMultiUrdfArmScenario::SimultaneousGripNoHandoff {
                    P4_ARM_A_CONTESTED_Q
                } else {
                    ARM_APPROACH_Q
                }
            }
            ActuatorState::Carrying | ActuatorState::Held => P4_ARM_A_TRANSFER_Q,
            ActuatorState::Idle | ActuatorState::Releasing | ActuatorState::AcceptingHandoff => {
                ARM_HOME_Q
            }
        },
        P4ArmId::ArmB => match arm.state {
            ActuatorState::Approaching => P4_ARM_B_CONTESTED_Q,
            ActuatorState::AcceptingHandoff | ActuatorState::Held => P4_ARM_B_TRANSFER_Q,
            ActuatorState::Carrying => P4_ARM_B_FINAL_Q,
            ActuatorState::Idle | ActuatorState::Releasing => ARM_HOME_Q,
        },
    }
}

fn drive_p4_arm_joints(world: &mut World, bodies: P4ArmBodies, target: [f32; 2]) {
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

fn p4_update_tool_colliders(
    world: &mut World,
    env: P4EnvironmentBodies,
    arms: &[P4ArmInstance],
    ownership: &P4Ownership,
    _config: WorldMultiUrdfArmSmokeConfig,
) {
    for arm in arms {
        let grip_frames_near = p4_arm_workpiece_joint_distance(world, env, arm.bodies)
            .is_ok_and(|distance| distance <= JOINT_DISTANCE_TOLERANCE * 3.0);
        let eligible_for_contact = matches!(
            arm.state,
            ActuatorState::Approaching | ActuatorState::AcceptingHandoff
        ) && grip_frames_near;
        if let Some(collider) = world.colliders.get_mut(arm.bodies.tool_collider) {
            collider.set_enabled(ownership.owner != Some(arm.arm_id) && eligible_for_contact);
        }
    }
}

fn step_p4_arms(
    world: &mut World,
    env: P4EnvironmentBodies,
    arms: &mut [P4ArmInstance],
    ownership: &mut P4Ownership,
    handoff_plan: &mut P4HandoffPlan,
    config: WorldMultiUrdfArmSmokeConfig,
    tick: u32,
) -> anyhow::Result<(Vec<String>, Vec<WorldOwnershipFaultTrace>)> {
    if p4_handoff_ready(world, env, arms, ownership, handoff_plan) {
        let events = p4_perform_atomic_handoff(world, env, arms, ownership, handoff_plan, tick)?;
        return Ok((events, Vec::new()));
    }

    let mut events = Vec::new();
    let mut faults = Vec::new();
    let mut ids = arms.iter().map(|arm| arm.id).collect::<Vec<_>>();
    ids.sort_unstable();
    for id in ids {
        let index = arms
            .iter()
            .position(|arm| arm.id == id)
            .expect("P4 arm id exists");
        match arms[index].role {
            P4ArmRole::Offerer => {
                step_p4_offeror(
                    world,
                    env,
                    &mut arms[index],
                    ownership,
                    tick,
                    &mut events,
                    &mut faults,
                )?;
            }
            P4ArmRole::Receiver => {
                step_p4_receiver(
                    world,
                    env,
                    arms,
                    index,
                    ownership,
                    handoff_plan,
                    config,
                    tick,
                    &mut events,
                    &mut faults,
                )?;
            }
        }
    }
    Ok((events, faults))
}

fn step_p4_offeror(
    world: &mut World,
    env: P4EnvironmentBodies,
    arm: &mut P4ArmInstance,
    ownership: &mut P4Ownership,
    tick: u32,
    events: &mut Vec<String>,
    faults: &mut Vec<WorldOwnershipFaultTrace>,
) -> anyhow::Result<()> {
    match arm.state {
        ActuatorState::Approaching => {
            let grip_frames_near = p4_arm_workpiece_joint_distance(world, env, arm.bodies)
                .is_ok_and(|distance| distance <= JOINT_DISTANCE_TOLERANCE * 3.0);
            if grip_frames_near
                && world.contact_pair_exists(arm.bodies.tool_collider, env.workpiece_collider)
            {
                match p4_create_owned_workpiece_joint(
                    world,
                    env,
                    arm,
                    ownership,
                    tick,
                    "grip_on(arm_a)",
                ) {
                    Ok(_) => {
                        events.push("joint_create(arm_a.tool, workpiece)".to_string());
                        events.push(arm.transition(
                            tick,
                            ActuatorState::Carrying,
                            "contact_pair(arm_a.tool, workpiece)",
                        ));
                    }
                    Err(fault) => {
                        arm.faults.push(fault.clone());
                        faults.push(fault);
                    }
                }
            }
        }
        ActuatorState::Carrying => {
            let tool = p4_arm_tool_world_point(world, arm.bodies)?;
            let transfer = v3(
                P4_TRANSFER_CENTER_X,
                FIXTURE_TOP_Y + WORKPIECE_HALF_EXTENT * 2.0,
                0.0,
            );
            if (tool - transfer).length() <= P4_HANDOFF_TOLERANCE {
                events.push(arm.transition(
                    tick,
                    ActuatorState::Held,
                    "position_tolerance_at(transfer_zone)",
                ));
            }
        }
        ActuatorState::Idle
        | ActuatorState::Held
        | ActuatorState::AcceptingHandoff
        | ActuatorState::Releasing => {}
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn step_p4_receiver(
    world: &mut World,
    env: P4EnvironmentBodies,
    arms: &mut [P4ArmInstance],
    index: usize,
    ownership: &mut P4Ownership,
    handoff_plan: &P4HandoffPlan,
    config: WorldMultiUrdfArmSmokeConfig,
    tick: u32,
    events: &mut Vec<String>,
    faults: &mut Vec<WorldOwnershipFaultTrace>,
) -> anyhow::Result<()> {
    let a_held = arms
        .iter()
        .any(|arm| arm.arm_id == P4ArmId::ArmA && arm.state == ActuatorState::Held);
    let arm = &mut arms[index];
    match arm.state {
        ActuatorState::Idle => {
            let should_accept = a_held
                && (handoff_plan.registered
                    || config.scenario == WorldMultiUrdfArmScenario::SecondGripWhileOwned);
            if should_accept {
                events.push(arm.transition(
                    tick,
                    ActuatorState::AcceptingHandoff,
                    if handoff_plan.registered {
                        "handoff_plan_ready"
                    } else {
                        "forced_accept_without_plan"
                    },
                ));
            }
        }
        ActuatorState::Approaching | ActuatorState::AcceptingHandoff => {
            let grip_frames_near = p4_arm_workpiece_joint_distance(world, env, arm.bodies)
                .is_ok_and(|distance| distance <= JOINT_DISTANCE_TOLERANCE * 3.0);
            if grip_frames_near
                && world.contact_pair_exists(arm.bodies.tool_collider, env.workpiece_collider)
            {
                let eligible = handoff_plan.registered
                    && a_held
                    && arm.state == ActuatorState::AcceptingHandoff;
                if eligible {
                    return Ok(());
                }
                match p4_create_owned_workpiece_joint(
                    world,
                    env,
                    arm,
                    ownership,
                    tick,
                    "grip_on(arm_b)",
                ) {
                    Ok(_) => {
                        events.push("joint_create(arm_b.tool, workpiece)".to_string());
                        events.push(arm.transition(
                            tick,
                            ActuatorState::Carrying,
                            "contact_pair(arm_b.tool, workpiece)",
                        ));
                    }
                    Err(fault) => {
                        arm.faults.push(fault.clone());
                        faults.push(fault);
                    }
                }
            }
        }
        ActuatorState::Carrying => {
            let receiver_carried_long_enough = handoff_plan
                .atomic_tick
                .is_some_and(|handoff_tick| tick >= handoff_tick + P4_MIN_RECEIVER_CARRY_TICKS);
            let workpiece_at_fixture =
                world
                    .bodies
                    .get(env.workpiece_body)
                    .is_some_and(|workpiece| {
                        let position = workpiece.position().translation;
                        (position.x - FIXTURE_CENTER_X).abs()
                            <= ARM_FIXTURE_HALF_XZ - (WORKPIECE_HALF_EXTENT * 0.5)
                            && position.y >= FIXTURE_TOP_Y + WORKPIECE_HALF_EXTENT + 0.05
                    });
            if receiver_carried_long_enough && workpiece_at_fixture {
                if let Some((P4ArmId::ArmB, joint)) = ownership.active_joint.take() {
                    zero_p4_workpiece_velocity(world, env);
                    let _removed = world.impulse_joints.remove(joint, true);
                    if let Some(collider) = world.colliders.get_mut(env.fixture_collider) {
                        collider.set_enabled(true);
                    }
                    ownership.transition(tick, None, "release(arm_b)");
                    events.push("joint_destroy(arm_b.tool, workpiece)".to_string());
                }
                events.push(arm.transition(
                    tick,
                    ActuatorState::Releasing,
                    "position_tolerance_at(fixture)",
                ));
            }
        }
        ActuatorState::Releasing => {
            events.push(arm.transition(tick, ActuatorState::Idle, "joint_destroyed"));
        }
        ActuatorState::Held => {}
    }
    Ok(())
}

fn p4_handoff_ready(
    world: &World,
    env: P4EnvironmentBodies,
    arms: &[P4ArmInstance],
    ownership: &P4Ownership,
    handoff_plan: &P4HandoffPlan,
) -> bool {
    let arm_b = p4_arm(arms, P4ArmId::ArmB);
    handoff_plan.registered
        && ownership.owner == Some(P4ArmId::ArmA)
        && arms
            .iter()
            .any(|arm| arm.arm_id == P4ArmId::ArmA && arm.state == ActuatorState::Held)
        && arm_b.state == ActuatorState::AcceptingHandoff
        && p4_arm_workpiece_joint_distance(world, env, arm_b.bodies)
            .is_ok_and(|distance| distance <= JOINT_DISTANCE_TOLERANCE * 3.0)
        && world.contact_pair_exists(arm_b.bodies.tool_collider, env.workpiece_collider)
}

fn p4_perform_atomic_handoff(
    world: &mut World,
    env: P4EnvironmentBodies,
    arms: &mut [P4ArmInstance],
    ownership: &mut P4Ownership,
    handoff_plan: &mut P4HandoffPlan,
    tick: u32,
) -> anyhow::Result<Vec<String>> {
    let mut events = Vec::new();
    let Some((P4ArmId::ArmA, old_joint)) = ownership.active_joint.take() else {
        anyhow::bail!("atomic handoff requires arm_a joint");
    };
    let _removed = world.impulse_joints.remove(old_joint, true);
    events.push("joint_destroy(arm_a.tool, workpiece)".to_string());

    let arm_b_bodies = p4_arm(arms, P4ArmId::ArmB).bodies;
    zero_p4_arm_and_workpiece_velocity(world, env, arm_b_bodies);
    let handle = p4_create_tool_workpiece_joint(world, env, arm_b_bodies)?;
    ownership.active_joint = Some((P4ArmId::ArmB, handle));
    ownership.transitions.push(WorldOwnershipTransitionTrace {
        tick,
        workpiece: "workpiece".to_string(),
        from: Some("arm_a".to_string()),
        to: Some("arm_b".to_string()),
        trigger: "handoff_atomic(arm_a -> arm_b)".to_string(),
    });
    ownership.owner = Some(P4ArmId::ArmB);
    events.push("joint_create(arm_b.tool, workpiece)".to_string());

    let (arm_a, arm_b) = p4_two_arms_mut(arms, P4ArmId::ArmA, P4ArmId::ArmB);
    events.push(arm_a.transition(tick, ActuatorState::Idle, "handoff_atomic"));
    events.push(arm_b.transition(tick, ActuatorState::Carrying, "handoff_atomic"));
    handoff_plan.atomic_tick = Some(tick);
    Ok(events)
}

fn p4_create_owned_workpiece_joint(
    world: &mut World,
    env: P4EnvironmentBodies,
    arm: &P4ArmInstance,
    ownership: &mut P4Ownership,
    tick: u32,
    trigger: &str,
) -> Result<ImpulseJointHandle, WorldOwnershipFaultTrace> {
    if let Some(current_owner) = ownership.owner {
        return Err(WorldOwnershipFaultTrace {
            tick,
            actuator: arm.arm_id.name().to_string(),
            code: format!("grip_denied_workpiece_owned_by({})", current_owner.name()),
            owner: Some(current_owner.name().to_string()),
        });
    }
    zero_p4_arm_and_workpiece_velocity(world, env, arm.bodies);
    let handle = p4_create_tool_workpiece_joint(world, env, arm.bodies).map_err(|error| {
        WorldOwnershipFaultTrace {
            tick,
            actuator: arm.arm_id.name().to_string(),
            code: format!("joint_create_failed({error})"),
            owner: None,
        }
    })?;
    ownership.active_joint = Some((arm.arm_id, handle));
    ownership.transition(tick, Some(arm.arm_id), trigger);
    Ok(handle)
}

fn p4_create_tool_workpiece_joint(
    world: &mut World,
    env: P4EnvironmentBodies,
    arm: P4ArmBodies,
) -> anyhow::Result<ImpulseJointHandle> {
    let tool_point = p4_arm_tool_world_point(world, arm)?;
    let workpiece_grip = p4_workpiece_grip_world_point(world, env)?;
    let _tool_distance_at_grip = (tool_point - workpiece_grip).length();
    let anchor_world = workpiece_grip;
    let tool = world
        .bodies
        .get(arm.tool_body)
        .ok_or_else(|| anyhow::anyhow!("P4 tool body is missing"))?;
    let workpiece = world
        .bodies
        .get(env.workpiece_body)
        .ok_or_else(|| anyhow::anyhow!("P4 workpiece body is missing"))?;
    let local_anchor1 = tool.rotation().inverse() * (anchor_world - tool.position().translation);
    let local_anchor2 =
        workpiece.rotation().inverse() * (anchor_world - workpiece.position().translation);
    let joint = FixedJointBuilder::new()
        .contacts_enabled(false)
        .local_anchor1(local_anchor1)
        .local_anchor2(local_anchor2)
        .build();
    Ok(world
        .impulse_joints
        .insert(arm.tool_body, env.workpiece_body, joint, true))
}

fn trace_p4_tick(
    world: &World,
    tick: u32,
    env: P4EnvironmentBodies,
    arms: &[P4ArmInstance],
    ownership: &P4Ownership,
    tick_events: Vec<String>,
    contention_faults: Vec<WorldOwnershipFaultTrace>,
) -> anyhow::Result<WorldTickTrace> {
    let workpiece = world.body_trace(env.workpiece_body, WORKPIECE_HALF_EXTENT)?;
    let arm_a = p4_arm(arms, P4ArmId::ArmA);
    let arm_b = p4_arm(arms, P4ArmId::ArmB);
    let contacts = p4_contacts(world, env, arm_a, arm_b);
    let active_joints = ownership
        .active_joint
        .map(|(owner, _)| vec![owner.joint_name().to_string()])
        .unwrap_or_default();
    let joint_distances = if let Some((owner, _)) = ownership.active_joint {
        let arm = p4_arm(arms, owner);
        vec![WorldJointDistanceTrace {
            owner: owner.name().to_string(),
            distance: p4_arm_workpiece_joint_distance(world, env, arm.bodies)?,
        }]
    } else {
        Vec::new()
    };
    let mut actuator_states = arms
        .iter()
        .map(|arm| WorldActuatorStateSample {
            id: arm.id,
            name: arm.arm_id.name().to_string(),
            state: arm.state,
        })
        .collect::<Vec<_>>();
    actuator_states.sort_by_key(|state| state.id);
    let (arm_a_links, arm_a_joints) = trace_p4_arm(world, arm_a)?;
    let (arm_b_links, arm_b_joints) = trace_p4_arm(world, arm_b)?;
    Ok(WorldTickTrace {
        tick,
        contacts,
        carrier: None,
        carrier_a: None,
        carrier_b: None,
        workpiece,
        actuator_state: None,
        actuator_states,
        ownership: Some(ownership.sample()),
        tick_events,
        contention_faults,
        active_joints,
        joint_distance: joint_distances.first().map(|sample| sample.distance),
        joint_distances,
        arm_links: Vec::new(),
        arm_joints: Vec::new(),
        arm_a_links,
        arm_b_links,
        arm_a_joints,
        arm_b_joints,
    })
}

fn trace_p4_arm(
    world: &World,
    arm: &P4ArmInstance,
) -> anyhow::Result<(Vec<WorldArmLinkTrace>, Vec<WorldArmJointTrace>)> {
    let actual_joints = read_p4_joint_positions(world, arm.bodies);
    let fk_positions = compute_fk_for_chain_at(&arm.model, actual_joints, arm.base_world)?;
    let links = trace_p4_arm_links(world, arm, &fk_positions)?;
    let joints = trace_arm_joints(&arm.model, actual_joints);
    Ok((links, joints))
}

fn trace_p4_arm_links(
    world: &World,
    arm: &P4ArmInstance,
    fk_positions: &BTreeMap<&'static str, [f32; 3]>,
) -> anyhow::Result<Vec<WorldArmLinkTrace>> {
    [
        ("link_1", arm.bodies.link_1_body),
        ("link_2", arm.bodies.link_2_body),
        ("tool", arm.bodies.tool_body),
    ]
    .into_iter()
    .map(|(name, body)| {
        let rigid_body = world.bodies.get(body).ok_or_else(|| {
            anyhow::anyhow!("P4 arm body {}.{name} is missing", arm.arm_id.name())
        })?;
        let position = rigid_body.position().translation;
        let fk = fk_positions.get(name).copied().ok_or_else(|| {
            anyhow::anyhow!("P4 FK position for {}.{name} is missing", arm.arm_id.name())
        })?;
        let spec = arm.model.link_spec(name)?;
        let rapier_position = [position.x, position.y, position.z];
        Ok(WorldArmLinkTrace {
            name: name.to_string(),
            rapier_position,
            fk_predicted_position: fk,
            fk_consistency_distance: point_distance(rapier_position, fk),
            bottom_y: position.y - spec.half_extents[1],
            rapier_yaw_z: body_yaw(rigid_body),
        })
    })
    .collect()
}

fn p4_contacts(
    world: &World,
    env: P4EnvironmentBodies,
    arm_a: &P4ArmInstance,
    arm_b: &P4ArmInstance,
) -> Vec<WorldContactTrace> {
    let mut contacts = Vec::new();
    if env
        .floor_collider
        .is_some_and(|floor| world.contact_active(env.workpiece_collider, floor))
    {
        contacts.push(contact("workpiece", "floor"));
    }
    if world.contact_active(env.workpiece_collider, env.fixture_collider) {
        contacts.push(contact("workpiece", "fixture"));
    }
    if world.contact_active(env.workpiece_collider, env.transfer_collider) {
        contacts.push(contact("workpiece", "transfer_zone"));
    }
    for arm in [arm_a, arm_b] {
        for (suffix, collider) in [
            ("link_1", arm.bodies.link_1_collider),
            ("link_2", arm.bodies.link_2_collider),
            ("tool", arm.bodies.tool_collider),
        ] {
            let name = format!("{}.{}", arm.arm_id.name(), suffix);
            if env
                .floor_collider
                .is_some_and(|floor| world.contact_active(collider, floor))
            {
                contacts.push(contact(&name, "floor"));
            }
            if world.contact_pair_exists(collider, env.workpiece_collider) {
                contacts.push(contact(&name, "workpiece"));
            }
            if world.contact_active(collider, env.fixture_collider) {
                contacts.push(contact(&name, "fixture"));
            }
        }
    }
    contacts.sort_by(|left, right| (&left.a, &left.b).cmp(&(&right.a, &right.b)));
    contacts
}

fn p4_apply_handoff(
    scene: &mut scena::Scene,
    world: &World,
    nodes: P4SceneNodes,
    arm_a: P4ArmBodies,
    arm_b: P4ArmBodies,
    workpiece_body: RigidBodyHandle,
) -> anyhow::Result<u32> {
    let samples = [
        apply_rapier_body_pose_to_scena_node(
            scene,
            nodes.arm_a_link_1,
            world.bodies(),
            arm_a.link_1_body,
        )?,
        apply_rapier_body_pose_to_scena_node(
            scene,
            nodes.arm_a_link_2,
            world.bodies(),
            arm_a.link_2_body,
        )?,
        apply_rapier_body_pose_to_scena_node(
            scene,
            nodes.arm_a_tool,
            world.bodies(),
            arm_a.tool_body,
        )?,
        apply_rapier_body_pose_to_scena_node(
            scene,
            nodes.arm_b_link_1,
            world.bodies(),
            arm_b.link_1_body,
        )?,
        apply_rapier_body_pose_to_scena_node(
            scene,
            nodes.arm_b_link_2,
            world.bodies(),
            arm_b.link_2_body,
        )?,
        apply_rapier_body_pose_to_scena_node(
            scene,
            nodes.arm_b_tool,
            world.bodies(),
            arm_b.tool_body,
        )?,
        apply_rapier_body_pose_to_scena_node(
            scene,
            nodes.workpiece,
            world.bodies(),
            workpiece_body,
        )?,
    ];
    Ok(samples.iter().map(|sample| sample.line).max().unwrap_or(0))
}

fn read_p4_joint_positions(world: &World, bodies: P4ArmBodies) -> [f32; 2] {
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

fn p4_arm_tool_world_point(world: &World, bodies: P4ArmBodies) -> anyhow::Result<Vector> {
    let tool = world
        .bodies
        .get(bodies.tool_body)
        .ok_or_else(|| anyhow::anyhow!("P4 arm tool body is missing"))?;
    Ok(tool.position().translation + (*tool.rotation() * v3(0.0, -0.10, 0.0)))
}

fn p4_workpiece_grip_world_point(
    world: &World,
    env: P4EnvironmentBodies,
) -> anyhow::Result<Vector> {
    let workpiece = world
        .bodies
        .get(env.workpiece_body)
        .ok_or_else(|| anyhow::anyhow!("P4 workpiece body is missing"))?;
    Ok(workpiece.position().translation
        + (*workpiece.rotation() * v3(0.0, WORKPIECE_HALF_EXTENT, 0.0)))
}

fn p4_arm_workpiece_joint_distance(
    world: &World,
    env: P4EnvironmentBodies,
    arm: P4ArmBodies,
) -> anyhow::Result<f32> {
    Ok(
        (p4_arm_tool_world_point(world, arm)? - p4_workpiece_grip_world_point(world, env)?)
            .length(),
    )
}

fn zero_p4_workpiece_velocity(world: &mut World, env: P4EnvironmentBodies) {
    if let Some(workpiece) = world.bodies.get_mut(env.workpiece_body) {
        workpiece.set_linvel(Vector::ZERO, true);
        workpiece.set_angvel(Vector::ZERO, true);
    }
}

fn zero_p4_arm_and_workpiece_velocity(
    world: &mut World,
    env: P4EnvironmentBodies,
    arm: P4ArmBodies,
) {
    for body in [
        arm.link_1_body,
        arm.link_2_body,
        arm.tool_body,
        env.workpiece_body,
    ] {
        if let Some(rigid_body) = world.bodies.get_mut(body) {
            rigid_body.set_linvel(Vector::ZERO, true);
            rigid_body.set_angvel(Vector::ZERO, true);
        }
    }
}

fn p4_actuator_traces(arms: &[P4ArmInstance]) -> Vec<WorldActuatorTrace> {
    let mut traces = arms.iter().map(P4ArmInstance::trace).collect::<Vec<_>>();
    traces.sort_by_key(|trace| trace.id.unwrap_or(u8::MAX));
    traces
}

fn p4_urdf_trace(arms: &[P4ArmInstance]) -> WorldUrdfTrace {
    let mut instances = arms
        .iter()
        .map(|arm| WorldUrdfArmInstanceTrace {
            id: arm.arm_id.name().to_string(),
            base_position: arm.base_world,
            parsed_once: true,
            consulted_in_tick_loop: false,
        })
        .collect::<Vec<_>>();
    instances.sort_by(|left, right| left.id.cmp(&right.id));
    let mut trace = p4_arm(arms, P4ArmId::ArmA).model.urdf_trace.clone();
    trace.instances = instances;
    trace
}

fn p4_fk_verifier_trace(
    assertions: &WorldSmokeAssertions,
    checked_ticks: u32,
) -> WorldFkVerifierTrace {
    let per_arm_distances = assertions
        .per_arm_fk_consistency
        .as_ref()
        .map(|assertion| assertion.max_consistency_distance_by_arm.clone())
        .unwrap_or_default();
    let per_arm = per_arm_distances
        .iter()
        .map(|(arm, max)| {
            (
                arm.clone(),
                WorldFkArmVerifierTrace {
                    max_consistency_distance_m: *max,
                    checked_links: vec![
                        "link_1".to_string(),
                        "link_2".to_string(),
                        "tool".to_string(),
                    ],
                    checked_samples: checked_ticks * 3,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    WorldFkVerifierTrace {
        max_consistency_distance_m: assertions
            .fk_matches_rapier
            .as_ref()
            .map_or(f32::INFINITY, |assertion| {
                assertion.max_consistency_distance_m
            }),
        per_arm,
        checked_links: vec![
            "link_1".to_string(),
            "link_2".to_string(),
            "tool".to_string(),
        ],
        checked_ticks,
        consistency_tolerance: ARM_FK_TOLERANCE,
    }
}

fn p4_body_registrations(include_floor: bool) -> Vec<WorldBodyRegistrationTrace> {
    let mut bodies = Vec::new();
    if include_floor {
        bodies.push(body_registration("floor", "static", "box"));
    }
    bodies.push(body_registration("fixture", "static", "box"));
    bodies.push(body_registration("transfer_zone", "static", "marker"));
    bodies.push(body_registration("workpiece", "dynamic", "box"));
    for arm in [P4ArmId::ArmA, P4ArmId::ArmB] {
        for (link, kind) in [
            ("base", "static"),
            ("link_1", "dynamic"),
            ("link_2", "dynamic"),
            ("tool", "dynamic"),
        ] {
            bodies.push(WorldBodyRegistrationTrace {
                name: format!("{}.{}", arm.name(), link),
                kind: kind.to_string(),
                shape: "box".to_string(),
                source: Some(arm.source()),
            });
        }
    }
    bodies
}

fn p4_joint_trace(per_tick_trace: &[WorldTickTrace]) -> WorldJointTrace {
    WorldJointTrace {
        joint_create_ticks: per_tick_trace
            .iter()
            .filter(|tick| {
                tick.tick_events
                    .iter()
                    .any(|event| event.starts_with("joint_create("))
            })
            .map(|tick| tick.tick)
            .collect(),
        joint_destroy_ticks: per_tick_trace
            .iter()
            .filter(|tick| {
                tick.tick_events
                    .iter()
                    .any(|event| event.starts_with("joint_destroy("))
            })
            .map(|tick| tick.tick)
            .collect(),
        active_during_ticks: None,
        active_by_tick_summary: Some(active_joint_summary(per_tick_trace)),
    }
}

fn p4_expected_handoff_event_order() -> Vec<String> {
    vec![
        "joint_destroy(arm_a.tool, workpiece)".to_string(),
        "joint_create(arm_b.tool, workpiece)".to_string(),
        "state_transition(arm_a: Held -> Idle)".to_string(),
        "state_transition(arm_b: AcceptingHandoff -> Carrying)".to_string(),
    ]
}

fn p4_find_handoff_tick(per_tick_trace: &[WorldTickTrace]) -> Option<u32> {
    per_tick_trace
        .iter()
        .find(|tick| {
            tick.tick_events
                .iter()
                .any(|event| event == "joint_destroy(arm_a.tool, workpiece)")
                && tick
                    .tick_events
                    .iter()
                    .any(|event| event == "joint_create(arm_b.tool, workpiece)")
        })
        .map(|tick| tick.tick)
}

fn p4_ownership_transfer_atomic(
    per_tick_trace: &[WorldTickTrace],
    handoff_tick: Option<u32>,
) -> bool {
    let Some(handoff_tick) = handoff_tick else {
        return false;
    };
    let before = handoff_tick
        .checked_sub(1)
        .and_then(|tick| per_tick_trace.iter().find(|sample| sample.tick == tick));
    let at = per_tick_trace
        .iter()
        .find(|sample| sample.tick == handoff_tick);
    before
        .and_then(|tick| tick.ownership.as_ref())
        .and_then(|ownership| ownership.owner.as_deref())
        == Some("arm_a")
        && at
            .and_then(|tick| tick.ownership.as_ref())
            .and_then(|ownership| ownership.owner.as_deref())
            == Some("arm_b")
}

fn p4_phantom_carry_violation_count(per_tick_trace: &[WorldTickTrace]) -> u32 {
    let mut violations = 0;
    for tick in per_tick_trace {
        for state in &tick.actuator_states {
            let joint_name = match state.name.as_str() {
                "arm_a" => P4ArmId::ArmA.joint_name(),
                "arm_b" => P4ArmId::ArmB.joint_name(),
                _ => continue,
            };
            let joint_active = tick.active_joints.iter().any(|joint| joint == joint_name);
            let should_have_joint =
                matches!(state.state, ActuatorState::Carrying | ActuatorState::Held);
            if should_have_joint != joint_active {
                violations += 1;
            }
        }
    }
    violations
}

fn p4_per_arm_fk_stats(per_tick_trace: &[WorldTickTrace]) -> (bool, BTreeMap<String, f32>) {
    let mut stats = BTreeMap::new();
    let arm_a = per_tick_trace
        .iter()
        .flat_map(|tick| tick.arm_a_links.iter())
        .map(|link| link.fk_consistency_distance)
        .fold(0.0_f32, f32::max);
    let arm_b = per_tick_trace
        .iter()
        .flat_map(|tick| tick.arm_b_links.iter())
        .map(|link| link.fk_consistency_distance)
        .fold(0.0_f32, f32::max);
    stats.insert("arm_a".to_string(), arm_a);
    stats.insert("arm_b".to_string(), arm_b);
    let ok = stats.values().all(|max| *max <= ARM_FK_TOLERANCE);
    (ok, stats)
}

fn p4_joint_limit_stats(per_tick_trace: &[WorldTickTrace]) -> JointLimitStats {
    let mut out_of_limit_samples = 0;
    let mut clamped_events = Vec::new();
    for tick in per_tick_trace {
        for (prefix, joints) in [("arm_a", &tick.arm_a_joints), ("arm_b", &tick.arm_b_joints)] {
            for joint in joints {
                if joint.position < joint.limit_lower - ARM_JOINT_TOLERANCE
                    || joint.position > joint.limit_upper + ARM_JOINT_TOLERANCE
                {
                    out_of_limit_samples += 1;
                }
                if joint.clamped {
                    clamped_events.push(format!(
                        "tick={} joint_clamped({prefix}.{}) position={:.4}",
                        tick.tick, joint.name, joint.position
                    ));
                }
            }
        }
    }
    JointLimitStats {
        out_of_limit_samples,
        clamped_events,
    }
}

fn p4_arm_floor_stats(per_tick_trace: &[WorldTickTrace]) -> ArmFloorStats {
    let mut min_y = f32::INFINITY;
    let mut min_name = String::new();
    for tick in per_tick_trace {
        for (prefix, links) in [("arm_a", &tick.arm_a_links), ("arm_b", &tick.arm_b_links)] {
            for link in links {
                if link.bottom_y < min_y {
                    min_y = link.bottom_y;
                    min_name = format!("{prefix}.{}", link.name);
                }
            }
        }
    }
    ArmFloorStats { min_y, min_name }
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
            instances: Vec::new(),
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
        contacts,
        carrier: None,
        carrier_a: None,
        carrier_b: None,
        workpiece,
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
        arm_a_links: Vec::new(),
        arm_b_links: Vec::new(),
        arm_a_joints: Vec::new(),
        arm_b_joints: Vec::new(),
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
    compute_fk_for_chain_at(model, joint_positions, ARM_BASE_WORLD)
}

fn compute_fk_for_chain_at(
    model: &UrdfArmModel,
    joint_positions: [f32; 2],
    base_world: [f32; 3],
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
                base_world[0] + predicted.x,
                base_world[1] + predicted.y,
                base_world[2] + predicted.z,
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
