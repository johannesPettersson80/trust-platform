USING Control;

(* Five periods of amplitude·sin(ω·t) from Start, then 0. *)
FUNCTION_BLOCK FivePeriodSine
VAR_INPUT
    Start : BOOL;
    Enable : BOOL;
    Amplitude : LREAL;
    OmegaRadS : LREAL;
END_VAR
VAR_OUTPUT
    Value : LREAL := 0.0;
    ElapsedS : LREAL := 0.0;
    Running : BOOL := FALSE;
END_VAR
IF Start THEN
    Running := TRUE;
    ElapsedS := LREAL#0.0;
END_IF;
IF NOT Enable THEN
    Running := FALSE;
END_IF;
IF Running AND ElapsedS >= LREAL#5.0 * LREAL#2.0 * LREAL#3.14159265358979 / OmegaRadS - LREAL#1.0E-9 THEN
    Running := FALSE;
END_IF;
IF Running THEN
    Value := Amplitude * SIN(OmegaRadS * ElapsedS);
    ElapsedS := ElapsedS + LREAL#0.1;
ELSE
    Value := LREAL#0.0;
END_IF;
END_FUNCTION_BLOCK

(* The loop transfer of this lesson, L = C·G with C = 5 + 1/s and G = 0.2·e^(−s)/(5s + 1): the PI zero cancels the
   heater's lag and L(jω) = 0.2·e^(−jω)/(jω) = −0.2·sin(ω)/ω − j·0.2·cos(ω)/ω. *)
FUNCTION LoopReal : LREAL
VAR_INPUT
    OmegaRadS : LREAL;
END_VAR
LoopReal := LREAL#-0.2 * SIN(OmegaRadS) / OmegaRadS;
END_FUNCTION

FUNCTION LoopImag : LREAL
VAR_INPUT
    OmegaRadS : LREAL;
END_VAR
LoopImag := LREAL#-0.2 * COS(OmegaRadS) / OmegaRadS;
END_FUNCTION

(* S = 1/(1 + L) and T = L/(1 + L). With 1 + L = a + jb, |S| = 1/√(a² + b²) and |T| = |L|·|S|. *)
FUNCTION SensitivityMagnitude : LREAL
VAR_INPUT
    LoopRealPart : LREAL;
    LoopImagPart : LREAL;
END_VAR
SensitivityMagnitude := LREAL#1.0 / SQRT((LREAL#1.0 + LoopRealPart) ** LREAL#2.0 + LoopImagPart ** LREAL#2.0);
END_FUNCTION

FUNCTION ComplementaryMagnitude : LREAL
VAR_INPUT
    LoopRealPart : LREAL;
    LoopImagPart : LREAL;
END_VAR
ComplementaryMagnitude := SQRT(LoopRealPart ** LREAL#2.0 + LoopImagPart ** LREAL#2.0)
    * SensitivityMagnitude(LoopRealPart := LoopRealPart, LoopImagPart := LoopImagPart);
END_FUNCTION

PROGRAM Main
VAR
    (* Two copies of the heater, each held at 35 °C by Control.PID as PI, Kp = 5 % per °C and Ki = 1 % per °C·s. *)
    LoadLoop : Control.PID;
    NoiseLoop : Control.PID;
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#5.0,
        IntegralGain := LREAL#1.0,
        DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#1.0,
        DerivativeSetpointWeight := LREAL#0.0,
        Bias := LREAL#0.0,
        OutputMinimum := LREAL#0.0,
        OutputMaximum := LREAL#100.0,
        RateMinimum := LREAL#-1000.0,
        RateMaximum := LREAL#1000.0,
        TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#BackCalculation
    );
    State : Control.PIDInitialState;
    LoadInput : Control.PIDInput;
    NoiseInput : Control.PIDInput;
    Initialized : BOOL := FALSE;
    SetpointC : LREAL := 35.0;
    (* The first heater: a heat load D enters the water. *)
    LoadTemperatureC : LREAL := 35.0;
    LoadCommandPct : LREAL := 75.0;
    LoadKw : LREAL := 0.0;
    (* The second heater: noise n enters only the reading its controller uses. *)
    NoiseTemperatureC : LREAL := 35.0;
    NoiseCommandPct : LREAL := 75.0;
    NoiseHeaterLoadKw : LREAL := 0.0;
    NoiseC : LREAL := 0.0;
    MeasuredTemperatureC : LREAL := 35.0;
    (* Control Studio's project experiment starts both injections: five periods at 0.2 rad/s. *)
    LoadSource : FivePeriodSine;
    NoiseSource : FivePeriodSine;
    LoadAmplitudeKw : LREAL := 10.0;
    NoiseAmplitudeC : LREAL := 0.05;
    OmegaRadS : LREAL := 0.2;
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    Start : BOOL := FALSE;
    (* |S| and |T| at OmegaRadS. *)
    SensitivityAtOmega : LREAL := 0.0;
    ComplementaryAtOmega : LREAL := 0.0;
END_VAR

SensitivityAtOmega := SensitivityMagnitude(LoopRealPart := LoopReal(OmegaRadS := OmegaRadS),
    LoopImagPart := LoopImag(OmegaRadS := OmegaRadS));
ComplementaryAtOmega := ComplementaryMagnitude(LoopRealPart := LoopReal(OmegaRadS := OmegaRadS),
    LoopImagPart := LoopImag(OmegaRadS := OmegaRadS));

SetpointC := LREAL#35.0;
Start := ExperimentEnabled AND NOT ExperimentWasEnabled;
ExperimentWasEnabled := ExperimentEnabled;
LoadSource(Start := Start, Enable := ExperimentEnabled, Amplitude := LoadAmplitudeKw, OmegaRadS := OmegaRadS);
NoiseSource(Start := Start, Enable := ExperimentEnabled, Amplitude := NoiseAmplitudeC, OmegaRadS := OmegaRadS);
LoadKw := LoadSource.Value;
NoiseC := NoiseSource.Value;
NoiseHeaterLoadKw := LREAL#0.0;
MeasuredTemperatureC := NoiseTemperatureC + NoiseC;

LoadLoop.Configure(Parameters := Parameters);
NoiseLoop.Configure(Parameters := Parameters);
IF NOT Initialized THEN
    State.Setpoint := SetpointC;
    State.ProcessValue := SetpointC;
    State.NominalOutput := LREAL#75.0;
    State.SaturatedOutput := LREAL#75.0;
    LoadLoop.Initialize(State := State);
    NoiseLoop.Initialize(State := State);
    Initialized := TRUE;
END_IF;
LoadInput.Mode := PIDMode#Auto;
LoadInput.CycleTime := LREAL#0.1;
LoadInput.Setpoint := SetpointC;
LoadInput.ProcessValue := LoadTemperatureC;
LoadCommandPct := LoadLoop.Update(Input := LoadInput);
NoiseInput.Mode := PIDMode#Auto;
NoiseInput.CycleTime := LREAL#0.1;
NoiseInput.Setpoint := SetpointC;
NoiseInput.ProcessValue := MeasuredTemperatureC;
NoiseCommandPct := NoiseLoop.Update(Input := NoiseInput);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.LoadCommandPct AT %QL0 : LREAL;
    P1.LoadKw AT %QL8 : LREAL;
    P1.NoiseCommandPct AT %QL16 : LREAL;
    P1.NoiseHeaterLoadKw AT %QL24 : LREAL;
    P1.LoadTemperatureC AT %IL0 : LREAL;
    P1.NoiseTemperatureC AT %IL8 : LREAL;
    P1.NoiseC AT %ML0 : LREAL;
    P1.MeasuredTemperatureC AT %ML8 : LREAL;
    P1.ExperimentEnabled AT %MX24.0 : BOOL;
    P1.SetpointC AT %ML16 : LREAL;
    P1.SensitivityAtOmega AT %ML32 : LREAL;
    P1.ComplementaryAtOmega AT %ML40 : LREAL;
END_VAR
END_CONFIGURATION
