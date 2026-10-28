USING Control;

(* The loop of this lesson, L(jω) = K·e^(−jω·td)/(1 + jωτ) with K = Kp·0.2 = 2 and τ = 5 s. With c = cos(ω·td) and
   q = sin(ω·td): Re L = K·(c − ωτ·q)/(1 + (ωτ)²) and Im L = −K·(q + ωτ·c)/(1 + (ωτ)²). *)
FUNCTION LoopReal : LREAL
VAR_INPUT
    LoopGain : LREAL;
    TimeConstantS : LREAL;
    DelayS : LREAL;
    OmegaRadS : LREAL;
END_VAR
LoopReal := LoopGain * (COS(OmegaRadS * DelayS) - OmegaRadS * TimeConstantS * SIN(OmegaRadS * DelayS))
    / (LREAL#1.0 + (OmegaRadS * TimeConstantS) ** LREAL#2.0);
END_FUNCTION

FUNCTION LoopImag : LREAL
VAR_INPUT
    LoopGain : LREAL;
    TimeConstantS : LREAL;
    DelayS : LREAL;
    OmegaRadS : LREAL;
END_VAR
LoopImag := -LoopGain * (SIN(OmegaRadS * DelayS) + OmegaRadS * TimeConstantS * COS(OmegaRadS * DelayS))
    / (LREAL#1.0 + (OmegaRadS * TimeConstantS) ** LREAL#2.0);
END_FUNCTION

(* |L| = K/√(1 + (ωτ)²) = 1 at the crossover ω_c = √(K² − 1)/τ, whatever the delay. *)
FUNCTION CrossoverRadS : LREAL
VAR_INPUT
    LoopGain : LREAL;
    TimeConstantS : LREAL;
END_VAR
CrossoverRadS := SQRT(LoopGain * LoopGain - LREAL#1.0) / TimeConstantS;
END_FUNCTION

(* The delay that brings the phase at ω_c to −180°, so that L(jω_c) = −1: arctan(ω_c·τ) + ω_c·td = π. *)
FUNCTION CriticalDelayS : LREAL
VAR_INPUT
    LoopGain : LREAL;
    TimeConstantS : LREAL;
END_VAR
VAR
    Crossover : LREAL;
END_VAR
Crossover := CrossoverRadS(LoopGain := LoopGain, TimeConstantS := TimeConstantS);
CriticalDelayS := (LREAL#3.14159265358979 - ATAN(Crossover * TimeConstantS)) / Crossover;
END_FUNCTION

PROGRAM Main
VAR
    (* Two copies of the heater, delivery delay 1 s and 8 s, each under Control.PID as P control: Kp = 10 % per °C
       about the resting command, u = 75 + 10·(SP − T). *)
    ShortDelayController : Control.PID;
    LongDelayController : Control.PID;
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#10.0,
        IntegralGain := LREAL#0.0,
        DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#1.0,
        DerivativeSetpointWeight := LREAL#0.0,
        Bias := LREAL#75.0,
        OutputMinimum := LREAL#0.0,
        OutputMaximum := LREAL#100.0,
        RateMinimum := LREAL#-1000.0,
        RateMaximum := LREAL#1000.0,
        TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#BackCalculation
    );
    State : Control.PIDInitialState;
    ShortDelayInput : Control.PIDInput;
    LongDelayInput : Control.PIDInput;
    Initialized : BOOL := FALSE;
    ShortDelayC : LREAL := 35.0;
    ShortDelayPct : LREAL := 75.0;
    LongDelayC : LREAL := 35.0;
    LongDelayPct : LREAL := 75.0;
    SetpointC : LREAL := 35.0;
    (* A heater that leaves 34–36 °C gets the resting 75 % until the experiment ends. *)
    ShortDelayAbort : BOOL := FALSE;
    LongDelayAbort : BOOL := FALSE;
    (* Control Studio's project experiment steps the setpoint of both loops to 35.1 °C. *)
    ExperimentEnabled : BOOL := FALSE;
END_VAR

IF ExperimentEnabled THEN
    SetpointC := LREAL#35.1;
ELSE
    SetpointC := LREAL#35.0;
    ShortDelayAbort := FALSE;
    LongDelayAbort := FALSE;
END_IF;
ShortDelayAbort := ShortDelayAbort OR ShortDelayC < LREAL#34.0 OR ShortDelayC > LREAL#36.0;
LongDelayAbort := LongDelayAbort OR LongDelayC < LREAL#34.0 OR LongDelayC > LREAL#36.0;

ShortDelayController.Configure(Parameters := Parameters);
LongDelayController.Configure(Parameters := Parameters);
IF NOT Initialized THEN
    State.Setpoint := LREAL#35.0;
    State.ProcessValue := LREAL#35.0;
    State.NominalOutput := LREAL#75.0;
    State.SaturatedOutput := LREAL#75.0;
    ShortDelayController.Initialize(State := State);
    LongDelayController.Initialize(State := State);
    Initialized := TRUE;
END_IF;
ShortDelayInput.Mode := PIDMode#Auto;
ShortDelayInput.CycleTime := LREAL#0.1;
ShortDelayInput.Setpoint := SetpointC;
ShortDelayInput.ProcessValue := ShortDelayC;
ShortDelayPct := ShortDelayController.Update(Input := ShortDelayInput);
LongDelayInput.Mode := PIDMode#Auto;
LongDelayInput.CycleTime := LREAL#0.1;
LongDelayInput.Setpoint := SetpointC;
LongDelayInput.ProcessValue := LongDelayC;
LongDelayPct := LongDelayController.Update(Input := LongDelayInput);
IF ShortDelayAbort THEN
    ShortDelayPct := LREAL#75.0;
END_IF;
IF LongDelayAbort THEN
    LongDelayPct := LREAL#75.0;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ShortDelayPct AT %QL0 : LREAL;
    P1.ShortDelayC AT %IL0 : LREAL;
    P1.LongDelayPct AT %QL8 : LREAL;
    P1.LongDelayC AT %IL8 : LREAL;
    P1.SetpointC AT %ML0 : LREAL;
    P1.ExperimentEnabled AT %MX16.0 : BOOL;
    P1.ShortDelayAbort AT %MX16.1 : BOOL;
    P1.LongDelayAbort AT %MX16.2 : BOOL;
END_VAR
END_CONFIGURATION
