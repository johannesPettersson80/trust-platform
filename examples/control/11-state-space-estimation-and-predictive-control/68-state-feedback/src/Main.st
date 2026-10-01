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

PROGRAM Main
VAR
    (* K1, K2 and N are written by Control Studio's Apply. FeedbackEnabled is the Auto/Manual mode: in Manual the pump
       delivers ManualFlowM3PerS, the operating flow 0.010 m³/s unless Control Studio writes another. The project
       experiment returns tank 2's setpoint to 0.25 m, which Apply's live check leaves at another level, and holds it for
       60 s. Then both pumps run one second at 0.020 m³/s, a slug; after it the first cascade runs under the feedback,
       the second at 0.010 m³/s. *)
    K1 : LREAL := 0.0;
    K2 : LREAL := 0.0;
    N : LREAL := 0.0;
    ManualFlowM3PerS : LREAL := 0.010;
    ControlledPumpM3PerS : LREAL := 0.010;
    UncorrectedPumpM3PerS : LREAL := 0.010;
    ControlledTank1M : LREAL := 0.25;
    ControlledTank2M : LREAL := 0.25;
    UncorrectedTank1M : LREAL := 0.25;
    UncorrectedTank2M : LREAL := 0.25;
    Tank2SetpointM : LREAL := 0.25;
    RequestedFlowM3PerS : LREAL := 0.010;
    ElapsedS : LREAL := 0.0;
    ExperimentEnabled : BOOL := FALSE;
    WasEnabled : BOOL := FALSE;
    FeedbackEnabled : BOOL := FALSE;
    Clipped : BOOL := FALSE;
    ControlledFault : BOOL := FALSE;
    UncorrectedFault : BOOL := FALSE;
    ClockPrimed : BOOL := FALSE;
    ClockValid : BOOL := TRUE;
    Slug : BOOL := FALSE;
    NowTime : TIME;
    PreviousTime : TIME;
END_VAR
(* A level outside 0.05–0.45 m latches zero flow on its own cascade; a wrong update interval latches both. *)
NowTime := TIME();
ClockValid := NOT ClockPrimed OR ((NowTime - PreviousTime) >= T#950ms AND (NowTime - PreviousTime) <= T#1050ms);
IF NOT ClockValid OR ControlledTank1M < LREAL#0.05 OR ControlledTank1M > LREAL#0.45
    OR ControlledTank2M < LREAL#0.05 OR ControlledTank2M > LREAL#0.45 THEN
    ControlledFault := TRUE;
END_IF;
IF NOT ClockValid OR UncorrectedTank1M < LREAL#0.05 OR UncorrectedTank1M > LREAL#0.45
    OR UncorrectedTank2M < LREAL#0.05 OR UncorrectedTank2M > LREAL#0.45 THEN
    UncorrectedFault := TRUE;
END_IF;
Tank2SetpointM := ValidSetpointM(SetpointM := Tank2SetpointM);
IF ExperimentEnabled AND NOT WasEnabled THEN
    ElapsedS := LREAL#0.0;
    Tank2SetpointM := LREAL#0.25;
ELSIF ExperimentEnabled THEN
    ElapsedS := ElapsedS + LREAL#1.0;
END_IF;
Slug := ExperimentEnabled AND ElapsedS >= LREAL#59.5 AND ElapsedS < LREAL#60.5;
IF Slug THEN
    RequestedFlowM3PerS := LREAL#0.020;
ELSIF FeedbackEnabled THEN
    RequestedFlowM3PerS := FeedbackFlowM3PerS(Tank1M := ControlledTank1M, Tank2M := ControlledTank2M, SetpointM := Tank2SetpointM,
        K1 := K1, K2 := K2, N := N);
ELSE
    RequestedFlowM3PerS := ManualFlowM3PerS;
END_IF;
ControlledPumpM3PerS := LimitedFlowM3PerS(RequestedM3PerS := RequestedFlowM3PerS);
Clipped := ControlledPumpM3PerS <> RequestedFlowM3PerS;
IF ControlledFault THEN
    ControlledPumpM3PerS := LREAL#0.0;
END_IF;
UncorrectedPumpM3PerS := LREAL#0.010;
IF Slug THEN
    UncorrectedPumpM3PerS := LREAL#0.020;
END_IF;
IF UncorrectedFault THEN
    UncorrectedPumpM3PerS := LREAL#0.0;
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
    P1.ControlledPumpM3PerS AT %QL0 : LREAL;
    P1.UncorrectedPumpM3PerS AT %QL8 : LREAL;
    P1.ControlledTank1M AT %IL0 : LREAL;
    P1.ControlledTank2M AT %IL8 : LREAL;
    P1.UncorrectedTank1M AT %IL16 : LREAL;
    P1.UncorrectedTank2M AT %IL24 : LREAL;
    P1.Tank2SetpointM AT %ML0 : LREAL;
    P1.RequestedFlowM3PerS AT %ML8 : LREAL;
    P1.ExperimentEnabled AT %MX16.0 : BOOL;
    P1.Clipped AT %MX16.2 : BOOL;
    P1.ControlledFault AT %MX16.3 : BOOL;
    P1.UncorrectedFault AT %MX16.4 : BOOL;
    P1.ClockValid AT %MX16.5 : BOOL;
END_VAR
END_CONFIGURATION
