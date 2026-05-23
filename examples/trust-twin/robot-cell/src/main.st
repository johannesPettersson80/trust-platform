PROGRAM Main
VAR
    RobotMotionStep : INT := 0;
    RobotCycleTick : INT := 0;
    RobotShoulderAngle : REAL := -0.60;
    RobotElbowAngle : REAL := 0.35;
    RobotWristAngle : REAL := 0.25;
    RobotGripperOpen : BOOL := TRUE;
    RobotGripperX : REAL := 0.00;
    RobotGripperY : REAL := 0.80;
    RobotGripperZ : REAL := 0.00;
    RobotBoxX : REAL := 0.00;
    RobotBoxY : REAL := 0.35;
    RobotBoxZ : REAL := 0.00;
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
    RobotGripperX := 0.00;
    RobotGripperY := 0.80;
    RobotGripperZ := 0.00;
    RobotBoxX := 0.00;
    RobotBoxY := 0.35;
    RobotBoxZ := 0.00;
    RobotStatusLight := FALSE;
ELSIF RobotCycleTick < 20 THEN
    (* close gripper *)
    RobotMotionStep := 2;
    RobotShoulderAngle := -0.72;
    RobotElbowAngle := 0.42;
    RobotWristAngle := 0.25;
    RobotGripperOpen := FALSE;
    RobotGripperX := 0.00;
    RobotGripperY := 0.55;
    RobotGripperZ := 0.00;
    RobotBoxX := 0.00;
    RobotBoxY := 0.35;
    RobotBoxZ := 0.00;
    RobotStatusLight := TRUE;
ELSIF RobotCycleTick < 30 THEN
    (* lift *)
    RobotMotionStep := 3;
    RobotShoulderAngle := -0.35;
    RobotElbowAngle := 0.72;
    RobotWristAngle := -0.45;
    RobotGripperOpen := FALSE;
    RobotGripperX := 0.80;
    RobotGripperY := 1.15;
    RobotGripperZ := 0.00;
    RobotBoxX := 0.80;
    RobotBoxY := 1.15;
    RobotBoxZ := 0.00;
    RobotStatusLight := TRUE;
ELSIF RobotCycleTick < 40 THEN
    (* translate *)
    RobotMotionStep := 4;
    RobotShoulderAngle := 0.35;
    RobotElbowAngle := 0.70;
    RobotWristAngle := -0.85;
    RobotGripperOpen := FALSE;
    RobotGripperX := 2.40;
    RobotGripperY := 1.15;
    RobotGripperZ := 0.00;
    RobotBoxX := 2.40;
    RobotBoxY := 1.15;
    RobotBoxZ := 0.00;
    RobotStatusLight := TRUE;
ELSIF RobotCycleTick < 50 THEN
    (* lower *)
    RobotMotionStep := 5;
    RobotShoulderAngle := 0.72;
    RobotElbowAngle := 0.40;
    RobotWristAngle := 0.85;
    RobotGripperOpen := FALSE;
    RobotGripperX := 4.00;
    RobotGripperY := 0.55;
    RobotGripperZ := 0.00;
    RobotBoxX := 4.00;
    RobotBoxY := 0.35;
    RobotBoxZ := 0.00;
    RobotStatusLight := TRUE;
ELSIF RobotCycleTick < 60 THEN
    (* open gripper *)
    RobotMotionStep := 6;
    RobotShoulderAngle := 0.72;
    RobotElbowAngle := 0.40;
    RobotWristAngle := 0.85;
    RobotGripperOpen := TRUE;
    RobotGripperX := 4.00;
    RobotGripperY := 0.80;
    RobotGripperZ := 0.00;
    RobotBoxX := 4.00;
    RobotBoxY := 0.35;
    RobotBoxZ := 0.00;
    RobotStatusLight := FALSE;
ELSE
    (* return *)
    RobotMotionStep := 7;
    RobotShoulderAngle := -0.15;
    RobotElbowAngle := 0.55;
    RobotWristAngle := 0.00;
    RobotGripperOpen := TRUE;
    RobotGripperX := 2.00;
    RobotGripperY := 1.20;
    RobotGripperZ := 0.00;
    RobotBoxX := 4.00;
    RobotBoxY := 0.35;
    RobotBoxZ := 0.00;
    RobotStatusLight := FALSE;
END_IF;

RobotEnabled := TRUE;
END_PROGRAM
