USING Control;

FUNCTION Lesson11FilterFraction : LREAL
VAR_INPUT
    CycleTime : LREAL;
    FilterTime : LREAL;
END_VAR
IF FilterTime <= LREAL#0.0 THEN
    Lesson11FilterFraction := LREAL#1.0;
ELSE
    Lesson11FilterFraction := CycleTime / (FilterTime + CycleTime);
END_IF;
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
Lesson11RequestedPosition := MAX(LREAL#0.0, MIN(LREAL#100.0,
    HoldingPosition + ProportionalGain * (Setpoint - Measurement)));
END_FUNCTION

PROGRAM Main
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#10.397336997569598,
        IntegralGain := LREAL#2.250325442359752,
        DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#0.5,
        DerivativeSetpointWeight := LREAL#0.0,
        Bias := LREAL#0.0,
        OutputMinimum := LREAL#0.0,
        OutputMaximum := LREAL#100.0,
        RateMinimum := LREAL#-10000.0,
        RateMaximum := LREAL#10000.0,
        TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#BackCalculation
    );
    InitialState : Control.PIDInitialState;
    ObservedState : Control.PIDInitialState;
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    TrialKind : INT := 0; (* 0:target step; 1:30kW extraction. *)
    KpPctPerC : LREAL := 10.397336997569598;
    KiPctPerCs : LREAL := 2.250325442359752;
    SetpointWeight : LREAL := 0.5;
    RawTemperature : LREAL := 35.0;
    TrueTemperature : LREAL := 35.0;
    FilteredTemperature : LREAL := 35.0;
    SetpointC : LREAL := 35.0;
    DisturbanceKw : LREAL := 0.0;
    FilterTime : LREAL := 1.0;
    CycleTime : LREAL := 0.1;
    FilterFraction : LREAL := 0.0;
    RawRequestedPosition : LREAL := 75.0;
    FilteredRequestedPosition : LREAL := 75.0;
    HoldingValvePosition : LREAL := 75.0;
    AppliedValvePosition : LREAL := 75.0;
    ErrorC : LREAL := 0.0;
    IntegralPct : LREAL := 75.0;
END_VAR

CycleTime := LREAL#0.1;
IF NOT (RawTemperature >= LREAL#0.0 AND RawTemperature <= LREAL#80.0)
    OR NOT (TrueTemperature >= LREAL#0.0 AND TrueTemperature <= LREAL#80.0)
    OR FilterTime < LREAL#0.0 OR SetpointWeight < LREAL#0.0
    OR SetpointWeight > LREAL#1.0 THEN
    SafetyAbort := TRUE;
END_IF;
FilterFraction := Lesson11FilterFraction(CycleTime := CycleTime, FilterTime := FilterTime);
IF NOT ExperimentEnabled THEN
    FilteredTemperature := RawTemperature;
    ExperimentWasEnabled := FALSE;
END_IF;
IF ExperimentEnabled AND NOT ExperimentWasEnabled THEN
    FilteredTemperature := RawTemperature;
END_IF;
FilteredTemperature := FilteredTemperature
    + FilterFraction * (RawTemperature - FilteredTemperature);

Parameters.ProportionalGain := KpPctPerC;
Parameters.IntegralGain := KiPctPerCs;
Parameters.ProportionalSetpointWeight := SetpointWeight;
Controller.Configure(Parameters := Parameters);
SetpointC := LREAL#35.0;
DisturbanceKw := LREAL#0.0;
IF ExperimentEnabled AND NOT SafetyAbort THEN
    IF TrialKind = 0 THEN SetpointC := LREAL#38.0;
    ELSIF TrialKind = 1 THEN DisturbanceKw := LREAL#30.0;
    END_IF;
    Input.Mode := PIDMode#Auto;
ELSE
    Input.Mode := PIDMode#Manual;
END_IF;
Input.Setpoint := SetpointC;
Input.ProcessValue := FilteredTemperature;
Input.CycleTime := CycleTime;
Input.ManualOutput := LREAL#75.0;
Input.FeedForward := LREAL#0.0;
Input.TrackingOutput := LREAL#75.0;
IF SafetyAbort THEN
    Input.Mode := PIDMode#Manual;
    Input.ManualOutput := LREAL#0.0;
    Input.TrackingOutput := LREAL#0.0;
    DisturbanceKw := LREAL#0.0;
END_IF;
IF NOT Initialized THEN
    InitialState.Setpoint := LREAL#35.0;
    InitialState.ProcessValue := FilteredTemperature;
    InitialState.NominalOutput := LREAL#75.0;
    InitialState.SaturatedOutput := LREAL#75.0;
    InitialState.SetpointRate := LREAL#0.0;
    InitialState.ProcessValueRate := LREAL#0.0;
    InitialState.FeedForward := LREAL#0.0;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;
AppliedValvePosition := Controller.Update(Input := Input);
ObservedState := Controller.GetState();
ErrorC := SetpointC - FilteredTemperature;
RawRequestedPosition := LREAL#75.0 + KpPctPerC * (SetpointC - RawTemperature);
FilteredRequestedPosition := ObservedState.NominalOutput;
IntegralPct := ObservedState.NominalOutput
    - KpPctPerC * (SetpointWeight * (SetpointC - LREAL#35.0)
        - (FilteredTemperature - LREAL#35.0));
IF SafetyAbort THEN AppliedValvePosition := LREAL#0.0; END_IF;
ExperimentWasEnabled := ExperimentEnabled;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.SetpointC AT %ML0 : LREAL;
    P1.FilteredTemperature AT %ML8 : LREAL;
    P1.TrueTemperature AT %IL8 : LREAL;
    P1.FilterTime AT %ML24 : LREAL;
    P1.CycleTime AT %ML32 : LREAL;
    P1.FilterFraction AT %ML40 : LREAL;
    P1.RawRequestedPosition AT %ML48 : LREAL;
    P1.FilteredRequestedPosition AT %ML56 : LREAL;
    P1.HoldingValvePosition AT %ML64 : LREAL;
    P1.KpPctPerC AT %ML72 : LREAL;
    P1.KiPctPerCs AT %ML80 : LREAL;
    P1.SetpointWeight AT %ML88 : LREAL;
    P1.ExperimentEnabled AT %MX80.0 : BOOL;
    P1.SafetyAbort AT %MX80.1 : BOOL;
    P1.RawTemperature AT %IL0 : LREAL;
    P1.AppliedValvePosition AT %QL0 : LREAL;
    P1.DisturbanceKw AT %QL8 : LREAL;
    P1.TrialKind AT %MW120 : INT;
END_VAR
END_CONFIGURATION
