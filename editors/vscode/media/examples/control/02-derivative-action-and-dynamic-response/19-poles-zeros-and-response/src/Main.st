USING Control;

(* Pole of the first-order lag τ·dx/dt + x = K·u: the root of τs + 1 = 0. *)
FUNCTION FirstOrderPole : LREAL
VAR_INPUT
    TimeConstantS : LREAL;
END_VAR
FirstOrderPole := LREAL#-1.0 / TimeConstantS;
END_FUNCTION

(* Natural response of a real pole p: a deviation decays, or grows, as e^(p·t). *)
FUNCTION RealPoleResponse : LREAL
VAR_INPUT
    Pole : LREAL;
    Seconds : LREAL;
END_VAR
RealPoleResponse := EXP(Pole * Seconds);
END_FUNCTION

(* Poles of the shaft under P control, J·s² + B·s + Kp = 0: s = σ ± jω with σ = −B/(2J) and ω = √(Kp/J − σ²),
   a complex pair while Kp/J > σ². *)
FUNCTION ShaftPoleRealPart : LREAL
VAR_INPUT
    Inertia : LREAL;
    Friction : LREAL;
END_VAR
ShaftPoleRealPart := -Friction / (LREAL#2.0 * Inertia);
END_FUNCTION

FUNCTION ShaftPoleImaginaryPart : LREAL
VAR_INPUT
    Inertia : LREAL;
    Friction : LREAL;
    ProportionalGain : LREAL;
END_VAR
ShaftPoleImaginaryPart := SQRT(ProportionalGain / Inertia - (Friction / (LREAL#2.0 * Inertia)) ** LREAL#2.0);
END_FUNCTION

(* Unit-step response of a system with unit steady gain and the complex poles σ ± jω:
   1 − e^(σ·t)·(cos(ω·t) − (σ/ω)·sin(ω·t)). *)
FUNCTION ComplexPoleStep : LREAL
VAR_INPUT
    Sigma : LREAL;
    Omega : LREAL;
    Seconds : LREAL;
END_VAR
ComplexPoleStep := LREAL#1.0 - EXP(Sigma * Seconds) * (COS(Omega * Seconds) - Sigma / Omega * SIN(Omega * Seconds));
END_FUNCTION

PROGRAM Main
VAR
    (* The heater of Lessons 12–30 in open loop: G(s) = 0.2/(5s + 1) with the pole −0.2 s⁻¹. It rests at 36 °C
       with 80 %; the experiment returns it to 75 %, the command for 35 °C. *)
    TemperatureC : LREAL := 36.0;
    HeaterPowerPct : LREAL := 80.0;
    HeaterTimeConstantS : LREAL := 5.0;
    HeaterPole : LREAL := 0.0;
    HeaterPoleResponseC : LREAL := 36.0;
    (* The shaft of Lesson 34 under P control, Kp = 4 N·m/rad: Θ/R = 4/(s² + s + 4) with the poles −0.5 ± j1.936. *)
    Controller : Control.PID;
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#4.0,
        IntegralGain := LREAL#0.0,
        DerivativeGain := LREAL#0.0,
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
    SetpointRad : LREAL := 0.0;
    AngleRad : LREAL := 0.0;
    VelocityRadS : LREAL := 0.0;
    TorqueNm : LREAL := 0.0;
    InertiaKgM2 : LREAL := 1.0;
    FrictionNmsRad : LREAL := 1.0;
    ShaftSigma : LREAL := 0.0;
    ShaftOmega : LREAL := 0.0;
    ShaftPoleResponseRad : LREAL := 0.0;
    (* Control Studio's project experiment starts both responses at once. *)
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    Active : BOOL := FALSE;
    ElapsedS : LREAL := 0.0;
END_VAR

IF ExperimentEnabled AND NOT ExperimentWasEnabled THEN
    Active := TRUE;
    ElapsedS := LREAL#0.0;
ELSIF NOT ExperimentEnabled THEN
    Active := FALSE;
END_IF;
ExperimentWasEnabled := ExperimentEnabled;

IF Active THEN
    HeaterPowerPct := LREAL#75.0;
    SetpointRad := LREAL#1.0;
ELSE
    HeaterPowerPct := LREAL#80.0;
    SetpointRad := LREAL#0.0;
END_IF;

Controller.Configure(Parameters := Parameters);
IF NOT Initialized THEN
    State.Setpoint := SetpointRad;
    State.ProcessValue := AngleRad;
    Controller.Initialize(State := State);
    Initialized := TRUE;
END_IF;
Input.Mode := PIDMode#Auto;
Input.CycleTime := LREAL#0.02;
Input.Setpoint := SetpointRad;
Input.ProcessValue := AngleRad;
TorqueNm := Controller.Update(Input := Input);

(* Responses read from the poles, timed from the experiment's start. The heater's command arrives 1 s later. *)
HeaterPole := FirstOrderPole(TimeConstantS := HeaterTimeConstantS);
ShaftSigma := ShaftPoleRealPart(Inertia := InertiaKgM2, Friction := FrictionNmsRad);
ShaftOmega := ShaftPoleImaginaryPart(Inertia := InertiaKgM2, Friction := FrictionNmsRad,
    ProportionalGain := Parameters.ProportionalGain);
IF Active AND ElapsedS >= LREAL#1.0 THEN
    HeaterPoleResponseC := LREAL#35.0 + RealPoleResponse(Pole := HeaterPole, Seconds := ElapsedS - LREAL#1.0);
ELSE
    HeaterPoleResponseC := LREAL#36.0;
END_IF;
IF Active THEN
    ShaftPoleResponseRad := ComplexPoleStep(Sigma := ShaftSigma, Omega := ShaftOmega, Seconds := ElapsedS);
    ElapsedS := ElapsedS + LREAL#0.02;
ELSE
    ShaftPoleResponseRad := LREAL#0.0;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.HeaterPowerPct AT %QL0 : LREAL;
    P1.TorqueNm AT %QL8 : LREAL;
    P1.TemperatureC AT %IL0 : LREAL;
    P1.AngleRad AT %IL8 : LREAL;
    P1.VelocityRadS AT %IL16 : LREAL;
    P1.SetpointRad AT %ML0 : LREAL;
    P1.HeaterPoleResponseC AT %ML8 : LREAL;
    P1.ShaftPoleResponseRad AT %ML16 : LREAL;
    P1.ExperimentEnabled AT %MX24.0 : BOOL;
    P1.Active AT %MX24.1 : BOOL;
END_VAR
END_CONFIGURATION
