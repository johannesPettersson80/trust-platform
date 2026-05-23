//! Shared deterministic simulation world primitives.

pub mod arm;

pub use arm::{
    assert_world_multi_urdf_arm_smoke_trace, assert_world_urdf_arm_smoke_trace,
    record_multi_urdf_arm_determinism_hash_stability, record_urdf_arm_determinism_hash_stability,
    run_world_multi_urdf_arm_smoke, run_world_urdf_arm_smoke, step_robot_p3_minimal_arm_bridge,
    ArmAboveFloorAssertion, ArmRenderedThroughHandoffAssertion, FkConsistencyAssertion,
    JointLimitAssertion, MultiUrdfArmsLoadedAssertion, PerArmFkConsistencyAssertion,
    RobotP3MinimalArmBridgeInput, RobotP3MinimalArmBridgeOutput, UrdfParsedOnceAssertion,
    WorldArmJointTrace, WorldArmLinkTrace, WorldFkArmVerifierTrace, WorldFkVerifierTrace,
    WorldMultiUrdfArmScenario, WorldMultiUrdfArmSmokeConfig, WorldUrdfArmInstanceTrace,
    WorldUrdfArmScenario, WorldUrdfArmSmokeConfig, WorldUrdfJointTrace, WorldUrdfTrace,
};

use rapier3d::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const FLOOR_Y: f32 = 0.0;
const CUBE_HALF_EXTENT: f32 = 0.5;
const CUBE_INITIAL_BOTTOM_Y: f32 = 2.0;
const DEFAULT_TICK_DT_SECONDS: f32 = 0.002;
const DEFAULT_TICK_COUNT: u32 = 2500;
const CONTACT_SETTLE_EPSILON: f32 = 0.02;
const ABOVE_FLOOR_EPSILON: f32 = 0.001;
const HANDOFF_FUNCTION: &str = "trust_runtime::world::apply_rapier_body_pose_to_scena_node";
const HANDOFF_FILE: &str = "crates/trust-runtime/src/world.rs";
const WORKPIECE_HALF_EXTENT: f32 = 0.25;
const CARRIER_HALF_XZ: f32 = 0.45;
const CARRIER_HALF_Y: f32 = 0.15;
const FLOOR_HALF_Y: f32 = 0.05;
const FIXTURE_CENTER_X: f32 = 2.0;
const FIXTURE_CENTER_Y: f32 = 0.30;
const FIXTURE_CENTER_Z: f32 = 0.0;
const FIXTURE_HALF_XZ: f32 = 0.75;
const FIXTURE_HALF_Y: f32 = 0.25;
const FIXTURE_TOP_Y: f32 = FIXTURE_CENTER_Y + FIXTURE_HALF_Y;
const WORKPIECE_INITIAL_CENTER_Y: f32 = FLOOR_Y + FLOOR_HALF_Y + WORKPIECE_HALF_EXTENT;
const CARRIER_INITIAL_Y: f32 = 1.40;
const APPROACH_TARGET_Y: f32 =
    WORKPIECE_INITIAL_CENTER_Y + WORKPIECE_HALF_EXTENT + CARRIER_HALF_Y - 0.02;
const CARRY_TARGET_Y: f32 = FIXTURE_TOP_Y + (WORKPIECE_HALF_EXTENT * 2.0) + CARRIER_HALF_Y + 0.10;
const RETRACT_TARGET_Y: f32 = 1.80;
const CARRIER_MAX_SPEED: f32 = 1.45;
const CARRIER_HOLD_TOLERANCE: f32 = 0.02;
const RELEASE_TOLERANCE: f32 = 0.05;
const JOINT_DISTANCE_TOLERANCE: f32 = 0.02;
const FIXTURE_INTERPENETRATION_TOLERANCE: f32 = 0.02;
const SETTLE_VELOCITY_TOLERANCE: f32 = 0.03;
const SETTLE_POSITION_TOLERANCE: f32 = 0.03;
const TRANSFER_CENTER_X: f32 = 1.0;
const TRANSFER_CENTER_Y: f32 = FLOOR_Y + FLOOR_HALF_Y + 0.02;
const TRANSFER_CENTER_Z: f32 = 0.0;
const TRANSFER_HALF_XZ: f32 = 0.25;
const TRANSFER_HALF_Y: f32 = 0.02;
const CARRIER_B_SIDE_Z: f32 = WORKPIECE_HALF_EXTENT + CARRIER_HALF_XZ - 0.03;
const CARRIER_A_RETRACT_Z: f32 = -0.85;
const CARRIER_B_RETRACT_Z: f32 = CARRIER_B_SIDE_Z + 0.65;
const CARRIER_B_HANDOFF_Y: f32 = CARRY_TARGET_Y - CARRIER_HALF_Y - WORKPIECE_HALF_EXTENT;
const CARRIER_B_FIXTURE_TARGET_Y: f32 = FIXTURE_TOP_Y + WORKPIECE_HALF_EXTENT;

/// Generic deterministic simulation world.
///
/// The abstraction is deliberately named `World`: rigid bodies are only the
/// first solver-backed state domain. Later solvers register into this same
/// world instead of creating parallel robot/process/HMI worlds.
pub struct World {
    gravity: Vector,
    integration_parameters: IntegrationParameters,
    pipeline: PhysicsPipeline,
    islands: IslandManager,
    broad_phase: DefaultBroadPhase,
    narrow_phase: NarrowPhase,
    bodies: RigidBodySet,
    colliders: ColliderSet,
    impulse_joints: ImpulseJointSet,
    multibody_joints: MultibodyJointSet,
    ccd_solver: CCDSolver,
    solvers_registered: Vec<&'static str>,
}

/// Handles for the smoke proof bodies registered in [`World`].
#[derive(Debug, Clone, Copy)]
pub struct WorldSmokeBodies {
    cube_body: RigidBodyHandle,
    cube_collider: ColliderHandle,
    floor_collider: Option<ColliderHandle>,
}

/// Configuration for the cube/floor physics smoke proof.
#[derive(Debug, Clone, Copy)]
pub struct WorldSmokeConfig {
    /// Fixed tick delta in seconds.
    pub tick_dt_seconds: f32,
    /// Number of fixed ticks to run.
    pub tick_count: u32,
    /// Whether to register the static floor collider.
    pub include_floor: bool,
}

impl Default for WorldSmokeConfig {
    fn default() -> Self {
        Self {
            tick_dt_seconds: DEFAULT_TICK_DT_SECONDS,
            tick_count: DEFAULT_TICK_COUNT,
            include_floor: true,
        }
    }
}

/// Machine-readable proof trace for the shared-world smoke test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSmokeTrace {
    /// World abstraction metadata.
    pub world_abstraction: WorldAbstractionTrace,
    /// The single allowed transform handoff claim.
    pub transform_handoff: TransformHandoffTrace,
    /// Renderer origin reported by the production Scena WASM renderer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub renderer_origin: Option<String>,
    /// Path to the initial-state screenshot. Required for every smoke proof.
    /// P0: cube hovering above floor. P1: carrier above workpiece, fixture empty.
    pub screenshot_initial_png: String,
    /// Path to the grip-event screenshot. P1 only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot_grip_png: Option<String>,
    /// Path to the mid-carry screenshot. P1 only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot_carry_png: Option<String>,
    /// Path to the transfer-zone screenshot. P2 only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot_transfer_png: Option<String>,
    /// Path to the post-atomic-handoff screenshot. P2 only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot_handoff_png: Option<String>,
    /// Path to the settled-final-state screenshot. Required for every smoke proof.
    /// P0: cube on floor. P1: workpiece on fixture after release.
    pub screenshot_final_png: String,
    /// P1 actuator trace, present only for the workpiece/fixture proof.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actuator: Option<WorldActuatorTrace>,
    /// P2 actuator traces, present only for the multi-actuator proof.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actuators: Option<Vec<WorldActuatorTrace>>,
    /// P2 ownership trace, present only for the multi-actuator proof.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ownership: Option<WorldOwnershipTrace>,
    /// P2 handoff plan trace, present only for the multi-actuator proof.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handoff_plan: Option<WorldHandoffPlanTrace>,
    /// P3 URDF load trace, present only for the URDF-arm proof.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub urdf: Option<WorldUrdfTrace>,
    /// P3 FK verifier trace, present only for the URDF-arm proof.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fk_verifier: Option<WorldFkVerifierTrace>,
    /// P1 joint lifecycle trace, present only for the workpiece/fixture proof.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub joints: Option<WorldJointTrace>,
    /// Per-tick sampled world state.
    pub per_tick_trace: Vec<WorldTickTrace>,
    /// SHA256 of `per_tick_trace`.
    pub determinism_trace_hash: String,
    /// Positive assertion results.
    pub assertions: WorldSmokeAssertions,
}

/// Metadata proving the smoke proof uses the generic world abstraction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldAbstractionTrace {
    /// Concrete type name.
    pub type_name: String,
    /// Rust module path for the world type.
    pub module_path: String,
    /// Solvers registered for this run.
    pub solvers_registered: Vec<String>,
    /// Whether the run uses deterministic fixed stepping.
    pub deterministic: bool,
    /// Fixed tick delta in seconds.
    pub tick_dt_seconds: f32,
    /// Number of fixed ticks.
    pub tick_count: u32,
    /// Registered body metadata for richer world proofs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bodies_registered: Vec<WorldBodyRegistrationTrace>,
}

/// Registered body metadata recorded in the proof artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldBodyRegistrationTrace {
    /// Stable body name.
    pub name: String,
    /// Static or dynamic.
    pub kind: String,
    /// Shape name.
    pub shape: String,
    /// Source descriptor, present for URDF-derived bodies.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// Metadata naming the single allowed dynamic-body transform handoff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformHandoffTrace {
    /// Fully qualified function name.
    pub function: String,
    /// Source file containing the handoff function.
    pub file: String,
    /// Source line recorded by the handoff function.
    pub line: u32,
    /// Runtime API read by the handoff function.
    pub read_source: String,
    /// Scene target written by the handoff function.
    pub write_target: String,
}

/// Per-tick world-state sample.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldTickTrace {
    /// Tick index.
    pub tick: u32,
    /// Active contact pairs by stable logical name.
    pub contacts: Vec<WorldContactTrace>,
    /// Carrier body sample, present in the P1 proof.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub carrier: Option<WorldBodyKinematicsTrace>,
    /// Carrier A body sample, present in the P2 proof.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub carrier_a: Option<WorldBodyKinematicsTrace>,
    /// Carrier B body sample, present in the P2 proof.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub carrier_b: Option<WorldBodyKinematicsTrace>,
    /// Workpiece body sample. In P0, the cube is the workpiece.
    pub workpiece: WorldBodyKinematicsTrace,
    /// Actuator state, present in the P1 proof.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actuator_state: Option<ActuatorState>,
    /// Per-actuator states, present in the P2 proof.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actuator_states: Vec<WorldActuatorStateSample>,
    /// Per-workpiece ownership state, present in the P2 proof.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ownership: Option<WorldOwnershipSample>,
    /// Ordered events observed at this tick.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tick_events: Vec<String>,
    /// Ownership/contention faults observed at this tick.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contention_faults: Vec<WorldOwnershipFaultTrace>,
    /// Active physics joints by stable logical name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub active_joints: Vec<String>,
    /// Distance between carrier tool point and workpiece grip frame.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub joint_distance: Option<f32>,
    /// Per-owner joint distance samples, present in the P2 proof.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub joint_distances: Vec<WorldJointDistanceTrace>,
    /// P3 URDF arm link samples.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arm_links: Vec<WorldArmLinkTrace>,
    /// P3 URDF arm joint samples.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arm_joints: Vec<WorldArmJointTrace>,
    /// P4 URDF arm A link samples.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arm_a_links: Vec<WorldArmLinkTrace>,
    /// P4 URDF arm B link samples.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arm_b_links: Vec<WorldArmLinkTrace>,
    /// P4 URDF arm A joint samples.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arm_a_joints: Vec<WorldArmJointTrace>,
    /// P4 URDF arm B joint samples.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arm_b_joints: Vec<WorldArmJointTrace>,
}

/// Per-body kinematic sample.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldBodyKinematicsTrace {
    /// Body bottom Y for above-floor checks.
    pub y: f32,
    /// Body center position.
    pub center: [f32; 3],
    /// Body vertical velocity.
    pub vy: f32,
    /// Body linear velocity.
    pub velocity: [f32; 3],
}

/// Per-actuator state sample for multi-actuator traces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldActuatorStateSample {
    /// Numeric actuator id used for deterministic ordering.
    pub id: u8,
    /// Stable actuator name.
    pub name: String,
    /// Current state.
    pub state: ActuatorState,
}

/// Per-workpiece ownership sample.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldOwnershipSample {
    /// Stable workpiece name.
    pub workpiece: String,
    /// Current owner, or `None` when free.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
}

/// Runtime ownership fault emitted by the P2 proof.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldOwnershipFaultTrace {
    /// Tick where the fault was observed.
    pub tick: u32,
    /// Actuator whose request was denied.
    pub actuator: String,
    /// Machine-readable fault code.
    pub code: String,
    /// Current owner that blocked the request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
}

/// Per-owner fixed-joint distance sample.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldJointDistanceTrace {
    /// Actuator owning the sampled joint.
    pub owner: String,
    /// Distance between actuator tool point and workpiece grip frame.
    pub distance: f32,
}

/// Stable logical contact pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldContactTrace {
    /// First body id.
    pub a: String,
    /// Second body id.
    pub b: String,
}

/// Assertion results for the smoke proof.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSmokeAssertions {
    /// Workpiece never penetrates below the floor.
    pub workpiece_above_floor: BodyAboveFloorAssertion,
    /// Gravity accelerated the cube before contact.
    pub gravity_applied: GravityAppliedAssertion,
    /// A cube/floor contact pair was produced.
    pub contact_fired: ContactFiredAssertion,
    /// Carrier never moves below the floor in the P1 proof.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub carrier_above_floor: Option<BodyAboveFloorAssertion>,
    /// Dynamic bodies do not interpenetrate the fixture beyond tolerance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_fixture_interpenetration: Option<FixtureInterpenetrationAssertion>,
    /// Grip-on transition is driven by a Rapier contact pair.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grip_event_has_contact: Option<GripEventContactAssertion>,
    /// Carry phase is constrained by a physics joint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub carry_constraint_driven: Option<CarryConstraintAssertion>,
    /// Release destroys the workpiece/carrier joint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_destroyed_joint: Option<ReleaseDestroyedJointAssertion>,
    /// Workpiece settles on the fixture after release.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workpiece_settled_on_fixture: Option<WorkpieceSettledAssertion>,
    /// P2: the workpiece is owned by at most one actuator at every tick.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclusive_ownership: Option<ExclusiveOwnershipAssertion>,
    /// P2: ownership transfer has no undefined tick and records destroy/create.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ownership_transfer_atomic: Option<OwnershipTransferAtomicAssertion>,
    /// P2: handoff event order matches the deterministic contract.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handoff_order_deterministic: Option<HandoffOrderAssertion>,
    /// P2: Carrying/Held actuator states agree with active joints.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_phantom_carry: Option<NoPhantomCarryAssertion>,
    /// P2: repeated runs produce the same deterministic trace hash.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub determinism_hash_stable: Option<DeterminismHashStableAssertion>,
    /// P3: URDF was parsed once and not consulted in the tick loop.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub urdf_parsed_once: Option<UrdfParsedOnceAssertion>,
    /// P3: every dynamic arm link was rendered through the audited handoff.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arm_rendered_through_handoff: Option<ArmRenderedThroughHandoffAssertion>,
    /// P3: FK predictions match Rapier-owned link positions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fk_matches_rapier: Option<FkConsistencyAssertion>,
    /// P3: URDF joint limits are respected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub joint_limits_enforced: Option<JointLimitAssertion>,
    /// P3: every arm link stays above the floor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arm_links_above_floor: Option<ArmAboveFloorAssertion>,
    /// P4: both URDF arm instances were loaded once at setup.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multi_urdf_arms_loaded: Option<MultiUrdfArmsLoadedAssertion>,
    /// P4: per-arm FK consistency stayed within tolerance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_arm_fk_consistency: Option<PerArmFkConsistencyAssertion>,
}

/// Fixture interpenetration assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureInterpenetrationAssertion {
    /// Whether the assertion passed.
    pub ok: bool,
    /// Maximum dynamic-body penetration into the fixture AABB.
    pub max_penetration: f32,
    /// Allowed tolerance.
    pub tolerance: f32,
}

/// Grip transition/contact assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GripEventContactAssertion {
    /// Whether the assertion passed.
    pub ok: bool,
    /// Tick where grip-on occurred.
    pub grip_tick: Option<u32>,
    /// Whether carrier/workpiece contact was present at the grip tick.
    pub contact_present: bool,
}

/// Constraint-driven carry assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CarryConstraintAssertion {
    /// Whether the assertion passed.
    pub ok: bool,
    /// Maximum carrier-tool/workpiece-grip distance while joint was active.
    pub max_joint_distance: f32,
    /// Allowed tolerance.
    pub tolerance: f32,
    /// Number of ticks checked.
    pub checked_ticks: u32,
}

/// Joint destruction assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseDestroyedJointAssertion {
    /// Whether the assertion passed.
    pub ok: bool,
    /// Tick where release occurred.
    pub release_tick: Option<u32>,
    /// Whether a joint was still active at release.
    pub active_joint_at_release: bool,
    /// Whether a joint appeared after release.
    pub active_joint_after_release: bool,
}

/// Workpiece-on-fixture settle assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkpieceSettledAssertion {
    /// Whether the assertion passed.
    pub ok: bool,
    /// Final workpiece bottom Y.
    pub final_workpiece_y: f32,
    /// Fixture top Y.
    pub fixture_top_y: f32,
    /// Final workpiece speed.
    pub final_speed: f32,
    /// Whether workpiece/fixture contact exists at the final tick.
    pub contact_present: bool,
}

/// P2 exclusive-ownership assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExclusiveOwnershipAssertion {
    /// Whether the assertion passed.
    pub ok: bool,
    /// Number of ticks with no active workpiece joint.
    pub ticks_with_zero_joints: u32,
    /// Number of ticks with exactly one active workpiece joint.
    pub ticks_with_one_joint: u32,
    /// Number of ticks with more than one active workpiece joint.
    pub ticks_with_two_joints: u32,
}

/// P2 atomic-ownership-transfer assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnershipTransferAtomicAssertion {
    /// Whether the assertion passed.
    pub ok: bool,
    /// Tick where handoff occurred.
    pub handoff_tick: Option<u32>,
    /// Whether destroy and create events both appeared at the handoff tick.
    pub destroy_and_create_same_tick: bool,
    /// Whether ownership is defined immediately before and after handoff.
    pub no_undefined_transfer_tick: bool,
}

/// P2 handoff-order assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffOrderAssertion {
    /// Whether the assertion passed.
    pub ok: bool,
    /// Tick where handoff occurred.
    pub handoff_tick: Option<u32>,
    /// Expected event order.
    pub expected_order: Vec<String>,
    /// Observed event order.
    pub observed_order: Vec<String>,
}

/// P2 phantom-carry assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoPhantomCarryAssertion {
    /// Whether the assertion passed.
    pub ok: bool,
    /// Number of state/joint mismatches.
    pub violation_count: u32,
}

/// P2 deterministic-rerun assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeterminismHashStableAssertion {
    /// Whether the assertion passed.
    pub ok: bool,
    /// Hash from the artifact-producing run.
    pub canonical_hash: String,
    /// Hash from the repeated run.
    pub repeat_hash: String,
}

/// Typed actuator state for P1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActuatorState {
    /// Not carrying a workpiece.
    Idle,
    /// Moving toward the workpiece.
    Approaching,
    /// Carrier/workpiece fixed joint is expected to be active.
    Carrying,
    /// Carrier owns the workpiece and is waiting at the transfer point.
    Held,
    /// Actuator is approaching a held workpiece as a handoff receiver.
    AcceptingHandoff,
    /// Joint has been destroyed and the workpiece is settling.
    Releasing,
}

/// P1 actuator trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldActuatorTrace {
    /// Numeric actuator id, present in P2 traces.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u8>,
    /// Stable actuator name, present in P2 traces.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Concrete actuator type name.
    pub type_name: String,
    /// State sequence observed in this run.
    pub states_observed: Vec<ActuatorState>,
    /// State transitions observed in this run.
    pub state_transitions: Vec<WorldActuatorTransitionTrace>,
}

/// P1 actuator state-transition trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldActuatorTransitionTrace {
    /// Tick where the transition occurred.
    pub tick: u32,
    /// Previous state.
    pub from: Option<ActuatorState>,
    /// Next state.
    pub to: ActuatorState,
    /// Trigger recorded by the state machine.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger: Option<String>,
}

/// P1 joint lifecycle trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldJointTrace {
    /// Ticks where the fixed joint was created.
    pub joint_create_ticks: Vec<u32>,
    /// Ticks where the fixed joint was destroyed.
    pub joint_destroy_ticks: Vec<u32>,
    /// Inclusive active tick range.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_during_ticks: Option<[u32; 2]>,
    /// P2 active-joint count summary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_by_tick_summary: Option<WorldJointActiveSummary>,
}

/// P2 active-joint summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldJointActiveSummary {
    /// Ticks with no workpiece joint.
    pub ticks_with_zero_joints: u32,
    /// Ticks with one workpiece joint.
    pub ticks_with_one_joint: u32,
    /// Ticks with two or more workpiece joints.
    pub ticks_with_two_joints: u32,
}

/// P2 ownership transition trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldOwnershipTrace {
    /// Ownership transitions observed in this run.
    pub transitions: Vec<WorldOwnershipTransitionTrace>,
}

/// One P2 ownership transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldOwnershipTransitionTrace {
    /// Tick where ownership changed.
    pub tick: u32,
    /// Workpiece name.
    pub workpiece: String,
    /// Previous owner.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// New owner.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// Transition trigger.
    pub trigger: String,
}

/// P2 handoff plan trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldHandoffPlanTrace {
    /// Registered handoff pairs.
    pub registered_pairs: Vec<WorldHandoffPairTrace>,
    /// Atomic handoff tick.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub atomic_tick: Option<u32>,
    /// Ordered events required at the atomic handoff tick.
    pub atomic_event_order: Vec<String>,
}

/// P2 registered handoff pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldHandoffPairTrace {
    /// Offering actuator.
    pub offerer: String,
    /// Receiving actuator.
    pub receiver: String,
    /// Transfer landmark.
    pub transfer_zone: String,
}

/// Result for the above-floor invariant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodyAboveFloorAssertion {
    /// Whether the assertion passed.
    pub ok: bool,
    /// Minimum body bottom Y observed.
    pub min_y: f32,
    /// Floor top Y.
    pub floor_y: f32,
}

/// Result for the gravity proof.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GravityAppliedAssertion {
    /// Whether the assertion passed.
    pub ok: bool,
    /// Maximum downward velocity before contact.
    pub max_downward_velocity_before_contact: f32,
}

/// Result for the contact proof.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactFiredAssertion {
    /// Whether the assertion passed.
    pub ok: bool,
    /// First tick where the cube/floor contact was active.
    pub first_contact_tick: Option<u32>,
}

/// One dynamic-body transform sample emitted by the handoff function.
#[derive(Debug, Clone, Copy)]
pub struct TransformHandoffSample {
    /// Source line recorded inside the handoff function.
    pub line: u32,
    /// Body translation read from Rapier.
    pub translation: [f32; 3],
}

/// Handles for P1 workpiece/fixture/actuator bodies.
#[derive(Debug, Clone, Copy)]
pub struct WorldActuatorSmokeBodies {
    floor_collider: Option<ColliderHandle>,
    fixture_collider: ColliderHandle,
    workpiece_body: RigidBodyHandle,
    workpiece_collider: ColliderHandle,
    carrier_body: RigidBodyHandle,
    carrier_collider: ColliderHandle,
}

/// Configuration for the P1 workpiece/fixture/actuator smoke proof.
#[derive(Debug, Clone, Copy)]
pub struct WorldActuatorSmokeConfig {
    /// Fixed tick delta in seconds.
    pub tick_dt_seconds: f32,
    /// Number of fixed ticks to run.
    pub tick_count: u32,
    /// Whether to register the static floor collider.
    pub include_floor: bool,
    /// Whether the actuator creates the fixed joint on contact.
    pub create_joint: bool,
    /// Whether the carrier motor command is active.
    pub drive_carrier: bool,
}

impl Default for WorldActuatorSmokeConfig {
    fn default() -> Self {
        Self {
            tick_dt_seconds: DEFAULT_TICK_DT_SECONDS,
            tick_count: DEFAULT_TICK_COUNT,
            include_floor: true,
            create_joint: true,
            drive_carrier: true,
        }
    }
}

/// P2 scenario variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldMultiActuatorScenario {
    /// Canonical A-to-B handoff.
    CanonicalHandoff,
    /// Two actuators contact the same free workpiece without a handoff plan.
    SimultaneousGripNoHandoff,
    /// B attempts to grip while A still owns the workpiece and no plan permits it.
    SecondGripWhileOwned,
}

/// Handles for P2 multi-actuator proof bodies.
#[derive(Debug, Clone, Copy)]
pub struct WorldMultiActuatorSmokeBodies {
    floor_collider: Option<ColliderHandle>,
    fixture_collider: ColliderHandle,
    transfer_collider: ColliderHandle,
    workpiece_body: RigidBodyHandle,
    workpiece_collider: ColliderHandle,
    carrier_a_body: RigidBodyHandle,
    carrier_a_collider: ColliderHandle,
    carrier_b_body: RigidBodyHandle,
    carrier_b_collider: ColliderHandle,
}

/// Configuration for the P2 multi-actuator smoke proof.
#[derive(Debug, Clone, Copy)]
pub struct WorldMultiActuatorSmokeConfig {
    /// Fixed tick delta in seconds.
    pub tick_dt_seconds: f32,
    /// Number of fixed ticks to run.
    pub tick_count: u32,
    /// Whether to register the static floor collider.
    pub include_floor: bool,
    /// Whether carrier motors are active.
    pub drive_actuators: bool,
    /// Scenario variant.
    pub scenario: WorldMultiActuatorScenario,
    /// Register actuators in reverse order to prove id-ordered stepping.
    pub reverse_actuator_registration: bool,
}

impl Default for WorldMultiActuatorSmokeConfig {
    fn default() -> Self {
        Self {
            tick_dt_seconds: DEFAULT_TICK_DT_SECONDS,
            tick_count: DEFAULT_TICK_COUNT,
            include_floor: true,
            drive_actuators: true,
            scenario: WorldMultiActuatorScenario::CanonicalHandoff,
            reverse_actuator_registration: false,
        }
    }
}

/// P1 actuator state machine.
#[derive(Debug, Clone)]
pub struct WorldActuator {
    state: ActuatorState,
    joint: Option<ImpulseJointHandle>,
    create_joint: bool,
    drive_carrier: bool,
    transitions: Vec<WorldActuatorTransitionTrace>,
    joint_create_ticks: Vec<u32>,
    joint_destroy_ticks: Vec<u32>,
}

impl WorldActuator {
    /// Creates an actuator for the P1 proof.
    #[must_use]
    pub fn new(create_joint: bool, drive_carrier: bool) -> Self {
        Self {
            state: ActuatorState::Idle,
            joint: None,
            create_joint,
            drive_carrier,
            transitions: Vec::new(),
            joint_create_ticks: Vec::new(),
            joint_destroy_ticks: Vec::new(),
        }
    }

    /// Starts the actuator proof.
    pub fn start(&mut self) {
        self.transition(0, ActuatorState::Approaching, "start");
    }

    /// Applies the carrier motor target for the current state.
    pub fn apply_motor(&self, world: &mut World, bodies: WorldActuatorSmokeBodies) {
        if !self.drive_carrier {
            return;
        }
        let target = match self.state {
            ActuatorState::Idle
            | ActuatorState::Held
            | ActuatorState::AcceptingHandoff
            | ActuatorState::Releasing => {
                vector![FIXTURE_CENTER_X, RETRACT_TARGET_Y, FIXTURE_CENTER_Z].into()
            }
            ActuatorState::Approaching => vector![0.0, APPROACH_TARGET_Y, 0.0].into(),
            ActuatorState::Carrying => {
                let lift_target = vector![0.0, CARRY_TARGET_Y, 0.0].into();
                if world.body_center_y_at_least(bodies.carrier_body, CARRY_TARGET_Y - 0.05) {
                    vector![FIXTURE_CENTER_X, CARRY_TARGET_Y, FIXTURE_CENTER_Z].into()
                } else {
                    lift_target
                }
            }
        };
        world.drive_body_toward(bodies.carrier_body, target, CARRIER_MAX_SPEED);
    }

    /// Evaluates contact/position-triggered state transitions after a physics step.
    pub fn after_step(
        &mut self,
        world: &mut World,
        bodies: WorldActuatorSmokeBodies,
        tick: u32,
    ) -> anyhow::Result<()> {
        match self.state {
            ActuatorState::Approaching => {
                if world.contact_active(bodies.carrier_collider, bodies.workpiece_collider) {
                    if self.create_joint {
                        let joint = world.create_carrier_workpiece_fixed_joint(bodies);
                        self.joint = Some(joint);
                        self.joint_create_ticks.push(tick);
                    }
                    self.transition(
                        tick,
                        ActuatorState::Carrying,
                        "contact_pair(carrier, workpiece)",
                    );
                }
            }
            ActuatorState::Carrying => {
                if world.body_near(
                    bodies.carrier_body,
                    vector![FIXTURE_CENTER_X, CARRY_TARGET_Y, FIXTURE_CENTER_Z].into(),
                    RELEASE_TOLERANCE,
                ) {
                    if let Some(joint) = self.joint.take() {
                        world.destroy_carrier_workpiece_fixed_joint(joint);
                        self.joint_destroy_ticks.push(tick);
                    }
                    self.transition(
                        tick,
                        ActuatorState::Releasing,
                        "position_tolerance_at(fixture)",
                    );
                }
            }
            ActuatorState::Releasing => {
                self.transition(tick, ActuatorState::Idle, "joint_destroyed");
            }
            ActuatorState::Idle | ActuatorState::Held | ActuatorState::AcceptingHandoff => {}
        }
        Ok(())
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
            name: None,
            type_name: "WorldActuator".to_string(),
            states_observed: states,
            state_transitions: self.transitions.clone(),
        }
    }

    fn joint_trace(&self) -> WorldJointTrace {
        let active_during_ticks = self
            .joint_create_ticks
            .first()
            .zip(self.joint_destroy_ticks.first())
            .map(|(created, destroyed)| [*created, destroyed.saturating_sub(1)]);
        WorldJointTrace {
            joint_create_ticks: self.joint_create_ticks.clone(),
            joint_destroy_ticks: self.joint_destroy_ticks.clone(),
            active_during_ticks,
            active_by_tick_summary: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum P2ActuatorRole {
    OffererA,
    ReceiverB,
}

#[derive(Debug, Clone)]
struct WorldMultiActuator {
    id: u8,
    name: &'static str,
    role: P2ActuatorRole,
    state: ActuatorState,
    transitions: Vec<WorldActuatorTransitionTrace>,
    faults: Vec<WorldOwnershipFaultTrace>,
}

impl WorldMultiActuator {
    fn new(id: u8, name: &'static str, role: P2ActuatorRole) -> Self {
        Self {
            id,
            name,
            role,
            state: ActuatorState::Idle,
            transitions: Vec::new(),
            faults: Vec::new(),
        }
    }

    fn start(&mut self, tick: u32) {
        if self.role == P2ActuatorRole::OffererA {
            self.transition(tick, ActuatorState::Approaching, "start");
        }
    }

    fn transition(&mut self, tick: u32, to: ActuatorState, trigger: &str) -> String {
        if self.state == to {
            return format!(
                "state_transition({}: {:?} -> {:?})",
                self.name, self.state, to
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
            self.name, previous, to
        )
    }

    fn record_fault(&mut self, fault: WorldOwnershipFaultTrace) {
        self.faults.push(fault);
    }

    fn trace(&self) -> WorldActuatorTrace {
        let mut states = vec![ActuatorState::Idle];
        for transition in &self.transitions {
            states.push(transition.to);
        }
        WorldActuatorTrace {
            id: Some(self.id),
            name: Some(self.name.to_string()),
            type_name: "WorldActuator".to_string(),
            states_observed: states,
            state_transitions: self.transitions.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum P2Owner {
    CarrierA,
    CarrierB,
}

impl P2Owner {
    fn name(self) -> &'static str {
        match self {
            Self::CarrierA => "carrier_a",
            Self::CarrierB => "carrier_b",
        }
    }

    fn joint_name(self) -> &'static str {
        match self {
            Self::CarrierA => "fixed(carrier_a_tool, workpiece_grip)",
            Self::CarrierB => "fixed(carrier_b_tool, workpiece_grip)",
        }
    }
}

#[derive(Debug, Clone)]
struct WorldOwnership {
    owner: Option<P2Owner>,
    active_joint: Option<(P2Owner, ImpulseJointHandle)>,
    transitions: Vec<WorldOwnershipTransitionTrace>,
}

impl WorldOwnership {
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
            owner: self.owner.map(P2Owner::name).map(str::to_string),
        }
    }

    fn transition(&mut self, tick: u32, to: Option<P2Owner>, trigger: &str) {
        let from = self.owner;
        self.transitions.push(WorldOwnershipTransitionTrace {
            tick,
            workpiece: "workpiece".to_string(),
            from: from.map(P2Owner::name).map(str::to_string),
            to: to.map(P2Owner::name).map(str::to_string),
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
struct WorldHandoffPlan {
    registered: bool,
    atomic_tick: Option<u32>,
    atomic_event_order: Vec<String>,
}

impl WorldHandoffPlan {
    fn new(registered: bool) -> Self {
        Self {
            registered,
            atomic_tick: None,
            atomic_event_order: expected_handoff_event_order(),
        }
    }

    fn trace(&self) -> WorldHandoffPlanTrace {
        WorldHandoffPlanTrace {
            registered_pairs: if self.registered {
                vec![WorldHandoffPairTrace {
                    offerer: "carrier_a".to_string(),
                    receiver: "carrier_b".to_string(),
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

impl World {
    /// Creates a deterministic world with a fixed timestep.
    #[must_use]
    pub fn deterministic(tick_dt_seconds: f32) -> Self {
        let integration_parameters = IntegrationParameters {
            dt: tick_dt_seconds,
            normalized_allowed_linear_error: 0.000_01,
            normalized_prediction_distance: 0.02,
            num_solver_iterations: 16,
            num_internal_pgs_iterations: 4,
            num_internal_stabilization_iterations: 4,
            max_ccd_substeps: 4,
            ..Default::default()
        };
        Self {
            gravity: vector![0.0, -9.81, 0.0].into(),
            integration_parameters,
            pipeline: PhysicsPipeline::new(),
            islands: IslandManager::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            bodies: RigidBodySet::new(),
            colliders: ColliderSet::new(),
            impulse_joints: ImpulseJointSet::new(),
            multibody_joints: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
            solvers_registered: vec!["rapier3d"],
        }
    }

    /// Registers one static floor and one dynamic cube for the smoke proof.
    pub fn register_cube_floor_smoke_bodies(&mut self, include_floor: bool) -> WorldSmokeBodies {
        let floor_collider = include_floor.then(|| {
            let floor_body = self.bodies.insert(
                RigidBodyBuilder::fixed()
                    .translation(vector![0.0, FLOOR_Y, 0.0].into())
                    .build(),
            );
            self.colliders.insert_with_parent(
                ColliderBuilder::cuboid(20.0, 0.05, 20.0)
                    .friction(0.9)
                    .restitution(0.0)
                    .build(),
                floor_body,
                &mut self.bodies,
            )
        });

        let cube_body = self.bodies.insert(
            RigidBodyBuilder::dynamic()
                .translation(vector![0.0, CUBE_INITIAL_BOTTOM_Y + CUBE_HALF_EXTENT, 0.0].into())
                .linear_damping(0.15)
                .angular_damping(0.3)
                .ccd_enabled(true)
                .build(),
        );
        let cube_collider = self.colliders.insert_with_parent(
            ColliderBuilder::cuboid(CUBE_HALF_EXTENT, CUBE_HALF_EXTENT, CUBE_HALF_EXTENT)
                .density(1.0)
                .friction(0.9)
                .restitution(0.0)
                .build(),
            cube_body,
            &mut self.bodies,
        );

        WorldSmokeBodies {
            cube_body,
            cube_collider,
            floor_collider,
        }
    }

    /// Steps every registered solver once.
    pub fn step(&mut self) {
        self.pipeline.step(
            self.gravity,
            &self.integration_parameters,
            &mut self.islands,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.bodies,
            &mut self.colliders,
            &mut self.impulse_joints,
            &mut self.multibody_joints,
            &mut self.ccd_solver,
            &(),
            &(),
        );
    }

    /// Immutable access to Rapier bodies for read-only proof code.
    #[must_use]
    pub fn bodies(&self) -> &RigidBodySet {
        &self.bodies
    }

    fn trace_tick(&self, tick: u32, bodies: WorldSmokeBodies) -> anyhow::Result<WorldTickTrace> {
        let cube = self
            .bodies
            .get(bodies.cube_body)
            .ok_or_else(|| anyhow::anyhow!("cube body is missing from world"))?;
        let cube_center = cube.position().translation;
        let cube_velocity = cube.linvel();
        let workpiece = WorldBodyKinematicsTrace {
            y: cube_center.y - CUBE_HALF_EXTENT,
            center: [cube_center.x, cube_center.y, cube_center.z],
            vy: cube_velocity.y,
            velocity: [cube_velocity.x, cube_velocity.y, cube_velocity.z],
        };
        let contacts = if bodies.floor_collider.is_some_and(|floor| {
            self.narrow_phase
                .contact_pair(bodies.cube_collider, floor)
                .is_some_and(ContactPair::has_any_active_contact)
        }) {
            vec![WorldContactTrace {
                a: "cube".to_string(),
                b: "floor".to_string(),
            }]
        } else {
            Vec::new()
        };
        Ok(WorldTickTrace {
            tick,
            contacts,
            carrier: None,
            carrier_a: None,
            carrier_b: None,
            workpiece,
            actuator_state: None,
            actuator_states: Vec::new(),
            ownership: None,
            tick_events: Vec::new(),
            contention_faults: Vec::new(),
            active_joints: Vec::new(),
            joint_distance: None,
            joint_distances: Vec::new(),
            arm_links: Vec::new(),
            arm_joints: Vec::new(),
            arm_a_links: Vec::new(),
            arm_b_links: Vec::new(),
            arm_a_joints: Vec::new(),
            arm_b_joints: Vec::new(),
        })
    }

    /// Registers the P1 workpiece/fixture/actuator proof bodies.
    pub fn register_actuator_smoke_bodies(
        &mut self,
        include_floor: bool,
    ) -> WorldActuatorSmokeBodies {
        let floor_collider = include_floor.then(|| {
            let floor_body = self.bodies.insert(
                RigidBodyBuilder::fixed()
                    .translation(vector![0.0, FLOOR_Y, 0.0].into())
                    .build(),
            );
            self.colliders.insert_with_parent(
                ColliderBuilder::cuboid(20.0, FLOOR_HALF_Y, 20.0)
                    .friction(0.9)
                    .restitution(0.0)
                    .build(),
                floor_body,
                &mut self.bodies,
            )
        });

        let fixture_body = self.bodies.insert(
            RigidBodyBuilder::fixed()
                .translation(vector![FIXTURE_CENTER_X, FIXTURE_CENTER_Y, FIXTURE_CENTER_Z].into())
                .build(),
        );
        let fixture_collider = self.colliders.insert_with_parent(
            ColliderBuilder::cuboid(FIXTURE_HALF_XZ, FIXTURE_HALF_Y, FIXTURE_HALF_XZ)
                .friction(0.95)
                .restitution(0.0)
                .build(),
            fixture_body,
            &mut self.bodies,
        );

        let workpiece_body = self.bodies.insert(
            RigidBodyBuilder::dynamic()
                .translation(vector![0.0, WORKPIECE_INITIAL_CENTER_Y, 0.0].into())
                .lock_rotations()
                .linear_damping(0.25)
                .angular_damping(1.0)
                .ccd_enabled(true)
                .build(),
        );
        let workpiece_collider = self.colliders.insert_with_parent(
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
            &mut self.bodies,
        );

        let carrier_body = self.bodies.insert(
            RigidBodyBuilder::dynamic()
                .translation(vector![0.0, CARRIER_INITIAL_Y, 0.0].into())
                .lock_rotations()
                .linear_damping(0.8)
                .angular_damping(1.0)
                .ccd_enabled(true)
                .build(),
        );
        let carrier_collider = self.colliders.insert_with_parent(
            ColliderBuilder::cuboid(CARRIER_HALF_XZ, CARRIER_HALF_Y, CARRIER_HALF_XZ)
                .density(1.0)
                .friction(0.7)
                .restitution(0.0)
                .build(),
            carrier_body,
            &mut self.bodies,
        );

        WorldActuatorSmokeBodies {
            floor_collider,
            fixture_collider,
            workpiece_body,
            workpiece_collider,
            carrier_body,
            carrier_collider,
        }
    }

    /// Registers the P2 multi-actuator proof bodies.
    pub fn register_multi_actuator_smoke_bodies(
        &mut self,
        include_floor: bool,
        scenario: WorldMultiActuatorScenario,
    ) -> WorldMultiActuatorSmokeBodies {
        let floor_collider = include_floor.then(|| {
            let floor_body = self.bodies.insert(
                RigidBodyBuilder::fixed()
                    .translation(vector![0.0, FLOOR_Y, 0.0].into())
                    .build(),
            );
            self.colliders.insert_with_parent(
                ColliderBuilder::cuboid(20.0, FLOOR_HALF_Y, 20.0)
                    .friction(0.9)
                    .restitution(0.0)
                    .build(),
                floor_body,
                &mut self.bodies,
            )
        });

        let transfer_body = self.bodies.insert(
            RigidBodyBuilder::fixed()
                .translation(
                    vector![TRANSFER_CENTER_X, TRANSFER_CENTER_Y, TRANSFER_CENTER_Z].into(),
                )
                .build(),
        );
        let transfer_collider = self.colliders.insert_with_parent(
            ColliderBuilder::cuboid(TRANSFER_HALF_XZ, TRANSFER_HALF_Y, TRANSFER_HALF_XZ)
                .sensor(true)
                .build(),
            transfer_body,
            &mut self.bodies,
        );

        let fixture_body = self.bodies.insert(
            RigidBodyBuilder::fixed()
                .translation(vector![FIXTURE_CENTER_X, FIXTURE_CENTER_Y, FIXTURE_CENTER_Z].into())
                .build(),
        );
        let fixture_collider = self.colliders.insert_with_parent(
            ColliderBuilder::cuboid(FIXTURE_HALF_XZ, FIXTURE_HALF_Y, FIXTURE_HALF_XZ)
                .friction(0.95)
                .restitution(0.0)
                .build(),
            fixture_body,
            &mut self.bodies,
        );

        let workpiece_body = self.bodies.insert(
            RigidBodyBuilder::dynamic()
                .translation(vector![0.0, WORKPIECE_INITIAL_CENTER_Y, 0.0].into())
                .lock_rotations()
                .linear_damping(0.25)
                .angular_damping(1.0)
                .ccd_enabled(true)
                .build(),
        );
        let workpiece_collider = self.colliders.insert_with_parent(
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
            &mut self.bodies,
        );

        let (carrier_a_position, carrier_b_position) = match scenario {
            WorldMultiActuatorScenario::SimultaneousGripNoHandoff => (
                vector![0.0, APPROACH_TARGET_Y, 0.0].into(),
                vector![0.0, WORKPIECE_INITIAL_CENTER_Y, CARRIER_B_SIDE_Z - 0.02].into(),
            ),
            WorldMultiActuatorScenario::CanonicalHandoff
            | WorldMultiActuatorScenario::SecondGripWhileOwned => (
                vector![0.0, CARRIER_INITIAL_Y, 0.0].into(),
                vector![TRANSFER_CENTER_X, RETRACT_TARGET_Y, CARRIER_B_SIDE_Z].into(),
            ),
        };

        let carrier_a_body = self.bodies.insert(
            RigidBodyBuilder::dynamic()
                .translation(carrier_a_position)
                .lock_rotations()
                .linear_damping(0.8)
                .angular_damping(1.0)
                .ccd_enabled(true)
                .build(),
        );
        let carrier_a_collider = self.colliders.insert_with_parent(
            ColliderBuilder::cuboid(CARRIER_HALF_XZ, CARRIER_HALF_Y, CARRIER_HALF_XZ)
                .density(1.0)
                .friction(0.7)
                .restitution(0.0)
                .build(),
            carrier_a_body,
            &mut self.bodies,
        );

        let carrier_b_body = self.bodies.insert(
            RigidBodyBuilder::dynamic()
                .translation(carrier_b_position)
                .lock_rotations()
                .linear_damping(0.8)
                .angular_damping(1.0)
                .ccd_enabled(true)
                .build(),
        );
        let carrier_b_collider = self.colliders.insert_with_parent(
            ColliderBuilder::cuboid(CARRIER_HALF_XZ, CARRIER_HALF_Y, CARRIER_HALF_XZ)
                .density(1.0)
                .friction(0.7)
                .restitution(0.0)
                .build(),
            carrier_b_body,
            &mut self.bodies,
        );

        WorldMultiActuatorSmokeBodies {
            floor_collider,
            fixture_collider,
            transfer_collider,
            workpiece_body,
            workpiece_collider,
            carrier_a_body,
            carrier_a_collider,
            carrier_b_body,
            carrier_b_collider,
        }
    }

    fn drive_body_toward(&mut self, body: RigidBodyHandle, target: Vector, max_speed: f32) {
        let Some(rigid_body) = self.bodies.get_mut(body) else {
            return;
        };
        let position = rigid_body.position().translation;
        let delta = target - position;
        let distance = delta.length();
        let velocity = if distance <= CARRIER_HOLD_TOLERANCE {
            Vector::ZERO
        } else {
            delta.normalize() * max_speed.min(distance / self.integration_parameters.dt)
        };
        rigid_body.set_linvel(velocity, true);
    }

    fn carrier_b_handoff_target(
        &self,
        carrier_body: RigidBodyHandle,
        workpiece_body: RigidBodyHandle,
    ) -> Option<Vector> {
        let position = self.bodies.get(carrier_body)?.position().translation;
        let workpiece = self.bodies.get(workpiece_body)?.position().translation;
        let staged_target: Vector = vector![workpiece.x, workpiece.y, CARRIER_B_RETRACT_Z].into();
        let staged_delta = staged_target - position;
        if staged_delta.x.abs() > 0.04 || staged_delta.y.abs() > 0.04 {
            return Some(staged_target);
        }
        Some(vector![workpiece.x, workpiece.y, CARRIER_B_SIDE_Z].into())
    }

    fn body_near(&self, body: RigidBodyHandle, target: Vector, tolerance: f32) -> bool {
        self.bodies.get(body).is_some_and(|rigid_body| {
            (rigid_body.position().translation - target).length() <= tolerance
        })
    }

    fn body_center_y_at_least(&self, body: RigidBodyHandle, y: f32) -> bool {
        self.bodies
            .get(body)
            .is_some_and(|rigid_body| rigid_body.position().translation.y >= y)
    }

    fn contact_active(&self, first: ColliderHandle, second: ColliderHandle) -> bool {
        self.narrow_phase
            .contact_pair(first, second)
            .is_some_and(ContactPair::has_any_active_contact)
    }

    fn contact_pair_exists(&self, first: ColliderHandle, second: ColliderHandle) -> bool {
        self.narrow_phase.contact_pair(first, second).is_some()
    }

    fn create_carrier_workpiece_fixed_joint(
        &mut self,
        bodies: WorldActuatorSmokeBodies,
    ) -> ImpulseJointHandle {
        let joint = FixedJointBuilder::new()
            .contacts_enabled(false)
            .local_anchor1(vector![0.0, -CARRIER_HALF_Y, 0.0].into())
            .local_anchor2(vector![0.0, WORKPIECE_HALF_EXTENT, 0.0].into())
            .build();
        self.impulse_joints
            .insert(bodies.carrier_body, bodies.workpiece_body, joint, true)
    }

    fn destroy_carrier_workpiece_fixed_joint(&mut self, joint: ImpulseJointHandle) {
        let _removed = self.impulse_joints.remove(joint, true);
    }

    fn create_owned_workpiece_fixed_joint(
        &mut self,
        bodies: WorldMultiActuatorSmokeBodies,
        owner: P2Owner,
        ownership: &mut WorldOwnership,
        tick: u32,
        trigger: &str,
    ) -> Result<ImpulseJointHandle, WorldOwnershipFaultTrace> {
        if let Some(current_owner) = ownership.owner {
            return Err(WorldOwnershipFaultTrace {
                tick,
                actuator: owner.name().to_string(),
                code: format!("grip_denied_workpiece_owned_by({})", current_owner.name()),
                owner: Some(current_owner.name().to_string()),
            });
        }
        let (carrier_body, local_anchor1, local_anchor2) = match owner {
            P2Owner::CarrierA => (
                bodies.carrier_a_body,
                vector![0.0, -CARRIER_HALF_Y, 0.0].into(),
                vector![0.0, WORKPIECE_HALF_EXTENT, 0.0].into(),
            ),
            P2Owner::CarrierB => (
                bodies.carrier_b_body,
                vector![0.0, 0.0, -CARRIER_HALF_XZ].into(),
                vector![0.0, 0.0, WORKPIECE_HALF_EXTENT].into(),
            ),
        };
        let joint = FixedJointBuilder::new()
            .contacts_enabled(false)
            .local_anchor1(local_anchor1)
            .local_anchor2(local_anchor2)
            .build();
        let handle = self
            .impulse_joints
            .insert(carrier_body, bodies.workpiece_body, joint, true);
        ownership.active_joint = Some((owner, handle));
        ownership.transition(tick, Some(owner), trigger);
        Ok(handle)
    }

    fn destroy_owned_workpiece_fixed_joint(
        &mut self,
        ownership: &mut WorldOwnership,
        tick: u32,
        trigger: &str,
    ) -> Option<P2Owner> {
        let (owner, joint) = ownership.active_joint.take()?;
        let _removed = self.impulse_joints.remove(joint, true);
        ownership.transition(tick, None, trigger);
        Some(owner)
    }

    fn trace_actuator_tick(
        &self,
        tick: u32,
        bodies: WorldActuatorSmokeBodies,
        actuator: &WorldActuator,
    ) -> anyhow::Result<WorldTickTrace> {
        let carrier = self.body_trace(bodies.carrier_body, CARRIER_HALF_Y)?;
        let workpiece = self.body_trace(bodies.workpiece_body, WORKPIECE_HALF_EXTENT)?;
        let contacts = self.actuator_contacts(bodies);
        let active_joints = if actuator.joint.is_some() {
            vec!["fixed(carrier_tool, workpiece_grip)".to_string()]
        } else {
            Vec::new()
        };
        let joint_distance = Some(self.carrier_workpiece_joint_distance(bodies)?);
        Ok(WorldTickTrace {
            tick,
            contacts,
            carrier: Some(carrier),
            carrier_a: None,
            carrier_b: None,
            workpiece,
            actuator_state: Some(actuator.state),
            actuator_states: Vec::new(),
            ownership: None,
            tick_events: Vec::new(),
            contention_faults: Vec::new(),
            active_joints,
            joint_distance,
            joint_distances: Vec::new(),
            arm_links: Vec::new(),
            arm_joints: Vec::new(),
            arm_a_links: Vec::new(),
            arm_b_links: Vec::new(),
            arm_a_joints: Vec::new(),
            arm_b_joints: Vec::new(),
        })
    }

    fn trace_multi_actuator_tick(
        &self,
        tick: u32,
        bodies: WorldMultiActuatorSmokeBodies,
        actuators: &[WorldMultiActuator],
        ownership: &WorldOwnership,
        tick_events: Vec<String>,
        contention_faults: Vec<WorldOwnershipFaultTrace>,
    ) -> anyhow::Result<WorldTickTrace> {
        let carrier_a = self.body_trace(bodies.carrier_a_body, CARRIER_HALF_Y)?;
        let carrier_b = self.body_trace(bodies.carrier_b_body, CARRIER_HALF_Y)?;
        let workpiece = self.body_trace(bodies.workpiece_body, WORKPIECE_HALF_EXTENT)?;
        let contacts = self.multi_actuator_contacts(bodies);
        let active_joints = ownership
            .active_joint
            .map(|(owner, _)| vec![owner.joint_name().to_string()])
            .unwrap_or_default();
        let joint_distances = if let Some((owner, _)) = ownership.active_joint {
            vec![WorldJointDistanceTrace {
                owner: owner.name().to_string(),
                distance: self.multi_carrier_workpiece_joint_distance(bodies, owner)?,
            }]
        } else {
            Vec::new()
        };
        let mut actuator_states = actuators
            .iter()
            .map(|actuator| WorldActuatorStateSample {
                id: actuator.id,
                name: actuator.name.to_string(),
                state: actuator.state,
            })
            .collect::<Vec<_>>();
        actuator_states.sort_by_key(|state| state.id);
        Ok(WorldTickTrace {
            tick,
            contacts,
            carrier: None,
            carrier_a: Some(carrier_a),
            carrier_b: Some(carrier_b),
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
            arm_a_links: Vec::new(),
            arm_b_links: Vec::new(),
            arm_a_joints: Vec::new(),
            arm_b_joints: Vec::new(),
        })
    }

    fn body_trace(
        &self,
        body: RigidBodyHandle,
        half_y: f32,
    ) -> anyhow::Result<WorldBodyKinematicsTrace> {
        let rigid_body = self
            .bodies
            .get(body)
            .ok_or_else(|| anyhow::anyhow!("body is missing from world"))?;
        let position = rigid_body.position().translation;
        let velocity = rigid_body.linvel();
        Ok(WorldBodyKinematicsTrace {
            y: position.y - half_y,
            center: [position.x, position.y, position.z],
            vy: velocity.y,
            velocity: [velocity.x, velocity.y, velocity.z],
        })
    }

    fn actuator_contacts(&self, bodies: WorldActuatorSmokeBodies) -> Vec<WorldContactTrace> {
        let mut contacts = Vec::new();
        if bodies
            .floor_collider
            .is_some_and(|floor| self.contact_active(bodies.workpiece_collider, floor))
        {
            contacts.push(WorldContactTrace {
                a: "workpiece".to_string(),
                b: "floor".to_string(),
            });
        }
        if bodies
            .floor_collider
            .is_some_and(|floor| self.contact_active(bodies.carrier_collider, floor))
        {
            contacts.push(WorldContactTrace {
                a: "carrier".to_string(),
                b: "floor".to_string(),
            });
        }
        if self.contact_active(bodies.carrier_collider, bodies.workpiece_collider) {
            contacts.push(WorldContactTrace {
                a: "carrier".to_string(),
                b: "workpiece".to_string(),
            });
        }
        if self.contact_active(bodies.workpiece_collider, bodies.fixture_collider) {
            contacts.push(WorldContactTrace {
                a: "workpiece".to_string(),
                b: "fixture".to_string(),
            });
        }
        if self.contact_active(bodies.carrier_collider, bodies.fixture_collider) {
            contacts.push(WorldContactTrace {
                a: "carrier".to_string(),
                b: "fixture".to_string(),
            });
        }
        contacts
    }

    fn multi_actuator_contacts(
        &self,
        bodies: WorldMultiActuatorSmokeBodies,
    ) -> Vec<WorldContactTrace> {
        let mut contacts = Vec::new();
        if bodies
            .floor_collider
            .is_some_and(|floor| self.contact_active(bodies.workpiece_collider, floor))
        {
            contacts.push(contact("workpiece", "floor"));
        }
        for (name, collider) in [
            ("carrier_a", bodies.carrier_a_collider),
            ("carrier_b", bodies.carrier_b_collider),
        ] {
            if bodies
                .floor_collider
                .is_some_and(|floor| self.contact_active(collider, floor))
            {
                contacts.push(contact(name, "floor"));
            }
            if self.contact_active(collider, bodies.workpiece_collider) {
                contacts.push(contact(name, "workpiece"));
            }
            if self.contact_active(collider, bodies.fixture_collider) {
                contacts.push(contact(name, "fixture"));
            }
        }
        if self.contact_active(bodies.workpiece_collider, bodies.fixture_collider) {
            contacts.push(contact("workpiece", "fixture"));
        }
        if self.contact_active(bodies.workpiece_collider, bodies.transfer_collider) {
            contacts.push(contact("workpiece", "transfer_zone"));
        }
        contacts
    }

    fn carrier_workpiece_joint_distance(
        &self,
        bodies: WorldActuatorSmokeBodies,
    ) -> anyhow::Result<f32> {
        let carrier = self
            .bodies
            .get(bodies.carrier_body)
            .ok_or_else(|| anyhow::anyhow!("carrier body is missing from world"))?;
        let workpiece = self
            .bodies
            .get(bodies.workpiece_body)
            .ok_or_else(|| anyhow::anyhow!("workpiece body is missing from world"))?;
        let carrier_tool_offset: Vector = vector![0.0, -CARRIER_HALF_Y, 0.0].into();
        let workpiece_grip_offset: Vector = vector![0.0, WORKPIECE_HALF_EXTENT, 0.0].into();
        let carrier_tool = carrier.position().translation + carrier_tool_offset;
        let workpiece_grip = workpiece.position().translation + workpiece_grip_offset;
        Ok((carrier_tool - workpiece_grip).length())
    }

    fn multi_carrier_workpiece_joint_distance(
        &self,
        bodies: WorldMultiActuatorSmokeBodies,
        owner: P2Owner,
    ) -> anyhow::Result<f32> {
        let (carrier_body, carrier_tool_offset, workpiece_grip_offset): (
            RigidBodyHandle,
            Vector,
            Vector,
        ) = match owner {
            P2Owner::CarrierA => (
                bodies.carrier_a_body,
                vector![0.0, -CARRIER_HALF_Y, 0.0].into(),
                vector![0.0, WORKPIECE_HALF_EXTENT, 0.0].into(),
            ),
            P2Owner::CarrierB => (
                bodies.carrier_b_body,
                vector![0.0, 0.0, -CARRIER_HALF_XZ].into(),
                vector![0.0, 0.0, WORKPIECE_HALF_EXTENT].into(),
            ),
        };
        let carrier = self
            .bodies
            .get(carrier_body)
            .ok_or_else(|| anyhow::anyhow!("carrier body is missing from world"))?;
        let workpiece = self
            .bodies
            .get(bodies.workpiece_body)
            .ok_or_else(|| anyhow::anyhow!("workpiece body is missing from world"))?;
        let carrier_tool = carrier.position().translation + carrier_tool_offset;
        let workpiece_grip = workpiece.position().translation + workpiece_grip_offset;
        Ok((carrier_tool - workpiece_grip).length())
    }
}

/// Runs the standalone cube/floor physics smoke proof.
///
/// The optional Scena scene/node arguments are intentionally narrow: this proof
/// records that the handoff function actually executed while the world ticked.
pub fn run_world_smoke(
    config: WorldSmokeConfig,
    scene: &mut scena::Scene,
    cube_node: scena::NodeKey,
) -> anyhow::Result<WorldSmokeTrace> {
    let mut world = World::deterministic(config.tick_dt_seconds);
    let bodies = world.register_cube_floor_smoke_bodies(config.include_floor);
    let mut per_tick_trace = Vec::with_capacity(config.tick_count as usize + 1);

    let initial_sample =
        apply_rapier_body_pose_to_scena_node(scene, cube_node, world.bodies(), bodies.cube_body)?;
    let mut handoff_line = initial_sample.line;
    per_tick_trace.push(world.trace_tick(0, bodies)?);

    for tick in 1..=config.tick_count {
        world.step();
        let sample = apply_rapier_body_pose_to_scena_node(
            scene,
            cube_node,
            world.bodies(),
            bodies.cube_body,
        )?;
        handoff_line = sample.line;
        per_tick_trace.push(world.trace_tick(tick, bodies)?);
    }

    let determinism_trace_hash = determinism_trace_hash(&per_tick_trace)?;
    let assertions = assert_world_smoke_trace(&per_tick_trace);
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
            bodies_registered: Vec::new(),
        },
        transform_handoff: TransformHandoffTrace {
            function: HANDOFF_FUNCTION.to_string(),
            file: HANDOFF_FILE.to_string(),
            line: handoff_line,
            read_source: "rapier3d::dynamics::RigidBody::position".to_string(),
            write_target: "scena scene-node transform for body 'cube'".to_string(),
        },
        renderer_origin: None,
        screenshot_initial_png: "target/gate-artifacts/world_smoke_initial.png".to_string(),
        screenshot_grip_png: None,
        screenshot_carry_png: None,
        screenshot_transfer_png: None,
        screenshot_handoff_png: None,
        screenshot_final_png: "target/gate-artifacts/world_smoke_final.png".to_string(),
        actuator: None,
        actuators: None,
        ownership: None,
        handoff_plan: None,
        urdf: None,
        fk_verifier: None,
        joints: None,
        per_tick_trace,
        determinism_trace_hash,
        assertions,
    })
}

/// Runs the P1 workpiece/fixture/actuator physics smoke proof.
pub fn run_world_actuator_smoke(
    config: WorldActuatorSmokeConfig,
    scene: &mut scena::Scene,
    carrier_node: scena::NodeKey,
    workpiece_node: scena::NodeKey,
) -> anyhow::Result<WorldSmokeTrace> {
    let mut world = World::deterministic(config.tick_dt_seconds);
    let bodies = world.register_actuator_smoke_bodies(config.include_floor);
    let mut actuator = WorldActuator::new(config.create_joint, config.drive_carrier);
    actuator.start();
    let mut per_tick_trace = Vec::with_capacity(config.tick_count as usize + 1);

    let carrier_sample = apply_rapier_body_pose_to_scena_node(
        scene,
        carrier_node,
        world.bodies(),
        bodies.carrier_body,
    )?;
    let workpiece_sample = apply_rapier_body_pose_to_scena_node(
        scene,
        workpiece_node,
        world.bodies(),
        bodies.workpiece_body,
    )?;
    let mut handoff_line = carrier_sample.line.max(workpiece_sample.line);
    per_tick_trace.push(world.trace_actuator_tick(0, bodies, &actuator)?);

    for tick in 1..=config.tick_count {
        actuator.apply_motor(&mut world, bodies);
        world.step();
        actuator.after_step(&mut world, bodies, tick)?;
        let carrier_sample = apply_rapier_body_pose_to_scena_node(
            scene,
            carrier_node,
            world.bodies(),
            bodies.carrier_body,
        )?;
        let workpiece_sample = apply_rapier_body_pose_to_scena_node(
            scene,
            workpiece_node,
            world.bodies(),
            bodies.workpiece_body,
        )?;
        handoff_line = carrier_sample.line.max(workpiece_sample.line);
        per_tick_trace.push(world.trace_actuator_tick(tick, bodies, &actuator)?);
    }

    let determinism_trace_hash = determinism_trace_hash(&per_tick_trace)?;
    let assertions = assert_world_actuator_smoke_trace(&per_tick_trace);
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
            bodies_registered: vec![
                body_registration("floor", "static", "box"),
                body_registration("fixture", "static", "box"),
                body_registration("workpiece", "dynamic", "box"),
                body_registration("carrier", "dynamic", "box"),
            ],
        },
        transform_handoff: TransformHandoffTrace {
            function: HANDOFF_FUNCTION.to_string(),
            file: HANDOFF_FILE.to_string(),
            line: handoff_line,
            read_source: "rapier3d::dynamics::RigidBody::position".to_string(),
            write_target: "scena scene-node transform for bodies 'carrier' and 'workpiece'"
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
        urdf: None,
        fk_verifier: None,
        joints: Some(actuator.joint_trace()),
        per_tick_trace,
        determinism_trace_hash,
        assertions,
    })
}

/// Runs the P2 multi-actuator ownership/handoff smoke proof.
pub fn run_world_multi_actuator_smoke(
    config: WorldMultiActuatorSmokeConfig,
    scene: &mut scena::Scene,
    carrier_a_node: scena::NodeKey,
    carrier_b_node: scena::NodeKey,
    workpiece_node: scena::NodeKey,
) -> anyhow::Result<WorldSmokeTrace> {
    let mut world = World::deterministic(config.tick_dt_seconds);
    let bodies = world.register_multi_actuator_smoke_bodies(config.include_floor, config.scenario);
    let mut actuators = create_multi_actuators(config);
    for actuator in &mut actuators {
        actuator.start(0);
    }
    if config.scenario == WorldMultiActuatorScenario::SimultaneousGripNoHandoff {
        actuator_mut(&mut actuators, P2Owner::CarrierB).transition(
            0,
            ActuatorState::Approaching,
            "start",
        );
    }
    let mut ownership = WorldOwnership::new();
    let mut handoff_plan =
        WorldHandoffPlan::new(config.scenario == WorldMultiActuatorScenario::CanonicalHandoff);
    let mut per_tick_trace = Vec::with_capacity(config.tick_count as usize + 1);

    let carrier_a_sample = apply_rapier_body_pose_to_scena_node(
        scene,
        carrier_a_node,
        world.bodies(),
        bodies.carrier_a_body,
    )?;
    let carrier_b_sample = apply_rapier_body_pose_to_scena_node(
        scene,
        carrier_b_node,
        world.bodies(),
        bodies.carrier_b_body,
    )?;
    let workpiece_sample = apply_rapier_body_pose_to_scena_node(
        scene,
        workpiece_node,
        world.bodies(),
        bodies.workpiece_body,
    )?;
    let mut handoff_line = carrier_a_sample
        .line
        .max(carrier_b_sample.line)
        .max(workpiece_sample.line);
    per_tick_trace.push(world.trace_multi_actuator_tick(
        0,
        bodies,
        &actuators,
        &ownership,
        Vec::new(),
        Vec::new(),
    )?);

    for tick in 1..=config.tick_count {
        apply_multi_actuator_motors(&mut world, bodies, &actuators, config);
        world.step();
        let (events, faults) = step_multi_actuators(
            &mut world,
            bodies,
            &mut actuators,
            &mut ownership,
            &mut handoff_plan,
            config,
            tick,
        )?;
        let carrier_a_sample = apply_rapier_body_pose_to_scena_node(
            scene,
            carrier_a_node,
            world.bodies(),
            bodies.carrier_a_body,
        )?;
        let carrier_b_sample = apply_rapier_body_pose_to_scena_node(
            scene,
            carrier_b_node,
            world.bodies(),
            bodies.carrier_b_body,
        )?;
        let workpiece_sample = apply_rapier_body_pose_to_scena_node(
            scene,
            workpiece_node,
            world.bodies(),
            bodies.workpiece_body,
        )?;
        handoff_line = carrier_a_sample
            .line
            .max(carrier_b_sample.line)
            .max(workpiece_sample.line);
        per_tick_trace.push(
            world
                .trace_multi_actuator_tick(tick, bodies, &actuators, &ownership, events, faults)?,
        );
    }

    let determinism_trace_hash = determinism_trace_hash(&per_tick_trace)?;
    let assertions = assert_world_multi_actuator_smoke_trace(&per_tick_trace);
    let joint_trace = multi_joint_trace(&per_tick_trace);
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
            bodies_registered: vec![
                body_registration("floor", "static", "box"),
                body_registration("fixture", "static", "box"),
                body_registration("transfer_zone", "static", "marker"),
                body_registration("workpiece", "dynamic", "box"),
                body_registration("carrier_a", "dynamic", "box"),
                body_registration("carrier_b", "dynamic", "box"),
            ],
        },
        transform_handoff: TransformHandoffTrace {
            function: HANDOFF_FUNCTION.to_string(),
            file: HANDOFF_FILE.to_string(),
            line: handoff_line,
            read_source: "rapier3d::dynamics::RigidBody::position".to_string(),
            write_target:
                "scena scene-node transform for bodies 'carrier_a', 'carrier_b', and 'workpiece'"
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
        actuators: Some(sorted_actuator_traces(&actuators)),
        ownership: Some(ownership.trace()),
        handoff_plan: Some(handoff_plan.trace()),
        urdf: None,
        fk_verifier: None,
        joints: Some(joint_trace),
        per_tick_trace,
        determinism_trace_hash,
        assertions,
    })
}

fn create_multi_actuators(config: WorldMultiActuatorSmokeConfig) -> Vec<WorldMultiActuator> {
    let mut actuators = vec![
        WorldMultiActuator::new(0, "carrier_a", P2ActuatorRole::OffererA),
        WorldMultiActuator::new(1, "carrier_b", P2ActuatorRole::ReceiverB),
    ];
    if config.reverse_actuator_registration {
        actuators.reverse();
    }
    actuators
}

fn sorted_actuator_traces(actuators: &[WorldMultiActuator]) -> Vec<WorldActuatorTrace> {
    let mut traces = actuators
        .iter()
        .map(WorldMultiActuator::trace)
        .collect::<Vec<_>>();
    traces.sort_by_key(|trace| trace.id.unwrap_or(u8::MAX));
    traces
}

fn apply_multi_actuator_motors(
    world: &mut World,
    bodies: WorldMultiActuatorSmokeBodies,
    actuators: &[WorldMultiActuator],
    config: WorldMultiActuatorSmokeConfig,
) {
    if !config.drive_actuators {
        return;
    }
    for actuator in actuators {
        let (body, target) = match actuator.role {
            P2ActuatorRole::OffererA => (
                bodies.carrier_a_body,
                match actuator.state {
                    ActuatorState::Idle | ActuatorState::Releasing => {
                        vector![TRANSFER_CENTER_X, RETRACT_TARGET_Y, CARRIER_A_RETRACT_Z].into()
                    }
                    ActuatorState::Approaching => vector![0.0, APPROACH_TARGET_Y, 0.0].into(),
                    ActuatorState::Carrying => {
                        let lifted = world
                            .body_center_y_at_least(bodies.carrier_a_body, CARRY_TARGET_Y - 0.05);
                        if lifted {
                            vector![TRANSFER_CENTER_X, CARRY_TARGET_Y, TRANSFER_CENTER_Z].into()
                        } else {
                            vector![0.0, CARRY_TARGET_Y, 0.0].into()
                        }
                    }
                    ActuatorState::Held => {
                        vector![TRANSFER_CENTER_X, CARRY_TARGET_Y, TRANSFER_CENTER_Z].into()
                    }
                    ActuatorState::AcceptingHandoff => {
                        vector![TRANSFER_CENTER_X, RETRACT_TARGET_Y, CARRIER_A_RETRACT_Z].into()
                    }
                },
            ),
            P2ActuatorRole::ReceiverB => (
                bodies.carrier_b_body,
                match actuator.state {
                    ActuatorState::Idle | ActuatorState::Releasing => {
                        vector![FIXTURE_CENTER_X, RETRACT_TARGET_Y, CARRIER_B_RETRACT_Z].into()
                    }
                    ActuatorState::Approaching | ActuatorState::AcceptingHandoff => world
                        .carrier_b_handoff_target(bodies.carrier_b_body, bodies.workpiece_body)
                        .unwrap_or_else(|| {
                            vector![TRANSFER_CENTER_X, CARRIER_B_HANDOFF_Y, CARRIER_B_RETRACT_Z]
                                .into()
                        }),
                    ActuatorState::Carrying => vector![
                        FIXTURE_CENTER_X,
                        CARRIER_B_FIXTURE_TARGET_Y,
                        CARRIER_B_SIDE_Z
                    ]
                    .into(),
                    ActuatorState::Held => {
                        vector![TRANSFER_CENTER_X, CARRIER_B_HANDOFF_Y, CARRIER_B_SIDE_Z].into()
                    }
                },
            ),
        };
        world.drive_body_toward(body, target, CARRIER_MAX_SPEED);
    }
}

fn step_multi_actuators(
    world: &mut World,
    bodies: WorldMultiActuatorSmokeBodies,
    actuators: &mut [WorldMultiActuator],
    ownership: &mut WorldOwnership,
    handoff_plan: &mut WorldHandoffPlan,
    config: WorldMultiActuatorSmokeConfig,
    tick: u32,
) -> anyhow::Result<(Vec<String>, Vec<WorldOwnershipFaultTrace>)> {
    if handoff_ready(world, bodies, actuators, ownership, handoff_plan) {
        let events =
            perform_atomic_handoff(world, bodies, actuators, ownership, handoff_plan, tick)?;
        return Ok((events, Vec::new()));
    }

    let mut events = Vec::new();
    let mut faults = Vec::new();
    let mut ids = actuators
        .iter()
        .map(|actuator| actuator.id)
        .collect::<Vec<_>>();
    ids.sort_unstable();
    for id in ids {
        let index = actuators
            .iter()
            .position(|actuator| actuator.id == id)
            .expect("actuator id exists");
        match actuators[index].role {
            P2ActuatorRole::OffererA => step_offeror_a(
                world,
                bodies,
                &mut actuators[index],
                ownership,
                tick,
                &mut events,
                &mut faults,
            )?,
            P2ActuatorRole::ReceiverB => step_receiver_b(
                world,
                bodies,
                actuators,
                index,
                ownership,
                handoff_plan,
                config,
                tick,
                &mut events,
                &mut faults,
            )?,
        }
    }
    Ok((events, faults))
}

fn step_offeror_a(
    world: &mut World,
    bodies: WorldMultiActuatorSmokeBodies,
    actuator: &mut WorldMultiActuator,
    ownership: &mut WorldOwnership,
    tick: u32,
    events: &mut Vec<String>,
    faults: &mut Vec<WorldOwnershipFaultTrace>,
) -> anyhow::Result<()> {
    match actuator.state {
        ActuatorState::Approaching => {
            if world.contact_active(bodies.carrier_a_collider, bodies.workpiece_collider) {
                match world.create_owned_workpiece_fixed_joint(
                    bodies,
                    P2Owner::CarrierA,
                    ownership,
                    tick,
                    "grip_on(carrier_a)",
                ) {
                    Ok(_) => {
                        events.push("joint_create(carrier_a, workpiece)".to_string());
                        events.push(actuator.transition(
                            tick,
                            ActuatorState::Carrying,
                            "contact_pair(carrier_a, workpiece)",
                        ));
                    }
                    Err(fault) => {
                        actuator.record_fault(fault.clone());
                        faults.push(fault);
                    }
                }
            }
        }
        ActuatorState::Carrying => {
            if world.body_near(
                bodies.carrier_a_body,
                vector![TRANSFER_CENTER_X, CARRY_TARGET_Y, TRANSFER_CENTER_Z].into(),
                RELEASE_TOLERANCE,
            ) {
                events.push(actuator.transition(
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
fn step_receiver_b(
    world: &mut World,
    bodies: WorldMultiActuatorSmokeBodies,
    actuators: &mut [WorldMultiActuator],
    index: usize,
    ownership: &mut WorldOwnership,
    handoff_plan: &WorldHandoffPlan,
    config: WorldMultiActuatorSmokeConfig,
    tick: u32,
    events: &mut Vec<String>,
    faults: &mut Vec<WorldOwnershipFaultTrace>,
) -> anyhow::Result<()> {
    let a_held = actuators.iter().any(|actuator| {
        actuator.role == P2ActuatorRole::OffererA && actuator.state == ActuatorState::Held
    });
    let actuator = &mut actuators[index];
    match actuator.state {
        ActuatorState::Idle => {
            let should_accept = a_held
                && (handoff_plan.registered
                    || config.scenario == WorldMultiActuatorScenario::SecondGripWhileOwned);
            if should_accept {
                events.push(actuator.transition(
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
            if world.contact_active(bodies.carrier_b_collider, bodies.workpiece_collider) {
                let eligible = handoff_plan.registered
                    && a_held
                    && actuator.state == ActuatorState::AcceptingHandoff;
                if eligible {
                    return Ok(());
                }
                match world.create_owned_workpiece_fixed_joint(
                    bodies,
                    P2Owner::CarrierB,
                    ownership,
                    tick,
                    "grip_on(carrier_b)",
                ) {
                    Ok(_) => {
                        events.push("joint_create(carrier_b, workpiece)".to_string());
                        events.push(actuator.transition(
                            tick,
                            ActuatorState::Carrying,
                            "contact_pair(carrier_b, workpiece)",
                        ));
                    }
                    Err(fault) => {
                        actuator.record_fault(fault.clone());
                        faults.push(fault);
                    }
                }
            }
        }
        ActuatorState::Carrying => {
            if world.body_near(
                bodies.carrier_b_body,
                vector![
                    FIXTURE_CENTER_X,
                    CARRIER_B_FIXTURE_TARGET_Y,
                    CARRIER_B_SIDE_Z
                ]
                .into(),
                RELEASE_TOLERANCE,
            ) {
                if world
                    .destroy_owned_workpiece_fixed_joint(ownership, tick, "release(carrier_b)")
                    .is_some()
                {
                    events.push("joint_destroy(carrier_b, workpiece)".to_string());
                }
                events.push(actuator.transition(
                    tick,
                    ActuatorState::Releasing,
                    "position_tolerance_at(fixture)",
                ));
            }
        }
        ActuatorState::Releasing => {
            events.push(actuator.transition(tick, ActuatorState::Idle, "joint_destroyed"));
        }
        ActuatorState::Held => {}
    }
    Ok(())
}

fn handoff_ready(
    world: &World,
    bodies: WorldMultiActuatorSmokeBodies,
    actuators: &[WorldMultiActuator],
    ownership: &WorldOwnership,
    handoff_plan: &WorldHandoffPlan,
) -> bool {
    handoff_plan.registered
        && ownership.owner == Some(P2Owner::CarrierA)
        && actuators.iter().any(|actuator| {
            actuator.role == P2ActuatorRole::OffererA && actuator.state == ActuatorState::Held
        })
        && actuators.iter().any(|actuator| {
            actuator.role == P2ActuatorRole::ReceiverB
                && actuator.state == ActuatorState::AcceptingHandoff
        })
        && world.contact_active(bodies.carrier_b_collider, bodies.workpiece_collider)
}

fn perform_atomic_handoff(
    world: &mut World,
    bodies: WorldMultiActuatorSmokeBodies,
    actuators: &mut [WorldMultiActuator],
    ownership: &mut WorldOwnership,
    handoff_plan: &mut WorldHandoffPlan,
    tick: u32,
) -> anyhow::Result<Vec<String>> {
    let mut events = Vec::new();
    let Some((P2Owner::CarrierA, old_joint)) = ownership.active_joint.take() else {
        anyhow::bail!("atomic handoff requires carrier_a joint");
    };
    let _removed = world.impulse_joints.remove(old_joint, true);
    events.push("joint_destroy(carrier_a, workpiece)".to_string());

    let joint = FixedJointBuilder::new()
        .contacts_enabled(false)
        .local_anchor1(vector![0.0, 0.0, -CARRIER_HALF_XZ].into())
        .local_anchor2(vector![0.0, 0.0, WORKPIECE_HALF_EXTENT].into())
        .build();
    let handle =
        world
            .impulse_joints
            .insert(bodies.carrier_b_body, bodies.workpiece_body, joint, true);
    ownership.active_joint = Some((P2Owner::CarrierB, handle));
    ownership.transitions.push(WorldOwnershipTransitionTrace {
        tick,
        workpiece: "workpiece".to_string(),
        from: Some("carrier_a".to_string()),
        to: Some("carrier_b".to_string()),
        trigger: "handoff_atomic(carrier_a -> carrier_b)".to_string(),
    });
    ownership.owner = Some(P2Owner::CarrierB);
    events.push("joint_create(carrier_b, workpiece)".to_string());

    let (offerer, receiver) = two_actuators_mut(actuators, P2Owner::CarrierA, P2Owner::CarrierB);
    events.push(offerer.transition(tick, ActuatorState::Idle, "handoff_atomic"));
    events.push(receiver.transition(tick, ActuatorState::Carrying, "handoff_atomic"));
    handoff_plan.atomic_tick = Some(tick);
    Ok(events)
}

fn actuator_mut(actuators: &mut [WorldMultiActuator], owner: P2Owner) -> &mut WorldMultiActuator {
    actuators
        .iter_mut()
        .find(|actuator| actuator.name == owner.name())
        .expect("actuator exists")
}

fn two_actuators_mut(
    actuators: &mut [WorldMultiActuator],
    first: P2Owner,
    second: P2Owner,
) -> (&mut WorldMultiActuator, &mut WorldMultiActuator) {
    let first_index = actuators
        .iter()
        .position(|actuator| actuator.name == first.name())
        .expect("first actuator exists");
    let second_index = actuators
        .iter()
        .position(|actuator| actuator.name == second.name())
        .expect("second actuator exists");
    assert_ne!(first_index, second_index);
    if first_index < second_index {
        let (left, right) = actuators.split_at_mut(second_index);
        (&mut left[first_index], &mut right[0])
    } else {
        let (left, right) = actuators.split_at_mut(first_index);
        (&mut right[0], &mut left[second_index])
    }
}

fn expected_handoff_event_order() -> Vec<String> {
    vec![
        "joint_destroy(carrier_a, workpiece)".to_string(),
        "joint_create(carrier_b, workpiece)".to_string(),
        "state_transition(carrier_a: Held -> Idle)".to_string(),
        "state_transition(carrier_b: AcceptingHandoff -> Carrying)".to_string(),
    ]
}

/// WORLD_DYNAMIC_TRANSFORM_HANDOFF_ALLOWED
/// Applies a Rapier-owned rigid-body pose to exactly one Scena scene node.
///
/// This is the single dynamic-body transform write path for the smoke proof.
/// Future dynamic visible bodies must enter Scena through this boundary or an
/// audited successor with the same world-state ownership contract.
pub fn apply_rapier_body_pose_to_scena_node(
    scene: &mut scena::Scene,
    node: scena::NodeKey,
    bodies: &RigidBodySet,
    body: RigidBodyHandle,
) -> anyhow::Result<TransformHandoffSample> {
    let rigid_body = bodies
        .get(body)
        .ok_or_else(|| anyhow::anyhow!("rigid body is missing from world"))?;
    let position = rigid_body.position();
    let translation = position.translation;
    scene.set_transform(
        node,
        scena::Transform {
            translation: scena::Vec3::new(translation.x, translation.y, translation.z),
            rotation: scena::Quat::IDENTITY,
            scale: scena::Vec3::new(1.0, 1.0, 1.0),
        },
    )?;
    Ok(TransformHandoffSample {
        line: line!(),
        translation: [translation.x, translation.y, translation.z],
    })
}

/// Computes the three positive smoke-proof assertions from a trace.
#[must_use]
pub fn assert_world_smoke_trace(per_tick_trace: &[WorldTickTrace]) -> WorldSmokeAssertions {
    let min_y = per_tick_trace
        .iter()
        .map(|tick| tick.workpiece.y)
        .fold(f32::INFINITY, f32::min);
    let first_contact_tick = per_tick_trace
        .iter()
        .find(|tick| contact_contains_cube_floor(&tick.contacts))
        .map(|tick| tick.tick);
    let max_downward_velocity_before_contact = per_tick_trace
        .iter()
        .take_while(|tick| Some(tick.tick) != first_contact_tick)
        .map(|tick| tick.workpiece.vy)
        .fold(0.0, f32::min);
    let monotonically_falling_until_contact =
        workpiece_falls_monotonically_until_contact(per_tick_trace, first_contact_tick);
    WorldSmokeAssertions {
        workpiece_above_floor: BodyAboveFloorAssertion {
            ok: min_y >= FLOOR_Y - ABOVE_FLOOR_EPSILON,
            min_y,
            floor_y: FLOOR_Y,
        },
        gravity_applied: GravityAppliedAssertion {
            ok: monotonically_falling_until_contact && max_downward_velocity_before_contact < -0.1,
            max_downward_velocity_before_contact,
        },
        contact_fired: ContactFiredAssertion {
            ok: first_contact_tick.is_some(),
            first_contact_tick,
        },
        carrier_above_floor: None,
        no_fixture_interpenetration: None,
        grip_event_has_contact: None,
        carry_constraint_driven: None,
        release_destroyed_joint: None,
        workpiece_settled_on_fixture: None,
        exclusive_ownership: None,
        ownership_transfer_atomic: None,
        handoff_order_deterministic: None,
        no_phantom_carry: None,
        determinism_hash_stable: None,
        urdf_parsed_once: None,
        arm_rendered_through_handoff: None,
        fk_matches_rapier: None,
        joint_limits_enforced: None,
        arm_links_above_floor: None,
        multi_urdf_arms_loaded: None,
        per_arm_fk_consistency: None,
    }
}

/// Computes P1 workpiece/fixture/actuator proof assertions from a trace.
#[must_use]
pub fn assert_world_actuator_smoke_trace(
    per_tick_trace: &[WorldTickTrace],
) -> WorldSmokeAssertions {
    let workpiece_min_y = per_tick_trace
        .iter()
        .map(|tick| tick.workpiece.y)
        .fold(f32::INFINITY, f32::min);
    let carrier_min_y = per_tick_trace
        .iter()
        .filter_map(|tick| tick.carrier.as_ref().map(|body| body.y))
        .fold(f32::INFINITY, f32::min);
    let max_downward_velocity = per_tick_trace
        .iter()
        .map(|tick| tick.workpiece.vy)
        .fold(0.0, f32::min);
    let grip_tick = per_tick_trace
        .iter()
        .find(|tick| tick.actuator_state == Some(ActuatorState::Carrying))
        .map(|tick| tick.tick);
    let grip_contact_present = grip_tick
        .and_then(|tick| per_tick_trace.iter().find(|sample| sample.tick == tick))
        .is_some_and(|tick| contact_contains(&tick.contacts, "carrier", "workpiece"));
    let release_tick = per_tick_trace
        .iter()
        .find(|tick| tick.actuator_state == Some(ActuatorState::Releasing))
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
    for tick in per_tick_trace
        .iter()
        .filter(|tick| !tick.active_joints.is_empty())
    {
        checked_joint_ticks += 1;
        max_joint_distance = max_joint_distance.max(tick.joint_distance.unwrap_or(f32::INFINITY));
    }
    let max_fixture_penetration = per_tick_trace.iter().fold(0.0_f32, |current, tick| {
        let workpiece = fixture_penetration(
            tick.workpiece.center,
            WORKPIECE_HALF_EXTENT,
            WORKPIECE_HALF_EXTENT,
        );
        let carrier = tick.carrier.as_ref().map_or(0.0, |body| {
            fixture_penetration(body.center, CARRIER_HALF_XZ, CARRIER_HALF_Y)
        });
        current.max(workpiece).max(carrier)
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
            ok: carrier_min_y >= FLOOR_Y - ABOVE_FLOOR_EPSILON,
            min_y: carrier_min_y,
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
            ok: checked_joint_ticks > 0 && max_joint_distance <= JOINT_DISTANCE_TOLERANCE,
            max_joint_distance,
            tolerance: JOINT_DISTANCE_TOLERANCE,
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
        joint_limits_enforced: None,
        arm_links_above_floor: None,
        multi_urdf_arms_loaded: None,
        per_arm_fk_consistency: None,
    }
}

/// Computes P2 multi-actuator proof assertions from a trace.
#[must_use]
pub fn assert_world_multi_actuator_smoke_trace(
    per_tick_trace: &[WorldTickTrace],
) -> WorldSmokeAssertions {
    let workpiece_min_y = per_tick_trace
        .iter()
        .map(|tick| tick.workpiece.y)
        .fold(f32::INFINITY, f32::min);
    let carrier_min_y = per_tick_trace
        .iter()
        .flat_map(|tick| [tick.carrier_a.as_ref(), tick.carrier_b.as_ref()])
        .flatten()
        .map(|body| body.y)
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
                .any(|event| event == "joint_create(carrier_a, workpiece)")
        })
        .map(|tick| tick.tick);
    let grip_contact_present = grip_tick
        .and_then(|tick| per_tick_trace.iter().find(|sample| sample.tick == tick))
        .is_some_and(|tick| contact_contains(&tick.contacts, "carrier_a", "workpiece"));
    let release_tick = per_tick_trace
        .iter()
        .find(|tick| {
            tick.tick_events
                .iter()
                .any(|event| event == "joint_destroy(carrier_b, workpiece)")
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
        let workpiece = fixture_penetration(
            tick.workpiece.center,
            WORKPIECE_HALF_EXTENT,
            WORKPIECE_HALF_EXTENT,
        );
        let carrier_a = tick.carrier_a.as_ref().map_or(0.0, |body| {
            fixture_penetration(body.center, CARRIER_HALF_XZ, CARRIER_HALF_Y)
        });
        let carrier_b = tick.carrier_b.as_ref().map_or(0.0, |body| {
            fixture_penetration(body.center, CARRIER_HALF_XZ, CARRIER_HALF_Y)
        });
        current.max(workpiece).max(carrier_a).max(carrier_b)
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
    let handoff_tick = find_handoff_tick(per_tick_trace);
    let expected_order = expected_handoff_event_order();
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
    let ownership_transfer_atomic = ownership_transfer_atomic(per_tick_trace, handoff_tick);
    let phantom_violations = phantom_carry_violation_count(per_tick_trace);
    let workpiece_above_floor = BodyAboveFloorAssertion {
        ok: workpiece_min_y >= FLOOR_Y - ABOVE_FLOOR_EPSILON,
        min_y: workpiece_min_y,
        floor_y: FLOOR_Y,
    };
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
            ok: carrier_min_y >= FLOOR_Y - ABOVE_FLOOR_EPSILON,
            min_y: carrier_min_y,
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
            ok: checked_joint_ticks > 0 && max_joint_distance <= JOINT_DISTANCE_TOLERANCE,
            max_joint_distance,
            tolerance: JOINT_DISTANCE_TOLERANCE,
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
        exclusive_ownership: Some(ExclusiveOwnershipAssertion {
            ok: joint_summary.ticks_with_two_joints == 0,
            ticks_with_zero_joints: joint_summary.ticks_with_zero_joints,
            ticks_with_one_joint: joint_summary.ticks_with_one_joint,
            ticks_with_two_joints: joint_summary.ticks_with_two_joints,
        }),
        ownership_transfer_atomic: Some(OwnershipTransferAtomicAssertion {
            ok: ownership_transfer_atomic,
            handoff_tick,
            destroy_and_create_same_tick: handoff_tick.is_some(),
            no_undefined_transfer_tick: ownership_transfer_atomic,
        }),
        handoff_order_deterministic: Some(HandoffOrderAssertion {
            ok: !observed_order.is_empty() && observed_order == expected_order,
            handoff_tick,
            expected_order,
            observed_order,
        }),
        no_phantom_carry: Some(NoPhantomCarryAssertion {
            ok: phantom_violations == 0,
            violation_count: phantom_violations,
        }),
        determinism_hash_stable: None,
        urdf_parsed_once: None,
        arm_rendered_through_handoff: None,
        fk_matches_rapier: None,
        joint_limits_enforced: None,
        arm_links_above_floor: None,
        multi_urdf_arms_loaded: None,
        per_arm_fk_consistency: None,
    }
}

/// Records the P2 deterministic-rerun assertion in an artifact trace.
pub fn record_determinism_hash_stability(trace: &mut WorldSmokeTrace, repeat_hash: String) {
    let canonical_hash = trace.determinism_trace_hash.clone();
    trace.assertions.determinism_hash_stable = Some(DeterminismHashStableAssertion {
        ok: canonical_hash == repeat_hash,
        canonical_hash,
        repeat_hash,
    });
}

/// Computes the deterministic trace hash used by the smoke artifact.
pub fn determinism_trace_hash(per_tick_trace: &[WorldTickTrace]) -> anyhow::Result<String> {
    let bytes = serde_json::to_vec(per_tick_trace)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(hex_lower(&hasher.finalize()))
}

fn multi_joint_trace(per_tick_trace: &[WorldTickTrace]) -> WorldJointTrace {
    let joint_create_ticks = per_tick_trace
        .iter()
        .filter(|tick| {
            tick.tick_events
                .iter()
                .any(|event| event.starts_with("joint_create("))
        })
        .map(|tick| tick.tick)
        .collect();
    let joint_destroy_ticks = per_tick_trace
        .iter()
        .filter(|tick| {
            tick.tick_events
                .iter()
                .any(|event| event.starts_with("joint_destroy("))
        })
        .map(|tick| tick.tick)
        .collect();
    WorldJointTrace {
        joint_create_ticks,
        joint_destroy_ticks,
        active_during_ticks: None,
        active_by_tick_summary: Some(active_joint_summary(per_tick_trace)),
    }
}

fn active_joint_summary(per_tick_trace: &[WorldTickTrace]) -> WorldJointActiveSummary {
    let mut ticks_with_zero_joints = 0;
    let mut ticks_with_one_joint = 0;
    let mut ticks_with_two_joints = 0;
    for tick in per_tick_trace {
        match tick.active_joints.len() {
            0 => ticks_with_zero_joints += 1,
            1 => ticks_with_one_joint += 1,
            _ => ticks_with_two_joints += 1,
        }
    }
    WorldJointActiveSummary {
        ticks_with_zero_joints,
        ticks_with_one_joint,
        ticks_with_two_joints,
    }
}

fn find_handoff_tick(per_tick_trace: &[WorldTickTrace]) -> Option<u32> {
    per_tick_trace
        .iter()
        .find(|tick| {
            tick.tick_events
                .iter()
                .any(|event| event == "joint_destroy(carrier_a, workpiece)")
                && tick
                    .tick_events
                    .iter()
                    .any(|event| event == "joint_create(carrier_b, workpiece)")
        })
        .map(|tick| tick.tick)
}

fn ownership_transfer_atomic(per_tick_trace: &[WorldTickTrace], handoff_tick: Option<u32>) -> bool {
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
        == Some("carrier_a")
        && at
            .and_then(|tick| tick.ownership.as_ref())
            .and_then(|ownership| ownership.owner.as_deref())
            == Some("carrier_b")
}

fn phantom_carry_violation_count(per_tick_trace: &[WorldTickTrace]) -> u32 {
    let mut violations = 0;
    for tick in per_tick_trace {
        for state in &tick.actuator_states {
            let joint_name = match state.name.as_str() {
                "carrier_a" => P2Owner::CarrierA.joint_name(),
                "carrier_b" => P2Owner::CarrierB.joint_name(),
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

fn workpiece_falls_monotonically_until_contact(
    per_tick_trace: &[WorldTickTrace],
    first_contact_tick: Option<u32>,
) -> bool {
    let mut previous_y = None;
    for tick in per_tick_trace {
        if Some(tick.tick) == first_contact_tick {
            break;
        }
        if let Some(previous) = previous_y {
            if tick.workpiece.y > previous + CONTACT_SETTLE_EPSILON {
                return false;
            }
        }
        previous_y = Some(tick.workpiece.y);
    }
    true
}

fn contact_contains_cube_floor(contacts: &[WorldContactTrace]) -> bool {
    contacts
        .iter()
        .any(|contact| contact.a == "cube" && contact.b == "floor")
}

fn contact_contains(contacts: &[WorldContactTrace], a: &str, b: &str) -> bool {
    contacts
        .iter()
        .any(|contact| contact.a == a && contact.b == b || contact.a == b && contact.b == a)
}

fn contact(a: &str, b: &str) -> WorldContactTrace {
    WorldContactTrace {
        a: a.to_string(),
        b: b.to_string(),
    }
}

fn body_registration(name: &str, kind: &str, shape: &str) -> WorldBodyRegistrationTrace {
    WorldBodyRegistrationTrace {
        name: name.to_string(),
        kind: kind.to_string(),
        shape: shape.to_string(),
        source: None,
    }
}

fn fixture_penetration(center: [f32; 3], half_xz: f32, half_y: f32) -> f32 {
    let overlap_x = half_xz + FIXTURE_HALF_XZ - (center[0] - FIXTURE_CENTER_X).abs();
    let overlap_y = half_y + FIXTURE_HALF_Y - (center[1] - FIXTURE_CENTER_Y).abs();
    let overlap_z = half_xz + FIXTURE_HALF_XZ - (center[2] - FIXTURE_CENTER_Z).abs();
    if overlap_x > 0.0 && overlap_y > 0.0 && overlap_z > 0.0 {
        overlap_x.min(overlap_y).min(overlap_z)
    } else {
        0.0
    }
}

fn vec3_length(value: [f32; 3]) -> f32 {
    value[0]
        .mul_add(value[0], value[1].mul_add(value[1], value[2] * value[2]))
        .sqrt()
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}
