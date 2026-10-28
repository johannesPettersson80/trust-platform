(* State feedback around the operating point: q = 0.010 + v with v = N·(r − 0.25) − K1·x1 − K2·x2, where x are the level
   deviations in m, measured or estimated, K1 and K2 are in m²/s and r is tank 2's setpoint. *)
FUNCTION FeedbackFlowM3PerS : LREAL
VAR_INPUT
    Deviation1M : LREAL;
    Deviation2M : LREAL;
    SetpointM : LREAL;
    K1 : LREAL;
    K2 : LREAL;
    N : LREAL;
END_VAR
FeedbackFlowM3PerS := LREAL#0.010 + N * (SetpointM - LREAL#0.25) - K1 * Deviation1M - K2 * Deviation2M;
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

(* Lesson 80's costs of one update: 400(x1² + x2²) for the levels and 40000(q − 0.010)² for the applied flow. *)
FUNCTION LevelCost : LREAL
VAR_INPUT
    Tank1M : LREAL;
    Tank2M : LREAL;
END_VAR
LevelCost := LREAL#400.0 * ((Tank1M - LREAL#0.25) * (Tank1M - LREAL#0.25) + (Tank2M - LREAL#0.25) * (Tank2M - LREAL#0.25));
END_FUNCTION

FUNCTION EffortCost : LREAL
VAR_INPUT
    FlowM3PerS : LREAL;
END_VAR
EffortCost := LREAL#40000.0 * (FlowM3PerS - LREAL#0.010) * (FlowM3PerS - LREAL#0.010);
END_FUNCTION

(* The one-second prediction of tank 2's deviation, 0.04·x1 + 0.96·x2, and the innovation of a reading against it. *)
FUNCTION Innovation : LREAL
VAR_INPUT
    ReadingM : LREAL;
    X1Hat : LREAL;
    X2Hat : LREAL;
END_VAR
Innovation := (ReadingM - LREAL#0.25) - (LREAL#0.04 * X1Hat + LREAL#0.96 * X2Hat);
END_FUNCTION

(* The corrected estimate of tank 1's deviation: the prediction 0.96·x1 + 2·v from the old estimate and the previous command,
   plus L1 times the innovation. *)
FUNCTION CorrectedTank1 : LREAL
VAR_INPUT
    X1Hat : LREAL;
    CommandDeviationM3PerS : LREAL;
    InnovationM : LREAL;
    L1 : LREAL;
END_VAR
CorrectedTank1 := LREAL#0.96 * X1Hat + LREAL#2.0 * CommandDeviationM3PerS + L1 * InnovationM;
END_FUNCTION

(* The corrected estimate of tank 2's deviation: the prediction 0.04·x1 + 0.96·x2 plus L2 times the innovation. *)
FUNCTION CorrectedTank2 : LREAL
VAR_INPUT
    X1Hat : LREAL;
    X2Hat : LREAL;
    InnovationM : LREAL;
    L2 : LREAL;
END_VAR
CorrectedTank2 := LREAL#0.04 * X1Hat + LREAL#0.96 * X2Hat + L2 * InnovationM;
END_FUNCTION

(* Lesson 79's reading noise with a 5 mm standard deviation; Restart begins the sequence again. *)
FUNCTION_BLOCK ReadingNoise
VAR_INPUT
    Restart : BOOL;
END_VAR
VAR_OUTPUT
    NoiseM : LREAL := 0.0;
END_VAR
VAR
    State : DINT := 92429;
END_VAR
IF Restart THEN
    State := DINT#92429;
END_IF;
State := (DINT#8121 * State + DINT#28411) MOD DINT#134456;
NoiseM := (LREAL#2.0 * DINT_TO_LREAL(State) / LREAL#134456.0 - LREAL#1.0) * SQRT(LREAL#3.0) * LREAL#0.005;
END_FUNCTION_BLOCK

(* Root-mean-square of the errors so far. *)
FUNCTION_BLOCK ErrorStatistics
VAR_INPUT
    ErrorM : LREAL;
END_VAR
VAR_OUTPUT
    RmsM : LREAL := 0.0;
END_VAR
VAR
    Count : LREAL := 0.0;
    SumOfSquares : LREAL := 0.0;
END_VAR
Count := Count + LREAL#1.0;
SumOfSquares := SumOfSquares + ErrorM * ErrorM;
RmsM := SQRT(SumOfSquares / Count);
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    (* Two cascades with the same gains, written by Control Studio's Apply: A measures both levels, the full-state
       reference; B measures only tank 2 and feeds the gains with its steady Kalman estimate X1Hat, X2Hat (L1, L2). The
       project experiment adds Lesson 79's reading noise, restarted, and both cascades read tank 2 with the same sample.
       B's estimator predicts from its old estimate and the command it gave last update, corrects with the new reading,
       and only then is the command calculated; it runs in Manual as well. FeedbackEnabled is the Auto/Manual mode. The
       experiment returns tank 2's setpoint to 0.25 m for 60 s, then both pumps deliver a slug, 0.020 m³/s for one
       second: water from outside, which B's estimator is not told. From 61 s to 120 s each cascade adds Lesson 80's
       costs and records its largest level deviation from 110 s, and B scores its tank-1 estimate against the true
       level, which it never uses. *)
    K1 : LREAL := 0.0;
    K2 : LREAL := 0.0;
    N : LREAL := 0.0;
    L1 : LREAL := 0.0;
    L2 : LREAL := 0.0;
    X1Hat : LREAL := 0.0;
    X2Hat : LREAL := 0.0;
    ManualFlowM3PerS : LREAL := 0.010;
    FullStatePumpM3PerS : LREAL := 0.010;
    EstimatedPumpM3PerS : LREAL := 0.010;
    FullStateTank1M : LREAL := 0.25;
    FullStateTank2M : LREAL := 0.25;
    EstimatedTank1M : LREAL := 0.25;
    EstimatedTank2M : LREAL := 0.25;
    Tank2SetpointM : LREAL := 0.25;
    ReadingM : LREAL := 0.25;
    CommandM3PerS : LREAL := 0.010;
    EstimateTank1M : LREAL := 0.25;
    EstimateTank2M : LREAL := 0.25;
    FullStateLevelCost : LREAL := 0.0;
    FullStateEffortCost : LREAL := 0.0;
    EstimatedLevelCost : LREAL := 0.0;
    EstimatedEffortCost : LREAL := 0.0;
    FullStateLatePeakM : LREAL := 0.0;
    EstimatedLatePeakM : LREAL := 0.0;
    EstimationRmsM : LREAL := 0.0;
    FullStateRequestedM3PerS : LREAL := 0.010;
    EstimatedRequestedM3PerS : LREAL := 0.010;
    InnovationM : LREAL := 0.0;
    ReadingNoiseM : LREAL := 0.0;
    (* The command B gave last update, unbound so that it holds the operating flow from the first update. *)
    LastCommandM3PerS : LREAL := 0.010;
    PreviousX1Hat : LREAL := 0.0;
    ElapsedS : LREAL := 0.0;
    Noise : ReadingNoise;
    EstimationErrors : ErrorStatistics;
    ExperimentEnabled : BOOL := FALSE;
    WasEnabled : BOOL := FALSE;
    FeedbackEnabled : BOOL := FALSE;
    FullStateClipped : BOOL := FALSE;
    EstimatedClipped : BOOL := FALSE;
    FullStateFault : BOOL := FALSE;
    EstimatedFault : BOOL := FALSE;
    ClockPrimed : BOOL := FALSE;
    ClockValid : BOOL := TRUE;
    Slug : BOOL := FALSE;
    NowTime : TIME;
    PreviousTime : TIME;
END_VAR
Noise(Restart := ExperimentEnabled AND NOT WasEnabled);
ReadingNoiseM := LREAL#0.0;
IF ExperimentEnabled THEN
    ReadingNoiseM := Noise.NoiseM;
END_IF;
ReadingM := EstimatedTank2M + ReadingNoiseM;
(* A wrong update interval latches both cascades; A latches on its levels, B on its reading. *)
NowTime := TIME();
ClockValid := NOT ClockPrimed OR ((NowTime - PreviousTime) >= T#950ms AND (NowTime - PreviousTime) <= T#1050ms);
IF NOT ClockValid OR FullStateTank1M < LREAL#0.05 OR FullStateTank1M > LREAL#0.45
    OR FullStateTank2M < LREAL#0.05 OR FullStateTank2M > LREAL#0.45 THEN
    FullStateFault := TRUE;
END_IF;
IF NOT ClockValid OR ReadingM < LREAL#0.05 OR ReadingM > LREAL#0.45 THEN
    EstimatedFault := TRUE;
END_IF;
Tank2SetpointM := ValidSetpointM(SetpointM := Tank2SetpointM);
IF ExperimentEnabled AND NOT WasEnabled THEN
    ElapsedS := LREAL#0.0;
    Tank2SetpointM := LREAL#0.25;
    FullStateLevelCost := LREAL#0.0;
    FullStateEffortCost := LREAL#0.0;
    EstimatedLevelCost := LREAL#0.0;
    EstimatedEffortCost := LREAL#0.0;
    FullStateLatePeakM := LREAL#0.0;
    EstimatedLatePeakM := LREAL#0.0;
ELSIF ExperimentEnabled THEN
    ElapsedS := ElapsedS + LREAL#1.0;
END_IF;
(* B's estimator: predict from the old estimate and the previous command, correct with the new reading. *)
IF NOT EstimatedFault THEN
    InnovationM := Innovation(ReadingM := ReadingM, X1Hat := X1Hat, X2Hat := X2Hat);
    PreviousX1Hat := X1Hat;
    X1Hat := CorrectedTank1(X1Hat := X1Hat, CommandDeviationM3PerS := LastCommandM3PerS - LREAL#0.010, InnovationM := InnovationM, L1 := L1);
    X2Hat := CorrectedTank2(X1Hat := PreviousX1Hat, X2Hat := X2Hat, InnovationM := InnovationM, L2 := L2);
END_IF;
EstimateTank1M := LREAL#0.25 + X1Hat;
EstimateTank2M := LREAL#0.25 + X2Hat;
IF FeedbackEnabled THEN
    FullStateRequestedM3PerS := FeedbackFlowM3PerS(Deviation1M := FullStateTank1M - LREAL#0.25,
        Deviation2M := FullStateTank2M + ReadingNoiseM - LREAL#0.25, SetpointM := Tank2SetpointM, K1 := K1, K2 := K2, N := N);
    EstimatedRequestedM3PerS := FeedbackFlowM3PerS(Deviation1M := X1Hat, Deviation2M := X2Hat, SetpointM := Tank2SetpointM,
        K1 := K1, K2 := K2, N := N);
ELSE
    FullStateRequestedM3PerS := ManualFlowM3PerS;
    EstimatedRequestedM3PerS := ManualFlowM3PerS;
END_IF;
FullStatePumpM3PerS := LimitedFlowM3PerS(RequestedM3PerS := FullStateRequestedM3PerS);
FullStateClipped := FullStatePumpM3PerS <> FullStateRequestedM3PerS;
IF NOT EstimatedFault THEN
    CommandM3PerS := LimitedFlowM3PerS(RequestedM3PerS := EstimatedRequestedM3PerS);
    LastCommandM3PerS := CommandM3PerS;
END_IF;
EstimatedClipped := CommandM3PerS <> EstimatedRequestedM3PerS;
EstimatedPumpM3PerS := CommandM3PerS;
(* The slug is water from outside: the pumps deliver it, B's estimator keeps the controller's command. *)
Slug := ExperimentEnabled AND ElapsedS >= LREAL#59.5 AND ElapsedS < LREAL#60.5;
IF Slug THEN
    FullStatePumpM3PerS := LREAL#0.020;
    EstimatedPumpM3PerS := LREAL#0.020;
END_IF;
IF FullStateFault THEN
    FullStatePumpM3PerS := LREAL#0.0;
END_IF;
IF EstimatedFault THEN
    EstimatedPumpM3PerS := LREAL#0.0;
END_IF;
IF ExperimentEnabled AND ElapsedS >= LREAL#60.5 AND ElapsedS < LREAL#120.5 THEN
    FullStateLevelCost := FullStateLevelCost + LevelCost(Tank1M := FullStateTank1M, Tank2M := FullStateTank2M);
    FullStateEffortCost := FullStateEffortCost + EffortCost(FlowM3PerS := FullStatePumpM3PerS);
    EstimatedLevelCost := EstimatedLevelCost + LevelCost(Tank1M := EstimatedTank1M, Tank2M := EstimatedTank2M);
    EstimatedEffortCost := EstimatedEffortCost + EffortCost(FlowM3PerS := EstimatedPumpM3PerS);
    EstimationErrors(ErrorM := EstimateTank1M - EstimatedTank1M);
    EstimationRmsM := EstimationErrors.RmsM;
    IF ElapsedS >= LREAL#109.5 THEN
        FullStateLatePeakM := MAX(FullStateLatePeakM, MAX(ABS(FullStateTank1M - LREAL#0.25), ABS(FullStateTank2M - LREAL#0.25)));
        EstimatedLatePeakM := MAX(EstimatedLatePeakM, MAX(ABS(EstimatedTank1M - LREAL#0.25), ABS(EstimatedTank2M - LREAL#0.25)));
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
    P1.FullStatePumpM3PerS AT %QL0 : LREAL;
    P1.EstimatedPumpM3PerS AT %QL8 : LREAL;
    P1.FullStateTank1M AT %IL0 : LREAL;
    P1.FullStateTank2M AT %IL8 : LREAL;
    P1.EstimatedTank1M AT %IL16 : LREAL;
    P1.EstimatedTank2M AT %IL24 : LREAL;
    P1.Tank2SetpointM AT %ML0 : LREAL;
    P1.ReadingM AT %ML8 : LREAL;
    P1.CommandM3PerS AT %ML16 : LREAL;
    P1.EstimateTank1M AT %ML24 : LREAL;
    P1.EstimateTank2M AT %ML32 : LREAL;
    P1.FullStateLevelCost AT %ML40 : LREAL;
    P1.FullStateEffortCost AT %ML48 : LREAL;
    P1.EstimatedLevelCost AT %ML56 : LREAL;
    P1.EstimatedEffortCost AT %ML64 : LREAL;
    P1.FullStateLatePeakM AT %ML72 : LREAL;
    P1.EstimatedLatePeakM AT %ML80 : LREAL;
    P1.EstimationRmsM AT %ML88 : LREAL;
    P1.ExperimentEnabled AT %MX96.0 : BOOL;
    P1.FullStateClipped AT %MX96.1 : BOOL;
    P1.EstimatedClipped AT %MX96.2 : BOOL;
    P1.FullStateFault AT %MX96.3 : BOOL;
    P1.EstimatedFault AT %MX96.4 : BOOL;
    P1.ClockValid AT %MX96.5 : BOOL;
END_VAR
END_CONFIGURATION
