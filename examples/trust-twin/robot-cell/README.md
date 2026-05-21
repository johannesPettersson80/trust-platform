# Trust Twin Robot Cell

This example is the first canonical moving robot-cell digital twin. It uses
the Q-H robot-cell kinematics decision and `asset_state = "packaged_asset"`.
The proof loads externally sourced packaged meshes through Scena: Universal
Robots UR10 visual meshes converted from COLLADA to glTF, Schunk WSG-50 gripper
meshes, a Drake manipulation table mesh, and YCB object meshes. UR10 source:
`https://github.com/UniversalRobots/Universal_Robots_ROS2_Description/tree/18dea90dedf24adea05d920d0d441e4523a41e50`.
Supporting cell asset source:
`https://github.com/RobotLocomotion/models/tree/5c942636d18013870403c17c8209558799122abd`.
Licenses: BSD-3-Clause for the UR10, Schunk WSG-50, and Drake manipulation
table meshes; CC-BY-4.0 for the YCB object meshes. Original authors: UR10
meshes from the Universal Robots ROS2 Description authors/Universal Robots A/S;
Schunk WSG-50 meshes by Toyota Research Institute; table meshes by the Robot
Locomotion Group at CSAIL; workpiece/beacon meshes from the YCB Object and
Model Set. Imported package SHA256 manifest:
`99dfc722f035472e684122fa8e77c3361242c103f7fe52dfacfaf845d1b470e5`.
Asset package version: Universal Robots ROS2 Description commit
18dea90dedf24adea05d920d0d441e4523a41e50 and RobotLocomotion/models commit
5c942636d18013870403c17c8209558799122abd imported 2026-05-16.

`src/main.st` owns the pick-and-place command state. The 3D view binds shoulder,
elbow, wrist, gripper, box, and status-light values to `PROGRAM Main` variables;
local time or CSS animation must not drive the canonical motion proof.

Review order:

1. Run the focused robot-cell tests.
2. Run the Playwright capture and frame-diff proof against the live scene.
3. Record the assistant visual review verdict in the gate artifact.
4. Request Johannes visual review only after the assistant visual review passes.

Johannes visual review should confirm that a recognizable robot arm moves
through the pick-and-place sequence and that the movement is acceptable for the
canonical trust-twin proof.
