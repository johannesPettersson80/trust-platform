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
        RateMinimum := LREAL#-10000.0,
        RateMaximum := LREAL#10000.0,
        TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#BackCalculation
    );
    InitialState : Control.PIDInitialState;
    BeforeState : Control.PIDInitialState;
    ObservedState : Control.PIDInitialState;
    Input : Control.PIDInput;
    ExperimentEnabled : BOOL := FALSE;
    HasStarted : BOOL := FALSE;
    Initialized : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    AutoMode : BOOL := FALSE;
    ExperimentScan : ULINT := ULINT#0;
    Setpoint : LREAL := 35.0;
    ProcessValue : LREAL := 35.0;
    Error : LREAL := 0.0;
    CycleTime : LREAL := 0.1;
    ProportionalGain : LREAL := 2.0;
    IntegralGain : LREAL := 0.5;
    TrackingTime : LREAL := 1.0;
    IntegralCorrection : LREAL := 75.0;
    ProportionalCorrection : LREAL := 0.0;
    IntegralIncrement : LREAL := 0.0;
    TrialIntegral : LREAL := 75.0;
    TrackingCorrection : LREAL := 0.0;
    RequestedHeaterPower : LREAL := 75.0;
    PreviousRequestedHeaterPower : LREAL := 75.0;
    PairedRequestedHeaterPower : LREAL := 75.0;
    HeaterCommand : LREAL := 75.0;
    AppliedHeaterPower : LREAL := 75.0;
    UnavailablePower : LREAL := 0.0;
    ManualHeaterPower : LREAL := 75.0;
END_VAR

(* Corrected commissioning setting: PID and the native heater both declare the same 0..100% range. *)
ProportionalGain := Parameters.ProportionalGain;
IntegralGain := Parameters.IntegralGain;
TrackingTime := Parameters.TrackingTime;
CycleTime := LREAL#0.1;
ManualHeaterPower := LREAL#75.0;
IF ProcessValue <> ProcessValue OR ProcessValue < LREAL#0.0
    OR ProcessValue > LREAL#80.0 THEN SafetyAbort := TRUE;
ELSE Input.ProcessValue := ProcessValue;
END_IF;
AutoMode := ExperimentEnabled AND NOT SafetyAbort;
IF AutoMode THEN
    HasStarted := TRUE;
    ExperimentScan := ExperimentScan + ULINT#1;
    IF ExperimentScan <= ULINT#100 THEN Setpoint := LREAL#45.0;
    ELSE Setpoint := LREAL#35.0;
    END_IF;
    Input.Mode := PIDMode#Auto;
ELSE
    ExperimentScan := ULINT#0;
    Setpoint := LREAL#35.0;
    Input.Mode := PIDMode#Manual;
END_IF;
Input.ManualOutput := LREAL#0.0;
IF NOT HasStarted AND NOT SafetyAbort THEN
    Input.ManualOutput := ManualHeaterPower;
END_IF;
Input.Setpoint := Setpoint;
Input.CycleTime := CycleTime;
Input.FeedForward := LREAL#0.0;
Input.TrackingOutput := LREAL#0.0;
Error := Setpoint - Input.ProcessValue;

Controller.Configure(Parameters := Parameters);
IF NOT Initialized THEN
    (* Preserve the 35 degC / 75% hot history, including the first SP step.
       The simulator separately owns the initial 75% transport history. *)
    InitialState.Setpoint := LREAL#35.0;
    InitialState.SetpointRate := LREAL#0.0;
    InitialState.ProcessValue := Input.ProcessValue;
    InitialState.ProcessValueRate := LREAL#0.0;
    InitialState.FeedForward := LREAL#0.0;
    InitialState.NominalOutput := LREAL#75.0;
    InitialState.SaturatedOutput := LREAL#75.0;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;
BeforeState := Controller.GetState();

(* Diagnostics reconstruct this call's candidate from its real prior state.
   Fixed Kp, weight one, and no D/feedforward make nominal = P + equivalent I.
   These values neither integrate independent state nor own the actuator. *)
ProportionalCorrection := LREAL#0.0;
IntegralIncrement := LREAL#0.0;
RequestedHeaterPower := Input.ManualOutput;
IF AutoMode THEN
    ProportionalCorrection := ProportionalGain * Error;
    IntegralIncrement := IntegralGain * Error * CycleTime;
    RequestedHeaterPower := BeforeState.NominalOutput
        + ProportionalGain * ((Setpoint - BeforeState.Setpoint)
            - (Input.ProcessValue - BeforeState.ProcessValue))
        + IntegralIncrement;
END_IF;
HeaterCommand := Controller.Update(Input := Input);
ObservedState := Controller.GetState();
TrialIntegral := RequestedHeaterPower - ProportionalCorrection;
IntegralCorrection := ObservedState.NominalOutput - ProportionalCorrection;
TrackingCorrection := ObservedState.NominalOutput - RequestedHeaterPower;

(* IL8 reports the completed prior actuator step. Pair it with that step's
   request; comparing it with the newly calculated request would mix scans. *)
PairedRequestedHeaterPower := PreviousRequestedHeaterPower;
UnavailablePower := PairedRequestedHeaterPower - AppliedHeaterPower;
PreviousRequestedHeaterPower := RequestedHeaterPower;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.Setpoint AT %ML0 : LREAL;
    P1.RequestedHeaterPower AT %ML8 : LREAL;
    P1.UnavailablePower AT %ML16 : LREAL;
    P1.ProportionalGain AT %ML24 : LREAL;
    P1.IntegralGain AT %ML32 : LREAL;
    P1.ManualHeaterPower AT %ML40 : LREAL;
    P1.ProportionalCorrection AT %ML48 : LREAL;
    P1.IntegralIncrement AT %ML56 : LREAL;
    P1.IntegralCorrection AT %ML64 : LREAL;
    P1.TrackingTime AT %ML72 : LREAL;
    P1.TrackingCorrection AT %ML80 : LREAL;
    P1.PairedRequestedHeaterPower AT %ML88 : LREAL;
    P1.ExperimentEnabled AT %MX112.0 : BOOL;
    P1.AutoMode AT %MX112.1 : BOOL;
    P1.SafetyAbort AT %MX112.2 : BOOL;
    P1.HeaterCommand AT %QL0 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
    P1.AppliedHeaterPower AT %IL8 : LREAL;
END_VAR
END_CONFIGURATION
