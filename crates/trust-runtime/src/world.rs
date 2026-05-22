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
        })
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

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}
