fn teleport_workpiece(workpiece: &mut rapier3d::prelude::RigidBody) {
    workpiece.set_translation(rapier3d::prelude::vector![2.0, 1.0, 0.0].into(), true);
}
