(* trust-twin-generated: robot function block v1 *)
(* model: P3MinimalArm *)
(* manifest_hash: sha256:9236a9ea889aa5615171e7fc0bbadf3539e12216c5f7789639905366b7f63375 *)
(* urdf: crates/trust-runtime/tests/fixtures/p3_minimal_arm.urdf *)
(* base_frame: base, tool_frame: tool, grip_frame: workpiece.grip *)
(* joints: base_to_link_1->Joint1, link_1_to_link_2->Joint2 *)
(* native_bridge: trust_runtime::world::arm::step_robot_p3_minimal_arm_bridge *)
(* FK and URDF-derived poses are verified in trust_runtime::world::arm; this FB does not write transforms. *)

FUNCTION_BLOCK Robot_P3MinimalArm
VAR_INPUT
    Enable : BOOL;
    Command : INT;
END_VAR
VAR_OUTPUT
    EnabledOut : BOOL;
    Busy : BOOL;
    Done : BOOL;
    Fault : BOOL;
    State : INT;
    Owner : INT;
    HasWorkpiece : BOOL;
    GripperOpen : BOOL;
    StatusLight : BOOL;
    Joint1 : REAL;
    Joint2 : REAL;
    ToolYaw : REAL;
    ToolX : REAL;
    ToolY : REAL;
    ToolZ : REAL;
    WorkpieceX : REAL;
    WorkpieceY : REAL;
    WorkpieceZ : REAL;
END_VAR
END_FUNCTION_BLOCK
