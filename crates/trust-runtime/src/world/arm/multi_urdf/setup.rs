use super::*;

pub(super) fn register_p4_environment(
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

pub(super) fn register_p4_arm_instance(
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
