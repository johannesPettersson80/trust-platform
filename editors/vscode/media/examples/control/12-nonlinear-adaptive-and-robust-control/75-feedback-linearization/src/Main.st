(* The PD law with gravity compensation: τ = Kp(r − φ) − Kd·ω − Ĝ·sin φ with Kp = 16 N·m/rad and Kd = 7 N·m·s/rad; φ is
   the angle from upright, ω the speed, r the target and Ĝ the gravity coefficient the controller assumes, in N·m. *)
FUNCTION CompensatedTorqueNm : LREAL
VAR_INPUT
    AngleRad : LREAL;
    SpeedRadPerS : LREAL;
    SetpointRad : LREAL;
    GravityEstimateNm : LREAL;
END_VAR
CompensatedTorqueNm := LREAL#16.0 * (SetpointRad - AngleRad) - LREAL#7.0 * SpeedRadPerS - GravityEstimateNm * SIN(AngleRad);
END_FUNCTION

PROGRAM Main
VAR
    (* Lesson 83's upright arm. GravityEstimateNm, the controller's Ĝ, is a Control Studio experiment setting; the arm's
       own G is 4 N·m. The project experiment sets the target to 0.1 rad and records the peak angle and the largest error
       from 4 s to 8 s. Outside the experiment the target is upright. The joint measures its angle from hanging down:
       φ = θ − π. An angle beyond 0.3 rad or a speed beyond 2 rad/s from upright, or an update interval outside
       20 ms ± 5 ms, latches zero torque. *)
    GravityEstimateNm : LREAL := 4.0;
    JointAngleRad : LREAL := 3.141592653589793;
    SpeedRadPerS : LREAL := 0.0;
    TorqueNm : LREAL := 0.0;
    RequestedTorqueNm : LREAL := 0.0;
    AngleRad : LREAL := 0.0;
    SetpointRad : LREAL := 0.0;
    PeakAngleRad : LREAL := 0.0;
    LateErrorRad : LREAL := 0.0;
    Updates : DINT := 0;
    ExperimentEnabled : BOOL := FALSE;
    WasEnabled : BOOL := FALSE;
    Saturated : BOOL := FALSE;
    FaultLatched : BOOL := FALSE;
    ClockPrimed : BOOL := FALSE;
    ClockValid : BOOL := TRUE;
    NowTime : TIME;
    PreviousTime : TIME;
END_VAR
AngleRad := JointAngleRad - LREAL#3.141592653589793;
NowTime := TIME();
ClockValid := NOT ClockPrimed OR ((NowTime - PreviousTime) >= T#15ms AND (NowTime - PreviousTime) <= T#25ms);
IF NOT ClockValid OR ABS(AngleRad) > LREAL#0.3 OR ABS(SpeedRadPerS) > LREAL#2.0 THEN
    FaultLatched := TRUE;
END_IF;
IF ExperimentEnabled AND NOT WasEnabled THEN
    Updates := 0;
    PeakAngleRad := AngleRad;
    LateErrorRad := LREAL#0.0;
ELSIF ExperimentEnabled THEN
    Updates := Updates + 1;
END_IF;
SetpointRad := LREAL#0.0;
IF ExperimentEnabled THEN
    SetpointRad := LREAL#0.1;
    PeakAngleRad := MAX(PeakAngleRad, AngleRad);
    IF Updates >= 200 AND Updates <= 400 THEN
        LateErrorRad := MAX(LateErrorRad, ABS(SetpointRad - AngleRad));
    END_IF;
END_IF;
RequestedTorqueNm := CompensatedTorqueNm(AngleRad := AngleRad, SpeedRadPerS := SpeedRadPerS, SetpointRad := SetpointRad,
    GravityEstimateNm := GravityEstimateNm);
TorqueNm := MIN(MAX(RequestedTorqueNm, LREAL#-20.0), LREAL#20.0);
Saturated := TorqueNm <> RequestedTorqueNm;
IF FaultLatched THEN
    TorqueNm := LREAL#0.0;
END_IF;
WasEnabled := ExperimentEnabled;
PreviousTime := NowTime;
ClockPrimed := TRUE;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.JointAngleRad AT %IL0 : LREAL;
    P1.SpeedRadPerS AT %IL8 : LREAL;
    P1.TorqueNm AT %QL0 : LREAL;
    P1.AngleRad AT %ML0 : LREAL;
    P1.SetpointRad AT %ML8 : LREAL;
    P1.PeakAngleRad AT %ML16 : LREAL;
    P1.LateErrorRad AT %ML24 : LREAL;
    P1.ExperimentEnabled AT %MX48.0 : BOOL;
    P1.Saturated AT %MX48.1 : BOOL;
    P1.FaultLatched AT %MX48.2 : BOOL;
    P1.ClockValid AT %MX48.3 : BOOL;
END_VAR
END_CONFIGURATION
