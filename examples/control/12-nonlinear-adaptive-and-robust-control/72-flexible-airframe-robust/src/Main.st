USING Control;

PROGRAM Main
VAR
    NominalController : Control.PID;
    RobustMeasurementFilter : Control.LowPass;
    RobustController : Control.PID;
    NominalParameters : Control.PIDParameters;
    RobustFilterParameters : Control.LowPassParameters;
    RobustParameters : Control.PIDParameters;
    NominalInitialState : Control.PIDInitialState;
    RobustInitialState : Control.PIDInitialState;
    NominalInput : Control.PIDInput;
    RobustInput : Control.PIDInput;
    Initialized : BOOL := FALSE;
    ControllerEnabled : BOOL := FALSE;
    DataHealthy : BOOL := TRUE;
    NominalSaturated : BOOL := FALSE;
    RobustSaturated : BOOL := FALSE;
    ScanCount : DINT := 0;
    ElapsedTime : LREAL := 0.0;
    Setpoint : LREAL := 0.0;
    GustLoad : LREAL := 0.0;
    NominalBending : LREAL := 0.0;
    RobustBending : LREAL := 0.0;
    FilteredRobustBending : LREAL := 0.0;
    LastFilteredBending : LREAL := 0.0;
    NominalRequested : LREAL := 0.0;
    RobustRequested : LREAL := 0.0;
    NominalApplied : LREAL := 0.0;
    RobustApplied : LREAL := 0.0;
    NominalGust : LREAL := 0.0;
    RobustGust : LREAL := 0.0;
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
END_VAR

IF NOT Initialized THEN
    NominalParameters.ProportionalGain := LREAL#40.0;
    NominalParameters.IntegralGain := LREAL#0.0;
    NominalParameters.DerivativeGain := LREAL#0.0;
    NominalParameters.ProportionalSetpointWeight := LREAL#1.0;
    NominalParameters.DerivativeSetpointWeight := LREAL#0.0;
    NominalParameters.Bias := LREAL#0.0;
    NominalParameters.OutputMinimum := LREAL#-15.0;
    NominalParameters.OutputMaximum := LREAL#15.0;
    NominalParameters.RateMinimum := LREAL#-1000.0;
    NominalParameters.RateMaximum := LREAL#1000.0;
    NominalParameters.TrackingTime := LREAL#0.1;
    NominalParameters.AntiWindup := PIDAntiWindup#Combined;
    NominalController.Configure(Parameters := NominalParameters);

    RobustFilterParameters.TimeConstant := LREAL#0.05;
    RobustFilterParameters.Order := FilterOrder#FirstOrder;
    RobustMeasurementFilter.Configure(Parameters := RobustFilterParameters);
    RobustMeasurementFilter.Initialize(Input := RobustBending);

    RobustParameters.ProportionalGain := LREAL#10.0;
    RobustParameters.IntegralGain := LREAL#0.0;
    RobustParameters.DerivativeGain := LREAL#3.0;
    RobustParameters.ProportionalSetpointWeight := LREAL#1.0;
    RobustParameters.DerivativeSetpointWeight := LREAL#0.0;
    RobustParameters.Bias := LREAL#0.0;
    RobustParameters.OutputMinimum := LREAL#-15.0;
    RobustParameters.OutputMaximum := LREAL#15.0;
    RobustParameters.RateMinimum := LREAL#-1000.0;
    RobustParameters.RateMaximum := LREAL#1000.0;
    RobustParameters.TrackingTime := LREAL#0.1;
    RobustParameters.AntiWindup := PIDAntiWindup#Combined;
    RobustController.Configure(Parameters := RobustParameters);

    NominalInitialState.Setpoint := Setpoint;
    NominalInitialState.ProcessValue := NominalBending;
    NominalInitialState.ProcessValueRate := LREAL#0.0;
    NominalInitialState.NominalOutput := LREAL#0.0;
    NominalInitialState.SaturatedOutput := LREAL#0.0;
    NominalController.Initialize(State := NominalInitialState);

    RobustInitialState.Setpoint := Setpoint;
    RobustInitialState.ProcessValue := RobustBending;
    RobustInitialState.ProcessValueRate := LREAL#0.0;
    RobustInitialState.NominalOutput := LREAL#0.0;
    RobustInitialState.SaturatedOutput := LREAL#0.0;
    RobustController.Initialize(State := RobustInitialState);
    LastFilteredBending := RobustBending;
    Initialized := TRUE;
END_IF;

(* Control Studio grants only authority. The project owns the nominal gust
   record; uncertainty-grid and quality cases remain native tests. *)
IF ExperimentEnabled AND InputsValid AND NOT SafetyAbort THEN
    ControllerEnabled := TRUE;
    DataHealthy := TRUE;
    IF (ScanCount >= DINT#250) AND (ScanCount < DINT#300) THEN
        GustLoad := LREAL#0.10;
    ELSE
        GustLoad := LREAL#0.0;
    END_IF;
    ScanCount := ScanCount + DINT#1;
    ElapsedTime := ElapsedTime + LREAL#0.02;
ELSE
    ControllerEnabled := FALSE;
    DataHealthy := TRUE;
    GustLoad := LREAL#0.0;
    ScanCount := DINT#0;
    ElapsedTime := LREAL#0.0;
END_IF;
NominalGust := GustLoad;
RobustGust := GustLoad;

FilteredRobustBending := RobustMeasurementFilter.Update(
    Input := RobustBending,
    CycleTime := LREAL#0.02
);

// Expose controller requests separately from the physically limited outputs.
NominalRequested := LREAL#-40.0 * NominalBending;
RobustRequested := LREAL#-10.0 * FilteredRobustBending
    - LREAL#3.0
        * (FilteredRobustBending - LastFilteredBending) / LREAL#0.02;

NominalInput.Setpoint := Setpoint;
NominalInput.ProcessValue := NominalBending;
NominalInput.FeedForward := LREAL#0.0;
NominalInput.CycleTime := LREAL#0.02;
RobustInput.Setpoint := Setpoint;
RobustInput.ProcessValue := FilteredRobustBending;
RobustInput.FeedForward := LREAL#0.0;
RobustInput.CycleTime := LREAL#0.02;

IF ControllerEnabled AND DataHealthy THEN
    NominalInput.Mode := PIDMode#Auto;
    RobustInput.Mode := PIDMode#Auto;
ELSE
    // Invalid bending data cannot command a safety-relevant surface.
    NominalInput.Mode := PIDMode#Manual;
    NominalInput.ManualOutput := LREAL#0.0;
    RobustInput.Mode := PIDMode#Manual;
    RobustInput.ManualOutput := LREAL#0.0;
END_IF;

NominalApplied := NominalController.Update(Input := NominalInput);
RobustApplied := RobustController.Update(Input := RobustInput);
NominalSaturated := ControllerEnabled AND DataHealthy
    AND (ABS(NominalRequested - NominalApplied) > LREAL#0.000000001);
RobustSaturated := ControllerEnabled AND DataHealthy
    AND (ABS(RobustRequested - RobustApplied) > LREAL#0.000000001);

LastFilteredBending := FilteredRobustBending;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ControllerEnabled AT %MX64.0 : BOOL;
    P1.DataHealthy AT %MX64.1 : BOOL;
    P1.ExperimentEnabled AT %MX64.2 : BOOL;
    P1.SafetyAbort AT %MX64.3 : BOOL;
    P1.InputsValid AT %MX64.4 : BOOL;
    P1.NominalSaturated AT %MX65.0 : BOOL;
    P1.RobustSaturated AT %MX65.1 : BOOL;
    P1.NominalRequested AT %ML72 : LREAL;
    P1.RobustRequested AT %ML80 : LREAL;
    P1.FilteredRobustBending AT %ML88 : LREAL;
    P1.GustLoad AT %ML96 : LREAL;
    P1.ElapsedTime AT %ML104 : LREAL;
    P1.NominalApplied AT %QL0 : LREAL;
    P1.NominalGust AT %QL8 : LREAL;
    P1.RobustApplied AT %QL16 : LREAL;
    P1.RobustGust AT %QL24 : LREAL;
    P1.NominalBending AT %IL0 : LREAL;
    P1.RobustBending AT %IL8 : LREAL;
END_VAR
END_CONFIGURATION
