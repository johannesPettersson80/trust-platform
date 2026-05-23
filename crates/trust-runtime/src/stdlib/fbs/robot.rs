use crate::error::RuntimeError;
use crate::memory::InstanceId;
use crate::value::Value;
use crate::world::arm::{step_robot_p3_minimal_arm_bridge, RobotP3MinimalArmBridgeInput};

use super::{instance, BuiltinExecContext};

pub(super) fn exec_robot_p3_minimal_arm(
    ctx: &mut BuiltinExecContext<'_>,
    instance_id: InstanceId,
) -> Result<(), RuntimeError> {
    let output = step_robot_p3_minimal_arm_bridge(RobotP3MinimalArmBridgeInput {
        enable: instance::read_bool(ctx, instance_id, "Enable")?,
        command: read_int(ctx, instance_id, "Command")?,
    });

    instance::write_bool(ctx, instance_id, "EnabledOut", output.enabled_out);
    instance::write_bool(ctx, instance_id, "Busy", output.busy);
    instance::write_bool(ctx, instance_id, "Done", output.done);
    instance::write_bool(ctx, instance_id, "Fault", output.fault);
    write_int(ctx, instance_id, "State", output.state);
    write_int(ctx, instance_id, "Owner", output.owner);
    instance::write_bool(ctx, instance_id, "HasWorkpiece", output.has_workpiece);
    instance::write_bool(ctx, instance_id, "GripperOpen", output.gripper_open);
    instance::write_bool(ctx, instance_id, "StatusLight", output.status_light);
    write_real(ctx, instance_id, "Joint1", output.joint1);
    write_real(ctx, instance_id, "Joint2", output.joint2);
    write_real(ctx, instance_id, "ToolYaw", output.tool_yaw);
    write_real(ctx, instance_id, "ToolX", output.tool_position[0]);
    write_real(ctx, instance_id, "ToolY", output.tool_position[1]);
    write_real(ctx, instance_id, "ToolZ", output.tool_position[2]);
    write_real(ctx, instance_id, "WorkpieceX", output.workpiece_position[0]);
    write_real(ctx, instance_id, "WorkpieceY", output.workpiece_position[1]);
    write_real(ctx, instance_id, "WorkpieceZ", output.workpiece_position[2]);
    Ok(())
}

fn read_int(
    ctx: &BuiltinExecContext<'_>,
    instance_id: InstanceId,
    name: &str,
) -> Result<i16, RuntimeError> {
    match instance::read_value_or_null(ctx, instance_id, name) {
        Value::Int(value) => Ok(value),
        Value::DInt(value) => i16::try_from(value).map_err(|_| RuntimeError::TypeMismatch),
        Value::Null => Ok(0),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn write_int(ctx: &mut BuiltinExecContext<'_>, instance_id: InstanceId, name: &str, value: i16) {
    instance::set_instance_value(ctx, instance_id, name, Value::Int(value));
}

fn write_real(ctx: &mut BuiltinExecContext<'_>, instance_id: InstanceId, name: &str, value: f32) {
    instance::set_instance_value(ctx, instance_id, name, Value::Real(value));
}
