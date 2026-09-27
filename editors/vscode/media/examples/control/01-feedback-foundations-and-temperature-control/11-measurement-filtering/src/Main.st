FUNCTION Lesson11FilterFraction : LREAL
VAR_INPUT
    CycleTime : LREAL;
    FilterTime : LREAL;
END_VAR
Lesson11FilterFraction := CycleTime / (FilterTime + CycleTime);
END_FUNCTION

FUNCTION Lesson11FilterUpdate : LREAL
VAR_INPUT
    PreviousFilteredValue : LREAL;
    RawValue : LREAL;
    FilterFraction : LREAL;
END_VAR
Lesson11FilterUpdate := PreviousFilteredValue
    + FilterFraction * (RawValue - PreviousFilteredValue);
END_FUNCTION

FUNCTION Lesson11RequestedPosition : LREAL
VAR_INPUT
    HoldingPosition : LREAL;
    ProportionalGain : LREAL;
    Setpoint : LREAL;
    Measurement : LREAL;
END_VAR
Lesson11RequestedPosition := MAX(
    LREAL#0.0,
    MIN(
        LREAL#100.0,
        HoldingPosition + ProportionalGain * (Setpoint - Measurement)
    )
);
END_FUNCTION

PROGRAM Main
VAR
    Initialized : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    ExperimentCycle : ULINT := ULINT#0;
    SafetyAbort : BOOL := FALSE;
    Setpoint : LREAL := 35.0;
    RawTemperature : LREAL := 20.0;
    TrueTemperature : LREAL := 20.0;
    FilteredTemperature : LREAL := 20.0;
    PreviousFilteredTemperature : LREAL := 20.0;
    CycleTime : LREAL := 0.1;
    FilterTime : LREAL := 1.0;
    FilterFraction : LREAL := 0.0;
    ProportionalGain : LREAL := 2.0;
    HoldingValvePosition : LREAL := 75.0;
    RawRequestedPosition : LREAL := 0.0;
    FilteredRequestedPosition : LREAL := 0.0;
    AppliedValvePosition : LREAL := 0.0;
END_VAR

IF NOT Initialized THEN
    PreviousFilteredTemperature := RawTemperature;
    FilteredTemperature := RawTemperature;
    Initialized := TRUE;
END_IF;

CycleTime := LREAL#0.1;
FilterTime := LREAL#1.0;
ProportionalGain := LREAL#2.0;
HoldingValvePosition := LREAL#75.0;
Setpoint := LREAL#35.0;
FilterFraction := Lesson11FilterFraction(
    CycleTime := CycleTime,
    FilterTime := FilterTime
);

IF ExperimentEnabled AND NOT SafetyAbort THEN
    IF NOT ExperimentWasEnabled THEN
        ExperimentCycle := ULINT#0;
        PreviousFilteredTemperature := RawTemperature;
    END_IF;
    ExperimentCycle := ExperimentCycle + ULINT#1;
    IF ExperimentCycle <= ULINT#800 THEN
        AppliedValvePosition := HoldingValvePosition;
    ELSE
        AppliedValvePosition := LREAL#0.0;
    END_IF;
ELSE
    ExperimentCycle := ULINT#0;
    AppliedValvePosition := LREAL#0.0;
END_IF;

FilteredTemperature := Lesson11FilterUpdate(
    PreviousFilteredValue := PreviousFilteredTemperature,
    RawValue := RawTemperature,
    FilterFraction := FilterFraction
);
PreviousFilteredTemperature := FilteredTemperature;

RawRequestedPosition := Lesson11RequestedPosition(
    HoldingPosition := HoldingValvePosition,
    ProportionalGain := ProportionalGain,
    Setpoint := Setpoint,
    Measurement := RawTemperature
);
FilteredRequestedPosition := Lesson11RequestedPosition(
    HoldingPosition := HoldingValvePosition,
    ProportionalGain := ProportionalGain,
    Setpoint := Setpoint,
    Measurement := FilteredTemperature
);
IF ExperimentEnabled AND (ExperimentCycle > ULINT#500)
    AND (ExperimentCycle <= ULINT#800) THEN
    (* Apply the filtered candidate after the common settling period. *)
    AppliedValvePosition := FilteredRequestedPosition;
END_IF;
ExperimentWasEnabled := ExperimentEnabled;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.Setpoint AT %ML0 : LREAL;
    P1.FilteredTemperature AT %ML8 : LREAL;
    P1.TrueTemperature AT %IL8 : LREAL;
    P1.FilterTime AT %ML24 : LREAL;
    P1.CycleTime AT %ML32 : LREAL;
    P1.FilterFraction AT %ML40 : LREAL;
    P1.RawRequestedPosition AT %ML48 : LREAL;
    P1.FilteredRequestedPosition AT %ML56 : LREAL;
    P1.HoldingValvePosition AT %ML64 : LREAL;
    P1.ExperimentEnabled AT %MX80.0 : BOOL;
    P1.SafetyAbort AT %MX80.1 : BOOL;
    P1.AppliedValvePosition AT %QL0 : LREAL;
    P1.RawTemperature AT %IL0 : LREAL;
END_VAR
END_CONFIGURATION
