use super::model::*;
use super::*;

#[derive(Debug, Clone, Copy)]
pub(super) struct WorldUrdfArmBodies {
    pub(super) floor_collider: Option<ColliderHandle>,
    pub(super) fixture_collider: ColliderHandle,
    pub(super) workpiece_body: RigidBodyHandle,
    pub(super) workpiece_collider: ColliderHandle,
    pub(super) link_1_body: RigidBodyHandle,
    pub(super) link_1_collider: ColliderHandle,
    pub(super) link_1_joint: MultibodyJointHandle,
    pub(super) link_2_body: RigidBodyHandle,
    pub(super) link_2_collider: ColliderHandle,
    pub(super) link_2_joint: MultibodyJointHandle,
    pub(super) tool_body: RigidBodyHandle,
    pub(super) tool_collider: ColliderHandle,
}

#[derive(Debug, Clone)]
pub(super) struct WorldUrdfArmActuator {
    pub(super) state: ActuatorState,
    pub(super) workpiece_joint: Option<ImpulseJointHandle>,
    pub(super) transitions: Vec<WorldActuatorTransitionTrace>,
    pub(super) joint_create_ticks: Vec<u32>,
    pub(super) joint_destroy_ticks: Vec<u32>,
}

impl WorldUrdfArmActuator {
    pub(super) fn new() -> Self {
        Self {
            state: ActuatorState::Idle,
            workpiece_joint: None,
            transitions: Vec::new(),
            joint_create_ticks: Vec::new(),
            joint_destroy_ticks: Vec::new(),
        }
    }

    pub(super) fn start(&mut self) {
        self.transition(0, ActuatorState::Approaching, "start");
    }

    pub(super) fn transition(&mut self, tick: u32, to: ActuatorState, trigger: &str) {
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

    pub(super) fn trace(&self) -> WorldActuatorTrace {
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

pub(super) fn register_urdf_arm_smoke_bodies(
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

pub(super) fn insert_arm_link_body(
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

pub(super) fn insert_arm_link_collider(
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

pub(super) fn revolute_joint(
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

pub(super) fn drive_arm_joints(
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

pub(super) fn arm_target_for_state(
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

pub(super) fn step_urdf_arm_actuator(
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

pub(super) fn trace_urdf_arm_tick(
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

pub(super) fn urdf_arm_contacts(
    world: &World,
    bodies: WorldUrdfArmBodies,
) -> Vec<WorldContactTrace> {
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

pub(super) fn trace_arm_links(
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

pub(super) fn trace_arm_joints(model: &UrdfArmModel, actual: [f32; 2]) -> Vec<WorldArmJointTrace> {
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

pub(super) fn read_joint_positions(world: &World, bodies: WorldUrdfArmBodies) -> [f32; 2] {
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

pub(super) fn body_yaw(rigid_body: &RigidBody) -> f32 {
    let (_, _, yaw) = rigid_body.rotation().to_euler(EulerRot::XYZ);
    normalize_angle(yaw)
}

pub(super) fn compute_fk_for_chain(
    model: &UrdfArmModel,
    joint_positions: [f32; 2],
) -> anyhow::Result<BTreeMap<&'static str, [f32; 3]>> {
    compute_fk_for_chain_at(model, joint_positions, ARM_BASE_WORLD)
}

pub(super) fn compute_fk_for_chain_at(
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

pub(super) fn arm_tool_workpiece_joint_distance(
    world: &World,
    bodies: WorldUrdfArmBodies,
) -> anyhow::Result<f32> {
    let tool_point = arm_tool_world_point(world, bodies)?;
    let workpiece_grip = workpiece_grip_world_point(world, bodies)?;
    Ok((tool_point - workpiece_grip).length())
}

pub(super) fn create_arm_workpiece_fixed_joint(
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

pub(super) fn set_arm_link_colliders_enabled(
    world: &mut World,
    bodies: WorldUrdfArmBodies,
    enabled: bool,
) {
    if let Some(collider) = world.colliders.get_mut(bodies.tool_collider) {
        collider.set_enabled(enabled);
    }
}

pub(super) fn set_fixture_collider_enabled(
    world: &mut World,
    bodies: WorldUrdfArmBodies,
    enabled: bool,
) {
    if let Some(collider) = world.colliders.get_mut(bodies.fixture_collider) {
        collider.set_enabled(enabled);
    }
}

pub(super) fn set_floor_collider_enabled(
    world: &mut World,
    bodies: WorldUrdfArmBodies,
    enabled: bool,
) {
    if let Some(floor) = bodies.floor_collider {
        if let Some(collider) = world.colliders.get_mut(floor) {
            collider.set_enabled(enabled);
        }
    }
}

pub(super) fn zero_arm_and_workpiece_velocity(world: &mut World, bodies: WorldUrdfArmBodies) {
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

pub(super) fn zero_workpiece_velocity(world: &mut World, bodies: WorldUrdfArmBodies) {
    if let Some(workpiece) = world.bodies.get_mut(bodies.workpiece_body) {
        workpiece.set_linvel(Vector::ZERO, true);
        workpiece.set_angvel(Vector::ZERO, true);
    }
}

pub(super) fn arm_tool_world_point(
    world: &World,
    bodies: WorldUrdfArmBodies,
) -> anyhow::Result<Vector> {
    let tool = world
        .bodies
        .get(bodies.tool_body)
        .ok_or_else(|| anyhow::anyhow!("arm tool body is missing"))?;
    Ok(tool.position().translation + (*tool.rotation() * v3(0.0, -0.10, 0.0)))
}

pub(super) fn workpiece_grip_world_point(
    world: &World,
    bodies: WorldUrdfArmBodies,
) -> anyhow::Result<Vector> {
    let workpiece = world
        .bodies
        .get(bodies.workpiece_body)
        .ok_or_else(|| anyhow::anyhow!("workpiece body is missing"))?;
    Ok(workpiece.position().translation
        + (*workpiece.rotation() * v3(0.0, WORKPIECE_HALF_EXTENT, 0.0)))
}

pub(super) fn arm_fixture_penetration(center: [f32; 3], half_xz: f32, half_y: f32) -> f32 {
    let overlap_x = half_xz + ARM_FIXTURE_HALF_XZ - (center[0] - FIXTURE_CENTER_X).abs();
    let overlap_y = half_y + FIXTURE_HALF_Y - (center[1] - FIXTURE_CENTER_Y).abs();
    let overlap_z = half_xz + ARM_FIXTURE_HALF_XZ - (center[2] - FIXTURE_CENTER_Z).abs();
    if overlap_x > 0.0 && overlap_y > 0.0 && overlap_z > 0.0 {
        overlap_x.min(overlap_y).min(overlap_z)
    } else {
        0.0
    }
}

pub(super) fn urdf_arm_joint_trace(per_tick_trace: &[WorldTickTrace]) -> WorldJointTrace {
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

pub(super) fn urdf_arm_body_registrations(include_floor: bool) -> Vec<WorldBodyRegistrationTrace> {
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

pub(super) fn point_distance(first: [f32; 3], second: [f32; 3]) -> f32 {
    ((first[0] - second[0]).powi(2)
        + (first[1] - second[1]).powi(2)
        + (first[2] - second[2]).powi(2))
    .sqrt()
}

pub(super) fn normalize_angle(mut value: f32) -> f32 {
    while value > PI {
        value -= 2.0 * PI;
    }
    while value < -PI {
        value += 2.0 * PI;
    }
    value
}

pub(super) fn v3(x: f32, y: f32, z: f32) -> Vector {
    vector![x, y, z].into()
}

#[derive(Debug, Clone, Copy)]
pub(super) struct FkStats {
    pub(super) max_distance: f32,
    pub(super) checked_samples: u32,
}

pub(super) fn fk_consistency_stats(per_tick_trace: &[WorldTickTrace]) -> FkStats {
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
pub(super) struct JointLimitStats {
    pub(super) out_of_limit_samples: u32,
    pub(super) clamped_events: Vec<String>,
}

pub(super) fn joint_limit_stats(per_tick_trace: &[WorldTickTrace]) -> JointLimitStats {
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
pub(super) struct ArmFloorStats {
    pub(super) min_y: f32,
    pub(super) min_name: String,
}

pub(super) fn arm_above_floor_stats(per_tick_trace: &[WorldTickTrace]) -> ArmFloorStats {
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
