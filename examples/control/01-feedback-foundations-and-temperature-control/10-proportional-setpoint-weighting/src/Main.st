USING Control;

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
    MIN(RequestedPosition,
        MIN(LREAL#100.0, PreviousPosition + MaximumRate * CycleTime))
);
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
    BeforeState : Control.PIDInitialState;
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
    TemperatureC : LREAL := 35.0;
    WaterTemperatureC : LREAL := 35.0;
    SetpointC : LREAL := 35.0;
    DisturbanceKw : LREAL := 0.0;
    ErrorC : LREAL := 0.0;
    SetpointChange : LREAL := 0.0;
    MeasurementChange : LREAL := 0.0;
    ProportionalChange : LREAL := 0.0;
    IntegralChange : LREAL := 0.0;
    RequestedValvePosition : LREAL := 75.0;
    AppliedValvePosition : LREAL := 75.0;
    IntegralPct : LREAL := 75.0;
END_VAR

(* The simulator owns the one-state heater and publishes its water temperature
   through normal PLC input. Only the actual Control.PID drives the output. *)
WaterTemperatureC := TemperatureC;
IF NOT (TemperatureC >= LREAL#0.0 AND TemperatureC <= LREAL#80.0)
    OR SetpointWeight < LREAL#0.0 OR SetpointWeight > LREAL#1.0 THEN
    SafetyAbort := TRUE;
END_IF;
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
Input.ProcessValue := TemperatureC;
Input.CycleTime := LREAL#0.1;
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
    InitialState.ProcessValue := TemperatureC;
    InitialState.NominalOutput := LREAL#75.0;
    InitialState.SaturatedOutput := LREAL#75.0;
    InitialState.SetpointRate := LREAL#0.0;
    InitialState.ProcessValueRate := LREAL#0.0;
    InitialState.FeedForward := LREAL#0.0;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;
BeforeState := Controller.GetState();
AppliedValvePosition := Controller.Update(Input := Input);
ObservedState := Controller.GetState();
ErrorC := SetpointC - TemperatureC;
SetpointChange := SetpointC - BeforeState.Setpoint;
MeasurementChange := TemperatureC - BeforeState.ProcessValue;
ProportionalChange := KpPctPerC
    * (SetpointWeight * (SetpointC - LREAL#35.0)
        - (TemperatureC - LREAL#35.0));
IntegralChange := KiPctPerCs * ErrorC * LREAL#0.1;
RequestedValvePosition := ObservedState.NominalOutput;
IntegralPct := ObservedState.NominalOutput - ProportionalChange;
IF NOT ExperimentEnabled THEN DisturbanceKw := LREAL#0.0; END_IF;
ExperimentWasEnabled := ExperimentEnabled;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.SetpointC AT %ML0 : LREAL;
    P1.RequestedValvePosition AT %ML8 : LREAL;
    P1.SetpointWeight AT %ML16 : LREAL;
    P1.KpPctPerC AT %ML24 : LREAL;
    P1.KiPctPerCs AT %ML32 : LREAL;
    P1.SetpointChange AT %ML40 : LREAL;
    P1.MeasurementChange AT %ML48 : LREAL;
    P1.ProportionalChange AT %ML56 : LREAL;
    P1.IntegralChange AT %ML64 : LREAL;
    P1.WaterTemperatureC AT %IL8 : LREAL;
    P1.TemperatureC AT %IL0 : LREAL;
    P1.AppliedValvePosition AT %QL0 : LREAL;
    P1.DisturbanceKw AT %QL8 : LREAL;
    P1.TrialKind AT %MW80 : INT;
    P1.ExperimentEnabled AT %MX128.0 : BOOL;
    P1.SafetyAbort AT %MX128.2 : BOOL;
END_VAR
END_CONFIGURATION
