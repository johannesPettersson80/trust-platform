(* The lead/lag element at the frequency ω: |F(jω)| = √(1 + (ω·Tz)²)/√(1 + (ω·Tp)²) and
   ∠F = arctan(ω·Tz) − arctan(ω·Tp). *)
FUNCTION LeadMagnitude : LREAL
VAR_INPUT
    ZeroTimeS : LREAL;
    PoleTimeS : LREAL;
    OmegaRadS : LREAL;
END_VAR
LeadMagnitude := SQRT(LREAL#1.0 + (OmegaRadS * ZeroTimeS) ** LREAL#2.0) / SQRT(LREAL#1.0 + (OmegaRadS * PoleTimeS) ** LREAL#2.0);
END_FUNCTION

FUNCTION LeadPhaseRad : LREAL
VAR_INPUT
    ZeroTimeS : LREAL;
    PoleTimeS : LREAL;
    OmegaRadS : LREAL;
END_VAR
LeadPhaseRad := ATAN(OmegaRadS * ZeroTimeS) - ATAN(OmegaRadS * PoleTimeS);
END_FUNCTION

(* PI with the lead/lag element F(s) = (1 + Tz·s)/(1 + Tp·s) on its command deviation w = P + I − 75, limited to
   0–100 %. F = a + (1 − a)/(1 + Tp·s) with a = Tz/Tp: the request uses the lag state z before it advances. The
   integral tracks the limited command through the element, I = I_trial + Δt·(u_applied − u_request)/a (Tt = 1 s),
   because a change of I moves the request a times as much. *)
FUNCTION_BLOCK ShapedPi
VAR_INPUT
    Enable : BOOL;
    SetpointC : LREAL;
    MeasuredC : LREAL;
    ZeroTimeS : LREAL;
    PoleTimeS : LREAL := 2.0;
END_VAR
VAR_OUTPUT
    RequestPct : LREAL := 75.0;
    CommandPct : LREAL := 75.0;
    IntegralPct : LREAL := 75.0;
END_VAR
VAR
    ProportionalGain : LREAL := 5.0;
    IntegralGain : LREAL := 1.0;
    UpdateS : LREAL := 0.1;
    ShapeRatio : LREAL := 1.0;
    ShapeStatePct : LREAL := 0.0;
    ErrorC : LREAL := 0.0;
    TrialIntegralPct : LREAL := 75.0;
    DeviationPct : LREAL := 0.0;
END_VAR
IF ZeroTimeS > LREAL#0.0 AND PoleTimeS > LREAL#0.0 THEN
    ShapeRatio := ZeroTimeS / PoleTimeS;
ELSE
    ShapeRatio := LREAL#1.0;
END_IF;
IF Enable THEN
    ErrorC := SetpointC - MeasuredC;
    TrialIntegralPct := IntegralPct + IntegralGain * ErrorC * UpdateS;
    DeviationPct := ProportionalGain * ErrorC + TrialIntegralPct - LREAL#75.0;
    RequestPct := LREAL#75.0 + ShapeRatio * DeviationPct + (LREAL#1.0 - ShapeRatio) * ShapeStatePct;
    ShapeStatePct := ShapeStatePct + UpdateS / PoleTimeS * (DeviationPct - ShapeStatePct);
    CommandPct := LIMIT(LREAL#0.0, RequestPct, LREAL#100.0);
    IntegralPct := TrialIntegralPct + UpdateS * (CommandPct - RequestPct) / ShapeRatio;
ELSE
    ShapeStatePct := LREAL#0.0;
    IntegralPct := LREAL#75.0;
    RequestPct := LREAL#75.0;
    CommandPct := LREAL#75.0;
END_IF;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    (* Three candidates for each case: F = 1 (Tz = Tp = 2 s), Tz = 3 s and Tz = 10 s. *)
    TargetBaselineController : ShapedPi;
    TargetLead3Controller : ShapedPi;
    TargetLead10Controller : ShapedPi;
    LoadBaselineController : ShapedPi;
    LoadLead3Controller : ShapedPi;
    LoadLead10Controller : ShapedPi;
    NoiseBaselineController : ShapedPi;
    NoiseLead3Controller : ShapedPi;
    NoiseLead10Controller : ShapedPi;
    ElapsedS : LREAL := 0.0;
    TargetBaselineC : LREAL := 35.0;
    TargetBaselinePct : LREAL := 75.0;
    TargetBaselineKw : LREAL := 0.0;
    TargetLead3C : LREAL := 35.0;
    TargetLead3Pct : LREAL := 75.0;
    TargetLead3Kw : LREAL := 0.0;
    TargetLead10C : LREAL := 35.0;
    TargetLead10Pct : LREAL := 75.0;
    TargetLead10Kw : LREAL := 0.0;
    LoadBaselineC : LREAL := 35.0;
    LoadBaselinePct : LREAL := 75.0;
    LoadBaselineKw : LREAL := 0.0;
    LoadLead3C : LREAL := 35.0;
    LoadLead3Pct : LREAL := 75.0;
    LoadLead3Kw : LREAL := 0.0;
    LoadLead10C : LREAL := 35.0;
    LoadLead10Pct : LREAL := 75.0;
    LoadLead10Kw : LREAL := 0.0;
    NoiseBaselineC : LREAL := 35.0;
    NoiseBaselinePct : LREAL := 75.0;
    NoiseBaselineKw : LREAL := 0.0;
    NoiseLead3C : LREAL := 35.0;
    NoiseLead3Pct : LREAL := 75.0;
    NoiseLead3Kw : LREAL := 0.0;
    NoiseLead10C : LREAL := 35.0;
    NoiseLead10Pct : LREAL := 75.0;
    NoiseLead10Kw : LREAL := 0.0;
    TargetSetpointC : LREAL := 35.0;
    DisturbanceKw : LREAL := 0.0;
    NoiseC : LREAL := 0.0;
    TargetLead3RequestPct : LREAL := 75.0;
    TargetLead3IntegralPct : LREAL := 75.0;
    TargetLead10RequestPct : LREAL := 75.0;
    TargetLead10IntegralPct : LREAL := 75.0;
    ExperimentEnabled : BOOL := FALSE;
END_VAR

(* Control Studio's project experiment starts all three cases: the target steps to 38 °C, a 30 kW load, and noise
   0.05·sin(t) °C in the third group's readings. *)
IF ExperimentEnabled THEN
    TargetSetpointC := LREAL#38.0;
    DisturbanceKw := LREAL#30.0;
    NoiseC := LREAL#0.05 * SIN(ElapsedS);
ELSE
    TargetSetpointC := LREAL#35.0;
    DisturbanceKw := LREAL#0.0;
    NoiseC := LREAL#0.0;
    ElapsedS := LREAL#0.0;
END_IF;
TargetBaselineController(Enable := ExperimentEnabled, SetpointC := TargetSetpointC, MeasuredC := TargetBaselineC, ZeroTimeS := LREAL#2.0);
TargetBaselinePct := TargetBaselineController.CommandPct;
TargetBaselineKw := LREAL#0.0;
TargetLead3Controller(Enable := ExperimentEnabled, SetpointC := TargetSetpointC, MeasuredC := TargetLead3C, ZeroTimeS := LREAL#3.0);
TargetLead3Pct := TargetLead3Controller.CommandPct;
TargetLead3Kw := LREAL#0.0;
TargetLead10Controller(Enable := ExperimentEnabled, SetpointC := TargetSetpointC, MeasuredC := TargetLead10C, ZeroTimeS := LREAL#10.0);
TargetLead10Pct := TargetLead10Controller.CommandPct;
TargetLead10Kw := LREAL#0.0;
LoadBaselineController(Enable := ExperimentEnabled, SetpointC := LREAL#35.0, MeasuredC := LoadBaselineC, ZeroTimeS := LREAL#2.0);
LoadBaselinePct := LoadBaselineController.CommandPct;
LoadBaselineKw := DisturbanceKw;
LoadLead3Controller(Enable := ExperimentEnabled, SetpointC := LREAL#35.0, MeasuredC := LoadLead3C, ZeroTimeS := LREAL#3.0);
LoadLead3Pct := LoadLead3Controller.CommandPct;
LoadLead3Kw := DisturbanceKw;
LoadLead10Controller(Enable := ExperimentEnabled, SetpointC := LREAL#35.0, MeasuredC := LoadLead10C, ZeroTimeS := LREAL#10.0);
LoadLead10Pct := LoadLead10Controller.CommandPct;
LoadLead10Kw := DisturbanceKw;
NoiseBaselineController(Enable := ExperimentEnabled, SetpointC := LREAL#35.0, MeasuredC := NoiseBaselineC + NoiseC, ZeroTimeS := LREAL#2.0);
NoiseBaselinePct := NoiseBaselineController.CommandPct;
NoiseBaselineKw := LREAL#0.0;
NoiseLead3Controller(Enable := ExperimentEnabled, SetpointC := LREAL#35.0, MeasuredC := NoiseLead3C + NoiseC, ZeroTimeS := LREAL#3.0);
NoiseLead3Pct := NoiseLead3Controller.CommandPct;
NoiseLead3Kw := LREAL#0.0;
NoiseLead10Controller(Enable := ExperimentEnabled, SetpointC := LREAL#35.0, MeasuredC := NoiseLead10C + NoiseC, ZeroTimeS := LREAL#10.0);
NoiseLead10Pct := NoiseLead10Controller.CommandPct;
NoiseLead10Kw := LREAL#0.0;
TargetLead3RequestPct := TargetLead3Controller.RequestPct;
TargetLead3IntegralPct := TargetLead3Controller.IntegralPct;
TargetLead10RequestPct := TargetLead10Controller.RequestPct;
TargetLead10IntegralPct := TargetLead10Controller.IntegralPct;
IF ExperimentEnabled THEN
    ElapsedS := ElapsedS + LREAL#0.1;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.TargetBaselinePct AT %QL0 : LREAL;
    P1.TargetBaselineKw AT %QL8 : LREAL;
    P1.TargetBaselineC AT %IL0 : LREAL;
    P1.TargetLead3Pct AT %QL16 : LREAL;
    P1.TargetLead3Kw AT %QL24 : LREAL;
    P1.TargetLead3C AT %IL8 : LREAL;
    P1.TargetLead10Pct AT %QL32 : LREAL;
    P1.TargetLead10Kw AT %QL40 : LREAL;
    P1.TargetLead10C AT %IL16 : LREAL;
    P1.LoadBaselinePct AT %QL48 : LREAL;
    P1.LoadBaselineKw AT %QL56 : LREAL;
    P1.LoadBaselineC AT %IL24 : LREAL;
    P1.LoadLead3Pct AT %QL64 : LREAL;
    P1.LoadLead3Kw AT %QL72 : LREAL;
    P1.LoadLead3C AT %IL32 : LREAL;
    P1.LoadLead10Pct AT %QL80 : LREAL;
    P1.LoadLead10Kw AT %QL88 : LREAL;
    P1.LoadLead10C AT %IL40 : LREAL;
    P1.NoiseBaselinePct AT %QL96 : LREAL;
    P1.NoiseBaselineKw AT %QL104 : LREAL;
    P1.NoiseBaselineC AT %IL48 : LREAL;
    P1.NoiseLead3Pct AT %QL112 : LREAL;
    P1.NoiseLead3Kw AT %QL120 : LREAL;
    P1.NoiseLead3C AT %IL56 : LREAL;
    P1.NoiseLead10Pct AT %QL128 : LREAL;
    P1.NoiseLead10Kw AT %QL136 : LREAL;
    P1.NoiseLead10C AT %IL64 : LREAL;
    P1.TargetSetpointC AT %ML0 : LREAL;
    P1.DisturbanceKw AT %ML8 : LREAL;
    P1.NoiseC AT %ML16 : LREAL;
    P1.TargetLead3RequestPct AT %ML24 : LREAL;
    P1.TargetLead3IntegralPct AT %ML32 : LREAL;
    P1.TargetLead10RequestPct AT %ML40 : LREAL;
    P1.TargetLead10IntegralPct AT %ML48 : LREAL;
    P1.ExperimentEnabled AT %MX64.0 : BOOL;
END_VAR
END_CONFIGURATION
