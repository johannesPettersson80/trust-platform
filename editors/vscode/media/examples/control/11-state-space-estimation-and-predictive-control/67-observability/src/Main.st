(* Tank 1's level deviation from tank 2's: the downstream balance ẏ = 0.04x1 − 0.04y gives x1 = y + 25ẏ, 25 in s. *)
FUNCTION UpstreamFromDownstream : LREAL
VAR_INPUT
    DeviationM : LREAL;
    SlopeMPerS : LREAL;
END_VAR
UpstreamFromDownstream := DeviationM + LREAL#25.0 * SlopeMPerS;
END_FUNCTION

(* Tank 1 reconstructed from tank 2's readings alone, one reading per second: ẏ is the change since the previous
   reading. The first reading only primes that change, so Ready stays FALSE and Tank1M keeps its rest value. *)
FUNCTION_BLOCK UpstreamReconstruction
VAR_INPUT
    MeasuredTank2M : LREAL;
END_VAR
VAR_OUTPUT
    Tank1M : LREAL := 0.25;
    SlopeMPerS : LREAL := 0.0;
    Ready : BOOL := FALSE;
END_VAR
VAR
    Primed : BOOL := FALSE;
    PreviousDeviationM : LREAL := 0.0;
END_VAR
IF Primed THEN
    SlopeMPerS := (MeasuredTank2M - LREAL#0.25) - PreviousDeviationM;
    Tank1M := LREAL#0.25 + UpstreamFromDownstream(DeviationM := MeasuredTank2M - LREAL#0.25, SlopeMPerS := SlopeMPerS);
    Ready := TRUE;
END_IF;
PreviousDeviationM := MeasuredTank2M - LREAL#0.25;
Primed := TRUE;
END_FUNCTION_BLOCK

(* Root-mean-square and peak of the errors so far: RMS = √((e1² + … + eN²)/N). *)
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

(* Lesson 75's pulse pair: +0.001 m³/s in the first 20 updates, −0.001 m³/s in the next 11, then nominal flow. *)
FUNCTION SteeringFlowDeviationM3PerS : LREAL
VAR_INPUT
    ElapsedS : LREAL;
END_VAR
SteeringFlowDeviationM3PerS := LREAL#0.0;
IF ElapsedS < LREAL#19.5 THEN
    SteeringFlowDeviationM3PerS := LREAL#0.001;
ELSIF ElapsedS < LREAL#30.5 THEN
    SteeringFlowDeviationM3PerS := LREAL#-0.001;
END_IF;
END_FUNCTION

(* Reading noise of the second cascade: −5 mm and +5 mm on alternate readings. *)
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
    (* Two copies of the cascade receive the same pulse pair. Tank 1 of each is reconstructed from its tank 2 readings,
       exact on the first copy and with ±5 mm noise on the second. The tank 1 levels are read only to score the
       reconstructions. *)
    CleanReconstruction : UpstreamReconstruction;
    NoisyReconstruction : UpstreamReconstruction;
    CleanErrors : ErrorStatistics;
    NoisyErrors : ErrorStatistics;
    CleanPumpM3PerS : LREAL := 0.010;
    NoisyPumpM3PerS : LREAL := 0.010;
    CleanTank1M : LREAL := 0.25;
    CleanTank2M : LREAL := 0.25;
    NoisyTank1M : LREAL := 0.25;
    NoisyTank2M : LREAL := 0.25;
    CleanReconstructionM : LREAL := 0.25;
    NoisyReadingM : LREAL := 0.25;
    NoisyReconstructionM : LREAL := 0.25;
    CleanRmsM : LREAL := 0.0;
    CleanPeakM : LREAL := 0.0;
    NoisyRmsM : LREAL := 0.0;
    NoisyPeakM : LREAL := 0.0;
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
(* ẏ is a change per second, so an interval outside 1 s ± 50 ms, or a level outside 0.05–0.45 m, latches zero flow. *)
NowTime := TIME();
ClockValid := NOT ClockPrimed OR ((NowTime - PreviousTime) >= T#950ms AND (NowTime - PreviousTime) <= T#1050ms);
IF NOT ClockValid
    OR CleanTank1M < LREAL#0.05 OR CleanTank1M > LREAL#0.45 OR CleanTank2M < LREAL#0.05 OR CleanTank2M > LREAL#0.45
    OR NoisyTank1M < LREAL#0.05 OR NoisyTank1M > LREAL#0.45 OR NoisyTank2M < LREAL#0.05 OR NoisyTank2M > LREAL#0.45 THEN
    FaultLatched := TRUE;
END_IF;
IF ExperimentEnabled AND NOT WasEnabled THEN
    ElapsedS := LREAL#0.0;
ELSIF ExperimentEnabled AND NOT FaultLatched THEN
    ElapsedS := ElapsedS + LREAL#1.0;
END_IF;
(* The second sensor is noisy at every update. *)
NoisyReadingM := NoisyTank2M + ReadingNoiseM(Reading := Reading);
Reading := Reading + DINT#1;
IF ExperimentEnabled AND NOT FaultLatched THEN
    CleanReconstruction(MeasuredTank2M := CleanTank2M);
    NoisyReconstruction(MeasuredTank2M := NoisyReadingM);
    IF CleanReconstruction.Ready THEN
        CleanErrors(ErrorM := CleanReconstruction.Tank1M - CleanTank1M);
        NoisyErrors(ErrorM := NoisyReconstruction.Tank1M - NoisyTank1M);
    END_IF;
END_IF;
CleanPumpM3PerS := LREAL#0.010;
IF ExperimentEnabled THEN
    CleanPumpM3PerS := LREAL#0.010 + SteeringFlowDeviationM3PerS(ElapsedS := ElapsedS);
END_IF;
IF FaultLatched THEN
    CleanPumpM3PerS := LREAL#0.0;
END_IF;
NoisyPumpM3PerS := CleanPumpM3PerS;
CleanReconstructionM := CleanReconstruction.Tank1M;
NoisyReconstructionM := NoisyReconstruction.Tank1M;
CleanRmsM := CleanErrors.RmsM;
CleanPeakM := CleanErrors.PeakM;
NoisyRmsM := NoisyErrors.RmsM;
NoisyPeakM := NoisyErrors.PeakM;
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
    P1.CleanPumpM3PerS AT %QL0 : LREAL;
    P1.NoisyPumpM3PerS AT %QL8 : LREAL;
    P1.CleanTank1M AT %IL0 : LREAL;
    P1.CleanTank2M AT %IL8 : LREAL;
    P1.NoisyTank1M AT %IL16 : LREAL;
    P1.NoisyTank2M AT %IL24 : LREAL;
    P1.CleanReconstructionM AT %ML0 : LREAL;
    P1.NoisyReadingM AT %ML8 : LREAL;
    P1.NoisyReconstructionM AT %ML16 : LREAL;
    P1.CleanRmsM AT %ML24 : LREAL;
    P1.CleanPeakM AT %ML32 : LREAL;
    P1.NoisyRmsM AT %ML40 : LREAL;
    P1.NoisyPeakM AT %ML48 : LREAL;
    P1.ExperimentEnabled AT %MX56.0 : BOOL;
    P1.ClockValid AT %MX56.1 : BOOL;
    P1.FaultLatched AT %MX56.2 : BOOL;
END_VAR
END_CONFIGURATION
