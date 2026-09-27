FUNCTION Lesson8ProportionalCorrection : LREAL
VAR_INPUT
    ProportionalGain : LREAL;
    Error : LREAL;
END_VAR
Lesson8ProportionalCorrection := ProportionalGain * Error;
END_FUNCTION

FUNCTION Lesson8IntegralIncrement : LREAL
VAR_INPUT
    IntegralGain : LREAL;
    Error : LREAL;
    CycleTime : LREAL;
END_VAR
Lesson8IntegralIncrement := IntegralGain * Error * CycleTime;
END_FUNCTION

FUNCTION Lesson8ReachablePosition : LREAL
VAR_INPUT
    RequestedPosition : LREAL;
    PreviousPosition : LREAL;
    MaximumRate : LREAL;
    CycleTime : LREAL;
END_VAR
VAR
    LowestPosition : LREAL;
    HighestPosition : LREAL;
END_VAR
LowestPosition := MAX(
    LREAL#0.0,
    PreviousPosition - MaximumRate * CycleTime
);
HighestPosition := MIN(
    LREAL#100.0,
    PreviousPosition + MaximumRate * CycleTime
);
Lesson8ReachablePosition := MAX(
    LowestPosition,
    MIN(RequestedPosition, HighestPosition)
);
END_FUNCTION

FUNCTION Lesson8TrackingCorrection : LREAL
VAR_INPUT
    RequestedPosition : LREAL;
    AppliedPosition : LREAL;
    CycleTime : LREAL;
    TrackingTime : LREAL;
END_VAR
Lesson8TrackingCorrection :=
    (CycleTime / TrackingTime) * (AppliedPosition - RequestedPosition);
END_FUNCTION

PROGRAM Main
VAR
    Initialized : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    ExperimentCycle : ULINT := ULINT#0;
    AutoMode : BOOL := FALSE;
    LastAutoMode : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    Setpoint : LREAL := 35.0;
    ProcessValue : LREAL := 20.0;
    Error : LREAL := 15.0;
    ProportionalGain : LREAL := 2.0;
    IntegralGain : LREAL := 0.5;
    CycleTime : LREAL := 0.1;
    TrackingTime : LREAL := 1.0;
    MaximumValveRate : LREAL := 5.0;
    ManualValvePosition : LREAL := 0.0;
    ProportionalCorrection : LREAL := 0.0;
    IntegralIncrement : LREAL := 0.0;
    TrackingCorrection : LREAL := 0.0;
    IntegralCorrection : LREAL := 0.0;
    RequestedValvePosition : LREAL := 0.0;
    AppliedValvePosition : LREAL := 0.0;
    PreviousAppliedPosition : LREAL := 0.0;
    PositionDifference : LREAL := 0.0;
END_VAR

IF NOT Initialized THEN
    AppliedValvePosition := LREAL#0.0;
    PreviousAppliedPosition := LREAL#0.0;
    IntegralCorrection := LREAL#0.0;
    ExperimentCycle := ULINT#0;
    Initialized := TRUE;
END_IF;

ProportionalGain := LREAL#2.0;
IntegralGain := LREAL#0.5;
CycleTime := LREAL#0.1;
TrackingTime := LREAL#1.0;
MaximumValveRate := LREAL#5.0;

(* Establish 35 degrees, request a reachable 38 degrees, then restore 35. *)
IF ExperimentEnabled AND NOT SafetyAbort THEN
    IF NOT ExperimentWasEnabled THEN
        ExperimentCycle := ULINT#0;
    END_IF;
    ExperimentCycle := ExperimentCycle + ULINT#1;

    IF ExperimentCycle = ULINT#1 THEN
        Setpoint := LREAL#35.0;
        ManualValvePosition := LREAL#0.0;
        AutoMode := FALSE;
    ELSIF ExperimentCycle <= ULINT#500 THEN
        Setpoint := LREAL#35.0;
        ManualValvePosition := LREAL#75.0;
        AutoMode := FALSE;
    ELSIF ExperimentCycle <= ULINT#550 THEN
        Setpoint := LREAL#35.0;
        ManualValvePosition := LREAL#75.0;
        AutoMode := TRUE;
    ELSIF ExperimentCycle <= ULINT#750 THEN
        Setpoint := LREAL#38.0;
        ManualValvePosition := LREAL#75.0;
        AutoMode := TRUE;
    ELSIF ExperimentCycle <= ULINT#1050 THEN
        Setpoint := LREAL#35.0;
        ManualValvePosition := LREAL#75.0;
        AutoMode := TRUE;
    ELSE
        Setpoint := LREAL#35.0;
        ManualValvePosition := LREAL#0.0;
        AutoMode := FALSE;
    END_IF;
ELSE
    ExperimentCycle := ULINT#0;
    Setpoint := LREAL#35.0;
    ManualValvePosition := LREAL#0.0;
    AutoMode := FALSE;
END_IF;

Error := Setpoint - ProcessValue;
PreviousAppliedPosition := AppliedValvePosition;

IF AutoMode THEN
    ProportionalCorrection := Lesson8ProportionalCorrection(
        ProportionalGain := ProportionalGain,
        Error := Error
    );

    IF NOT LastAutoMode THEN
        IntegralCorrection := PreviousAppliedPosition
            - ProportionalCorrection;
    END_IF;

    IntegralIncrement := Lesson8IntegralIncrement(
        IntegralGain := IntegralGain,
        Error := Error,
        CycleTime := CycleTime
    );
    RequestedValvePosition := ProportionalCorrection
        + IntegralCorrection + IntegralIncrement;
ELSE
    ProportionalCorrection := LREAL#0.0;
    IntegralIncrement := LREAL#0.0;
    RequestedValvePosition := ManualValvePosition;
END_IF;

AppliedValvePosition := Lesson8ReachablePosition(
    RequestedPosition := RequestedValvePosition,
    PreviousPosition := PreviousAppliedPosition,
    MaximumRate := MaximumValveRate,
    CycleTime := CycleTime
);
PositionDifference := RequestedValvePosition - AppliedValvePosition;

IF AutoMode THEN
    TrackingCorrection := Lesson8TrackingCorrection(
        RequestedPosition := RequestedValvePosition,
        AppliedPosition := AppliedValvePosition,
        CycleTime := CycleTime,
        TrackingTime := TrackingTime
    );
    IntegralCorrection := IntegralCorrection
        + IntegralIncrement + TrackingCorrection;
ELSE
    TrackingCorrection := LREAL#0.0;
    IntegralCorrection := AppliedValvePosition;
END_IF;

LastAutoMode := AutoMode;
ExperimentWasEnabled := ExperimentEnabled;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.Setpoint AT %ML0 : LREAL;
    P1.RequestedValvePosition AT %ML8 : LREAL;
    P1.PositionDifference AT %ML16 : LREAL;
    P1.ProportionalGain AT %ML24 : LREAL;
    P1.IntegralGain AT %ML32 : LREAL;
    P1.ManualValvePosition AT %ML40 : LREAL;
    P1.ProportionalCorrection AT %ML48 : LREAL;
    P1.IntegralIncrement AT %ML56 : LREAL;
    P1.IntegralCorrection AT %ML64 : LREAL;
    P1.TrackingTime AT %ML72 : LREAL;
    P1.TrackingCorrection AT %ML80 : LREAL;
    P1.MaximumValveRate AT %ML88 : LREAL;
    P1.PreviousAppliedPosition AT %ML96 : LREAL;
    P1.ExperimentEnabled AT %MX128.0 : BOOL;
    P1.AutoMode AT %MX128.1 : BOOL;
    P1.SafetyAbort AT %MX128.2 : BOOL;
    P1.AppliedValvePosition AT %QL0 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
END_VAR
END_CONFIGURATION
