/// Input contract for the generated `Robot_P3MinimalArm` native FB bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RobotP3MinimalArmBridgeInput {
    /// Whether the generated robot FB is enabled by PLC code.
    pub enable: bool,
    /// Command/state id requested by PLC code.
    pub command: i16,
}

/// Output contract produced by the generated `Robot_P3MinimalArm` native FB bridge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RobotP3MinimalArmBridgeOutput {
    /// Echo of the enabled state after native bridge evaluation.
    pub enabled_out: bool,
    /// Robot is moving or holding a workpiece.
    pub busy: bool,
    /// Command has reached a stable terminal presentation state.
    pub done: bool,
    /// Command was rejected by the bridge contract.
    pub fault: bool,
    /// State id emitted to PLC/HMI outputs.
    pub state: i16,
    /// Workpiece owner id; zero means unowned, one means this robot owns it.
    pub owner: i16,
    /// Whether the tool currently owns the workpiece.
    pub has_workpiece: bool,
    /// Whether the gripper jaws are open.
    pub gripper_open: bool,
    /// Status-light output for the sample HMI.
    pub status_light: bool,
    /// First URDF revolute joint presentation angle.
    pub joint1: f32,
    /// Second URDF revolute joint presentation angle.
    pub joint2: f32,
    /// Tool yaw presentation angle.
    pub tool_yaw: f32,
    /// Tool center presentation position.
    pub tool_position: [f32; 3],
    /// Workpiece presentation position.
    pub workpiece_position: [f32; 3],
}

/// Executes the writer-free product bridge for the generated P3 minimal-arm FB.
///
/// The bridge intentionally returns typed PLC/HMI outputs only. It does not
/// write Rapier bodies, scene nodes, FK results, or visible transforms; the
/// physics proofs keep those ownership paths in the shared [`World`].
#[must_use]
pub fn step_robot_p3_minimal_arm_bridge(
    input: RobotP3MinimalArmBridgeInput,
) -> RobotP3MinimalArmBridgeOutput {
    if !input.enable {
        return robot_p3_minimal_arm_disabled_output();
    }

    match input.command {
        0 => robot_p3_minimal_arm_output(RobotP3MinimalArmPose {
            state: 0,
            busy: false,
            done: true,
            has_workpiece: false,
            gripper_open: true,
            joints: [-0.60, 0.35],
            tool_yaw: 0.25,
            tool_position: [0.0, 0.80, 0.0],
            workpiece_position: [0.0, 0.35, 0.0],
        }),
        1 => robot_p3_minimal_arm_output(RobotP3MinimalArmPose {
            state: 1,
            busy: true,
            done: false,
            has_workpiece: false,
            gripper_open: true,
            joints: [-0.72, 0.42],
            tool_yaw: 0.25,
            tool_position: [0.0, 0.80, 0.0],
            workpiece_position: [0.0, 0.35, 0.0],
        }),
        2 => robot_p3_minimal_arm_output(RobotP3MinimalArmPose {
            state: 2,
            busy: true,
            done: false,
            has_workpiece: true,
            gripper_open: false,
            joints: [-0.72, 0.42],
            tool_yaw: 0.25,
            tool_position: [0.0, 0.55, 0.0],
            workpiece_position: [0.0, 0.35, 0.0],
        }),
        3 => robot_p3_minimal_arm_output(RobotP3MinimalArmPose {
            state: 3,
            busy: true,
            done: false,
            has_workpiece: true,
            gripper_open: false,
            joints: [-0.35, 0.72],
            tool_yaw: -0.45,
            tool_position: [0.80, 1.15, 0.0],
            workpiece_position: [0.80, 1.15, 0.0],
        }),
        4 => robot_p3_minimal_arm_output(RobotP3MinimalArmPose {
            state: 4,
            busy: true,
            done: false,
            has_workpiece: true,
            gripper_open: false,
            joints: [0.35, 0.70],
            tool_yaw: -0.85,
            tool_position: [2.40, 1.15, 0.0],
            workpiece_position: [2.40, 1.15, 0.0],
        }),
        5 => robot_p3_minimal_arm_output(RobotP3MinimalArmPose {
            state: 5,
            busy: true,
            done: false,
            has_workpiece: true,
            gripper_open: false,
            joints: [0.72, 0.40],
            tool_yaw: 0.85,
            tool_position: [4.0, 0.55, 0.0],
            workpiece_position: [4.0, 0.35, 0.0],
        }),
        6 => robot_p3_minimal_arm_output(RobotP3MinimalArmPose {
            state: 6,
            busy: false,
            done: true,
            has_workpiece: false,
            gripper_open: true,
            joints: [0.72, 0.40],
            tool_yaw: 0.85,
            tool_position: [4.0, 0.80, 0.0],
            workpiece_position: [4.0, 0.35, 0.0],
        }),
        7 => robot_p3_minimal_arm_output(RobotP3MinimalArmPose {
            state: 7,
            busy: false,
            done: true,
            has_workpiece: false,
            gripper_open: true,
            joints: [-0.15, 0.55],
            tool_yaw: 0.0,
            tool_position: [2.0, 1.20, 0.0],
            workpiece_position: [4.0, 0.35, 0.0],
        }),
        rejected => RobotP3MinimalArmBridgeOutput {
            fault: true,
            state: rejected,
            ..robot_p3_minimal_arm_disabled_output()
        },
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct RobotP3MinimalArmPose {
    pub(super) state: i16,
    pub(super) busy: bool,
    pub(super) done: bool,
    pub(super) has_workpiece: bool,
    pub(super) gripper_open: bool,
    pub(super) joints: [f32; 2],
    pub(super) tool_yaw: f32,
    pub(super) tool_position: [f32; 3],
    pub(super) workpiece_position: [f32; 3],
}

pub(super) fn robot_p3_minimal_arm_disabled_output() -> RobotP3MinimalArmBridgeOutput {
    RobotP3MinimalArmBridgeOutput {
        enabled_out: false,
        busy: false,
        done: false,
        fault: false,
        state: 0,
        owner: 0,
        has_workpiece: false,
        gripper_open: true,
        status_light: false,
        joint1: -0.60,
        joint2: 0.35,
        tool_yaw: 0.25,
        tool_position: [0.0, 0.80, 0.0],
        workpiece_position: [0.0, 0.35, 0.0],
    }
}

pub(super) fn robot_p3_minimal_arm_output(
    pose: RobotP3MinimalArmPose,
) -> RobotP3MinimalArmBridgeOutput {
    RobotP3MinimalArmBridgeOutput {
        enabled_out: true,
        busy: pose.busy,
        done: pose.done,
        fault: false,
        state: pose.state,
        owner: if pose.has_workpiece { 1 } else { 0 },
        has_workpiece: pose.has_workpiece,
        gripper_open: pose.gripper_open,
        status_light: pose.has_workpiece,
        joint1: pose.joints[0],
        joint2: pose.joints[1],
        tool_yaw: pose.tool_yaw,
        tool_position: pose.tool_position,
        workpiece_position: pose.workpiece_position,
    }
}
