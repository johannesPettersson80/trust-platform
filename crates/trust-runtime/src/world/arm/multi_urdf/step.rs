use super::*;

pub(super) fn p4_apply_motors(
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

pub(super) fn p4_target_for_arm(
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

pub(super) fn drive_p4_arm_joints(world: &mut World, bodies: P4ArmBodies, target: [f32; 2]) {
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

pub(super) fn p4_update_tool_colliders(
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

pub(super) fn step_p4_arms(
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

pub(super) fn step_p4_offeror(
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
pub(super) fn step_p4_receiver(
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

pub(super) fn p4_handoff_ready(
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

pub(super) fn p4_perform_atomic_handoff(
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

pub(super) fn p4_create_owned_workpiece_joint(
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

pub(super) fn p4_create_tool_workpiece_joint(
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
