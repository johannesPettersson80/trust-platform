USING Control;

FUNCTION Lesson58InstalledTargetFlow : LREAL
VAR_INPUT StemPercent : LREAL; END_VAR
Lesson58InstalledTargetFlow := LREAL#100.0
    * (StemPercent / LREAL#100.0) * (StemPercent / LREAL#100.0);
END_FUNCTION

FUNCTION Lesson58ReviewedManualDemand : LREAL
VAR_INPUT
    ExperimentElapsedSeconds : LREAL;
    ExperimentEnabled : BOOL;
    InputsValid : BOOL;
    SafetyAbort : BOOL;
END_VAR
IF NOT ExperimentEnabled OR NOT InputsValid OR SafetyAbort THEN
    Lesson58ReviewedManualDemand := LREAL#0.0;
ELSIF ExperimentElapsedSeconds < LREAL#10.0 THEN Lesson58ReviewedManualDemand := LREAL#0.0;
ELSIF ExperimentElapsedSeconds < LREAL#20.0 THEN Lesson58ReviewedManualDemand := LREAL#20.0;
ELSIF ExperimentElapsedSeconds < LREAL#30.0 THEN Lesson58ReviewedManualDemand := LREAL#40.0;
ELSIF ExperimentElapsedSeconds < LREAL#40.0 THEN Lesson58ReviewedManualDemand := LREAL#60.0;
ELSIF ExperimentElapsedSeconds < LREAL#50.0 THEN Lesson58ReviewedManualDemand := LREAL#80.0;
ELSIF ExperimentElapsedSeconds < LREAL#60.0 THEN Lesson58ReviewedManualDemand := LREAL#100.0;
ELSIF ExperimentElapsedSeconds < LREAL#70.0 THEN Lesson58ReviewedManualDemand := LREAL#70.0;
ELSIF ExperimentElapsedSeconds < LREAL#71.0 THEN Lesson58ReviewedManualDemand := LREAL#70.5;
ELSIF ExperimentElapsedSeconds < LREAL#72.0 THEN Lesson58ReviewedManualDemand := LREAL#71.0;
ELSIF ExperimentElapsedSeconds < LREAL#73.0 THEN Lesson58ReviewedManualDemand := LREAL#71.5;
ELSIF ExperimentElapsedSeconds < LREAL#74.0 THEN Lesson58ReviewedManualDemand := LREAL#72.0;
ELSIF ExperimentElapsedSeconds < LREAL#75.0 THEN Lesson58ReviewedManualDemand := LREAL#72.5;
ELSIF ExperimentElapsedSeconds < LREAL#76.0 THEN Lesson58ReviewedManualDemand := LREAL#73.0;
ELSIF ExperimentElapsedSeconds < LREAL#77.0 THEN Lesson58ReviewedManualDemand := LREAL#73.5;
ELSIF ExperimentElapsedSeconds < LREAL#78.0 THEN Lesson58ReviewedManualDemand := LREAL#74.0;
ELSIF ExperimentElapsedSeconds < LREAL#79.0 THEN Lesson58ReviewedManualDemand := LREAL#74.5;
ELSIF ExperimentElapsedSeconds < LREAL#80.0 THEN Lesson58ReviewedManualDemand := LREAL#75.0;
ELSIF ExperimentElapsedSeconds < LREAL#82.0 THEN Lesson58ReviewedManualDemand := LREAL#75.5;
ELSIF ExperimentElapsedSeconds < LREAL#83.0 THEN Lesson58ReviewedManualDemand := LREAL#75.0;
ELSIF ExperimentElapsedSeconds < LREAL#84.0 THEN Lesson58ReviewedManualDemand := LREAL#74.5;
ELSIF ExperimentElapsedSeconds < LREAL#85.0 THEN Lesson58ReviewedManualDemand := LREAL#74.0;
ELSIF ExperimentElapsedSeconds < LREAL#86.0 THEN Lesson58ReviewedManualDemand := LREAL#73.5;
ELSIF ExperimentElapsedSeconds < LREAL#87.0 THEN Lesson58ReviewedManualDemand := LREAL#73.0;
ELSIF ExperimentElapsedSeconds < LREAL#88.0 THEN Lesson58ReviewedManualDemand := LREAL#72.5;
ELSIF ExperimentElapsedSeconds < LREAL#89.0 THEN Lesson58ReviewedManualDemand := LREAL#72.0;
ELSIF ExperimentElapsedSeconds < LREAL#90.0 THEN Lesson58ReviewedManualDemand := LREAL#71.5;
ELSIF ExperimentElapsedSeconds < LREAL#91.0 THEN Lesson58ReviewedManualDemand := LREAL#71.0;
ELSIF ExperimentElapsedSeconds < LREAL#92.0 THEN Lesson58ReviewedManualDemand := LREAL#70.5;
ELSIF ExperimentElapsedSeconds < LREAL#94.0 THEN Lesson58ReviewedManualDemand := LREAL#70.0;
ELSE Lesson58ReviewedManualDemand := LREAL#80.0;
END_IF;
END_FUNCTION

FUNCTION Lesson58ReviewedAutomaticMode : BOOL
VAR_INPUT
    ExperimentElapsedSeconds : LREAL;
    ExperimentEnabled : BOOL;
    InputsValid : BOOL;
    SafetyAbort : BOOL;
END_VAR
Lesson58ReviewedAutomaticMode := ExperimentEnabled AND InputsValid
    AND (NOT SafetyAbort) AND (ExperimentElapsedSeconds >= LREAL#110.0);
END_FUNCTION

FUNCTION Lesson58ReviewedFlowSetpoint : LREAL
VAR_INPUT AutomaticMode : BOOL; END_VAR
IF AutomaticMode THEN
    Lesson58ReviewedFlowSetpoint := LREAL#70.0;
ELSE
    Lesson58ReviewedFlowSetpoint := LREAL#64.0;
END_IF;
END_FUNCTION

PROGRAM Main
VAR
    HealthyController : Control.PID;
    DeadbandController : Control.PID;
    StictionController : Control.PID;
    Parameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    HealthyInput : Control.PIDInput;
    DeadbandInput : Control.PIDInput;
    StictionInput : Control.PIDInput;
    Initialized : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ExperimentElapsed : LREAL := 0.0;
    AutoMode : BOOL := FALSE;
    ManualDemand : LREAL := 80.0;
    FlowSetpoint : LREAL := 64.0;
    HealthyFlow : LREAL := 64.0;
    DeadbandFlow : LREAL := 64.0;
    StictionFlow : LREAL := 64.0;
    HealthyRequest : LREAL := 80.0;
    DeadbandRequest : LREAL := 80.0;
    StictionRequest : LREAL := 80.0;
END_VAR

IF NOT Initialized THEN
    ManualDemand := LREAL#80.0;
    (* One parameter record keeps tuning identical while only valve condition changes. *)
    Parameters.ProportionalGain := LREAL#0.5;
    Parameters.IntegralGain := LREAL#0.25;
    Parameters.DerivativeGain := LREAL#0.0;
    Parameters.ProportionalSetpointWeight := LREAL#1.0;
    Parameters.DerivativeSetpointWeight := LREAL#0.0;
    Parameters.Bias := LREAL#80.0;
    Parameters.OutputMinimum := LREAL#0.0;
    Parameters.OutputMaximum := LREAL#100.0;
    Parameters.RateMinimum := LREAL#-1000.0;
    Parameters.RateMaximum := LREAL#1000.0;
    Parameters.TrackingTime := LREAL#0.5;
    Parameters.AntiWindup := PIDAntiWindup#Combined;
    HealthyController.Configure(Parameters := Parameters);
    DeadbandController.Configure(Parameters := Parameters);
    StictionController.Configure(Parameters := Parameters);

    (* Initialize at the measured operating point for bumpless Manual or Auto entry. *)
    InitialState.Setpoint := FlowSetpoint;
    InitialState.ProcessValue := HealthyFlow;
    InitialState.NominalOutput := LREAL#80.0;
    InitialState.SaturatedOutput := LREAL#80.0;
    HealthyController.Initialize(State := InitialState);
    InitialState.ProcessValue := DeadbandFlow;
    DeadbandController.Initialize(State := InitialState);
    InitialState.ProcessValue := StictionFlow;
    StictionController.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;

(* Control Studio grants only authority. The project owns characterization,
   one reversal probe, the closed-loop observation, and safe closure. *)
IF ExperimentEnabled AND InputsValid AND NOT SafetyAbort THEN
    ExperimentElapsed := ExperimentElapsed + LREAL#0.1;
ELSE
    ExperimentElapsed := LREAL#0.0;
END_IF;
AutoMode := Lesson58ReviewedAutomaticMode(
    ExperimentElapsedSeconds := ExperimentElapsed,
    ExperimentEnabled := ExperimentEnabled,
    InputsValid := InputsValid,
    SafetyAbort := SafetyAbort);
FlowSetpoint := Lesson58ReviewedFlowSetpoint(AutomaticMode := AutoMode);
ManualDemand := Lesson58ReviewedManualDemand(
    ExperimentElapsedSeconds := ExperimentElapsed,
    ExperimentEnabled := ExperimentEnabled,
    InputsValid := InputsValid,
    SafetyAbort := SafetyAbort);

HealthyInput.Setpoint := FlowSetpoint;
HealthyInput.ProcessValue := HealthyFlow;
HealthyInput.CycleTime := LREAL#0.1;
DeadbandInput.Setpoint := FlowSetpoint;
DeadbandInput.ProcessValue := DeadbandFlow;
DeadbandInput.CycleTime := LREAL#0.1;
StictionInput.Setpoint := FlowSetpoint;
StictionInput.ProcessValue := StictionFlow;
StictionInput.CycleTime := LREAL#0.1;

IF AutoMode THEN
    HealthyInput.Mode := PIDMode#Auto;
    DeadbandInput.Mode := PIDMode#Auto;
    StictionInput.Mode := PIDMode#Auto;
ELSE
    (* The common manual demand makes characterization and reversal probes equivalent. *)
    HealthyInput.Mode := PIDMode#Manual;
    DeadbandInput.Mode := PIDMode#Manual;
    StictionInput.Mode := PIDMode#Manual;
    HealthyInput.ManualOutput := ManualDemand;
    DeadbandInput.ManualOutput := ManualDemand;
    StictionInput.ManualOutput := ManualDemand;
END_IF;

HealthyRequest := HealthyController.Update(Input := HealthyInput);
DeadbandRequest := DeadbandController.Update(Input := DeadbandInput);
StictionRequest := StictionController.Update(Input := StictionInput);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.AutoMode AT %MX91.0 : BOOL;
    P1.ExperimentEnabled AT %MX91.1 : BOOL;
    P1.SafetyAbort AT %MX91.2 : BOOL;
    P1.InputsValid AT %MX91.3 : BOOL;
    P1.ManualDemand AT %ML328 : LREAL;
    P1.FlowSetpoint AT %ML336 : LREAL;
    P1.HealthyFlow AT %IL168 : LREAL;
    P1.DeadbandFlow AT %IL176 : LREAL;
    P1.StictionFlow AT %IL184 : LREAL;
    P1.HealthyRequest AT %QL184 : LREAL;
    P1.DeadbandRequest AT %QL192 : LREAL;
    P1.StictionRequest AT %QL200 : LREAL;
END_VAR
END_CONFIGURATION
