USING Control;

PROGRAM Main
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#2.0,
        IntegralGain := LREAL#0.5,
        DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#1.0,
        DerivativeSetpointWeight := LREAL#0.0,
        Bias := LREAL#0.0,
        OutputMinimum := LREAL#0.0,
        OutputMaximum := LREAL#100.0,
        RateMinimum := LREAL#-1000.0,
        RateMaximum := LREAL#1000.0,
        TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#BackCalculation
    );
    InitialState : Control.PIDInitialState;
    ObservedState : Control.PIDInitialState;
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    WasAutoMode : BOOL := FALSE;
    AutoMode : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    ExperimentScan : ULINT := ULINT#0;
    HeatExtractionKw : LREAL := 0.0;
    SafetyAbort : BOOL := FALSE;
    ProportionalGain : LREAL := 2.0;
    IntegralGain : LREAL := 0.5;
    CycleTime : LREAL := 0.1;
    Setpoint : LREAL := 35.0;
    ProcessValue : LREAL := 20.0;
    Error : LREAL := 15.0;
    ProportionalContribution : LREAL := 0.0;
    IntegralIncrement : LREAL := 0.0;
    IntegralContribution : LREAL := 0.0;
    NominalHeaterPower : LREAL := 0.0;
    AppliedHeaterPower : LREAL := 0.0;
END_VAR

(* These are the lesson settings. Later lessons show how production programs
   validate and change controller settings safely. *)
Setpoint := LREAL#35.0;
ProportionalGain := Parameters.ProportionalGain;
IntegralGain := Parameters.IntegralGain;
CycleTime := LREAL#0.1;

IF ProcessValue <> ProcessValue OR ProcessValue < LREAL#0.0
    OR ProcessValue > LREAL#80.0 THEN SafetyAbort := TRUE;
ELSE
    Input.ProcessValue := ProcessValue;
END_IF;
AutoMode := ExperimentEnabled AND NOT SafetyAbort;
HeatExtractionKw := LREAL#0.0;
IF AutoMode THEN
    ExperimentScan := ExperimentScan + ULINT#1;
    IF ExperimentScan >= ULINT#601 THEN HeatExtractionKw := LREAL#30.0; END_IF;
ELSE ExperimentScan := ULINT#0;
END_IF;
Error := Setpoint - ProcessValue;

Controller.Configure(Parameters := Parameters);
IF NOT Initialized OR (AutoMode AND NOT WasAutoMode) THEN
    InitialState.Setpoint := Setpoint;
    InitialState.SetpointRate := LREAL#0.0;
    InitialState.ProcessValue := Input.ProcessValue;
    InitialState.ProcessValueRate := LREAL#0.0;
    InitialState.FeedForward := LREAL#0.0;
    InitialState.NominalOutput := LREAL#0.0;
    IF AutoMode THEN
        (* The lesson intentionally starts with zero equivalent I and the
           measured P demand: 30% nominal, 0% previously applied at 20 degC.
           This start-edge seed preserves the first 30.75% PI command. *)
        InitialState.NominalOutput := ProportionalGain * (Setpoint - Input.ProcessValue);
    END_IF;
    InitialState.SaturatedOutput := AppliedHeaterPower;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;
Input.Setpoint := Setpoint;
Input.CycleTime := CycleTime;
Input.ManualOutput := LREAL#0.0;
IF AutoMode THEN Input.Mode := PIDMode#Auto;
ELSE Input.Mode := PIDMode#Manual;
END_IF;
AppliedHeaterPower := Controller.Update(Input := Input);

(* Observe the actual retained nominal state after the library's internal
   anti-windup correction. This is not its pre-limit candidate. *)
ObservedState := Controller.GetState();
NominalHeaterPower := ObservedState.NominalOutput;
IF AutoMode THEN
    ProportionalContribution := ProportionalGain * Error;
    IntegralIncrement := IntegralGain * Error * CycleTime;
    (* Fixed Kp, weight one and no D/feedforward give the equivalent I.
       This diagnostic never accumulates a second controller state. *)
    IntegralContribution := NominalHeaterPower - ProportionalContribution;
ELSE
    ProportionalContribution := LREAL#0.0;
    IntegralIncrement := LREAL#0.0;
    IntegralContribution := LREAL#0.0;
END_IF;
WasAutoMode := AutoMode;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX80.0 : BOOL;
    P1.SafetyAbort AT %MX80.1 : BOOL;
    P1.AutoMode AT %MX80.3 : BOOL;
    P1.Setpoint AT %ML0 : LREAL;
    P1.ProportionalGain AT %ML8 : LREAL;
    P1.IntegralGain AT %ML16 : LREAL;
    P1.Error AT %ML24 : LREAL;
    P1.NominalHeaterPower AT %ML32 : LREAL;
    P1.ProportionalContribution AT %ML40 : LREAL;
    P1.IntegralIncrement AT %ML48 : LREAL;
    P1.IntegralContribution AT %ML56 : LREAL;
    P1.AppliedHeaterPower AT %QL0 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
    P1.HeatExtractionKw AT %QL8 : LREAL;
END_VAR
END_CONFIGURATION
