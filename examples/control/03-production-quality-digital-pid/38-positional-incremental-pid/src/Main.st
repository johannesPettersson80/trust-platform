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

(* The positional form of this lesson's PID (β = 1, γ = 0): the complete request every update, from its own
   retained integral. Three rules make it the same controller as the incremental form of Control.PID:
   tracking corrects the integral after the clamp, in Manual the integral follows the manual torque, and a
   gain revision moves the change of the previous P and D into the integral. *)
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
(* Gain revision: P and D of the previous update change by (Kp,old − Kp,new)·e and (Kd,new − Kd,old)·ω_f;
   the integral takes that over, so the retained request stays the same. *)
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
    (* Manual: the integral follows the manual torque, so the first Auto update continues from it. *)
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
    Controller : Control.PID;
    (* The discrete PID of Lesson 38: Kp = 16 N·m/rad, Ki = 4 N·m/(rad·s), Kd = 7 N·m·s/rad on the
       filtered angle, the full target change in P, ±20 N·m with back-calculation (Tt = 1 s). *)
    Parameters : Control.PIDParameters := (
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
    (* Drum load of the simulated test rig, in N·m: the experiment couples a 2 kg weight at the 0.1 m drum
       radius, 1.962 N·m. The controller does not use it. *)
    LoadTorqueNm : LREAL := 0.0;
    (* Measurement filter on the angle: Control.PID differentiates the filtered angle, so D uses the
       filtered velocity. Tf = 0 bypasses the filter. *)
    FilterTimeS : LREAL := 0.1;
    FilterAlpha : LREAL := 1.0;
    FilteredAngleRad : LREAL := 0.0;
    FilterActive : BOOL := FALSE;
    FilterWasActive : BOOL := FALSE;
    (* Three other forms, each updated from the same Input as Controller. None of them drives the axis. *)
    Positional : PositionalPid;
    Unpreloaded : PositionalPid;
    RetainedApplied : Control.PID;
    RetainedAppliedParameters : Control.PIDParameters;
    PositionalTorqueNm : LREAL := 0.0;
    UnpreloadedTorqueNm : LREAL := 0.0;
    RetainedAppliedTorqueNm : LREAL := 0.0;
    ErrorRad : LREAL := 0.0;
    ProportionalNm : LREAL := 0.0;
    DerivativeNm : LREAL := 0.0;
    RequestNm : LREAL := 0.0;
    IntegralNm : LREAL := 0.0;
    (* Project experiment. *)
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    Active : BOOL := FALSE;
    ExperimentUpdates : DINT := 0;
    GainBeforeRevision : LREAL := 16.0;
    (* Software travel and speed limits of the axis. *)
    AngleLimitRad : LREAL := 2.0;
    VelocityLimitRadS : LREAL := 10.0;
END_VAR

(* Leaving the travel or speed range removes the torque until a full restart. *)
IF ABS(AngleRad) > AngleLimitRad OR ABS(VelocityRadS) > VelocityLimitRadS THEN
    SafetyAbort := TRUE;
END_IF;

(* Control Studio's project experiment couples the drum load and steps the target to 1.5 rad in Auto; 1 s
   later it applies Kp = 24 N·m/rad, as a gain revision would. Its end uncouples the load, restores the gain
   and returns to Manual with zero torque. *)
IF ExperimentEnabled AND NOT ExperimentWasEnabled THEN
    Active := TRUE;
    ExperimentUpdates := 0;
    LoadTorqueNm := LREAL#1.962;
    SetpointRad := LREAL#1.5;
    AutoMode := TRUE;
ELSIF NOT ExperimentEnabled AND ExperimentWasEnabled THEN
    Active := FALSE;
    LoadTorqueNm := LREAL#0.0;
    AutoMode := FALSE;
    SetpointRad := AngleRad;
    IF ExperimentUpdates > 50 THEN
        Parameters.ProportionalGain := GainBeforeRevision;
    END_IF;
END_IF;
ExperimentWasEnabled := ExperimentEnabled;
IF Active THEN
    IF ExperimentUpdates = 50 THEN
        GainBeforeRevision := Parameters.ProportionalGain;
        Parameters.ProportionalGain := LREAL#24.0;
    END_IF;
    ExperimentUpdates := ExperimentUpdates + 1;
END_IF;

(* Control Studio's Apply replaces this complete record at the task boundary. *)
IF NOT Initialized OR NOT SamePidParameters(Left := Parameters, Right := ConfiguredParameters) THEN
    ConfiguredParameters := Parameters;
    ParameterRevision := ParameterRevision + 1;
END_IF;
Controller.Configure(Parameters := Parameters);
(* Tracking with Tt = Δt removes the whole excess: this controller retains the applied torque. *)
RetainedAppliedParameters := Parameters;
RetainedAppliedParameters.TrackingTime := LREAL#0.02;
RetainedApplied.Configure(Parameters := RetainedAppliedParameters);

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
    RetainedApplied.Initialize(State := InitialState);
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

(* The positional form with its three rules, the same form without the gain-revision rule, and Control.PID
   retaining the applied torque. *)
Positional(Parameters := Parameters, Input := Input);
Unpreloaded(Parameters := Parameters, Input := Input, PreloadOnGainRevision := FALSE);
RetainedAppliedTorqueNm := RetainedApplied.Update(Input := Input);
PositionalTorqueNm := Positional.TorqueNm;
UnpreloadedTorqueNm := Unpreloaded.TorqueNm;
ErrorRad := Positional.ErrorRad;
ProportionalNm := Positional.ProportionalNm;
DerivativeNm := Positional.DerivativeNm;
RequestNm := Positional.RequestNm;
IntegralNm := Positional.IntegralNm;

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
    P1.RequestNm AT %ML72 : LREAL;
    P1.IntegralNm AT %ML80 : LREAL;
    P1.ExperimentEnabled AT %MX88.0 : BOOL;
    P1.Active AT %MX88.1 : BOOL;
    P1.PositionalTorqueNm AT %ML96 : LREAL;
    P1.RetainedAppliedTorqueNm AT %ML104 : LREAL;
    P1.UnpreloadedTorqueNm AT %ML112 : LREAL;
END_VAR
END_CONFIGURATION
