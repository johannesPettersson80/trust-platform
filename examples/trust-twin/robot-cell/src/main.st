PROGRAM Main
VAR
    RobotMotionStep : INT := 0;
    RobotCycleTick : INT := 0;
    RobotShoulderAngle : REAL := -0.60;
    RobotElbowAngle : REAL := 0.35;
    RobotWristAngle : REAL := 0.25;
    RobotGripperOpen : BOOL := TRUE;
    RobotBoxParentState : INT := 0;
    RobotEnabled : BOOL := TRUE;
    RobotStatusLight : BOOL := FALSE;
END_VAR

RobotCycleTick := RobotCycleTick + 1;
IF RobotCycleTick >= 70 THEN
    RobotCycleTick := 0;
END_IF;

IF RobotCycleTick < 10 THEN
    (* approach pickup *)
    RobotMotionStep := 1;
    RobotShoulderAngle := -0.72;
    RobotElbowAngle := 0.42;
    RobotWristAngle := 0.25;
    RobotGripperOpen := TRUE;
    RobotBoxParentState := 0;
    RobotStatusLight := FALSE;
ELSIF RobotCycleTick < 20 THEN
    (* close gripper *)
    RobotMotionStep := 2;
    RobotShoulderAngle := -0.72;
    RobotElbowAngle := 0.42;
    RobotWristAngle := 0.25;
    RobotGripperOpen := FALSE;
    RobotBoxParentState := 1;
    RobotStatusLight := TRUE;
ELSIF RobotCycleTick < 30 THEN
    (* lift *)
    RobotMotionStep := 3;
    RobotShoulderAngle := -0.35;
    RobotElbowAngle := 0.72;
    RobotWristAngle := -0.45;
    RobotGripperOpen := FALSE;
    RobotBoxParentState := 1;
    RobotStatusLight := TRUE;
ELSIF RobotCycleTick < 40 THEN
    (* translate *)
    RobotMotionStep := 4;
    RobotShoulderAngle := 0.35;
    RobotElbowAngle := 0.70;
    RobotWristAngle := -0.85;
    RobotGripperOpen := FALSE;
    RobotBoxParentState := 1;
    RobotStatusLight := TRUE;
ELSIF RobotCycleTick < 50 THEN
    (* lower *)
    RobotMotionStep := 5;
    RobotShoulderAngle := 0.42;
    RobotElbowAngle := 0.44;
    RobotWristAngle := -0.20;
    RobotGripperOpen := FALSE;
    RobotBoxParentState := 1;
    RobotStatusLight := TRUE;
ELSIF RobotCycleTick < 60 THEN
    (* open gripper *)
    RobotMotionStep := 6;
    RobotShoulderAngle := 0.42;
    RobotElbowAngle := 0.44;
    RobotWristAngle := -0.20;
    RobotGripperOpen := TRUE;
    RobotBoxParentState := 2;
    RobotStatusLight := FALSE;
ELSE
    (* return *)
    RobotMotionStep := 7;
    RobotShoulderAngle := -0.15;
    RobotElbowAngle := 0.55;
    RobotWristAngle := 0.00;
    RobotGripperOpen := TRUE;
    RobotBoxParentState := 2;
    RobotStatusLight := FALSE;
END_IF;

RobotEnabled := TRUE;
END_PROGRAM
