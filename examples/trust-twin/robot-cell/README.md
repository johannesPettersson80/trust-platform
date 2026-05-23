# Trust Twin Robot Cell

This example is the first canonical moving robot-cell digital twin. It uses
the Q-H robot-cell kinematics decision and `asset_state = "procedural_robot"`
for the 3D view, while the PLC-facing robot surface is the generated
`Robot_P3MinimalArm` function block from `robot/p3-minimal-arm.robot.toml`.

`src/main.st` owns the pick-and-place command state and calls the generated
robot FB. The native FB body is implemented through
`trust_runtime::world::arm::step_robot_p3_minimal_arm_bridge`, so the example
crosses the product-layer seam without adding any scene transform writer. The
3D view still binds shoulder, elbow, wrist, gripper, box, and status-light
values to `PROGRAM Main` variables; local time or CSS animation must not drive
the canonical motion proof.

Review order:

1. Run the focused robot-cell tests.
2. Run the Playwright capture and frame-diff proof against the live scene.
3. Record the assistant visual review verdict in the gate artifact.
4. Request Johannes visual review only after the assistant visual review passes.

Johannes visual review should confirm that a recognizable robot arm moves
through the pick-and-place sequence and that the movement is acceptable for the
canonical trust-twin proof.
