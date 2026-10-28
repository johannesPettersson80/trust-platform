USING Control;

PROGRAM Main
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#2.0,
        IntegralGain := LREAL#0.0,
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
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    AutoMode : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    ExperimentScan : ULINT := ULINT#0;
    Setpoint : LREAL := 35.0;
    ProportionalGain : LREAL := 2.0;
    ProcessValue : LREAL := 20.0;
    ErrorC : LREAL := 15.0;
    RequestedHeaterPower : LREAL := 0.0;
    AppliedHeaterPower : LREAL := 0.0;
    HeatExtractionKw : LREAL := 0.0;
END_VAR
Setpoint := LREAL#35.0;
ProportionalGain := Parameters.ProportionalGain;
IF ProcessValue <> ProcessValue OR ProcessValue < LREAL#0.0
    OR ProcessValue > LREAL#80.0 THEN SafetyAbort := TRUE;
ELSE
    (* Keep invalid measurements outside the library's arithmetic domain. *)
    Input.ProcessValue := ProcessValue;
END_IF;
AutoMode := ExperimentEnabled AND NOT SafetyAbort;
HeatExtractionKw := LREAL#0.0;
ErrorC := Setpoint - ProcessValue;
IF AutoMode THEN
    ExperimentScan := ExperimentScan + ULINT#1;
    IF ExperimentScan >= ULINT#601 THEN HeatExtractionKw := LREAL#30.0; END_IF;
    (* Diagnostic P request for the plot; only Controller.Update drives heat. *)
    RequestedHeaterPower := ProportionalGain * ErrorC;
ELSE
    ExperimentScan := ULINT#0;
    RequestedHeaterPower := LREAL#0.0;
END_IF;
Controller.Configure(Parameters := Parameters);
IF NOT Initialized THEN
    InitialState.Setpoint := Setpoint;
    InitialState.ProcessValue := Input.ProcessValue;
    InitialState.NominalOutput := AppliedHeaterPower;
    InitialState.SaturatedOutput := AppliedHeaterPower;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;
Input.Setpoint := Setpoint;
Input.CycleTime := LREAL#0.1;
Input.ManualOutput := LREAL#0.0;
IF AutoMode THEN Input.Mode := PIDMode#Auto;
ELSE Input.Mode := PIDMode#Manual;
END_IF;
(* Ki=Kd=Bias=0 selects the library's P equation and its own output limits.
   The 1000 point/s rate bounds permit the complete 0..100% range per scan. *)
AppliedHeaterPower := Controller.Update(Input := Input);
END_PROGRAM
CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX24.1 : BOOL;
    P1.SafetyAbort AT %MX24.2 : BOOL;
    P1.AutoMode AT %MX24.3 : BOOL;
    P1.Setpoint AT %ML0 : LREAL;
    P1.ProportionalGain AT %ML8 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
    P1.ErrorC AT %ML32 : LREAL;
    P1.RequestedHeaterPower AT %ML40 : LREAL;
    P1.AppliedHeaterPower AT %QL0 : LREAL;
    P1.HeatExtractionKw AT %QL8 : LREAL;
END_VAR
END_CONFIGURATION
