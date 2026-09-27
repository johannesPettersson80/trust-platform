PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    WasEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    StartRejected : BOOL := FALSE;
    Active : BOOL := FALSE;
    MeasurementQuality : UINT := UINT#1;
    MeasurementValid : BOOL := TRUE;
    TemperatureC : LREAL := 35.0;
    AppliedPowerPct : LREAL := 75.0;
    SelectedStepPoints : REAL := 10.0;
    HoldSeconds : REAL := 30.0;
    ProposedCommandPct : REAL := 85.0;
    PredictedFinalC : REAL := 37.0;
    BaselineQualified : BOOL := FALSE;
    BaselineRangeC : REAL := 0.0;
    BaselineSlopeCPerS : REAL := 0.0;
    BaselineValues : ARRAY[0..50] OF LREAL;
    SampleCount : INT := 0;
    SampleIndex : INT := 0;
    ReadIndex : INT := 0;
    MinimumC : LREAL := 35.0;
    MaximumC : LREAL := 35.0;
    OldestC : LREAL := 35.0;
    StepScan : DINT := DINT#0;
END_VAR
MeasurementValid := MeasurementQuality = UINT#1;
IF NOT MeasurementValid OR TemperatureC <> TemperatureC
    OR TemperatureC < LREAL#33.0 OR TemperatureC > LREAL#39.0 THEN
    SafetyAbort := TRUE;
END_IF;
(* PROVIDED baseline recorder:51 samples span5s at the configured100ms task.
   Control Studio still checks the captured timestamps and sensor quality.
   No baseline range or slope is assumed from nominal model coefficients. *)
IF NOT Active AND NOT SafetyAbort THEN
    BaselineValues[SampleIndex] := TemperatureC;
    SampleIndex := (SampleIndex + 1) MOD 51;
    IF SampleCount < 51 THEN SampleCount := SampleCount + 1; END_IF;
    IF SampleCount = 51 THEN
        MinimumC := BaselineValues[0]; MaximumC := BaselineValues[0];
        FOR ReadIndex := 1 TO 50 DO
            MinimumC := MIN(MinimumC, BaselineValues[ReadIndex]);
            MaximumC := MAX(MaximumC, BaselineValues[ReadIndex]);
        END_FOR;
        OldestC := BaselineValues[SampleIndex];
        BaselineRangeC := LREAL_TO_REAL(MaximumC - MinimumC);
        BaselineSlopeCPerS := LREAL_TO_REAL((TemperatureC - OldestC) / LREAL#5.0);
        BaselineQualified := MinimumC >= LREAL#34.95 AND MaximumC <= LREAL#35.05
            AND BaselineRangeC <= REAL#0.05 AND ABS(BaselineSlopeCPerS) <= REAL#0.01;
    ELSE BaselineQualified := FALSE;
    END_IF;
END_IF;
(* Learner calculation and checks. Predictions remain explicitly supplied. *)
ProposedCommandPct := REAL#75.0 + SelectedStepPoints;
PredictedFinalC := REAL#20.0 + REAL#0.2 * ProposedCommandPct;
IF NOT ExperimentEnabled THEN Active := FALSE; END_IF;
IF ExperimentEnabled AND NOT WasEnabled THEN
    StartRejected := NOT (BaselineQualified AND MeasurementValid AND NOT SafetyAbort
        AND SelectedStepPoints >= REAL#5.0 AND SelectedStepPoints <= REAL#15.0
        AND HoldSeconds >= REAL#20.0 AND HoldSeconds <= REAL#40.0
        AND ProposedCommandPct >= REAL#0.0 AND ProposedCommandPct <= REAL#100.0);
    IF NOT StartRejected THEN Active := TRUE; StepScan := DINT#0; END_IF;
END_IF;
IF Active AND DINT_TO_REAL(StepScan) * REAL#0.1 >= HoldSeconds THEN
    Active := FALSE;
    SampleCount := 0;
    BaselineQualified := FALSE;
END_IF;
IF SafetyAbort THEN
    Active := FALSE; BaselineQualified := FALSE; SampleCount := 0;
    AppliedPowerPct := LREAL#0.0;
ELSIF Active THEN
    AppliedPowerPct := REAL_TO_LREAL(ProposedCommandPct);
    StepScan := StepScan + DINT#1;
ELSE AppliedPowerPct := LREAL#75.0;
END_IF;
WasEnabled := ExperimentEnabled;
END_PROGRAM
CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX64.0 : BOOL;
    P1.SafetyAbort AT %MX64.1 : BOOL;
    P1.BaselineQualified AT %MX64.2 : BOOL;
    P1.Active AT %MX64.3 : BOOL;
    P1.StartRejected AT %MX64.4 : BOOL;
    P1.ProposedCommandPct AT %MD0 : REAL;
    P1.PredictedFinalC AT %MD4 : REAL;
    P1.BaselineRangeC AT %MD8 : REAL;
    P1.BaselineSlopeCPerS AT %MD12 : REAL;
    P1.MeasurementQuality AT %IW16 : UINT;
    P1.TemperatureC AT %IL0 : LREAL;
    P1.AppliedPowerPct AT %QL0 : LREAL;
END_VAR
END_CONFIGURATION
