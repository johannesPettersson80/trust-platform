PROGRAM Main
VAR
    Robot : Robot_P3MinimalArm;
    RobotCommand : INT := 0;
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
    RobotCommand := 1;
ELSIF RobotCycleTick < 20 THEN
    (* close gripper *)
    RobotCommand := 2;
ELSIF RobotCycleTick < 30 THEN
    (* lift *)
    RobotCommand := 3;
ELSIF RobotCycleTick < 40 THEN
    (* translate *)
    RobotCommand := 4;
ELSIF RobotCycleTick < 50 THEN
    (* lower *)
    RobotCommand := 5;
ELSIF RobotCycleTick < 60 THEN
    (* open gripper *)
    RobotCommand := 6;
ELSE
    (* return *)
    RobotCommand := 7;
END_IF;

Robot(Enable := RobotEnabled, Command := RobotCommand);

RobotMotionStep := Robot.State;
RobotShoulderAngle := Robot.Joint1;
RobotElbowAngle := Robot.Joint2;
RobotWristAngle := Robot.ToolYaw;
RobotGripperOpen := Robot.GripperOpen;
RobotGripperX := Robot.ToolX;
RobotGripperY := Robot.ToolY;
RobotGripperZ := Robot.ToolZ;
RobotBoxX := Robot.WorkpieceX;
RobotBoxY := Robot.WorkpieceY;
RobotBoxZ := Robot.WorkpieceZ;
RobotEnabled := Robot.EnabledOut;
RobotStatusLight := Robot.StatusLight;
END_PROGRAM
