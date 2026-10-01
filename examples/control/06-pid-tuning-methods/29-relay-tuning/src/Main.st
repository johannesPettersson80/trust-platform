USING Control;

FUNCTION SamePidParameters : BOOL
VAR_INPUT
    Left : Control.PIDParameters;
    Right : Control.PIDParameters;
END_VAR
SamePidParameters :=
    Left.ProportionalGain = Right.ProportionalGain
    AND Left.IntegralGain = Right.IntegralGain
    AND Left.DerivativeGain = Right.DerivativeGain
    AND Left.ProportionalSetpointWeight = Right.ProportionalSetpointWeight
    AND Left.DerivativeSetpointWeight = Right.DerivativeSetpointWeight
    AND Left.Bias = Right.Bias
    AND Left.OutputMinimum = Right.OutputMinimum
    AND Left.OutputMaximum = Right.OutputMaximum
    AND Left.RateMinimum = Right.RateMinimum
    AND Left.RateMaximum = Right.RateMaximum
    AND Left.TrackingTime = Right.TrackingTime
    AND Left.AntiWindup = Right.AntiWindup;
END_FUNCTION

(* The relay switching rule that Control Studio applies during the bounded test,
   written in ST, and the relay-rule calculations used for the startup design. *)
FUNCTION Lesson29RelayHighState : BOOL
VAR_INPUT
    CurrentHighState : BOOL;
    MeasuredTemperature : LREAL;
    CenterTemperature : LREAL;
    HysteresisHalfWidth : LREAL;
END_VAR
IF MeasuredTemperature >= CenterTemperature + HysteresisHalfWidth THEN
    Lesson29RelayHighState := FALSE;
ELSIF MeasuredTemperature <= CenterTemperature - HysteresisHalfWidth THEN
    Lesson29RelayHighState := TRUE;
ELSE
    Lesson29RelayHighState := CurrentHighState;
END_IF;
END_FUNCTION

FUNCTION Lesson29RelayOutput : LREAL
VAR_INPUT
    Bias : LREAL;
    Amplitude : LREAL;
    HighState : BOOL;
END_VAR
IF HighState THEN
    Lesson29RelayOutput := Bias + Amplitude;
ELSE
    Lesson29RelayOutput := Bias - Amplitude;
END_IF;
END_FUNCTION

FUNCTION Lesson29FundamentalRelayAmplitude : LREAL
VAR_INPUT
    RelayAmplitude : LREAL;
END_VAR
Lesson29FundamentalRelayAmplitude := LREAL#4.0 * RelayAmplitude
    / LREAL#3.141592653589793;
END_FUNCTION

FUNCTION Lesson29CriticalGainEstimate : LREAL
VAR_INPUT
    RelayAmplitude : LREAL;
    ProcessAmplitude : LREAL;
END_VAR
Lesson29CriticalGainEstimate := Lesson29FundamentalRelayAmplitude(
    RelayAmplitude := RelayAmplitude
) / ProcessAmplitude;
END_FUNCTION

FUNCTION Lesson29HistoricalProportionalGain : LREAL
VAR_INPUT
    UltimateGain : LREAL;
END_VAR
Lesson29HistoricalProportionalGain := LREAL#0.45 * UltimateGain;
END_FUNCTION

FUNCTION Lesson29HistoricalIntegralTime : LREAL
VAR_INPUT
    UltimatePeriodSeconds : LREAL;
END_VAR
Lesson29HistoricalIntegralTime := UltimatePeriodSeconds / LREAL#1.2;
END_FUNCTION

PROGRAM Main
VAR
    Controller : Control.PID;
    (* Declaration defaults only apply before the startup design below replaces the gains. *)
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#0.0,
        IntegralGain := LREAL#0.0,
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
    ConfiguredParameters : Control.PIDParameters;
    ParameterRevision : DINT := 0;
    InitialState : Control.PIDInitialState;
    ObservedState : Control.PIDInitialState;
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    AutoMode : BOOL := FALSE;
    ManualOutput : LREAL := 75.0;
    SafetyAbort : BOOL := FALSE;
    (* The relay record measured in Control Studio: command half-amplitude d,
       temperature half-amplitude a and full cycle period Pu. *)
    RelayAmplitudePct : LREAL := 10.0;
    CycleAmplitudeC : LREAL := 0.45;
    CyclePeriodS : LREAL := 4.5;
    RelayDataValid : BOOL := TRUE;
    RelayRevision : UDINT := 0; (* 0 denotes the provided illustration, not your measurement. *)
    RelayGainsUsable : BOOL := FALSE;
    KuPctPerC : LREAL := 0.0;
    KpPctPerC : LREAL := 0.0;
    TiS : LREAL := 0.0;
    KiPctPerCs : LREAL := 0.0;
    DataUsable : BOOL := FALSE;
    SetpointC : LREAL := 35.0;
    DisturbanceKw : LREAL := 0.0;
    WaterInputC : LREAL := 35.0;
    WaterTemperatureC : LREAL := 35.0;
    TemperatureC : LREAL := 35.0;
    SafeProcessValueC : LREAL := 35.0;
    AppliedPowerPct : LREAL := 75.0;
    RequestedPowerPct : LREAL := 75.0;
    ErrorC : LREAL := 0.0;
    ProportionalPct : LREAL := 0.0;
    IntegralPct : LREAL := 75.0;
    PreviousAppliedPct : LREAL := 75.0;
    CommandActivityPoints : LREAL := 0.0;
END_VAR

(* The same heater as Lessons 15-19; its temperature measurement carries ±0.02 °C noise. *)
WaterTemperatureC := WaterInputC;
DataUsable := TemperatureC >= LREAL#0.0 AND TemperatureC <= LREAL#80.0;
IF DataUsable THEN
    SafeProcessValueC := TemperatureC;
ELSE
    SafetyAbort := TRUE;
END_IF;

IF NOT Initialized THEN
    (* Startup design from the relay cycle: Ku = 4d/(pi*a), Kp = 0.45*Ku,
       Ti = Pu/1.2 and Ki = Kp/Ti. Divide only for a valid positive record;
       otherwise keep zero gains and stay Manual. *)
    RelayGainsUsable := RelayDataValid AND RelayAmplitudePct > LREAL#0.0
        AND CycleAmplitudeC > LREAL#0.0 AND CyclePeriodS > LREAL#0.0;
    IF RelayGainsUsable THEN
        KuPctPerC := Lesson29CriticalGainEstimate(RelayAmplitude := RelayAmplitudePct,
            ProcessAmplitude := CycleAmplitudeC);
        KpPctPerC := Lesson29HistoricalProportionalGain(UltimateGain := KuPctPerC);
        TiS := Lesson29HistoricalIntegralTime(UltimatePeriodSeconds := CyclePeriodS);
        KiPctPerCs := KpPctPerC / TiS;
        Parameters.ProportionalGain := KpPctPerC;
        Parameters.IntegralGain := KiPctPerCs;
    END_IF;
END_IF;

(* Control Studio's Apply replaces this complete record at the task boundary; the
   program copies it into the actual controller and never overwrites it cyclically. *)
IF NOT Initialized OR NOT SamePidParameters(Left := Parameters, Right := ConfiguredParameters) THEN
    ConfiguredParameters := Parameters;
    ParameterRevision := ParameterRevision + 1;
END_IF;
Controller.Configure(Parameters := Parameters);

IF NOT Initialized THEN
    (* Common hot reset: the heater already holds 35 °C with 75 % output, so the
       controller's retained output starts at the holding command. *)
    ManualOutput := LREAL#75.0;
    IF NOT AutoMode THEN
        SetpointC := SafeProcessValueC;
    END_IF;
    InitialState.Setpoint := SetpointC;
    InitialState.ProcessValue := SafeProcessValueC;
    InitialState.NominalOutput := ManualOutput;
    InitialState.SaturatedOutput := ManualOutput;
    InitialState.SetpointRate := LREAL#0.0;
    InitialState.ProcessValueRate := LREAL#0.0;
    InitialState.FeedForward := LREAL#0.0;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;

Input.Setpoint := SetpointC;
Input.ProcessValue := SafeProcessValueC;
Input.CycleTime := LREAL#0.1;
Input.FeedForward := LREAL#0.0;
Input.TrackingOutput := ManualOutput;
IF SafetyAbort THEN
    (* An abort or invalid measurement removes heat and the test load until a full restart. *)
    Input.Mode := PIDMode#Manual;
    Input.ManualOutput := LREAL#0.0;
    DisturbanceKw := LREAL#0.0;
ELSIF AutoMode AND Parameters.ProportionalGain > LREAL#0.0 AND Parameters.IntegralGain > LREAL#0.0 THEN
    Input.Mode := PIDMode#Auto;
    Input.ManualOutput := ManualOutput;
ELSE
    Input.Mode := PIDMode#Manual;
    Input.ManualOutput := ManualOutput;
END_IF;
ErrorC := SetpointC - SafeProcessValueC;
AppliedPowerPct := Controller.Update(Input := Input);

(* Diagnostics come from the actual controller state; the PI equations live in Control.PID. *)
ObservedState := Controller.GetState();
RequestedPowerPct := ObservedState.NominalOutput;
IF Input.Mode = PIDMode#Auto THEN
    ProportionalPct := Parameters.ProportionalGain * ErrorC;
ELSE
    ProportionalPct := LREAL#0.0;
END_IF;
IntegralPct := RequestedPowerPct - ProportionalPct;
CommandActivityPoints := CommandActivityPoints + ABS(AppliedPowerPct - PreviousAppliedPct);
PreviousAppliedPct := AppliedPowerPct;

END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.SafetyAbort AT %MX96.3 : BOOL;
    P1.DataUsable AT %MX96.2 : BOOL;
    P1.AppliedPowerPct AT %QL0 : LREAL;
    P1.DisturbanceKw AT %QL8 : LREAL;
    P1.WaterInputC AT %IL0 : LREAL;
    P1.TemperatureC AT %IL8 : LREAL;
    P1.SetpointC AT %ML0 : LREAL;
    P1.IntegralPct AT %ML16 : LREAL;
    P1.RequestedPowerPct AT %ML32 : LREAL;
    P1.WaterTemperatureC AT %ML48 : LREAL;
    P1.ProportionalPct AT %ML56 : LREAL;
    P1.CommandActivityPoints AT %ML64 : LREAL;
    P1.ManualOutput AT %ML72 : LREAL;
END_VAR
END_CONFIGURATION
