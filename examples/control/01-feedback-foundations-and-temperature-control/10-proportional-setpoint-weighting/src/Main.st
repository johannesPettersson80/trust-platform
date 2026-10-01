USING Control;

(* Change of the weighted proportional term: Kp·(β·ΔSP − ΔPV). *)
FUNCTION WeightedProportionalChange : LREAL
VAR_INPUT
    ProportionalGain : LREAL;
    SetpointWeight : LREAL;
    SetpointChange : LREAL;
    MeasurementChange : LREAL;
END_VAR
WeightedProportionalChange := ProportionalGain
    * (SetpointWeight * SetpointChange - MeasurementChange);
END_FUNCTION

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

PROGRAM Main
VAR
    Controller : Control.PID;
    (* Tune replaces this record with each candidate; the defaults are the controller
       Lesson 21 selected (the relay candidate), with the full setpoint weight. *)
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#13.0,
        IntegralGain := LREAL#3.5,
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
    (* Reference of the weighted proportional term: the 35 °C hot operating point. *)
    OperatingPointC : LREAL := 35.0;
    ProportionalPct : LREAL := 0.0;
    IntegralPct : LREAL := 75.0;
    PreviousAppliedPct : LREAL := 75.0;
    CommandActivityPoints : LREAL := 0.0;
END_VAR

(* The Lesson 21 heater: both feedback bindings observe the heated water, and the
   temperature measurement carries ±0.02 °C noise. *)
WaterTemperatureC := WaterInputC;
DataUsable := TemperatureC >= LREAL#0.0 AND TemperatureC <= LREAL#80.0;
IF DataUsable THEN
    SafeProcessValueC := TemperatureC;
ELSE
    SafetyAbort := TRUE;
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

(* Diagnostics come from the actual controller state; the PI equations live in Control.PID.
   Command activity adds every applied-command movement, starting from the 75% holding command. *)
ObservedState := Controller.GetState();
RequestedPowerPct := ObservedState.NominalOutput;
IF Input.Mode = PIDMode#Auto THEN
    (* Weighted P = Kp·(β·(SP − T0) − (PV − T0)); the integral uses the full error. *)
    ProportionalPct := Parameters.ProportionalGain * (Parameters.ProportionalSetpointWeight * (SetpointC - OperatingPointC)
        - (SafeProcessValueC - OperatingPointC));
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
