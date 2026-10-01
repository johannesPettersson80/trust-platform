(* State feedback around the operating point: q = 0.010 + v with v = N·(r − 0.25) − K1·x1 − K2·x2, where x = h − 0.25 are
   the level deviations in m, K1 and K2 are in m²/s (m³/s per m) and r is tank 2's setpoint. *)
FUNCTION FeedbackFlowM3PerS : LREAL
VAR_INPUT
    Tank1M : LREAL;
    Tank2M : LREAL;
    SetpointM : LREAL;
    K1 : LREAL;
    K2 : LREAL;
    N : LREAL;
END_VAR
FeedbackFlowM3PerS := LREAL#0.010 + N * (SetpointM - LREAL#0.25) - K1 * (Tank1M - LREAL#0.25) - K2 * (Tank2M - LREAL#0.25);
END_FUNCTION

(* The pump delivers 0–0.020 m³/s. *)
FUNCTION LimitedFlowM3PerS : LREAL
VAR_INPUT
    RequestedM3PerS : LREAL;
END_VAR
LimitedFlowM3PerS := MIN(MAX(RequestedM3PerS, LREAL#0.0), LREAL#0.020);
END_FUNCTION

(* A setpoint outside the 0.05–0.45 m levels, such as the unwritten 0 of a memory-bound value, means the operating
   level. *)
FUNCTION ValidSetpointM : LREAL
VAR_INPUT
    SetpointM : LREAL;
END_VAR
IF SetpointM >= LREAL#0.05 AND SetpointM <= LREAL#0.45 THEN
    ValidSetpointM := SetpointM;
ELSE
    ValidSetpointM := LREAL#0.25;
END_IF;
END_FUNCTION

(* The level cost of one update, xᵀQx with Q = diag(400, 400) m⁻²: a deviation of 0.05 m costs 1. *)
FUNCTION LevelCost : LREAL
VAR_INPUT
    Tank1M : LREAL;
    Tank2M : LREAL;
END_VAR
LevelCost := LREAL#400.0 * ((Tank1M - LREAL#0.25) * (Tank1M - LREAL#0.25) + (Tank2M - LREAL#0.25) * (Tank2M - LREAL#0.25));
END_FUNCTION

(* The effort cost of one update, 40000·v² s²/m⁶ for the applied pump deviation v = q − 0.010: 0.005 m³/s costs 1. *)
FUNCTION EffortCost : LREAL
VAR_INPUT
    FlowM3PerS : LREAL;
END_VAR
EffortCost := LREAL#40000.0 * (FlowM3PerS - LREAL#0.010) * (FlowM3PerS - LREAL#0.010);
END_FUNCTION

PROGRAM Main
VAR
    (* K1, K2 and N are written by Control Studio's Apply. FeedbackEnabled is the Auto/Manual mode: in Manual the pump
       delivers ManualFlowM3PerS, the operating flow 0.010 m³/s unless Control Studio writes another. The project
       experiment returns tank 2's setpoint to 0.25 m, which Apply's live check leaves at another level, and holds it for
       60 s. Then the pump runs one second at 0.020 m³/s, a slug, and the feedback recovers the levels. From 61 s to 120 s
       the program adds each update's level and effort costs, with the same scales for every design, and records the
       largest level deviation from 110 s to 120 s: 50–60 s after the slug. *)
    K1 : LREAL := 0.0;
    K2 : LREAL := 0.0;
    N : LREAL := 0.0;
    ManualFlowM3PerS : LREAL := 0.010;
    PumpM3PerS : LREAL := 0.010;
    Tank1M : LREAL := 0.25;
    Tank2M : LREAL := 0.25;
    Tank2SetpointM : LREAL := 0.25;
    RequestedFlowM3PerS : LREAL := 0.010;
    LevelCostSum : LREAL := 0.0;
    EffortCostSum : LREAL := 0.0;
    LatePeakM : LREAL := 0.0;
    ElapsedS : LREAL := 0.0;
    ExperimentEnabled : BOOL := FALSE;
    WasEnabled : BOOL := FALSE;
    FeedbackEnabled : BOOL := FALSE;
    Clipped : BOOL := FALSE;
    FaultLatched : BOOL := FALSE;
    ClockPrimed : BOOL := FALSE;
    ClockValid : BOOL := TRUE;
    Slug : BOOL := FALSE;
    NowTime : TIME;
    PreviousTime : TIME;
END_VAR
(* A wrong update interval or a level outside 0.05–0.45 m latches zero flow. *)
NowTime := TIME();
ClockValid := NOT ClockPrimed OR ((NowTime - PreviousTime) >= T#950ms AND (NowTime - PreviousTime) <= T#1050ms);
IF NOT ClockValid OR Tank1M < LREAL#0.05 OR Tank1M > LREAL#0.45 OR Tank2M < LREAL#0.05 OR Tank2M > LREAL#0.45 THEN
    FaultLatched := TRUE;
END_IF;
Tank2SetpointM := ValidSetpointM(SetpointM := Tank2SetpointM);
IF ExperimentEnabled AND NOT WasEnabled THEN
    ElapsedS := LREAL#0.0;
    Tank2SetpointM := LREAL#0.25;
    LevelCostSum := LREAL#0.0;
    EffortCostSum := LREAL#0.0;
    LatePeakM := LREAL#0.0;
ELSIF ExperimentEnabled THEN
    ElapsedS := ElapsedS + LREAL#1.0;
END_IF;
Slug := ExperimentEnabled AND ElapsedS >= LREAL#59.5 AND ElapsedS < LREAL#60.5;
IF Slug THEN
    RequestedFlowM3PerS := LREAL#0.020;
ELSIF FeedbackEnabled THEN
    RequestedFlowM3PerS := FeedbackFlowM3PerS(Tank1M := Tank1M, Tank2M := Tank2M, SetpointM := Tank2SetpointM,
        K1 := K1, K2 := K2, N := N);
ELSE
    RequestedFlowM3PerS := ManualFlowM3PerS;
END_IF;
PumpM3PerS := LimitedFlowM3PerS(RequestedM3PerS := RequestedFlowM3PerS);
Clipped := PumpM3PerS <> RequestedFlowM3PerS;
IF FaultLatched THEN
    PumpM3PerS := LREAL#0.0;
END_IF;
(* The costs charge the measured levels and the applied flow, clipped or not. *)
IF ExperimentEnabled AND ElapsedS >= LREAL#60.5 AND ElapsedS < LREAL#120.5 THEN
    LevelCostSum := LevelCostSum + LevelCost(Tank1M := Tank1M, Tank2M := Tank2M);
    EffortCostSum := EffortCostSum + EffortCost(FlowM3PerS := PumpM3PerS);
    IF ElapsedS >= LREAL#109.5 THEN
        LatePeakM := MAX(LatePeakM, MAX(ABS(Tank1M - LREAL#0.25), ABS(Tank2M - LREAL#0.25)));
    END_IF;
END_IF;
WasEnabled := ExperimentEnabled;
PreviousTime := NowTime;
ClockPrimed := TRUE;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#1s, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.PumpM3PerS AT %QL0 : LREAL;
    P1.Tank1M AT %IL0 : LREAL;
    P1.Tank2M AT %IL8 : LREAL;
    P1.Tank2SetpointM AT %ML0 : LREAL;
    P1.RequestedFlowM3PerS AT %ML8 : LREAL;
    P1.LevelCostSum AT %ML16 : LREAL;
    P1.EffortCostSum AT %ML24 : LREAL;
    P1.LatePeakM AT %ML32 : LREAL;
    P1.ExperimentEnabled AT %MX40.0 : BOOL;
    P1.Clipped AT %MX40.1 : BOOL;
    P1.FaultLatched AT %MX40.2 : BOOL;
    P1.ClockValid AT %MX40.3 : BOOL;
END_VAR
END_CONFIGURATION
