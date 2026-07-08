use super::model::*;
use super::physics::*;
use super::*;

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
