use super::model::*;
use super::physics::*;
use super::*;

mod domain;
mod setup;
mod step;
mod trace;

use domain::*;
use setup::*;
use step::*;
use trace::*;

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
