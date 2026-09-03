USING Control;

PROGRAM Main
VAR
    ControllerEnabled : BOOL := TRUE;
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ExperimentScan : DINT := DINT#0;
    ExperimentElapsed : LREAL := 0.0;
    FlowQualityGood : BOOL := TRUE;
    ReferenceQualityGood : BOOL := TRUE;
    ScheduleTransition : BOOL := FALSE;
    Setpoint : LREAL := 50.0;
    ProcessValue : LREAL := 50.0;
    ReferenceFlow : LREAL := 50.0;
    ValveStemPosition : LREAL := 50.0;
    UpstreamPressure : LREAL := 8.0;
    AdjacentValveDemand : LREAL := 0.0;
    MeasuredCycleTime : LREAL := 0.1;
    ConfiguredProportionalGain : LREAL := 2.0;
    ConfiguredIntegralGain : LREAL := 0.2;
    OperatingRegion : INT := 0;
    ControllerMode : INT := 1;
    ValveRequested : LREAL := 50.0;
    Controller : Control.PID;
    Parameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
END_VAR

IF NOT Initialized THEN
    Parameters.ProportionalGain := ConfiguredProportionalGain;
    Parameters.IntegralGain := ConfiguredIntegralGain;
    Parameters.DerivativeGain := LREAL#0.0;
    Parameters.Bias := LREAL#50.0;
    Parameters.ProportionalSetpointWeight := LREAL#1.0;
    Parameters.DerivativeSetpointWeight := LREAL#0.0;
    Parameters.OutputMinimum := LREAL#0.0;
    Parameters.OutputMaximum := LREAL#100.0;
    Parameters.RateMinimum := LREAL#-1000.0;
    Parameters.RateMaximum := LREAL#1000.0;
    Parameters.TrackingTime := LREAL#1.0;
    Parameters.AntiWindup := PIDAntiWindup#Combined;
    InitialState.Setpoint := Setpoint;
    InitialState.ProcessValue := ProcessValue;
    InitialState.NominalOutput := LREAL#50.0;
    InitialState.SaturatedOutput := LREAL#50.0;
    Controller.Configure(Parameters := Parameters);
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;

(* The visible run is one healthy reference record. Control Studio grants one
   Boolean authority; this project owns its setpoint event and restoration.
   The seven degraded configurations remain separately blinded native cases. *)
IF ExperimentEnabled AND InputsValid AND NOT SafetyAbort THEN
    IF NOT ExperimentWasEnabled THEN
        ExperimentScan := DINT#0;
        ExperimentElapsed := LREAL#0.0;
    END_IF;
    ExperimentScan := ExperimentScan + DINT#1;
    ExperimentElapsed := ExperimentElapsed + LREAL#0.1;
    ControllerEnabled := TRUE;
    IF ExperimentScan <= DINT#100 THEN
        Setpoint := LREAL#50.0;
    ELSIF ExperimentScan <= DINT#400 THEN
        Setpoint := LREAL#60.0;
    ELSE
        Setpoint := LREAL#50.0;
    END_IF;
ELSIF ExperimentWasEnabled THEN
    ExperimentScan := DINT#0;
    ExperimentElapsed := LREAL#0.0;
    ControllerEnabled := FALSE;
    Setpoint := LREAL#50.0;
END_IF;

Input.Setpoint := Setpoint;
Input.ProcessValue := ProcessValue;
Input.FeedForward := LREAL#0.0;
IF (MeasuredCycleTime >= LREAL#0.01) AND (MeasuredCycleTime <= LREAL#1.0) THEN
    Input.CycleTime := MeasuredCycleTime;
ELSE
    Input.CycleTime := LREAL#0.1;
END_IF;

IF ControllerEnabled AND FlowQualityGood AND (ControllerMode = INT#1) THEN
    Input.Mode := PIDMode#Auto;
ELSE
    (* Track measured stem position so re-entry does not add a controller step. *)
    Input.Mode := PIDMode#Track;
    Input.TrackingOutput := ValveStemPosition;
END_IF;
ValveRequested := Controller.Update(Input := Input);
ExperimentWasEnabled := ExperimentEnabled;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ControllerEnabled AT %MX16.0 : BOOL;
    P1.FlowQualityGood AT %MX16.1 : BOOL;
    P1.ReferenceQualityGood AT %MX16.2 : BOOL;
    P1.ScheduleTransition AT %MX16.3 : BOOL;
    P1.ExperimentEnabled AT %MX16.4 : BOOL;
    P1.SafetyAbort AT %MX16.5 : BOOL;
    P1.InputsValid AT %MX16.6 : BOOL;
    P1.ControllerMode AT %MW70 : INT;
    P1.OperatingRegion AT %MW72 : INT;
    P1.Setpoint AT %ML0 : LREAL;
    P1.ReferenceFlow AT %IL8 : LREAL;
    P1.ValveStemPosition AT %IL16 : LREAL;
    P1.UpstreamPressure AT %IL24 : LREAL;
    P1.AdjacentValveDemand AT %IL32 : LREAL;
    P1.MeasuredCycleTime AT %ML40 : LREAL;
    P1.ConfiguredProportionalGain AT %ML48 : LREAL;
    P1.ConfiguredIntegralGain AT %ML56 : LREAL;
    P1.ExperimentElapsed AT %ML80 : LREAL;
    P1.ValveRequested AT %QL0 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
END_VAR
END_CONFIGURATION
