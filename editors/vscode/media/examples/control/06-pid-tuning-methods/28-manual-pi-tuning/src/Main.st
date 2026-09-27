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
    Initialized : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    HasStarted : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    AutoMode : BOOL := FALSE;
    TrialKind : INT := 0; (* 0:35→38°C target; 1:30kW extraction at SP35. *)
    KpPctPerC : LREAL := 2.0;
    KiPctPerCs : LREAL := 0.5;
    WaterTemperatureC : LREAL := 35.0;
    TemperatureC : LREAL := 35.0;
    SetpointC : LREAL := 35.0;
    DisturbanceKw : LREAL := 0.0;
    ErrorC : LREAL := 0.0;
    ProportionalPct : LREAL := 0.0;
    IntegralPct : LREAL := 75.0;
    RequestedPowerPct : LREAL := 75.0;
    HeaterCommandPct : LREAL := 75.0;
    AppliedPowerPct : LREAL := 75.0;
    ObservedKp : LREAL := 2.0;
    ObservedKi : LREAL := 0.5;
END_VAR

(* One native process owns both heat inputs and publishes the measured water
   temperature. The controller does not construct a plant response in ST. *)
WaterTemperatureC := TemperatureC;
IF NOT (TemperatureC >= LREAL#0.0 AND TemperatureC <= LREAL#80.0) THEN
    SafetyAbort := TRUE;
ELSE Input.ProcessValue := TemperatureC;
END_IF;
KpPctPerC := Parameters.ProportionalGain;
KiPctPerCs := Parameters.IntegralGain;
(* Practice bounds for this exercise, not universal stability limits. *)
IF NOT (Parameters.ProportionalGain >= LREAL#1.0 AND Parameters.ProportionalGain <= LREAL#4.0
    AND Parameters.IntegralGain >= LREAL#0.2 AND Parameters.IntegralGain <= LREAL#1.0) THEN
    SafetyAbort := TRUE;
END_IF;
ObservedKp := Parameters.ProportionalGain;
ObservedKi := Parameters.IntegralGain;
AutoMode := ExperimentEnabled AND NOT SafetyAbort;
SetpointC := LREAL#35.0;
DisturbanceKw := LREAL#0.0;
IF AutoMode THEN
    HasStarted := TRUE;
    IF TrialKind = 0 THEN SetpointC := LREAL#38.0;
    ELSIF TrialKind = 1 THEN DisturbanceKw := LREAL#30.0;
    END_IF;
    Input.Mode := PIDMode#Auto;
ELSE
    Input.Mode := PIDMode#Manual;
END_IF;
Input.ManualOutput := LREAL#0.0;
IF NOT HasStarted AND NOT SafetyAbort THEN Input.ManualOutput := LREAL#75.0;
END_IF;
Input.Setpoint := SetpointC;
Input.CycleTime := LREAL#0.1;
Input.FeedForward := LREAL#0.0;
Input.TrackingOutput := LREAL#0.0;
ErrorC := SetpointC - Input.ProcessValue;

Controller.Configure(Parameters := Parameters);
IF NOT Initialized THEN
    InitialState.Setpoint := LREAL#35.0;
    InitialState.ProcessValue := Input.ProcessValue;
    InitialState.NominalOutput := LREAL#75.0;
    InitialState.SaturatedOutput := LREAL#75.0;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;
BeforeState := Controller.GetState();

(* Fixed PI gains and weight one make nominal output = P + equivalent I.
   These observations use the library's retained state and own no actuator. *)
ProportionalPct := LREAL#0.0;
RequestedPowerPct := Input.ManualOutput;
IF AutoMode THEN
    ProportionalPct := KpPctPerC * ErrorC;
    RequestedPowerPct := BeforeState.NominalOutput
        + KpPctPerC * ((SetpointC - BeforeState.Setpoint)
            - (Input.ProcessValue - BeforeState.ProcessValue))
        + KiPctPerCs * ErrorC * Input.CycleTime;
END_IF;
HeaterCommandPct := Controller.Update(Input := Input);
ObservedState := Controller.GetState();
IntegralPct := ObservedState.NominalOutput - ProportionalPct;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ObservedKp AT %ML96 : LREAL;
    P1.ObservedKi AT %ML104 : LREAL;
    P1.ExperimentEnabled AT %MX112.0 : BOOL;
    P1.AutoMode AT %MX112.1 : BOOL;
    P1.SafetyAbort AT %MX112.2 : BOOL;
    P1.SetpointC AT %ML0 : LREAL;
    P1.ErrorC AT %ML8 : LREAL;
    P1.IntegralPct AT %ML16 : LREAL;
    P1.RequestedPowerPct AT %ML32 : LREAL;
    P1.WaterTemperatureC AT %ML48 : LREAL;
    P1.ProportionalPct AT %ML56 : LREAL;
    P1.TemperatureC AT %IL0 : LREAL;
    P1.AppliedPowerPct AT %IL8 : LREAL;
    P1.HeaterCommandPct AT %QL0 : LREAL;
    P1.DisturbanceKw AT %QL8 : LREAL;
END_VAR
END_CONFIGURATION
