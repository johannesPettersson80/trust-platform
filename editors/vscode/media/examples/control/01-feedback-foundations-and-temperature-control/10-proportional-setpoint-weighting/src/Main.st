FUNCTION Lesson10ProportionalChange : LREAL
VAR_INPUT
    ProportionalGain : LREAL;
    SetpointWeight : LREAL;
    SetpointChange : LREAL;
    MeasurementChange : LREAL;
END_VAR
Lesson10ProportionalChange := ProportionalGain
    * (SetpointWeight * SetpointChange - MeasurementChange);
END_FUNCTION

FUNCTION Lesson10ReachablePosition : LREAL
VAR_INPUT
    RequestedPosition : LREAL;
    PreviousPosition : LREAL;
    MaximumRate : LREAL;
    CycleTime : LREAL;
END_VAR
Lesson10ReachablePosition := MAX(
    MAX(LREAL#0.0, PreviousPosition - MaximumRate * CycleTime),
    MIN(
        RequestedPosition,
        MIN(LREAL#100.0, PreviousPosition + MaximumRate * CycleTime)
    )
);
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
    PreviousSetpoint : LREAL := 35.0;
    ProcessValue : LREAL := 20.0;
    PreviousProcessValue : LREAL := 20.0;
    Error : LREAL := 15.0;
    ProportionalGain : LREAL := 2.0;
    IntegralGain : LREAL := 0.5;
    SetpointWeight : LREAL := 0.5;
    CycleTime : LREAL := 0.1;
    TrackingTime : LREAL := 1.0;
    MaximumValveRate : LREAL := 5.0;
    ManualValvePosition : LREAL := 0.0;
    SetpointChange : LREAL := 0.0;
    MeasurementChange : LREAL := 0.0;
    ProportionalChange : LREAL := 0.0;
    IntegralChange : LREAL := 0.0;
    TrackingCorrection : LREAL := 0.0;
    ControllerOutputState : LREAL := 0.0;
    RequestedValvePosition : LREAL := 0.0;
    AppliedValvePosition : LREAL := 0.0;
    PreviousAppliedPosition : LREAL := 0.0;
END_VAR

IF NOT Initialized THEN
    PreviousSetpoint := Setpoint;
    PreviousProcessValue := ProcessValue;
    AppliedValvePosition := LREAL#0.0;
    ControllerOutputState := LREAL#0.0;
    Initialized := TRUE;
END_IF;

ProportionalGain := LREAL#2.0;
IntegralGain := LREAL#0.5;
SetpointWeight := LREAL#0.5;
CycleTime := LREAL#0.1;
TrackingTime := LREAL#1.0;
MaximumValveRate := LREAL#5.0;

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
    ELSIF ExperimentCycle <= ULINT#850 THEN
        Setpoint := LREAL#38.0;
        ManualValvePosition := LREAL#75.0;
        AutoMode := TRUE;
    ELSIF ExperimentCycle <= ULINT#1150 THEN
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
    IF NOT LastAutoMode THEN
        ControllerOutputState := PreviousAppliedPosition;
        PreviousSetpoint := Setpoint;
        PreviousProcessValue := ProcessValue;
    END_IF;
    SetpointChange := Setpoint - PreviousSetpoint;
    MeasurementChange := ProcessValue - PreviousProcessValue;
    ProportionalChange := Lesson10ProportionalChange(
        ProportionalGain := ProportionalGain,
        SetpointWeight := SetpointWeight,
        SetpointChange := SetpointChange,
        MeasurementChange := MeasurementChange
    );
    IntegralChange := IntegralGain * Error * CycleTime;
    RequestedValvePosition := ControllerOutputState
        + ProportionalChange + IntegralChange;
ELSE
    SetpointChange := LREAL#0.0;
    MeasurementChange := LREAL#0.0;
    ProportionalChange := LREAL#0.0;
    IntegralChange := LREAL#0.0;
    RequestedValvePosition := ManualValvePosition;
END_IF;

AppliedValvePosition := Lesson10ReachablePosition(
    RequestedPosition := RequestedValvePosition,
    PreviousPosition := PreviousAppliedPosition,
    MaximumRate := MaximumValveRate,
    CycleTime := CycleTime
);

IF AutoMode THEN
    TrackingCorrection := (CycleTime / TrackingTime)
        * (AppliedValvePosition - RequestedValvePosition);
    ControllerOutputState := RequestedValvePosition + TrackingCorrection;
ELSE
    TrackingCorrection := LREAL#0.0;
    ControllerOutputState := AppliedValvePosition;
END_IF;

PreviousSetpoint := Setpoint;
PreviousProcessValue := ProcessValue;
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
    P1.SetpointWeight AT %ML16 : LREAL;
    P1.ProportionalGain AT %ML24 : LREAL;
    P1.IntegralGain AT %ML32 : LREAL;
    P1.SetpointChange AT %ML40 : LREAL;
    P1.MeasurementChange AT %ML48 : LREAL;
    P1.ProportionalChange AT %ML56 : LREAL;
    P1.IntegralChange AT %ML64 : LREAL;
    P1.TrackingCorrection AT %ML72 : LREAL;
    P1.ControllerOutputState AT %ML80 : LREAL;
    P1.Error AT %ML88 : LREAL;
    P1.PreviousAppliedPosition AT %ML96 : LREAL;
    P1.ExperimentEnabled AT %MX128.0 : BOOL;
    P1.AutoMode AT %MX128.1 : BOOL;
    P1.SafetyAbort AT %MX128.2 : BOOL;
    P1.AppliedValvePosition AT %QL0 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
END_VAR
END_CONFIGURATION
