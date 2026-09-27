USING Control;

PROGRAM Main
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#5.0, IntegralGain := LREAL#1.0,
        DerivativeGain := LREAL#0.0, ProportionalSetpointWeight := LREAL#1.0,
        DerivativeSetpointWeight := LREAL#0.0, Bias := LREAL#0.0,
        OutputMinimum := LREAL#0.0, OutputMaximum := LREAL#100.0,
        RateMinimum := LREAL#-10000.0, RateMaximum := LREAL#10000.0,
        TrackingTime := LREAL#1.0, AntiWindup := PIDAntiWindup#BackCalculation
    );
    InitialState : Control.PIDInitialState;
    ObservedState : Control.PIDInitialState;
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    TrialKind : INT := 0;
    CandidateRevision : LREAL := 0.0;
    KpPctPerC : LREAL := 5.0;
    KiPctPerCs : LREAL := 1.0;
    ActiveKpPctPerC : LREAL := 5.0;
    ActiveKiPctPerCs : LREAL := 1.0;
    DataUsable : BOOL := FALSE;
    TemperatureC : LREAL := 35.0;
    WaterInputC : LREAL := 35.0;
    WaterTemperatureC : LREAL := 35.0;
    SetpointC : LREAL := 35.0;
    DisturbanceKw : LREAL := 0.0;
    ErrorC : LREAL := 0.0;
    ProportionalPct : LREAL := 0.0;
    IntegralPct : LREAL := 75.0;
    RequestedPowerPct : LREAL := 75.0;
    AppliedPowerPct : LREAL := 75.0;
    PreviousAppliedPct : LREAL := 75.0;
    CommandActivityPoints : LREAL := 0.0;
END_VAR

(* The simulator owns the heater and publishes measured water temperature
   through normal PLC I/O. Control.PID owns retained state and output. *)
WaterTemperatureC := WaterInputC;
DataUsable := KpPctPerC >= LREAL#0.0 AND KpPctPerC <= LREAL#20.0
    AND KiPctPerCs >= LREAL#0.0 AND KiPctPerCs <= LREAL#10.0
    AND TemperatureC >= LREAL#0.0 AND TemperatureC <= LREAL#80.0;
IF NOT DataUsable THEN SafetyAbort := TRUE; END_IF;
IF ExperimentEnabled AND ExperimentWasEnabled
    AND (KpPctPerC <> ActiveKpPctPerC OR KiPctPerCs <> ActiveKiPctPerCs) THEN
    SafetyAbort := TRUE;
END_IF;
IF NOT ExperimentEnabled THEN
    ActiveKpPctPerC := KpPctPerC;
    ActiveKiPctPerCs := KiPctPerCs;
END_IF;
Parameters.ProportionalGain := KpPctPerC;
Parameters.IntegralGain := KiPctPerCs;
Controller.Configure(Parameters := Parameters);

SetpointC := LREAL#35.0;
DisturbanceKw := LREAL#0.0;
IF ExperimentEnabled AND DataUsable AND NOT SafetyAbort THEN
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
AppliedPowerPct := Controller.Update(Input := Input);
ObservedState := Controller.GetState();
RequestedPowerPct := ObservedState.NominalOutput;
ErrorC := SetpointC - TemperatureC;
ProportionalPct := KpPctPerC * ErrorC;
IntegralPct := ObservedState.NominalOutput - ProportionalPct;
IF ExperimentEnabled AND NOT ExperimentWasEnabled THEN
    CommandActivityPoints := LREAL#0.0;
    PreviousAppliedPct := LREAL#75.0;
END_IF;
IF ExperimentEnabled AND NOT SafetyAbort THEN
    CommandActivityPoints := CommandActivityPoints
        + ABS(AppliedPowerPct - PreviousAppliedPct);
    PreviousAppliedPct := AppliedPowerPct;
ELSIF NOT ExperimentEnabled THEN
    PreviousAppliedPct := LREAL#75.0;
END_IF;
IF SafetyAbort THEN AppliedPowerPct := LREAL#0.0; END_IF;
ExperimentWasEnabled := ExperimentEnabled;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.SafetyAbort AT %MX104.2 : BOOL;
    P1.ExperimentEnabled AT %MX104.0 : BOOL;
    P1.DataUsable AT %MX104.1 : BOOL;
    P1.TrialKind AT %MW80 : INT;
    P1.CandidateRevision AT %ML96 : LREAL;
    P1.KpPctPerC AT %ML72 : LREAL;
    P1.KiPctPerCs AT %ML88 : LREAL;
    P1.AppliedPowerPct AT %QL0 : LREAL;
    P1.DisturbanceKw AT %QL8 : LREAL;
    P1.TemperatureC AT %IL0 : LREAL;
    P1.WaterInputC AT %IL8 : LREAL;
    P1.SetpointC AT %ML0 : LREAL;
    P1.IntegralPct AT %ML16 : LREAL;
    P1.RequestedPowerPct AT %ML32 : LREAL;
    P1.WaterTemperatureC AT %ML48 : LREAL;
    P1.ProportionalPct AT %ML56 : LREAL;
    P1.CommandActivityPoints AT %ML64 : LREAL;
END_VAR
END_CONFIGURATION
