USING Control;

(* The torque with which the axis follows the planned motion: J·a + B·v. *)
FUNCTION FeedforwardTorque : LREAL
VAR_INPUT
    Inertia : LREAL;
    Friction : LREAL;
    AccelerationRadS2 : LREAL;
    VelocityRadS : LREAL;
END_VAR
FeedforwardTorque := Inertia * AccelerationRadS2 + Friction * VelocityRadS;
END_FUNCTION

(* A trapezoidal move: accelerate at A to V, cruise, decelerate at A; a move shorter than V²/A never reaches V. *)
FUNCTION MoveDurationS : LREAL
VAR_INPUT
    DistanceRad : LREAL;
    MaxVelocityRadS : LREAL;
    AccelerationRadS2 : LREAL;
END_VAR
IF DistanceRad >= MaxVelocityRadS * MaxVelocityRadS / AccelerationRadS2 THEN
    MoveDurationS := DistanceRad / MaxVelocityRadS + MaxVelocityRadS / AccelerationRadS2;
ELSE
    MoveDurationS := LREAL#2.0 * SQRT(DistanceRad / AccelerationRadS2);
END_IF;
END_FUNCTION

(* D on the measurement alone subtracts Kd·v from the torque while the axis moves. *)
FUNCTION DerivativeBrakeTorque : LREAL
VAR_INPUT
    DerivativeGain : LREAL;
    VelocityRadS : LREAL;
END_VAR
DerivativeBrakeTorque := DerivativeGain * VelocityRadS;
END_FUNCTION

PROGRAM Main
VAR
    Controller : Control.PID;
    (* Lesson 42's PID; its derivative acts on the error, because the planned angle has no jumps. Tune replaces the gains. *)
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#18.77,
        IntegralGain := LREAL#17.46,
        DerivativeGain := LREAL#7.27,
        ProportionalSetpointWeight := LREAL#1.0,
        DerivativeSetpointWeight := LREAL#1.0,
        Bias := LREAL#0.0,
        OutputMinimum := LREAL#-20.0,
        OutputMaximum := LREAL#20.0,
        RateMinimum := LREAL#-10000.0,
        RateMaximum := LREAL#10000.0,
        TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#BackCalculation
    );
    (* Trajectory feedforward Kv·ṙ + Ka·r̈; Tune sets Kv and Ka from the model. *)
    RateGain : LREAL := 0.0;
    AccelerationGain : LREAL := 0.0;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    AutoMode : BOOL := FALSE;
    ManualTorqueNm : LREAL := 0.0;
    SafetyAbort : BOOL := FALSE;
    SetpointRad : LREAL := 0.0;
    AngleRad : LREAL := 0.0;
    TorqueNm : LREAL := 0.0;
    FeedForwardNm : LREAL := 0.0;
    (* The motion profile: a new setpoint starts a move from the planned angle, at most 1 rad/s and 2 rad/s². *)
    MaxVelocityRadS : LREAL := 1.0;
    AccelerationRadS2 : LREAL := 2.0;
    TargetRad : LREAL := 0.0;
    MoveStartRad : LREAL := 0.0;
    MoveTimeS : LREAL := 0.0;
    ReferenceRad : LREAL := 0.0;
    ReferenceVelocityRadS : LREAL := 0.0;
    ReferenceAccelerationRadS2 : LREAL := 0.0;
    Distance : LREAL;
    Direction : LREAL;
    PeakVelocity : LREAL;
    AccelerationTime : LREAL;
    CruiseTime : LREAL;
    Duration : LREAL;
    Remaining : LREAL;
    Travelled : LREAL;
    Speed : LREAL;
    Acceleration : LREAL;
    CycleTimeS : LREAL := 0.02;
    AngleLimitRad : LREAL := 1.8;
END_VAR

(* Leaving the travel range removes the torque until a full restart. *)
IF ABS(AngleRad) > AngleLimitRad THEN
    SafetyAbort := TRUE;
END_IF;

(* In Manual the plan follows the axis; in Auto a new setpoint starts a move from the planned angle. *)
IF NOT AutoMode OR SafetyAbort THEN
    TargetRad := AngleRad;
    MoveStartRad := AngleRad;
    MoveTimeS := LREAL#0.0;
ELSIF SetpointRad <> TargetRad THEN
    TargetRad := SetpointRad;
    MoveStartRad := ReferenceRad;
    MoveTimeS := LREAL#0.0;
END_IF;
(* The trapezoid for the distance: its peak speed, acceleration time and cruise time. *)
Distance := ABS(TargetRad - MoveStartRad);
IF TargetRad >= MoveStartRad THEN
    Direction := LREAL#1.0;
ELSE
    Direction := LREAL#-1.0;
END_IF;
IF Distance >= MaxVelocityRadS * MaxVelocityRadS / AccelerationRadS2 THEN
    PeakVelocity := MaxVelocityRadS;
    CruiseTime := (Distance - MaxVelocityRadS * MaxVelocityRadS / AccelerationRadS2) / MaxVelocityRadS;
ELSE
    PeakVelocity := SQRT(Distance * AccelerationRadS2);
    CruiseTime := LREAL#0.0;
END_IF;
AccelerationTime := PeakVelocity / AccelerationRadS2;
Duration := LREAL#2.0 * AccelerationTime + CruiseTime;
IF MoveTimeS < AccelerationTime THEN
    Travelled := LREAL#0.5 * AccelerationRadS2 * MoveTimeS * MoveTimeS;
    Speed := AccelerationRadS2 * MoveTimeS;
    Acceleration := AccelerationRadS2;
ELSIF MoveTimeS < AccelerationTime + CruiseTime THEN
    Travelled := LREAL#0.5 * AccelerationRadS2 * AccelerationTime * AccelerationTime + PeakVelocity * (MoveTimeS - AccelerationTime);
    Speed := PeakVelocity;
    Acceleration := LREAL#0.0;
ELSIF MoveTimeS < Duration THEN
    Remaining := Duration - MoveTimeS;
    Travelled := Distance - LREAL#0.5 * AccelerationRadS2 * Remaining * Remaining;
    Speed := AccelerationRadS2 * Remaining;
    Acceleration := -AccelerationRadS2;
ELSE
    Travelled := Distance;
    Speed := LREAL#0.0;
    Acceleration := LREAL#0.0;
END_IF;
ReferenceRad := MoveStartRad + Direction * Travelled;
ReferenceVelocityRadS := Direction * Speed;
ReferenceAccelerationRadS2 := Direction * Acceleration;
IF MoveTimeS < Duration THEN
    MoveTimeS := MoveTimeS + CycleTimeS;
END_IF;
FeedForwardNm := RateGain * ReferenceVelocityRadS + AccelerationGain * ReferenceAccelerationRadS2;

Controller.Configure(Parameters := Parameters);
IF NOT Initialized THEN
    (* Hot reset: the axis rests without torque and the setpoint follows the measured angle. *)
    ManualTorqueNm := LREAL#0.0;
    IF NOT AutoMode THEN
        SetpointRad := AngleRad;
    END_IF;
    InitialState.Setpoint := ReferenceRad;
    InitialState.ProcessValue := AngleRad;
    InitialState.NominalOutput := ManualTorqueNm;
    InitialState.SaturatedOutput := ManualTorqueNm;
    InitialState.SetpointRate := LREAL#0.0;
    InitialState.ProcessValueRate := LREAL#0.0;
    InitialState.FeedForward := LREAL#0.0;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;

Input.Setpoint := ReferenceRad;
Input.ProcessValue := AngleRad;
Input.CycleTime := CycleTimeS;
Input.FeedForward := FeedForwardNm;
Input.TrackingOutput := ManualTorqueNm;
IF SafetyAbort THEN
    Input.Mode := PIDMode#Manual;
    Input.ManualOutput := LREAL#0.0;
ELSIF AutoMode THEN
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
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.SafetyAbort AT %MX96.0 : BOOL;
    P1.TorqueNm AT %QL0 : LREAL;
    P1.AngleRad AT %IL0 : LREAL;
    P1.SetpointRad AT %ML0 : LREAL;
    P1.ManualTorqueNm AT %ML8 : LREAL;
END_VAR
END_CONFIGURATION
