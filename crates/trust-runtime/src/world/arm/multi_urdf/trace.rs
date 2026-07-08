use super::*;

pub(super) fn trace_p4_tick(
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

pub(super) fn trace_p4_arm(
    world: &World,
    arm: &P4ArmInstance,
) -> anyhow::Result<(Vec<WorldArmLinkTrace>, Vec<WorldArmJointTrace>)> {
    let actual_joints = read_p4_joint_positions(world, arm.bodies);
    let fk_positions = compute_fk_for_chain_at(&arm.model, actual_joints, arm.base_world)?;
    let links = trace_p4_arm_links(world, arm, &fk_positions)?;
    let joints = trace_arm_joints(&arm.model, actual_joints);
    Ok((links, joints))
}

pub(super) fn trace_p4_arm_links(
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

pub(super) fn p4_contacts(
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

pub(super) fn p4_apply_handoff(
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

pub(super) fn read_p4_joint_positions(world: &World, bodies: P4ArmBodies) -> [f32; 2] {
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

pub(super) fn p4_arm_tool_world_point(
    world: &World,
    bodies: P4ArmBodies,
) -> anyhow::Result<Vector> {
    let tool = world
        .bodies
        .get(bodies.tool_body)
        .ok_or_else(|| anyhow::anyhow!("P4 arm tool body is missing"))?;
    Ok(tool.position().translation + (*tool.rotation() * v3(0.0, -0.10, 0.0)))
}

pub(super) fn p4_workpiece_grip_world_point(
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

pub(super) fn p4_arm_workpiece_joint_distance(
    world: &World,
    env: P4EnvironmentBodies,
    arm: P4ArmBodies,
) -> anyhow::Result<f32> {
    Ok(
        (p4_arm_tool_world_point(world, arm)? - p4_workpiece_grip_world_point(world, env)?)
            .length(),
    )
}

pub(super) fn zero_p4_workpiece_velocity(world: &mut World, env: P4EnvironmentBodies) {
    if let Some(workpiece) = world.bodies.get_mut(env.workpiece_body) {
        workpiece.set_linvel(Vector::ZERO, true);
        workpiece.set_angvel(Vector::ZERO, true);
    }
}

pub(super) fn zero_p4_arm_and_workpiece_velocity(
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

pub(super) fn p4_actuator_traces(arms: &[P4ArmInstance]) -> Vec<WorldActuatorTrace> {
    let mut traces = arms.iter().map(P4ArmInstance::trace).collect::<Vec<_>>();
    traces.sort_by_key(|trace| trace.id.unwrap_or(u8::MAX));
    traces
}

pub(super) fn p4_urdf_trace(arms: &[P4ArmInstance]) -> WorldUrdfTrace {
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

pub(super) fn p4_fk_verifier_trace(
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

pub(super) fn p4_body_registrations(include_floor: bool) -> Vec<WorldBodyRegistrationTrace> {
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

pub(super) fn p4_joint_trace(per_tick_trace: &[WorldTickTrace]) -> WorldJointTrace {
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

pub(super) fn p4_expected_handoff_event_order() -> Vec<String> {
    vec![
        "joint_destroy(arm_a.tool, workpiece)".to_string(),
        "joint_create(arm_b.tool, workpiece)".to_string(),
        "state_transition(arm_a: Held -> Idle)".to_string(),
        "state_transition(arm_b: AcceptingHandoff -> Carrying)".to_string(),
    ]
}

pub(super) fn p4_find_handoff_tick(per_tick_trace: &[WorldTickTrace]) -> Option<u32> {
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

pub(super) fn p4_ownership_transfer_atomic(
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

pub(super) fn p4_phantom_carry_violation_count(per_tick_trace: &[WorldTickTrace]) -> u32 {
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

pub(super) fn p4_per_arm_fk_stats(
    per_tick_trace: &[WorldTickTrace],
) -> (bool, BTreeMap<String, f32>) {
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

pub(super) fn p4_joint_limit_stats(per_tick_trace: &[WorldTickTrace]) -> JointLimitStats {
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

pub(super) fn p4_arm_floor_stats(per_tick_trace: &[WorldTickTrace]) -> ArmFloorStats {
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
