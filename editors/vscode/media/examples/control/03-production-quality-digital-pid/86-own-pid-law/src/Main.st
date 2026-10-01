USING Control;

(* Fraction of the raw-minus-filtered difference applied per update: alpha = dt/(Tf + dt). *)
FUNCTION FilterFraction : LREAL
VAR_INPUT
    CycleTime : LREAL;
    FilterTime : LREAL;
END_VAR
FilterFraction := CycleTime / (FilterTime + CycleTime);
END_FUNCTION

(* One first-order filter update: the filtered value moves alpha of the way toward the raw value. *)
FUNCTION FilterUpdate : LREAL
VAR_INPUT
    PreviousFilteredValue : LREAL;
    RawValue : LREAL;
    FilterFraction : LREAL;
END_VAR
FilterUpdate := PreviousFilteredValue + FilterFraction * (RawValue - PreviousFilteredValue);
END_FUNCTION

(* Lesson 39's positional PID (β = 1, γ = 0): the complete request every update, from its own retained
   integral, with tracking after the clamp, the integral following the manual torque, and the optional
   gain-revision rule. *)
FUNCTION_BLOCK PositionalPid
VAR_INPUT
    Parameters : Control.PIDParameters;
    Input : Control.PIDInput;
    PreloadOnGainRevision : BOOL := TRUE;
END_VAR
VAR_OUTPUT
    ErrorRad : LREAL;
    ProportionalNm : LREAL;
    TrialIntegralNm : LREAL;
    DerivativeNm : LREAL;
    RequestNm : LREAL;
    TorqueNm : LREAL;
    IntegralNm : LREAL;
END_VAR
VAR
    ProportionalGain : LREAL;
    DerivativeGain : LREAL;
    VelocityRadS : LREAL;
    PreviousAngleRad : LREAL;
    PreviousErrorRad : LREAL;
    PreviousVelocityRadS : LREAL;
END_VAR
IF PreloadOnGainRevision THEN
    IntegralNm := IntegralNm
        + (ProportionalGain - Parameters.ProportionalGain) * PreviousErrorRad
        - (DerivativeGain - Parameters.DerivativeGain) * PreviousVelocityRadS;
END_IF;
ProportionalGain := Parameters.ProportionalGain;
DerivativeGain := Parameters.DerivativeGain;

ErrorRad := Input.Setpoint - Input.ProcessValue;
VelocityRadS := (Input.ProcessValue - PreviousAngleRad) / Input.CycleTime;
ProportionalNm := ProportionalGain * ErrorRad;
DerivativeNm := -DerivativeGain * VelocityRadS;
IF Input.Mode = PIDMode#Auto THEN
    TrialIntegralNm := IntegralNm + Parameters.IntegralGain * ErrorRad * Input.CycleTime;
    RequestNm := ProportionalNm + TrialIntegralNm + DerivativeNm;
    TorqueNm := LIMIT(Parameters.OutputMinimum, RequestNm, Parameters.OutputMaximum);
    IntegralNm := TrialIntegralNm + Input.CycleTime / Parameters.TrackingTime * (TorqueNm - RequestNm);
ELSE
    RequestNm := Input.ManualOutput;
    TorqueNm := LIMIT(Parameters.OutputMinimum, Input.ManualOutput, Parameters.OutputMaximum);
    IntegralNm := Input.ManualOutput - ProportionalNm - DerivativeNm;
    TrialIntegralNm := IntegralNm;
END_IF;
PreviousAngleRad := Input.ProcessValue;
PreviousErrorRad := ErrorRad;
PreviousVelocityRadS := VelocityRadS;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    (* The program's own PID drives the axis. Control Studio designs and applies the three gains and sets
       Own.IntegralNm and Own.PreviousAngleRad at the operating point. *)
    Own : PositionalPid;
    (* Lesson 38's gains until Control Studio applies its design: ±20 N·m with back-calculation, Tt = 1 s. *)
    OwnParameters : Control.PIDParameters := (
        ProportionalGain := LREAL#16.0,
        IntegralGain := LREAL#4.0,
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
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    AutoMode : BOOL := FALSE;
    ManualTorqueNm : LREAL := 1.962;
    SafetyAbort : BOOL := FALSE;
    SetpointRad : LREAL := 0.0;
    AngleRad : LREAL := 0.0;
    VelocityRadS : LREAL := 0.0;
    TorqueNm : LREAL := 1.962;
    (* The 2 kg weight on the 0.1 m drum pulls with 1.962 N·m. An abort engages the brake, which then
       carries it, so the motor sees no load. *)
    LoadTorqueNm : LREAL := 1.962;
    (* Measurement filter on the angle: the own law differentiates the filtered angle. *)
    FilterTimeS : LREAL := 0.1;
    FilterAlpha : LREAL := 1.0;
    FilteredAngleRad : LREAL := 0.0;
    FilterActive : BOOL := FALSE;
    FilterWasActive : BOOL := FALSE;
    ErrorRad : LREAL := 0.0;
    ProportionalNm : LREAL := 0.0;
    DerivativeNm : LREAL := 0.0;
    IntegralNm : LREAL := 1.962;
    (* Software travel and speed limits of the axis. *)
    AngleLimitRad : LREAL := 2.0;
    VelocityLimitRadS : LREAL := 10.0;
END_VAR

(* Leaving the travel or speed range removes the torque and brakes the drum until a full restart. *)
IF ABS(AngleRad) > AngleLimitRad OR ABS(VelocityRadS) > VelocityLimitRadS THEN
    SafetyAbort := TRUE;
END_IF;
IF SafetyAbort THEN
    LoadTorqueNm := LREAL#0.0;
ELSE
    LoadTorqueNm := LREAL#1.962;
END_IF;

IF NOT Initialized THEN
    (* Hot reset: in Manual the motor holds the weight at rest with 1.962 N·m and the target is the angle. *)
    ManualTorqueNm := LREAL#1.962;
    IF NOT AutoMode THEN
        SetpointRad := AngleRad;
    END_IF;
    Initialized := TRUE;
END_IF;

(* The filter runs while the law is in Auto. In Manual and in the Auto-entry scan it restarts from the
   measurement, so entering Auto is exactly bumpless; filtering starts with the next scan. *)
FilterAlpha := FilterFraction(CycleTime := LREAL#0.02, FilterTime := MAX(FilterTimeS, LREAL#0.0));
FilterActive := AutoMode AND NOT SafetyAbort AND OwnParameters.ProportionalGain > LREAL#0.0;
IF FilterActive AND FilterWasActive THEN
    FilteredAngleRad := FilterUpdate(PreviousFilteredValue := FilteredAngleRad, RawValue := AngleRad, FilterFraction := FilterAlpha);
ELSE
    FilteredAngleRad := AngleRad;
END_IF;
FilterWasActive := FilterActive;

Input.Setpoint := SetpointRad;
Input.ProcessValue := FilteredAngleRad;
Input.CycleTime := LREAL#0.02;
Input.FeedForward := LREAL#0.0;
Input.TrackingOutput := ManualTorqueNm;
IF SafetyAbort THEN
    Input.Mode := PIDMode#Manual;
    Input.ManualOutput := LREAL#0.0;
ELSIF AutoMode AND OwnParameters.ProportionalGain > LREAL#0.0 THEN
    Input.Mode := PIDMode#Auto;
    Input.ManualOutput := ManualTorqueNm;
ELSE
    Input.Mode := PIDMode#Manual;
    Input.ManualOutput := ManualTorqueNm;
END_IF;
Own(Parameters := OwnParameters, Input := Input);
TorqueNm := Own.TorqueNm;
ErrorRad := Own.ErrorRad;
ProportionalNm := Own.ProportionalNm;
DerivativeNm := Own.DerivativeNm;
IntegralNm := Own.IntegralNm;

END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.SafetyAbort AT %MX24.0 : BOOL;
    P1.TorqueNm AT %QL0 : LREAL;
    P1.LoadTorqueNm AT %QL8 : LREAL;
    P1.AngleRad AT %IL0 : LREAL;
    P1.VelocityRadS AT %IL8 : LREAL;
    P1.SetpointRad AT %ML0 : LREAL;
    P1.ManualTorqueNm AT %ML8 : LREAL;
    P1.FilteredAngleRad AT %ML32 : LREAL;
    P1.ErrorRad AT %ML40 : LREAL;
    P1.ProportionalNm AT %ML48 : LREAL;
    P1.DerivativeNm AT %ML64 : LREAL;
    P1.IntegralNm AT %ML80 : LREAL;
END_VAR
END_CONFIGURATION
