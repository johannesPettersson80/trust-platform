USING Control;

PROGRAM Main
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#5.0,
        IntegralGain := LREAL#1.0,
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
    ObservedState : Control.PIDInitialState;
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    TrialKind : INT := 0; (* 0:35→38°C target; 1:30kW extraction at SP35. *)
    ProcessGainCPerPoint : LREAL := 0.2;
    TimeConstantS : LREAL := 5.0;
    DelayS : LREAL := 1.0;
    DesiredResponseS : LREAL := 5.0;
    ModelDataValid : BOOL := TRUE;
    ModelRevision : UDINT := 0; (* 0 denotes the provided nominal illustration. *)
    KpPctPerC : LREAL := 5.0;
    KiPctPerCs : LREAL := 1.0;
    DataUsable : BOOL := FALSE;
    AppliedPowerPct : LREAL := 75.0;
    RequestedPowerPct : LREAL := 75.0;
    DisturbanceKw : LREAL := 0.0;
    SetpointC : LREAL := 35.0;
    WaterInputC : LREAL := 35.0;
    WaterTemperatureC : LREAL := 35.0;
    ProbeInputC : LREAL := 35.0;
    TemperatureC : LREAL := 35.0;
    ErrorC : LREAL := 0.0;
    ProportionalPct : LREAL := 0.0;
    IntegralPct : LREAL := 75.0;
    PreviousAppliedPct : LREAL := 75.0;
    CommandActivityPoints : LREAL := 0.0;
    SafeProcessValueC : LREAL := 35.0;
END_VAR

(* The native state-space plant publishes water and probe temperatures through
   ordinary PLC inputs. ST owns controller supervision and diagnostics only. *)
WaterTemperatureC := WaterInputC;
TemperatureC := ProbeInputC;
IF TemperatureC >= LREAL#0.0 AND TemperatureC <= LREAL#80.0 THEN
    SafeProcessValueC := TemperatureC;
    DataUsable := ModelDataValid AND ProcessGainCPerPoint > LREAL#0.0
        AND TimeConstantS > LREAL#0.0 AND DesiredResponseS > LREAL#0.0
        AND DelayS >= LREAL#0.0;
ELSE
    DataUsable := FALSE;
    SafetyAbort := TRUE;
END_IF;

IF NOT ExperimentWasEnabled AND DataUsable THEN
    KpPctPerC := TimeConstantS / (ProcessGainCPerPoint * DesiredResponseS);
    KiPctPerCs := LREAL#1.0 / (ProcessGainCPerPoint * DesiredResponseS);
    Parameters.ProportionalGain := KpPctPerC;
    Parameters.IntegralGain := KiPctPerCs;
END_IF;
Controller.Configure(Parameters := Parameters);

SetpointC := LREAL#35.0;
DisturbanceKw := LREAL#0.0;
IF ExperimentEnabled AND DataUsable AND NOT SafetyAbort THEN
    IF TrialKind = 0 THEN SetpointC := LREAL#38.0;
    ELSE DisturbanceKw := LREAL#30.0;
    END_IF;
    Input.Mode := PIDMode#Auto;
ELSE
    Input.Mode := PIDMode#Manual;
END_IF;
Input.Setpoint := SetpointC;
Input.ProcessValue := SafeProcessValueC;
Input.CycleTime := LREAL#0.1;
Input.ManualOutput := LREAL#75.0;
Input.FeedForward := LREAL#0.0;
Input.TrackingOutput := LREAL#75.0;
ErrorC := SetpointC - Input.ProcessValue;

IF NOT Initialized THEN
    InitialState.Setpoint := LREAL#35.0;
    InitialState.ProcessValue := SafeProcessValueC;
    InitialState.NominalOutput := LREAL#75.0;
    InitialState.SaturatedOutput := LREAL#75.0;
    InitialState.SetpointRate := LREAL#0.0;
    InitialState.ProcessValueRate := LREAL#0.0;
    InitialState.FeedForward := LREAL#0.0;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;
ProportionalPct := LREAL#0.0;
RequestedPowerPct := Input.ManualOutput;
IF Input.Mode = PIDMode#Auto THEN
    ProportionalPct := KpPctPerC * ErrorC;
END_IF;

IF SafetyAbort THEN
    Input.Mode := PIDMode#Manual;
    Input.ManualOutput := LREAL#0.0;
    Input.TrackingOutput := LREAL#0.0;
    RequestedPowerPct := LREAL#0.0;
END_IF;
IF NOT ExperimentWasEnabled AND ExperimentEnabled THEN
    CommandActivityPoints := LREAL#0.0;
END_IF;
AppliedPowerPct := Controller.Update(Input := Input);
ObservedState := Controller.GetState();
RequestedPowerPct := ObservedState.NominalOutput;
IntegralPct := ObservedState.NominalOutput - ProportionalPct;
IF SafetyAbort THEN
    AppliedPowerPct := LREAL#0.0;
    DisturbanceKw := LREAL#0.0;
END_IF;
CommandActivityPoints := CommandActivityPoints
    + ABS(AppliedPowerPct - PreviousAppliedPct);
PreviousAppliedPct := AppliedPowerPct;
ExperimentWasEnabled := ExperimentEnabled;

END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.SafetyAbort AT %MX96.3 : BOOL;
    P1.ExperimentEnabled AT %MX96.1 : BOOL;
    P1.DataUsable AT %MX96.2 : BOOL;
    P1.AppliedPowerPct AT %QL0 : LREAL;
    P1.DisturbanceKw AT %QL8 : LREAL;
    P1.WaterInputC AT %IL0 : LREAL;
    P1.ProbeInputC AT %IL8 : LREAL;
    P1.SetpointC AT %ML0 : LREAL;
    P1.IntegralPct AT %ML16 : LREAL;
    P1.RequestedPowerPct AT %ML32 : LREAL;
    P1.TemperatureC AT %ML40 : LREAL;
    P1.WaterTemperatureC AT %ML48 : LREAL;
    P1.ProportionalPct AT %ML56 : LREAL;
    P1.CommandActivityPoints AT %ML64 : LREAL;
END_VAR
END_CONFIGURATION
