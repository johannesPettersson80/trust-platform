NAMESPACE Control
TYPE SmithPredictorParameters :
STRUCT
    ProcessGain : LREAL;
    TimeConstant : LREAL;
    DeadTime : LREAL;
    OperatingInput : LREAL;
    OperatingOutput : LREAL;
    MaximumCorrection : LREAL;
END_STRUCT
END_TYPE

TYPE SmithPredictorInitialState :
STRUCT
    ModelOutput : LREAL;
    ProcessValue : LREAL;
END_STRUCT
END_TYPE

TYPE SmithPredictorInput :
STRUCT
    ManipulatedValue : LREAL;
    ProcessValue : LREAL;
    CycleTime : LREAL;
    Enabled : BOOL;
END_STRUCT
END_TYPE

TYPE SmithPredictorOutput :
STRUCT
    PredictedProcessValue : LREAL;
    DelayFreeModel : LREAL;
    DelayedModel : LREAL;
    Correction : LREAL;
    Active : BOOL;
    HistoryComplete : BOOL;
END_STRUCT
END_TYPE

TYPE PIDTuning :
STRUCT
    ProportionalGain : LREAL;
    IntegralGain : LREAL;
    DerivativeGain : LREAL;
    ProportionalSetpointWeight : LREAL;
    DerivativeSetpointWeight : LREAL;
    TrackingTime : LREAL;
END_STRUCT
END_TYPE

TYPE PIDGainSchedulePoint :
STRUCT
    OperatingPoint : LREAL;
    Tuning : PIDTuning;
END_STRUCT
END_TYPE

TYPE PIDGainSchedulePoints : ARRAY[0..7] OF PIDGainSchedulePoint; END_TYPE

TYPE PIDGainScheduleParameters :
STRUCT
    Points : PIDGainSchedulePoints;
    PointCount : DINT;
    CommonParameters : PIDParameters;
    FilterTimeConstant : LREAL;
END_STRUCT
END_TYPE

TYPE PIDGainScheduleOutput :
STRUCT
    Parameters : PIDParameters;
    FilteredOperatingPoint : LREAL;
    LowerIndex : DINT;
    UpperIndex : DINT;
    Fraction : LREAL;
    Clamped : BOOL;
    Valid : BOOL;
END_STRUCT
END_TYPE

TYPE ControlVector2 :
STRUCT
    First : LREAL;
    Second : LREAL;
END_STRUCT
END_TYPE

TYPE ProcessGainMatrix2x2 :
STRUCT
    G11 : LREAL;
    G12 : LREAL;
    G21 : LREAL;
    G22 : LREAL;
END_STRUCT
END_TYPE

TYPE StaticDecoupler2x2Parameters :
STRUCT
    GainMatrix : ProcessGainMatrix2x2;
    ActuatorBias : ControlVector2;
    SingularityTolerance : LREAL;
END_STRUCT
END_TYPE

TYPE StaticDecoupler2x2Status :
STRUCT
    Configured : BOOL;
    Determinant : LREAL;
    Inverse11 : LREAL;
    Inverse12 : LREAL;
    Inverse21 : LREAL;
    Inverse22 : LREAL;
END_STRUCT
END_TYPE

TYPE PIDParameterForm :
(
    Parallel := 0,
    Ideal := 1,
    Series := 2
);
END_TYPE

TYPE PIDFormParameters :
STRUCT
    Form : PIDParameterForm;
    Proportional : LREAL;
    Integral : LREAL;
    Derivative : LREAL;
END_STRUCT
END_TYPE

TYPE PIDFormConversionStatus :
(
    Valid := 0,
    InvalidDomain := 1,
    NonRepresentable := 2
);
END_TYPE

TYPE PIDFormConversionResult :
STRUCT
    Parameters : PIDFormParameters;
    Status : PIDFormConversionStatus;
END_STRUCT
END_TYPE

FUNCTION ConvertPIDForm : PIDFormConversionResult
VAR_INPUT
    Input : PIDFormParameters;
    Target : PIDParameterForm;
END_VAR
VAR
    Result : PIDFormConversionResult;
    Kp : LREAL;
    Ki : LREAL;
    Kd : LREAL;
    Discriminant : LREAL;
    ControllerGain : LREAL;
    SourceValid : BOOL;
    ParallelValid : BOOL;
END_VAR
Result.Status := PIDFormConversionStatus#InvalidDomain;

IF Input.Form = PIDParameterForm#Parallel THEN
    Kp := Input.Proportional;
    Ki := Input.Integral;
    Kd := Input.Derivative;
    SourceValid := (Kp = Kp) AND (Ki = Ki) AND (Kd = Kd)
        AND (ABS(Kp) <= LREAL#1.0e100)
        AND (ABS(Ki) <= LREAL#1.0e100)
        AND (ABS(Kd) <= LREAL#1.0e100)
        AND (((Kp > LREAL#0.0) AND (Ki > LREAL#0.0)
                AND (Kd >= LREAL#0.0))
            OR ((Kp < LREAL#0.0) AND (Ki < LREAL#0.0)
                AND (Kd <= LREAL#0.0)));
ELSIF (Input.Form = PIDParameterForm#Ideal)
    OR (Input.Form = PIDParameterForm#Series)
THEN
    SourceValid := (Input.Proportional = Input.Proportional)
        AND (Input.Integral = Input.Integral)
        AND (Input.Derivative = Input.Derivative)
        AND (ABS(Input.Proportional) <= LREAL#1.0e100)
        AND (Input.Integral <= LREAL#1.0e100)
        AND (Input.Derivative <= LREAL#1.0e100)
        AND (Input.Proportional <> LREAL#0.0)
        AND (Input.Integral > LREAL#0.0)
        AND (Input.Derivative >= LREAL#0.0);
    IF SourceValid THEN
        Ki := Input.Proportional / Input.Integral;
        Kd := Input.Proportional * Input.Derivative;
        IF Input.Form = PIDParameterForm#Series THEN
            Kp := Input.Proportional
                * (LREAL#1.0 + Input.Derivative / Input.Integral);
        ELSE
            Kp := Input.Proportional;
        END_IF;
    END_IF;
END_IF;

ParallelValid := SourceValid
    AND (Kp = Kp) AND (Ki = Ki) AND (Kd = Kd)
    AND (ABS(Kp) <= LREAL#1.0e100)
    AND (ABS(Ki) <= LREAL#1.0e100)
    AND (ABS(Kd) <= LREAL#1.0e100)
    AND (((Kp > LREAL#0.0) AND (Ki > LREAL#0.0)
            AND (Kd >= LREAL#0.0))
        OR ((Kp < LREAL#0.0) AND (Ki < LREAL#0.0)
            AND (Kd <= LREAL#0.0)));

IF ParallelValid THEN
    Result.Parameters.Form := Target;
    IF Target = PIDParameterForm#Parallel THEN
        Result.Parameters.Proportional := Kp;
        Result.Parameters.Integral := Ki;
        Result.Parameters.Derivative := Kd;
        Result.Status := PIDFormConversionStatus#Valid;
    ELSIF Target = PIDParameterForm#Ideal THEN
        Result.Parameters.Proportional := Kp;
        Result.Parameters.Integral := Kp / Ki;
        Result.Parameters.Derivative := Kd / Kp;
        Result.Status := PIDFormConversionStatus#Valid;
    ELSIF Target = PIDParameterForm#Series THEN
        Discriminant := Kp * Kp - LREAL#4.0 * Ki * Kd;
        IF Discriminant >= LREAL#0.0 THEN
            IF Kp > LREAL#0.0 THEN
                ControllerGain := (Kp + SQRT(Discriminant)) * LREAL#0.5;
            ELSE
                ControllerGain := (Kp - SQRT(Discriminant)) * LREAL#0.5;
            END_IF;
            Result.Parameters.Proportional := ControllerGain;
            Result.Parameters.Integral := ControllerGain / Ki;
            Result.Parameters.Derivative := Kd / ControllerGain;
            Result.Status := PIDFormConversionStatus#Valid;
        ELSE
            Result.Parameters.Proportional := LREAL#0.0;
            Result.Parameters.Integral := LREAL#0.0;
            Result.Parameters.Derivative := LREAL#0.0;
            Result.Status := PIDFormConversionStatus#NonRepresentable;
        END_IF;
    END_IF;
END_IF;
ConvertPIDForm := Result;
END_FUNCTION

FUNCTION_BLOCK SmithPredictorFOPDT
VAR
    ParametersValue : SmithPredictorParameters;
    Configured : BOOL;
    Initialized : BOOL;
    DelayFreeModelValue : LREAL;
    InitialModelOutput : LREAL;
    CorrectionStateValue : LREAL;
    ElapsedTime : LREAL;
    HistoryTimes : ARRAY[0..255] OF LREAL;
    HistoryValues : ARRAY[0..255] OF LREAL;
    HistoryCount : DINT;
    HistoryWriteIndex : DINT;
    HistoryReadIndex : DINT;
END_VAR

METHOD PUBLIC Configure : BOOL
VAR_INPUT
    Parameters : SmithPredictorParameters;
END_VAR
IF (Parameters.TimeConstant > LREAL#0.0)
    AND (Parameters.DeadTime >= LREAL#0.0)
THEN
    ParametersValue := Parameters;
    Configured := TRUE;
    Configure := TRUE;
ELSE
    Configure := FALSE;
END_IF;
END_METHOD

METHOD PUBLIC Initialize
VAR_INPUT
    State : SmithPredictorInitialState;
END_VAR
DelayFreeModelValue := State.ModelOutput;
InitialModelOutput := State.ModelOutput;
CorrectionStateValue := State.ProcessValue - State.ModelOutput;
ElapsedTime := LREAL#0.0;
HistoryTimes[0] := LREAL#0.0;
HistoryValues[0] := State.ModelOutput;
HistoryCount := DINT#1;
HistoryWriteIndex := DINT#1;
HistoryReadIndex := DINT#0;
Initialized := TRUE;
END_METHOD

METHOD PUBLIC Update : SmithPredictorOutput
VAR_INPUT
    Input : SmithPredictorInput;
END_VAR
VAR
    Result : SmithPredictorOutput;
    Coefficient : LREAL;
    ModelTarget : LREAL;
    TargetTime : LREAL;
    DelayedModelValue : LREAL;
    CorrectionValue : LREAL;
    Fraction : LREAL;
    OldestIndex : DINT;
    LatestIndex : DINT;
    NextIndex : DINT;
    HistoryCompleteValue : BOOL;
END_VAR

Result.PredictedProcessValue := Input.ProcessValue;
Result.DelayFreeModel := DelayFreeModelValue;
Result.DelayedModel := DelayFreeModelValue;
Result.Correction := CorrectionStateValue;

IF Configured AND Initialized AND (Input.CycleTime > LREAL#0.0) THEN
    Coefficient := Input.CycleTime
        / (ParametersValue.TimeConstant + LREAL#0.5 * Input.CycleTime);
    ModelTarget := ParametersValue.OperatingOutput
        + ParametersValue.ProcessGain
            * (Input.ManipulatedValue - ParametersValue.OperatingInput);
    DelayFreeModelValue := DelayFreeModelValue
        + Coefficient * (ModelTarget - DelayFreeModelValue);
    ElapsedTime := ElapsedTime + Input.CycleTime;

    HistoryTimes[HistoryWriteIndex] := ElapsedTime;
    HistoryValues[HistoryWriteIndex] := DelayFreeModelValue;
    HistoryWriteIndex := (HistoryWriteIndex + DINT#1) MOD DINT#256;
    IF HistoryCount < DINT#256 THEN
        HistoryCount := HistoryCount + DINT#1;
    END_IF;

    TargetTime := ElapsedTime - ParametersValue.DeadTime;
    HistoryCompleteValue := TRUE;
    IF HistoryCount < DINT#256 THEN
        OldestIndex := DINT#0;
    ELSE
        OldestIndex := HistoryWriteIndex;
    END_IF;
    LatestIndex := (HistoryWriteIndex + DINT#255) MOD DINT#256;
    IF TargetTime <= LREAL#0.0 THEN
        DelayedModelValue := InitialModelOutput;
    ELSIF HistoryTimes[OldestIndex] > TargetTime THEN
        HistoryCompleteValue := FALSE;
        DelayedModelValue := HistoryValues[OldestIndex];
    ELSIF TargetTime >= HistoryTimes[LatestIndex] THEN
        HistoryReadIndex := LatestIndex;
        DelayedModelValue := HistoryValues[LatestIndex];
    ELSE
        IF HistoryTimes[HistoryReadIndex] > TargetTime THEN
            HistoryReadIndex := OldestIndex;
        END_IF;
        NextIndex := (HistoryReadIndex + DINT#1) MOD DINT#256;
        WHILE (NextIndex <> HistoryWriteIndex)
            AND (HistoryTimes[NextIndex] <= TargetTime)
        DO
            HistoryReadIndex := NextIndex;
            NextIndex := (HistoryReadIndex + DINT#1) MOD DINT#256;
        END_WHILE;
        Fraction := (TargetTime - HistoryTimes[HistoryReadIndex])
            / (HistoryTimes[NextIndex] - HistoryTimes[HistoryReadIndex]);
        DelayedModelValue := HistoryValues[HistoryReadIndex]
            + Fraction * (HistoryValues[NextIndex] - HistoryValues[HistoryReadIndex]);
    END_IF;

    CorrectionValue := Input.ProcessValue - DelayedModelValue;
    IF ParametersValue.MaximumCorrection > LREAL#0.0 THEN
        CorrectionValue := MAX(
            -ParametersValue.MaximumCorrection,
            MIN(ParametersValue.MaximumCorrection, CorrectionValue)
        );
    END_IF;

    CorrectionStateValue := CorrectionValue;
    Result.DelayFreeModel := DelayFreeModelValue;
    Result.DelayedModel := DelayedModelValue;
    Result.Correction := CorrectionValue;
    Result.HistoryComplete := HistoryCompleteValue;
    Result.Active := Input.Enabled AND HistoryCompleteValue;
    IF Result.Active THEN
        Result.PredictedProcessValue := DelayFreeModelValue + CorrectionValue;
    END_IF;
END_IF;

Update := Result;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK PIDGainScheduler
VAR
    ParametersValue : PIDGainScheduleParameters;
    Configured : BOOL;
    Initialized : BOOL;
    FilteredOperatingPointValue : LREAL;
    LastOutput : PIDGainScheduleOutput;
END_VAR

METHOD PUBLIC Configure : BOOL
VAR_INPUT
    Parameters : PIDGainScheduleParameters;
END_VAR
VAR
    CandidateValid : BOOL;
    Index : DINT;
END_VAR
CandidateValid := (Parameters.PointCount >= DINT#2)
    AND (Parameters.PointCount <= DINT#8)
    AND (Parameters.FilterTimeConstant >= LREAL#0.0)
    AND (Parameters.CommonParameters.OutputMinimum
        <= Parameters.CommonParameters.OutputMaximum)
    AND (Parameters.CommonParameters.RateMinimum <= LREAL#0.0)
    AND (Parameters.CommonParameters.RateMaximum >= LREAL#0.0);

IF CandidateValid THEN
    FOR Index := DINT#0 TO Parameters.PointCount - DINT#1 DO
        IF (Parameters.Points[Index].Tuning.IntegralGain <> LREAL#0.0)
            AND (Parameters.Points[Index].Tuning.TrackingTime <= LREAL#0.0)
        THEN
            CandidateValid := FALSE;
        END_IF;
        IF (Index > DINT#0)
            AND (Parameters.Points[Index].OperatingPoint
                <= Parameters.Points[Index - DINT#1].OperatingPoint)
        THEN
            CandidateValid := FALSE;
        END_IF;
    END_FOR;
END_IF;

IF CandidateValid THEN
    ParametersValue := Parameters;
    Configured := TRUE;
    Configure := TRUE;
ELSE
    Configure := FALSE;
END_IF;
END_METHOD

METHOD PUBLIC Initialize
VAR_INPUT
    OperatingPoint : LREAL;
END_VAR
FilteredOperatingPointValue := OperatingPoint;
Initialized := TRUE;
END_METHOD

METHOD PUBLIC Update : PIDGainScheduleOutput
VAR_INPUT
    OperatingPoint : LREAL;
    CycleTime : LREAL;
END_VAR
VAR
    Result : PIDGainScheduleOutput;
    LowerIndexValue : DINT;
    UpperIndexValue : DINT;
    LastIndex : DINT;
    Index : DINT;
    FractionValue : LREAL;
    Alpha : LREAL;
    Found : BOOL;
END_VAR

Result := LastOutput;
IF Configured AND Initialized AND (CycleTime > LREAL#0.0) THEN
    IF ParametersValue.FilterTimeConstant > LREAL#0.0 THEN
        Alpha := CycleTime / (ParametersValue.FilterTimeConstant + CycleTime);
        FilteredOperatingPointValue := FilteredOperatingPointValue
            + Alpha * (OperatingPoint - FilteredOperatingPointValue);
    ELSE
        FilteredOperatingPointValue := OperatingPoint;
    END_IF;

    LastIndex := ParametersValue.PointCount - DINT#1;
    Result.Clamped := FALSE;
    IF FilteredOperatingPointValue <= ParametersValue.Points[0].OperatingPoint THEN
        LowerIndexValue := DINT#0;
        UpperIndexValue := DINT#1;
        FractionValue := LREAL#0.0;
        Result.Clamped := FilteredOperatingPointValue
            < ParametersValue.Points[0].OperatingPoint;
    ELSIF FilteredOperatingPointValue
        >= ParametersValue.Points[LastIndex].OperatingPoint
    THEN
        LowerIndexValue := LastIndex - DINT#1;
        UpperIndexValue := LastIndex;
        FractionValue := LREAL#1.0;
        Result.Clamped := FilteredOperatingPointValue
            > ParametersValue.Points[LastIndex].OperatingPoint;
    ELSE
        Found := FALSE;
        FOR Index := DINT#1 TO LastIndex DO
            IF (NOT Found) AND (ParametersValue.Points[Index].OperatingPoint
                >= FilteredOperatingPointValue)
            THEN
                LowerIndexValue := Index - DINT#1;
                UpperIndexValue := Index;
                Found := TRUE;
            END_IF;
        END_FOR;
        FractionValue := (FilteredOperatingPointValue
                - ParametersValue.Points[LowerIndexValue].OperatingPoint)
            / (ParametersValue.Points[UpperIndexValue].OperatingPoint
                - ParametersValue.Points[LowerIndexValue].OperatingPoint);
    END_IF;

    Result.Parameters := ParametersValue.CommonParameters;
    Result.Parameters.ProportionalGain :=
        ParametersValue.Points[LowerIndexValue].Tuning.ProportionalGain
        + FractionValue * (
            ParametersValue.Points[UpperIndexValue].Tuning.ProportionalGain
            - ParametersValue.Points[LowerIndexValue].Tuning.ProportionalGain);
    Result.Parameters.IntegralGain :=
        ParametersValue.Points[LowerIndexValue].Tuning.IntegralGain
        + FractionValue * (
            ParametersValue.Points[UpperIndexValue].Tuning.IntegralGain
            - ParametersValue.Points[LowerIndexValue].Tuning.IntegralGain);
    Result.Parameters.DerivativeGain :=
        ParametersValue.Points[LowerIndexValue].Tuning.DerivativeGain
        + FractionValue * (
            ParametersValue.Points[UpperIndexValue].Tuning.DerivativeGain
            - ParametersValue.Points[LowerIndexValue].Tuning.DerivativeGain);
    Result.Parameters.ProportionalSetpointWeight :=
        ParametersValue.Points[LowerIndexValue].Tuning.ProportionalSetpointWeight
        + FractionValue * (
            ParametersValue.Points[UpperIndexValue].Tuning.ProportionalSetpointWeight
            - ParametersValue.Points[LowerIndexValue].Tuning.ProportionalSetpointWeight);
    Result.Parameters.DerivativeSetpointWeight :=
        ParametersValue.Points[LowerIndexValue].Tuning.DerivativeSetpointWeight
        + FractionValue * (
            ParametersValue.Points[UpperIndexValue].Tuning.DerivativeSetpointWeight
            - ParametersValue.Points[LowerIndexValue].Tuning.DerivativeSetpointWeight);
    Result.Parameters.TrackingTime :=
        ParametersValue.Points[LowerIndexValue].Tuning.TrackingTime
        + FractionValue * (
            ParametersValue.Points[UpperIndexValue].Tuning.TrackingTime
            - ParametersValue.Points[LowerIndexValue].Tuning.TrackingTime);
    Result.FilteredOperatingPoint := FilteredOperatingPointValue;
    Result.LowerIndex := LowerIndexValue;
    Result.UpperIndex := UpperIndexValue;
    Result.Fraction := FractionValue;
    Result.Valid := TRUE;
    LastOutput := Result;
END_IF;
Update := Result;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK StaticDecoupler2x2
VAR
    ParametersValue : StaticDecoupler2x2Parameters;
    StatusValue : StaticDecoupler2x2Status;
END_VAR

METHOD PUBLIC Configure : BOOL
VAR_INPUT
    Parameters : StaticDecoupler2x2Parameters;
END_VAR
VAR
    DeterminantValue : LREAL;
    RowNorm : LREAL;
    ColumnNorm : LREAL;
    MatrixScale : LREAL;
    Candidate : StaticDecoupler2x2Status;
END_VAR
DeterminantValue := Parameters.GainMatrix.G11 * Parameters.GainMatrix.G22
    - Parameters.GainMatrix.G12 * Parameters.GainMatrix.G21;
RowNorm := MAX(
    ABS(Parameters.GainMatrix.G11) + ABS(Parameters.GainMatrix.G12),
    ABS(Parameters.GainMatrix.G21) + ABS(Parameters.GainMatrix.G22)
);
ColumnNorm := MAX(
    ABS(Parameters.GainMatrix.G11) + ABS(Parameters.GainMatrix.G21),
    ABS(Parameters.GainMatrix.G12) + ABS(Parameters.GainMatrix.G22)
);
MatrixScale := RowNorm * ColumnNorm;
IF (Parameters.SingularityTolerance > LREAL#0.0)
    AND (MatrixScale > LREAL#0.0)
    AND (ABS(DeterminantValue) / MatrixScale
        > Parameters.SingularityTolerance)
THEN
    Candidate.Configured := TRUE;
    Candidate.Determinant := DeterminantValue;
    Candidate.Inverse11 := Parameters.GainMatrix.G22 / DeterminantValue;
    Candidate.Inverse12 := -Parameters.GainMatrix.G12 / DeterminantValue;
    Candidate.Inverse21 := -Parameters.GainMatrix.G21 / DeterminantValue;
    Candidate.Inverse22 := Parameters.GainMatrix.G11 / DeterminantValue;
    ParametersValue := Parameters;
    StatusValue := Candidate;
    Configure := TRUE;
ELSE
    Configure := FALSE;
END_IF;
END_METHOD

METHOD PUBLIC ToActuators : ControlVector2
VAR_INPUT
    VirtualCommand : ControlVector2;
END_VAR
VAR
    Result : ControlVector2;
END_VAR
IF StatusValue.Configured THEN
    Result.First := ParametersValue.ActuatorBias.First
        + StatusValue.Inverse11 * VirtualCommand.First
        + StatusValue.Inverse12 * VirtualCommand.Second;
    Result.Second := ParametersValue.ActuatorBias.Second
        + StatusValue.Inverse21 * VirtualCommand.First
        + StatusValue.Inverse22 * VirtualCommand.Second;
END_IF;
ToActuators := Result;
END_METHOD

METHOD PUBLIC ToVirtual : ControlVector2
VAR_INPUT
    ActuatorCommand : ControlVector2;
END_VAR
VAR
    Result : ControlVector2;
    FirstDeviation : LREAL;
    SecondDeviation : LREAL;
END_VAR
IF StatusValue.Configured THEN
    FirstDeviation := ActuatorCommand.First - ParametersValue.ActuatorBias.First;
    SecondDeviation := ActuatorCommand.Second - ParametersValue.ActuatorBias.Second;
    Result.First := ParametersValue.GainMatrix.G11 * FirstDeviation
        + ParametersValue.GainMatrix.G12 * SecondDeviation;
    Result.Second := ParametersValue.GainMatrix.G21 * FirstDeviation
        + ParametersValue.GainMatrix.G22 * SecondDeviation;
END_IF;
ToVirtual := Result;
END_METHOD

METHOD PUBLIC Snapshot : StaticDecoupler2x2Status
Snapshot := StatusValue;
END_METHOD
END_FUNCTION_BLOCK
END_NAMESPACE
