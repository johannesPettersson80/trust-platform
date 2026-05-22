//! Shared deterministic simulation world primitives.

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
    /// Path to the initial screenshot.
    pub screenshot_t0_png: String,
    /// Path to the settled screenshot.
    pub screenshot_t_n_png: String,
    /// Path to the P1 initial screenshot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot_t_initial_png: Option<String>,
    /// Path to the P1 grip screenshot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot_t_grip_png: Option<String>,
    /// Path to the P1 carry screenshot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot_t_carry_png: Option<String>,
    /// Path to the P1 release screenshot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot_t_release_png: Option<String>,
    /// P1 actuator trace, present only for the workpiece/fixture proof.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actuator: Option<WorldActuatorTrace>,
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
    /// Cube bottom Y, measured against floor top Y.
    pub cube_y: f32,
    /// Cube center Y, used by the renderer node transform.
    pub cube_center_y: f32,
    /// Cube vertical velocity.
    pub cube_vy: f32,
    /// Active contact pairs by stable logical name.
    pub contacts: Vec<WorldContactTrace>,
    /// Carrier body sample, present in the P1 proof.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub carrier: Option<WorldBodyKinematicsTrace>,
    /// Workpiece body sample, present in the P1 proof.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workpiece: Option<WorldBodyKinematicsTrace>,
    /// Actuator state, present in the P1 proof.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actuator_state: Option<ActuatorState>,
    /// Active physics joints by stable logical name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub active_joints: Vec<String>,
    /// Distance between carrier tool point and workpiece grip frame.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub joint_distance: Option<f32>,
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
    /// Cube never penetrates below the floor.
    pub cube_above_floor: CubeAboveFloorAssertion,
    /// Gravity accelerated the cube before contact.
    pub gravity_applied: GravityAppliedAssertion,
    /// A cube/floor contact pair was produced.
    pub contact_fired: ContactFiredAssertion,
    /// Workpiece never moves below the floor in the P1 proof.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workpiece_above_floor: Option<CubeAboveFloorAssertion>,
    /// Carrier never moves below the floor in the P1 proof.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub carrier_above_floor: Option<CubeAboveFloorAssertion>,
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

/// Typed actuator state for P1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActuatorState {
    /// Not carrying a workpiece.
    Idle,
    /// Moving toward the workpiece.
    Approaching,
    /// Carrier/workpiece fixed joint is expected to be active.
    Carrying,
    /// Joint has been destroyed and the workpiece is settling.
    Releasing,
}

/// P1 actuator trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldActuatorTrace {
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
}

/// Result for the above-floor invariant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CubeAboveFloorAssertion {
    /// Whether the assertion passed.
    pub ok: bool,
    /// Minimum cube bottom Y observed.
    pub min_cube_y: f32,
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
            ActuatorState::Idle | ActuatorState::Releasing => {
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
            ActuatorState::Idle => {}
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
        let cube_center_y = cube.position().translation.y;
        let cube_y = cube_center_y - CUBE_HALF_EXTENT;
        let cube_vy = cube.linvel().y;
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
            cube_y,
            cube_center_y,
            cube_vy,
            contacts,
            carrier: None,
            workpiece: None,
            actuator_state: None,
            active_joints: Vec::new(),
            joint_distance: None,
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
            cube_y: workpiece.y,
            cube_center_y: workpiece.center[1],
            cube_vy: workpiece.vy,
            contacts,
            carrier: Some(carrier),
            workpiece: Some(workpiece),
            actuator_state: Some(actuator.state),
            active_joints,
            joint_distance,
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
        screenshot_t0_png: "target/gate-artifacts/world_smoke_t0.png".to_string(),
        screenshot_t_n_png: "target/gate-artifacts/world_smoke_tN.png".to_string(),
        screenshot_t_initial_png: None,
        screenshot_t_grip_png: None,
        screenshot_t_carry_png: None,
        screenshot_t_release_png: None,
        actuator: None,
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
        screenshot_t0_png: "target/gate-artifacts/world_smoke_t_initial.png".to_string(),
        screenshot_t_n_png: "target/gate-artifacts/world_smoke_t_release.png".to_string(),
        screenshot_t_initial_png: Some(
            "target/gate-artifacts/world_smoke_t_initial.png".to_string(),
        ),
        screenshot_t_grip_png: Some("target/gate-artifacts/world_smoke_t_grip.png".to_string()),
        screenshot_t_carry_png: Some("target/gate-artifacts/world_smoke_t_carry.png".to_string()),
        screenshot_t_release_png: Some(
            "target/gate-artifacts/world_smoke_t_release.png".to_string(),
        ),
        actuator: Some(actuator.trace()),
        joints: Some(actuator.joint_trace()),
        per_tick_trace,
        determinism_trace_hash,
        assertions,
    })
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
    let min_cube_y = per_tick_trace
        .iter()
        .map(|tick| tick.cube_y)
        .fold(f32::INFINITY, f32::min);
    let first_contact_tick = per_tick_trace
        .iter()
        .find(|tick| contact_contains_cube_floor(&tick.contacts))
        .map(|tick| tick.tick);
    let max_downward_velocity_before_contact = per_tick_trace
        .iter()
        .take_while(|tick| Some(tick.tick) != first_contact_tick)
        .map(|tick| tick.cube_vy)
        .fold(0.0, f32::min);
    let monotonically_falling_until_contact =
        cube_falls_monotonically_until_contact(per_tick_trace, first_contact_tick);
    WorldSmokeAssertions {
        cube_above_floor: CubeAboveFloorAssertion {
            ok: min_cube_y >= FLOOR_Y - ABOVE_FLOOR_EPSILON,
            min_cube_y,
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
        workpiece_above_floor: None,
        carrier_above_floor: None,
        no_fixture_interpenetration: None,
        grip_event_has_contact: None,
        carry_constraint_driven: None,
        release_destroyed_joint: None,
        workpiece_settled_on_fixture: None,
    }
}

/// Computes P1 workpiece/fixture/actuator proof assertions from a trace.
#[must_use]
pub fn assert_world_actuator_smoke_trace(
    per_tick_trace: &[WorldTickTrace],
) -> WorldSmokeAssertions {
    let workpiece_min_y = per_tick_trace
        .iter()
        .filter_map(|tick| tick.workpiece.as_ref().map(|body| body.y))
        .fold(f32::INFINITY, f32::min);
    let carrier_min_y = per_tick_trace
        .iter()
        .filter_map(|tick| tick.carrier.as_ref().map(|body| body.y))
        .fold(f32::INFINITY, f32::min);
    let max_downward_velocity = per_tick_trace
        .iter()
        .filter_map(|tick| tick.workpiece.as_ref().map(|body| body.vy))
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
        let workpiece = tick.workpiece.as_ref().map_or(0.0, |body| {
            fixture_penetration(body.center, WORKPIECE_HALF_EXTENT, WORKPIECE_HALF_EXTENT)
        });
        let carrier = tick.carrier.as_ref().map_or(0.0, |body| {
            fixture_penetration(body.center, CARRIER_HALF_XZ, CARRIER_HALF_Y)
        });
        current.max(workpiece).max(carrier)
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
            ok: carrier_min_y >= FLOOR_Y - ABOVE_FLOOR_EPSILON,
            min_cube_y: carrier_min_y,
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
    }
}

/// Computes the deterministic trace hash used by the smoke artifact.
pub fn determinism_trace_hash(per_tick_trace: &[WorldTickTrace]) -> anyhow::Result<String> {
    let bytes = serde_json::to_vec(per_tick_trace)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(hex_lower(&hasher.finalize()))
}

fn cube_falls_monotonically_until_contact(
    per_tick_trace: &[WorldTickTrace],
    first_contact_tick: Option<u32>,
) -> bool {
    let mut previous_y = None;
    for tick in per_tick_trace {
        if Some(tick.tick) == first_contact_tick {
            break;
        }
        if let Some(previous) = previous_y {
            if tick.cube_y > previous + CONTACT_SETTLE_EPSILON {
                return false;
            }
        }
        previous_y = Some(tick.cube_y);
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

fn body_registration(name: &str, kind: &str, shape: &str) -> WorldBodyRegistrationTrace {
    WorldBodyRegistrationTrace {
        name: name.to_string(),
        kind: kind.to_string(),
        shape: shape.to_string(),
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
