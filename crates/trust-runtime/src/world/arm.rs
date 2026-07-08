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
    CarryConstraintAssertion, ContactFiredAssertion, DeterminismHashStableAssertion,
    ExclusiveOwnershipAssertion, FixtureInterpenetrationAssertion, GravityAppliedAssertion,
    GripEventContactAssertion, HandoffOrderAssertion, NoPhantomCarryAssertion,
    OwnershipTransferAtomicAssertion, ReleaseDestroyedJointAssertion, TransformHandoffTrace,
    WorkpieceSettledAssertion, World, WorldAbstractionTrace, WorldActuatorStateSample,
    WorldActuatorTrace, WorldActuatorTransitionTrace, WorldBodyRegistrationTrace,
    WorldContactTrace, WorldHandoffPairTrace, WorldHandoffPlanTrace, WorldJointDistanceTrace,
    WorldJointTrace, WorldOwnershipFaultTrace, WorldOwnershipSample, WorldOwnershipTrace,
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

mod model;
mod multi_urdf;
mod p3_bridge;
mod physics;
mod types;
mod urdf;

pub use multi_urdf::{
    assert_world_multi_urdf_arm_smoke_trace, record_multi_urdf_arm_determinism_hash_stability,
    run_world_multi_urdf_arm_smoke,
};
pub use p3_bridge::{
    step_robot_p3_minimal_arm_bridge, RobotP3MinimalArmBridgeInput, RobotP3MinimalArmBridgeOutput,
};
pub use types::{
    ArmAboveFloorAssertion, ArmRenderedThroughHandoffAssertion, FkConsistencyAssertion,
    JointLimitAssertion, MultiUrdfArmsLoadedAssertion, PerArmFkConsistencyAssertion,
    UrdfParsedOnceAssertion, WorldArmJointTrace, WorldArmLinkTrace, WorldFkArmVerifierTrace,
    WorldFkVerifierTrace, WorldMultiUrdfArmScenario, WorldMultiUrdfArmSmokeConfig,
    WorldUrdfArmInstanceTrace, WorldUrdfArmScenario, WorldUrdfArmSmokeConfig, WorldUrdfJointTrace,
    WorldUrdfTrace,
};
pub use urdf::{
    assert_world_urdf_arm_smoke_trace, record_urdf_arm_determinism_hash_stability,
    run_world_urdf_arm_smoke,
};
