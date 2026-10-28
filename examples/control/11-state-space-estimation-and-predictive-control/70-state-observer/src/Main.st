(* Observer gains from the observer poles: det(sI − A + LC) = s² + (0.08 + l2)s + 0.0016 + 0.04(l1 + l2) must equal
   (s + Pole1)(s + Pole2), Pole in s⁻¹. *)
FUNCTION ObserverGain2 : LREAL
VAR_INPUT
    Pole1 : LREAL;
    Pole2 : LREAL;
END_VAR
ObserverGain2 := Pole1 + Pole2 - LREAL#0.08;
END_FUNCTION

FUNCTION ObserverGain1 : LREAL
VAR_INPUT
    Pole1 : LREAL;
    Pole2 : LREAL;
END_VAR
ObserverGain1 := (Pole1 * Pole2 - LREAL#0.0016) / LREAL#0.04 - ObserverGain2(Pole1 := Pole1, Pole2 := Pole2);
END_FUNCTION

(* A state observer of the cascade, x̂' = Ax̂ + Bv + L(y − Cx̂), advanced by one second per call from the old estimate. It
   reads tank 2 and the applied flow deviation only. Start takes tank 2 from its reading and guesses tank 1 at rest, so
   the first innovation is zero. The estimates are 0.25 m plus the estimated deviations. *)
FUNCTION_BLOCK TankObserver
VAR_INPUT
    ReadingM : LREAL;
    FlowDeviationM3PerS : LREAL;
    L1 : LREAL;
    L2 : LREAL;
END_VAR
VAR_OUTPUT
    Tank1M : LREAL := 0.25;
    Tank2M : LREAL := 0.25;
    InnovationM : LREAL := 0.0;
END_VAR
VAR
    Estimate : ARRAY[1..2] OF LREAL;
    Rate1 : LREAL;
    Rate2 : LREAL;
END_VAR
METHOD PUBLIC Start
VAR_INPUT
    ReadingM : LREAL;
END_VAR
Estimate[1] := LREAL#0.0;
Estimate[2] := ReadingM - LREAL#0.25;
InnovationM := LREAL#0.0;
Tank1M := LREAL#0.25 + Estimate[1];
Tank2M := LREAL#0.25 + Estimate[2];
END_METHOD
InnovationM := (ReadingM - LREAL#0.25) - Estimate[2];
Rate1 := -LREAL#0.04 * Estimate[1] + LREAL#2.0 * FlowDeviationM3PerS + L1 * InnovationM;
Rate2 := LREAL#0.04 * Estimate[1] - LREAL#0.04 * Estimate[2] + L2 * InnovationM;
Estimate[1] := Estimate[1] + Rate1;
Estimate[2] := Estimate[2] + Rate2;
Tank1M := LREAL#0.25 + Estimate[1];
Tank2M := LREAL#0.25 + Estimate[2];
END_FUNCTION_BLOCK

(* Root-mean-square and peak of the errors so far, as in Lesson 76. *)
FUNCTION_BLOCK ErrorStatistics
VAR_INPUT
    ErrorM : LREAL;
END_VAR
VAR_OUTPUT
    RmsM : LREAL := 0.0;
    PeakM : LREAL := 0.0;
END_VAR
VAR
    Count : LREAL := 0.0;
    SumOfSquares : LREAL := 0.0;
END_VAR
Count := Count + LREAL#1.0;
SumOfSquares := SumOfSquares + ErrorM * ErrorM;
RmsM := SQRT(SumOfSquares / Count);
PeakM := MAX(PeakM, ABS(ErrorM));
END_FUNCTION_BLOCK

(* Reading noise of the noisy observers: −5 mm and +5 mm on alternate readings, as in Lesson 76. *)
FUNCTION ReadingNoiseM : LREAL
VAR_INPUT
    Reading : DINT;
END_VAR
IF (Reading MOD DINT#2) = DINT#0 THEN
    ReadingNoiseM := LREAL#-0.005;
ELSE
    ReadingNoiseM := LREAL#0.005;
END_IF;
END_FUNCTION

PROGRAM Main
VAR
    (* One cascade and four observers of its tank 1: exact readings with L for the poles −0.16 and −0.20 s⁻¹ and with the
       stronger L for −0.32 and −0.40 s⁻¹, and the same two with readings 5 mm off in alternate directions. The project
       experiment pours a slug into tank 1, 0.020 m³/s for one second, and starts the observers one update later. Tank 1
       is read only to score the estimates, from 30 s after the observers start. *)
    Clean : TankObserver;
    CleanFast : TankObserver;
    Noisy : TankObserver;
    NoisyFast : TankObserver;
    CleanErrors : ErrorStatistics;
    CleanFastErrors : ErrorStatistics;
    NoisyErrors : ErrorStatistics;
    NoisyFastErrors : ErrorStatistics;
    PumpM3PerS : LREAL := 0.010;
    Tank1M : LREAL := 0.25;
    Tank2M : LREAL := 0.25;
    NoisyReadingM : LREAL := 0.25;
    CleanEstimate1M : LREAL := 0.25;
    CleanFastEstimate1M : LREAL := 0.25;
    NoisyEstimate1M : LREAL := 0.25;
    NoisyFastEstimate1M : LREAL := 0.25;
    CleanRmsM : LREAL := 0.0;
    CleanFastRmsM : LREAL := 0.0;
    NoisyRmsM : LREAL := 0.0;
    NoisyFastRmsM : LREAL := 0.0;
    CleanEstimate2M : LREAL := 0.25;
    CleanInnovationM : LREAL := 0.0;
    L1 : LREAL := 0.0;
    L2 : LREAL := 0.0;
    FastL1 : LREAL := 0.0;
    FastL2 : LREAL := 0.0;
    ElapsedS : LREAL := 0.0;
    Reading : DINT := 0;
    ExperimentEnabled : BOOL := FALSE;
    WasEnabled : BOOL := FALSE;
    ClockPrimed : BOOL := FALSE;
    ClockValid : BOOL := TRUE;
    FaultLatched : BOOL := FALSE;
    NowTime : TIME;
    PreviousTime : TIME;
END_VAR
(* The observers advance one second per update, so an interval outside 1 s ± 50 ms, or a level outside 0.05–0.45 m,
   latches zero flow and stops them. *)
NowTime := TIME();
ClockValid := NOT ClockPrimed OR ((NowTime - PreviousTime) >= T#950ms AND (NowTime - PreviousTime) <= T#1050ms);
IF NOT ClockValid OR Tank1M < LREAL#0.05 OR Tank1M > LREAL#0.45 OR Tank2M < LREAL#0.05 OR Tank2M > LREAL#0.45 THEN
    FaultLatched := TRUE;
END_IF;
L1 := ObserverGain1(Pole1 := LREAL#0.16, Pole2 := LREAL#0.20);
L2 := ObserverGain2(Pole1 := LREAL#0.16, Pole2 := LREAL#0.20);
FastL1 := ObserverGain1(Pole1 := LREAL#0.32, Pole2 := LREAL#0.40);
FastL2 := ObserverGain2(Pole1 := LREAL#0.32, Pole2 := LREAL#0.40);
NoisyReadingM := Tank2M + ReadingNoiseM(Reading := Reading);
Reading := Reading + DINT#1;
IF ExperimentEnabled AND NOT WasEnabled THEN
    ElapsedS := LREAL#0.0;
ELSIF ExperimentEnabled AND NOT FaultLatched THEN
    ElapsedS := ElapsedS + LREAL#1.0;
    IF ElapsedS < LREAL#1.5 THEN
        Clean.Start(ReadingM := Tank2M);
        CleanFast.Start(ReadingM := Tank2M);
        Noisy.Start(ReadingM := NoisyReadingM);
        NoisyFast.Start(ReadingM := NoisyReadingM);
    ELSE
        (* The applied flow deviation of the second that has passed: the pump runs 0.010 m³/s after the slug. *)
        Clean(ReadingM := Tank2M, FlowDeviationM3PerS := PumpM3PerS - LREAL#0.010, L1 := L1, L2 := L2);
        CleanFast(ReadingM := Tank2M, FlowDeviationM3PerS := PumpM3PerS - LREAL#0.010, L1 := FastL1, L2 := FastL2);
        Noisy(ReadingM := NoisyReadingM, FlowDeviationM3PerS := PumpM3PerS - LREAL#0.010, L1 := L1, L2 := L2);
        NoisyFast(ReadingM := NoisyReadingM, FlowDeviationM3PerS := PumpM3PerS - LREAL#0.010, L1 := FastL1, L2 := FastL2);
    END_IF;
    IF ElapsedS >= LREAL#30.5 THEN
        CleanErrors(ErrorM := Clean.Tank1M - Tank1M);
        CleanFastErrors(ErrorM := CleanFast.Tank1M - Tank1M);
        NoisyErrors(ErrorM := Noisy.Tank1M - Tank1M);
        NoisyFastErrors(ErrorM := NoisyFast.Tank1M - Tank1M);
    END_IF;
END_IF;
PumpM3PerS := LREAL#0.010;
IF ExperimentEnabled AND ElapsedS < LREAL#0.5 THEN
    PumpM3PerS := LREAL#0.020;
END_IF;
IF FaultLatched THEN
    PumpM3PerS := LREAL#0.0;
END_IF;
CleanEstimate1M := Clean.Tank1M;
CleanFastEstimate1M := CleanFast.Tank1M;
NoisyEstimate1M := Noisy.Tank1M;
NoisyFastEstimate1M := NoisyFast.Tank1M;
CleanEstimate2M := Clean.Tank2M;
CleanInnovationM := Clean.InnovationM;
CleanRmsM := CleanErrors.RmsM;
CleanFastRmsM := CleanFastErrors.RmsM;
NoisyRmsM := NoisyErrors.RmsM;
NoisyFastRmsM := NoisyFastErrors.RmsM;
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
    P1.NoisyReadingM AT %ML0 : LREAL;
    P1.CleanEstimate1M AT %ML8 : LREAL;
    P1.CleanFastEstimate1M AT %ML16 : LREAL;
    P1.NoisyEstimate1M AT %ML24 : LREAL;
    P1.NoisyFastEstimate1M AT %ML32 : LREAL;
    P1.CleanRmsM AT %ML40 : LREAL;
    P1.CleanFastRmsM AT %ML48 : LREAL;
    P1.NoisyRmsM AT %ML56 : LREAL;
    P1.NoisyFastRmsM AT %ML64 : LREAL;
    P1.CleanEstimate2M AT %ML72 : LREAL;
    P1.CleanInnovationM AT %ML80 : LREAL;
    P1.ExperimentEnabled AT %MX88.0 : BOOL;
    P1.ClockValid AT %MX88.1 : BOOL;
    P1.FaultLatched AT %MX88.2 : BOOL;
END_VAR
END_CONFIGURATION
