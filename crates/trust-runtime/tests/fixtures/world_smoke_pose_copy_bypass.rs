fn copy_carrier_pose_to_workpiece(
    carrier: &rapier3d::prelude::RigidBody,
    workpiece: &mut rapier3d::prelude::RigidBody,
) {
    workpiece.set_position(*carrier.position(), true);
}
