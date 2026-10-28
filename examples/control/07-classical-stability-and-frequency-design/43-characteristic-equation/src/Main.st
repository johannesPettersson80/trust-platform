USING Control;

(* Characteristic equation of the shaft under PD control, J·s² + (B + Kd)·s + Kp = 0, divided by J:
   s² + a1·s + a0 = 0 with a1 = (B + Kd)/J in s⁻¹ and a0 = Kp/J in s⁻². *)
FUNCTION DampingCoefficientOf : LREAL
VAR_INPUT
    Inertia : LREAL;
    Friction : LREAL;
    DerivativeGain : LREAL;
END_VAR
DampingCoefficientOf := (Friction + DerivativeGain) / Inertia;
END_FUNCTION

FUNCTION StiffnessCoefficientOf : LREAL
VAR_INPUT
    Inertia : LREAL;
    ProportionalGain : LREAL;
END_VAR
StiffnessCoefficientOf := ProportionalGain / Inertia;
END_FUNCTION

(* Natural motion for a repeated root p: z = (z1 + (v1 − p·z1)·t)·e^(p·t) from the displacement z1 and the
   velocity v1 at t = 0, and its velocity (v1 + p·(v1 − p·z1)·t)·e^(p·t). *)
FUNCTION RepeatedRootDisplacement : LREAL
VAR_INPUT
    Displacement : LREAL;
    Velocity : LREAL;
    Root : LREAL;
    Seconds : LREAL;
END_VAR
RepeatedRootDisplacement := (Displacement + (Velocity - Root * Displacement) * Seconds) * EXP(Root * Seconds);
END_FUNCTION

FUNCTION RepeatedRootVelocity : LREAL
VAR_INPUT
    Displacement : LREAL;
    Velocity : LREAL;
    Root : LREAL;
    Seconds : LREAL;
END_VAR
RepeatedRootVelocity := (Velocity + Root * (Velocity - Root * Displacement) * Seconds) * EXP(Root * Seconds);
END_FUNCTION

(* A real root of a·s² + b·s + c = 0: (−b + sign·√(b² − 4ac))/(2a), with sign +1 or −1. *)
FUNCTION QuadraticRoot : LREAL
VAR_INPUT
    SquareCoefficient : LREAL;
    LinearCoefficient : LREAL;
    ConstantCoefficient : LREAL;
    Sign : LREAL;
END_VAR
QuadraticRoot := (-LinearCoefficient + Sign * SQRT(LinearCoefficient * LinearCoefficient
    - LREAL#4.0 * SquareCoefficient * ConstantCoefficient)) / (LREAL#2.0 * SquareCoefficient);
END_FUNCTION

(* Natural motion for two distinct real roots p1 and p2: z = c1·e^(p1·t) + c2·e^(p2·t) with c1 + c2 = z1 and
   p1·c1 + p2·c2 = v1, and its velocity p1·c1·e^(p1·t) + p2·c2·e^(p2·t). *)
FUNCTION TwoRootDisplacement : LREAL
VAR_INPUT
    Displacement : LREAL;
    Velocity : LREAL;
    FirstRoot : LREAL;
    SecondRoot : LREAL;
    Seconds : LREAL;
END_VAR
TwoRootDisplacement := ((Velocity - SecondRoot * Displacement) * EXP(FirstRoot * Seconds)
    + (FirstRoot * Displacement - Velocity) * EXP(SecondRoot * Seconds)) / (FirstRoot - SecondRoot);
END_FUNCTION

FUNCTION TwoRootVelocity : LREAL
VAR_INPUT
    Displacement : LREAL;
    Velocity : LREAL;
    FirstRoot : LREAL;
    SecondRoot : LREAL;
    Seconds : LREAL;
END_VAR
TwoRootVelocity := (FirstRoot * (Velocity - SecondRoot * Displacement) * EXP(FirstRoot * Seconds)
    + SecondRoot * (FirstRoot * Displacement - Velocity) * EXP(SecondRoot * Seconds)) / (FirstRoot - SecondRoot);
END_FUNCTION

(* The heater of Lesson 49 under PI control, without its delay: 5·dx/dt + x = 0.2·v with v = −2.5·x + i and
   di/dt = −0.5·x, where x is the temperature and i the integral, as deviations from 35 °C and 75 %. *)
FUNCTION HeaterTemperatureRate : LREAL
VAR_INPUT
    TemperatureDeviationC : LREAL;
    IntegralDeviationPct : LREAL;
END_VAR
HeaterTemperatureRate := LREAL#-0.3 * TemperatureDeviationC + LREAL#0.04 * IntegralDeviationPct;
END_FUNCTION

FUNCTION HeaterIntegralRate : LREAL
VAR_INPUT
    TemperatureDeviationC : LREAL;
END_VAR
HeaterIntegralRate := LREAL#-0.5 * TemperatureDeviationC;
END_FUNCTION

PROGRAM Main
VAR
    (* The shaft of Lesson 35 under PD control: u = Kp·(r − θ) − Kd·dθ/dt with Kp = 16 N·m/rad and
       Kd = 7 N·m·s/rad, limited to ±20 N·m. *)
    Controller : Control.PID;
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#16.0,
        IntegralGain := LREAL#0.0,
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
    State : Control.PIDInitialState;
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    InertiaKgM2 : LREAL := 1.0;
    FrictionNmsRad : LREAL := 1.0;
    SetpointRad : LREAL := 0.0;
    AngleRad : LREAL := 0.0;
    VelocityRadS : LREAL := 0.0;
    TorqueNm : LREAL := 0.0;
    (* Control Studio's project experiment knocks the shaft: 10 N·m for five 20 ms updates. *)
    KnockTorqueNm : LREAL := 0.0;
    KnockUpdates : INT := 0;
    (* s² + a1·s + a0 = 0 and its repeated root −a1/2, which requires a1² = 4·a0. *)
    DampingCoefficient : LREAL := 0.0;
    StiffnessCoefficient : LREAL := 0.0;
    RootPerS : LREAL := 0.0;
    (* The sampled law: each torque is held for one update, and D uses the angle change over that update. P acts on
       average half an update late and D a whole update late; with e^(−s·τ) ≈ 1 − s·τ the equation becomes
       (J − Kd·Δt)·s² + (B + Kd − Kp·Δt/2)·s + Kp = 0. *)
    UpdateS : LREAL := 0.02;
    SampledSquareCoefficient : LREAL := 0.0;
    SampledLinearCoefficient : LREAL := 0.0;
    SlowRootPerS : LREAL := 0.0;
    FastRootPerS : LREAL := 0.0;
    (* The natural motion from the state measured when the knock ends; before that, the measured state. *)
    StartDisplacementRad : LREAL := 0.0;
    StartVelocityRadS : LREAL := 0.0;
    Predicting : BOOL := FALSE;
    AfterKnockS : LREAL := 0.0;
    ModeAngleRad : LREAL := 0.0;
    ModeVelocityRadS : LREAL := 0.0;
    SampledModeAngleRad : LREAL := 0.0;
    SampledModeVelocityRadS : LREAL := 0.0;
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    Active : BOOL := FALSE;
END_VAR

DampingCoefficient := DampingCoefficientOf(Inertia := InertiaKgM2, Friction := FrictionNmsRad,
    DerivativeGain := Parameters.DerivativeGain);
StiffnessCoefficient := StiffnessCoefficientOf(Inertia := InertiaKgM2, ProportionalGain := Parameters.ProportionalGain);
RootPerS := -DampingCoefficient / LREAL#2.0;
SampledSquareCoefficient := InertiaKgM2 - Parameters.DerivativeGain * UpdateS;
SampledLinearCoefficient := FrictionNmsRad + Parameters.DerivativeGain - Parameters.ProportionalGain * UpdateS / LREAL#2.0;
SlowRootPerS := QuadraticRoot(SquareCoefficient := SampledSquareCoefficient, LinearCoefficient := SampledLinearCoefficient,
    ConstantCoefficient := Parameters.ProportionalGain, Sign := LREAL#1.0);
FastRootPerS := QuadraticRoot(SquareCoefficient := SampledSquareCoefficient, LinearCoefficient := SampledLinearCoefficient,
    ConstantCoefficient := Parameters.ProportionalGain, Sign := LREAL#-1.0);

IF ExperimentEnabled AND NOT ExperimentWasEnabled THEN
    Active := TRUE;
    KnockUpdates := 0;
    Predicting := FALSE;
ELSIF NOT ExperimentEnabled THEN
    Active := FALSE;
    Predicting := FALSE;
END_IF;
ExperimentWasEnabled := ExperimentEnabled;

KnockTorqueNm := LREAL#0.0;
IF Active AND KnockUpdates < 5 THEN
    KnockTorqueNm := LREAL#10.0;
    KnockUpdates := KnockUpdates + 1;
ELSIF Active AND NOT Predicting THEN
    StartDisplacementRad := AngleRad - SetpointRad;
    StartVelocityRadS := VelocityRadS;
    AfterKnockS := LREAL#0.0;
    Predicting := TRUE;
END_IF;

Controller.Configure(Parameters := Parameters);
IF NOT Initialized THEN
    State.Setpoint := SetpointRad;
    State.ProcessValue := AngleRad;
    Controller.Initialize(State := State);
    Initialized := TRUE;
END_IF;
Input.Mode := PIDMode#Auto;
Input.CycleTime := UpdateS;
Input.Setpoint := SetpointRad;
Input.ProcessValue := AngleRad;
TorqueNm := Controller.Update(Input := Input);

IF Predicting THEN
    ModeAngleRad := SetpointRad + RepeatedRootDisplacement(Displacement := StartDisplacementRad,
        Velocity := StartVelocityRadS, Root := RootPerS, Seconds := AfterKnockS);
    ModeVelocityRadS := RepeatedRootVelocity(Displacement := StartDisplacementRad,
        Velocity := StartVelocityRadS, Root := RootPerS, Seconds := AfterKnockS);
    SampledModeAngleRad := SetpointRad + TwoRootDisplacement(Displacement := StartDisplacementRad,
        Velocity := StartVelocityRadS, FirstRoot := SlowRootPerS, SecondRoot := FastRootPerS, Seconds := AfterKnockS);
    SampledModeVelocityRadS := TwoRootVelocity(Displacement := StartDisplacementRad,
        Velocity := StartVelocityRadS, FirstRoot := SlowRootPerS, SecondRoot := FastRootPerS, Seconds := AfterKnockS);
    AfterKnockS := AfterKnockS + UpdateS;
ELSE
    ModeAngleRad := AngleRad;
    ModeVelocityRadS := VelocityRadS;
    SampledModeAngleRad := AngleRad;
    SampledModeVelocityRadS := VelocityRadS;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.TorqueNm AT %QL0 : LREAL;
    P1.KnockTorqueNm AT %QL8 : LREAL;
    P1.AngleRad AT %IL0 : LREAL;
    P1.VelocityRadS AT %IL8 : LREAL;
    P1.SetpointRad AT %ML0 : LREAL;
    P1.ModeAngleRad AT %ML8 : LREAL;
    P1.ModeVelocityRadS AT %ML16 : LREAL;
    P1.ExperimentEnabled AT %MX24.0 : BOOL;
    P1.Active AT %MX24.1 : BOOL;
    P1.DampingCoefficient AT %ML32 : LREAL;
    P1.StiffnessCoefficient AT %ML40 : LREAL;
    P1.RootPerS AT %ML48 : LREAL;
    P1.SampledModeAngleRad AT %ML56 : LREAL;
    P1.SampledModeVelocityRadS AT %ML64 : LREAL;
    P1.SlowRootPerS AT %ML72 : LREAL;
    P1.FastRootPerS AT %ML80 : LREAL;
END_VAR
END_CONFIGURATION
