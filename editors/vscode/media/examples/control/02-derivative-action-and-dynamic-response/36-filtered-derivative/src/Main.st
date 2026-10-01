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
    (* PID position control: the integral supplies the holding torque of a load at zero error.
       P acts on 0.3 of the target change (Lesson 23), so the integral does not overshoot a step; D acts
       on the measured angle. The output is motor torque in N·m, limited to the drive's ±20 N·m with
       back-calculation anti-windup. Tune replaces this record. *)
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#16.0,
        IntegralGain := LREAL#12.0,
        DerivativeGain := LREAL#7.0,
        ProportionalSetpointWeight := LREAL#0.3,
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
    (* Drum load of the simulated test rig, in N·m: the loaded trial couples a 2 kg weight at the
       0.1 m drum radius, 1.962 N·m. The controller does not use it. *)
    LoadTorqueNm : LREAL := 0.0;
    (* Measurement filter on the angle: Control.PID differentiates the filtered angle, so D uses the
       filtered velocity. Tf = 0 bypasses the filter; Control Studio applies Tf with the gains. *)
    FilterTimeS : LREAL := 0.1;
    FilterAlpha : LREAL := 1.0;
    FilteredAngleRad : LREAL := 0.0;
    FilterActive : BOOL := FALSE;
    FilterWasActive : BOOL := FALSE;
    (* Software travel and speed limits of the axis. *)
    AngleLimitRad : LREAL := 2.0;
    VelocityLimitRadS : LREAL := 10.0;
    (* Arrival: the measured velocity at the first sample that reaches a new target. *)
    ArrivalSetpointRad : LREAL := 0.0;
    ApproachRad : LREAL := 0.0;
    Arrived : BOOL := TRUE;
    ArrivalVelocityRadS : LREAL := 0.0;
END_VAR

(* Leaving the travel or speed range removes the torque until a full restart. *)
IF ABS(AngleRad) > AngleLimitRad OR ABS(VelocityRadS) > VelocityLimitRadS THEN
    SafetyAbort := TRUE;
END_IF;

(* Control Studio's Apply replaces this complete record at the task boundary. *)
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
    ArrivalSetpointRad := SetpointRad;
    Initialized := TRUE;
END_IF;

(* The filter runs while the controller is in Auto. In Manual and in the Auto-entry scan it restarts
   from the measurement, so entering Auto is exactly bumpless; filtering starts with the next scan. *)
FilterAlpha := FilterFraction(CycleTime := LREAL#0.02, FilterTime := MAX(FilterTimeS, LREAL#0.0));
FilterActive := AutoMode AND NOT SafetyAbort AND Parameters.ProportionalGain > LREAL#0.0;
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
ELSIF AutoMode AND Parameters.ProportionalGain > LREAL#0.0 THEN
    Input.Mode := PIDMode#Auto;
    Input.ManualOutput := ManualTorqueNm;
ELSE
    Input.Mode := PIDMode#Manual;
    Input.ManualOutput := ManualTorqueNm;
END_IF;
TorqueNm := Controller.Update(Input := Input);

IF SetpointRad <> ArrivalSetpointRad THEN
    ArrivalSetpointRad := SetpointRad;
    ApproachRad := SetpointRad - AngleRad;
    Arrived := FALSE;
END_IF;
IF NOT Arrived AND ApproachRad * (SetpointRad - AngleRad) <= LREAL#0.0 THEN
    Arrived := TRUE;
    ArrivalVelocityRadS := VelocityRadS;
END_IF;

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
    P1.ArrivalVelocityRadS AT %ML16 : LREAL;
END_VAR
END_CONFIGURATION
