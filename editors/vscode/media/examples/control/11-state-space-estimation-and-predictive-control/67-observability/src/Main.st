FUNCTION UpstreamFromDownstreamHistory : LREAL
VAR_INPUT
    MeasuredDeviationM : LREAL;
    SlopeMPerS : LREAL;
END_VAR
UpstreamFromDownstreamHistory := MeasuredDeviationM + LREAL#25.0 * SlopeMPerS;
END_FUNCTION

PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ClockValid : BOOL := FALSE;
    ClockPrimed : BOOL := FALSE;
    WasEnabled : BOOL := FALSE;
    InitialConditionValid : BOOL := FALSE;
    FaultLatched : BOOL := FALSE;
    EstimateReady : BOOL := FALSE;
    PreviousSampleValid : BOOL := FALSE;
    NowTime : TIME;
    PreviousTime : TIME;
    StartedAt : TIME;
    SampleIndex : DINT := 0;
    SampleCount : LREAL := 0.0;
    ExperimentElapsedS : LREAL := 0.0;
    NoiseAmplitudeM : LREAL := 0.0;
    NoiseValueM : LREAL := 0.0;
    PumpFlow : LREAL := 0.010;
    Tank1Truth : LREAL := 0.27;
    Tank2Truth : LREAL := 0.25;
    MeasuredTank2LevelM : LREAL := 0.25;
    MeasuredDeviationM : LREAL := 0.0;
    PreviousDeviationM : LREAL := 0.0;
    SlopeMPerS : LREAL := 0.0;
    UpstreamEstimateM : LREAL := 0.0;
    TruthDeviationM : LREAL := 0.02;
    EstimateErrorM : LREAL := 0.0;
    SumSquaredError : LREAL := 0.0;
    RmseM : LREAL := 0.0;
    PeakAbsoluteErrorM : LREAL := 0.0;
END_VAR

NowTime := TIME();
ClockValid := ClockPrimed AND ((NowTime - PreviousTime) = T#1s);
IF SafetyAbort OR NOT InputsValid OR
   NOT ((Tank1Truth >= LREAL#0.05) AND (Tank1Truth <= LREAL#0.45)) OR
   NOT ((Tank2Truth >= LREAL#0.05) AND (Tank2Truth <= LREAL#0.45)) OR
   NOT ((NoiseAmplitudeM >= LREAL#0.0) AND (NoiseAmplitudeM <= LREAL#0.005)) OR
   (ClockPrimed AND NOT ClockValid) THEN
    FaultLatched := TRUE;
END_IF;
IF WasEnabled AND NOT ExperimentEnabled AND ((NowTime - StartedAt) < T#20s) THEN
    FaultLatched := TRUE;
END_IF;
IF ExperimentEnabled AND NOT WasEnabled THEN
    StartedAt := NowTime;
    InitialConditionValid := (ABS(Tank1Truth - LREAL#0.27) <= LREAL#0.000001) AND
                             (ABS(Tank2Truth - LREAL#0.25) <= LREAL#0.000001);
    IF NOT InitialConditionValid THEN FaultLatched := TRUE; END_IF;
    ExperimentElapsedS := LREAL#0.0;
    SampleIndex := DINT#0;
    SampleCount := LREAL#0.0;
    SumSquaredError := LREAL#0.0;
    RmseM := LREAL#0.0;
    PeakAbsoluteErrorM := LREAL#0.0;
    PreviousSampleValid := FALSE;
ELSIF ExperimentEnabled AND NOT FaultLatched THEN
    ExperimentElapsedS := ExperimentElapsedS + LREAL#1.0;
    SampleIndex := SampleIndex + DINT#1;
END_IF;

(* This deterministic sensor-noise experiment never modifies physical levels. *)
IF (SampleIndex MOD DINT#2) = DINT#0 THEN
    NoiseValueM := -NoiseAmplitudeM;
ELSE
    NoiseValueM := NoiseAmplitudeM;
END_IF;
MeasuredTank2LevelM := Tank2Truth + NoiseValueM;
MeasuredDeviationM := MeasuredTank2LevelM - LREAL#0.25;
TruthDeviationM := Tank1Truth - LREAL#0.25;
EstimateReady := FALSE;
IF ExperimentEnabled AND NOT FaultLatched THEN
    IF PreviousSampleValid AND ClockValid THEN
        (* A measured one-second interval is required. The upstream truth is
           deliberately absent from the reconstruction function's inputs. *)
        SlopeMPerS := (MeasuredDeviationM - PreviousDeviationM) / LREAL#1.0;
        UpstreamEstimateM := UpstreamFromDownstreamHistory(MeasuredDeviationM := MeasuredDeviationM, SlopeMPerS := SlopeMPerS);
        EstimateReady := TRUE;
        (* Truth is used only to evaluate the already-calculated estimate. *)
        EstimateErrorM := UpstreamEstimateM - TruthDeviationM;
        SampleCount := SampleCount + LREAL#1.0;
        SumSquaredError := SumSquaredError + EstimateErrorM * EstimateErrorM;
        RmseM := SQRT(SumSquaredError / SampleCount);
        PeakAbsoluteErrorM := MAX(PeakAbsoluteErrorM, ABS(EstimateErrorM));
    END_IF;
    PreviousDeviationM := MeasuredDeviationM;
    PreviousSampleValid := TRUE;
END_IF;
IF FaultLatched THEN PumpFlow := LREAL#0.0; ELSE PumpFlow := LREAL#0.010; END_IF;
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
    P1.PumpFlow AT %QL0 : LREAL;
    P1.Tank1Truth AT %IL0 : LREAL;
    P1.Tank2Truth AT %IL8 : LREAL;
    P1.UpstreamEstimateM AT %ML0 : LREAL;
    P1.MeasuredDeviationM AT %ML8 : LREAL;
    P1.SlopeMPerS AT %ML16 : LREAL;
    P1.EstimateErrorM AT %ML24 : LREAL;
    P1.RmseM AT %ML32 : LREAL;
    P1.SampleCount AT %ML40 : LREAL;
    P1.TruthDeviationM AT %ML48 : LREAL;
    P1.MeasuredTank2LevelM AT %ML56 : LREAL;
    P1.NoiseValueM AT %ML64 : LREAL;
    P1.NoiseAmplitudeM AT %ML72 : LREAL;
    P1.ExperimentEnabled AT %MX80.0 : BOOL;
    P1.SafetyAbort AT %MX80.1 : BOOL;
    P1.ClockValid AT %MX80.3 : BOOL;
    P1.FaultLatched AT %MX80.4 : BOOL;
    P1.InitialConditionValid AT %MX80.5 : BOOL;
    P1.EstimateReady AT %MX80.6 : BOOL;
    P1.PeakAbsoluteErrorM AT %ML88 : LREAL;
    P1.ExperimentElapsedS AT %ML96 : LREAL;
END_VAR
END_CONFIGURATION
