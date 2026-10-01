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

(* The steady error of P control on a self-regulating process: e = Δr / (1 + K·Kp). *)
FUNCTION ProportionalOffset : LREAL
VAR_INPUT
    ReferenceChange : LREAL;
    ProcessGain : LREAL;
    ProportionalGain : LREAL;
END_VAR
ProportionalOffset := ReferenceChange / (LREAL#1.0 + ProcessGain * ProportionalGain);
END_FUNCTION

(* The shaft's natural decay rate with P or PD: σ = (B + Kd) / (2J); P alone keeps σ = B / (2J). *)
FUNCTION ShaftDecayRate : LREAL
VAR_INPUT
    Inertia : LREAL;
    Friction : LREAL;
    DerivativeGain : LREAL;
END_VAR
ShaftDecayRate := (Friction + DerivativeGain) / (LREAL#2.0 * Inertia);
END_FUNCTION

(* The steady error of PD under a constant load torque: e = τL / Kp. *)
FUNCTION LoadOffset : LREAL
VAR_INPUT
    LoadTorque : LREAL;
    ProportionalGain : LREAL;
END_VAR
LoadOffset := LoadTorque / ProportionalGain;
END_FUNCTION

(* The heater of Lessons 18–22 under Control.PID. *)
PROGRAM Heater
VAR
    Controller : Control.PID;
    (* Control Studio's Tune replaces the gains; the defaults are Lesson 19's design (lambda = 5 s). *)
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
    ConfiguredParameters : Control.PIDParameters;
    ParameterRevision : DINT := 0;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    AutoMode : BOOL := FALSE;
    ManualOutput : LREAL := 75.0;
    SafetyAbort : BOOL := FALSE;
    DataUsable : BOOL := FALSE;
    SetpointC : LREAL := 35.0;
    DisturbanceKw : LREAL := 0.0;
    WaterInputC : LREAL := 35.0;
    TemperatureC : LREAL := 35.0;
    SafeProcessValueC : LREAL := 35.0;
    AppliedPowerPct : LREAL := 75.0;
END_VAR

DataUsable := TemperatureC >= LREAL#0.0 AND TemperatureC <= LREAL#80.0;
IF DataUsable THEN
    SafeProcessValueC := TemperatureC;
ELSE
    SafetyAbort := TRUE;
END_IF;

(* Control Studio's Apply replaces this complete record at the task boundary. *)
IF NOT Initialized OR NOT SamePidParameters(Left := Parameters, Right := ConfiguredParameters) THEN
    ConfiguredParameters := Parameters;
    ParameterRevision := ParameterRevision + 1;
END_IF;
Controller.Configure(Parameters := Parameters);

IF NOT Initialized THEN
    (* Hot reset: the heater holds 35 °C with 75 % output. *)
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
Input.CycleTime := LREAL#0.02;
Input.FeedForward := LREAL#0.0;
Input.TrackingOutput := ManualOutput;
IF SafetyAbort THEN
    (* An abort or invalid measurement removes heat and the test load until a full restart. *)
    Input.Mode := PIDMode#Manual;
    Input.ManualOutput := LREAL#0.0;
    DisturbanceKw := LREAL#0.0;
ELSIF AutoMode AND Parameters.ProportionalGain > LREAL#0.0 THEN
    Input.Mode := PIDMode#Auto;
    Input.ManualOutput := ManualOutput;
ELSE
    Input.Mode := PIDMode#Manual;
    Input.ManualOutput := ManualOutput;
END_IF;
AppliedPowerPct := Controller.Update(Input := Input);
END_PROGRAM

(* The drum axis of Lessons 34–37 under Control.PID. *)
PROGRAM Axis
VAR
    Controller : Control.PID;
    (* Control Studio's Tune replaces the gains; the defaults are Lesson 36's PID. *)
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#16.0,
        IntegralGain := LREAL#12.0,
        DerivativeGain := LREAL#7.0,
        ProportionalSetpointWeight := LREAL#1.0,
        DerivativeSetpointWeight := LREAL#0.0,
        Bias := LREAL#0.0,
        OutputMinimum := LREAL#-20.0,
        OutputMaximum := LREAL#20.0,
        RateMinimum := LREAL#-10000.0,
        RateMaximum := LREAL#10000.0,
        TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#BackCalculation
    );
    ConfiguredParameters : Control.PIDParameters;
    ParameterRevision : DINT := 0;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    AutoMode : BOOL := FALSE;
    ManualTorqueNm : LREAL := 0.0;
    SafetyAbort : BOOL := FALSE;
    SetpointRad : LREAL := 0.0;
    AngleRad : LREAL := 0.0;
    VelocityRadS : LREAL := 0.0;
    TorqueNm : LREAL := 0.0;
    (* The weight on the drum, 1.962 N·m when coupled; the controller does not use it. *)
    LoadTorqueNm : LREAL := 0.0;
    AngleLimitRad : LREAL := 2.0;
    VelocityLimitRadS : LREAL := 10.0;
END_VAR

(* Leaving the travel or speed range removes the torque until a full restart. *)
IF ABS(AngleRad) > AngleLimitRad OR ABS(VelocityRadS) > VelocityLimitRadS THEN
    SafetyAbort := TRUE;
END_IF;

IF NOT Initialized OR NOT SamePidParameters(Left := Parameters, Right := ConfiguredParameters) THEN
    ConfiguredParameters := Parameters;
    ParameterRevision := ParameterRevision + 1;
END_IF;
Controller.Configure(Parameters := Parameters);

IF NOT Initialized THEN
    (* Hot reset: the axis rests without torque and the setpoint follows the measured angle. *)
    ManualTorqueNm := LREAL#0.0;
    IF NOT AutoMode THEN
        SetpointRad := AngleRad;
    END_IF;
    InitialState.Setpoint := SetpointRad;
    InitialState.ProcessValue := AngleRad;
    InitialState.NominalOutput := ManualTorqueNm;
    InitialState.SaturatedOutput := ManualTorqueNm;
    InitialState.SetpointRate := LREAL#0.0;
    InitialState.ProcessValueRate := LREAL#0.0;
    InitialState.FeedForward := LREAL#0.0;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;

Input.Setpoint := SetpointRad;
Input.ProcessValue := AngleRad;
Input.CycleTime := LREAL#0.02;
Input.FeedForward := LREAL#0.0;
Input.TrackingOutput := ManualTorqueNm;
IF SafetyAbort THEN
    Input.Mode := PIDMode#Manual;
    Input.ManualOutput := LREAL#0.0;
    LoadTorqueNm := LREAL#0.0;
ELSIF AutoMode AND Parameters.ProportionalGain > LREAL#0.0 THEN
    Input.Mode := PIDMode#Auto;
    Input.ManualOutput := ManualTorqueNm;
ELSE
    Input.Mode := PIDMode#Manual;
    Input.ManualOutput := ManualTorqueNm;
END_IF;
TorqueNm := Controller.Update(Input := Input);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Heater;
    PROGRAM P2 WITH MainTask : Axis;
END_RESOURCE
VAR_CONFIG
    P1.SafetyAbort AT %MX96.3 : BOOL;
    P1.DataUsable AT %MX96.2 : BOOL;
    P1.AppliedPowerPct AT %QL0 : LREAL;
    P1.DisturbanceKw AT %QL8 : LREAL;
    P1.WaterInputC AT %IL0 : LREAL;
    P1.TemperatureC AT %IL8 : LREAL;
    P1.SetpointC AT %ML0 : LREAL;
    P1.ManualOutput AT %ML72 : LREAL;
    P2.SafetyAbort AT %MX224.0 : BOOL;
    P2.TorqueNm AT %QL16 : LREAL;
    P2.LoadTorqueNm AT %QL24 : LREAL;
    P2.AngleRad AT %IL16 : LREAL;
    P2.VelocityRadS AT %IL24 : LREAL;
    P2.SetpointRad AT %ML200 : LREAL;
    P2.ManualTorqueNm AT %ML208 : LREAL;
END_VAR
END_CONFIGURATION
