PROGRAM MotorStarter
VAR
    StartPb : BOOL := FALSE;
    StopPb : BOOL := FALSE;
    OverloadTrip : BOOL := FALSE;
    MotorRun : BOOL := FALSE;
    SealInContact : BOOL := FALSE;
END_VAR

IF StopPb OR OverloadTrip THEN
    MotorRun := FALSE;
ELSIF StartPb OR SealInContact THEN
    MotorRun := TRUE;
END_IF;

SealInContact := MotorRun;
END_PROGRAM
