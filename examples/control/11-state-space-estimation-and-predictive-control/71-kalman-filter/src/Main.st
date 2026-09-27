FUNCTION Lesson71ReviewedCommand : LREAL
VAR_INPUT
    ExperimentElapsedSeconds : LREAL;
    ControlAuthority : BOOL;
END_VAR
IF NOT ControlAuthority THEN
    Lesson71ReviewedCommand := LREAL#0.0;
ELSIF ExperimentElapsedSeconds < LREAL#5.0 THEN
    Lesson71ReviewedCommand := LREAL#5.0;
ELSIF ExperimentElapsedSeconds < LREAL#30.0 THEN
    Lesson71ReviewedCommand := LREAL#10.0;
ELSE
    Lesson71ReviewedCommand := LREAL#0.0;
END_IF;
END_FUNCTION

FUNCTION Lesson71CovarianceInputsValid : BOOL
VAR_INPUT
    ProcessVariance : LREAL;
    MeasurementVariance : LREAL;
    CorrectedCovariance : LREAL;
END_VAR
Lesson71CovarianceInputsValid :=
    (ProcessVariance >= LREAL#0.0)
    AND (ProcessVariance <= LREAL#1000.0)
    AND (MeasurementVariance > LREAL#0.0)
    AND (MeasurementVariance <= LREAL#1000.0)
    AND (CorrectedCovariance >= LREAL#0.0)
    AND (CorrectedCovariance <= LREAL#1000000.0);
END_FUNCTION

FUNCTION Lesson71PredictedCovariance : LREAL
VAR_INPUT
    CorrectedCovariance : LREAL;
    ProcessVariance : LREAL;
END_VAR
Lesson71PredictedCovariance := LREAL#0.818730753077982
    * CorrectedCovariance + ProcessVariance;
END_FUNCTION

FUNCTION Lesson71KalmanGain : LREAL
VAR_INPUT
    PredictedCovariance : LREAL;
    MeasurementVariance : LREAL;
END_VAR
IF (PredictedCovariance + MeasurementVariance) > LREAL#0.0 THEN
    Lesson71KalmanGain := PredictedCovariance
        / (PredictedCovariance + MeasurementVariance);
ELSE
    Lesson71KalmanGain := LREAL#0.0;
END_IF;
END_FUNCTION

PROGRAM Main
VAR
    Enabled : BOOL := FALSE;
    UseKalman : BOOL := TRUE;
    EstimatorValid : BOOL := TRUE;
    ProcessNoiseVariance : LREAL := 0.1;
    MeasurementNoiseVariance : LREAL := 3.0;
    BeltCommandRequested : LREAL := 5.0;
    PredictedEstimate : LREAL := 5.0;
    PredictedCovariance : LREAL := 0.0;
    KalmanGain : LREAL := 0.0;
    Estimate : LREAL := 5.0;
    Covariance : LREAL := 1.9;
    SimpleEstimate : LREAL := 5.0;
    SelectedEstimate : LREAL := 5.0;
    Innovation : LREAL := 0.0;
    InnovationVariance : LREAL := 0.0;
    NormalizedInnovationSquared : LREAL := 0.0;
    PreviousBeltCommand : LREAL := 5.0;
    BeltCommandApplied : LREAL := 0.0;
    MassFlowTruth : LREAL := 5.0;
    MassFlowMeasurement : LREAL := 5.0;
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ExperimentElapsed : LREAL := 0.0;
END_VAR

(* Control Studio grants only authority. The project owns the nominal command
   record; covariance alternatives and faults remain independent native tests. *)
IF ExperimentEnabled AND InputsValid AND NOT SafetyAbort THEN
    ExperimentElapsed := ExperimentElapsed + LREAL#0.1;
    Enabled := TRUE;
    UseKalman := TRUE;
    ProcessNoiseVariance := LREAL#0.1;
    MeasurementNoiseVariance := LREAL#3.0;
    BeltCommandRequested := Lesson71ReviewedCommand(
        ExperimentElapsedSeconds := ExperimentElapsed,
        ControlAuthority := TRUE);
ELSE
    ExperimentElapsed := LREAL#0.0;
    Enabled := FALSE;
    UseKalman := TRUE;
    ProcessNoiseVariance := LREAL#0.1;
    MeasurementNoiseVariance := LREAL#3.0;
    BeltCommandRequested := LREAL#0.0;
END_IF;

IF (MassFlowMeasurement >= LREAL#-1000.0)
    AND (MassFlowMeasurement <= LREAL#1000.0) THEN
    SimpleEstimate := SimpleEstimate + LREAL#0.2
        * (MassFlowMeasurement - SimpleEstimate);
END_IF;

EstimatorValid := Lesson71CovarianceInputsValid(
        ProcessVariance := ProcessNoiseVariance,
        MeasurementVariance := MeasurementNoiseVariance,
        CorrectedCovariance := Covariance)
    AND (Estimate >= LREAL#-1000000.0)
    AND (Estimate <= LREAL#1000000.0)
    AND (MassFlowMeasurement >= LREAL#-1000.0)
    AND (MassFlowMeasurement <= LREAL#1000.0);

IF EstimatorValid THEN
    // Exact 0.1 s discretisation of the one-second feeder model.
    PredictedEstimate := LREAL#0.9048374180359595 * Estimate
        + LREAL#0.0951625819640405 * PreviousBeltCommand;
    PredictedCovariance := Lesson71PredictedCovariance(
        CorrectedCovariance := Covariance,
        ProcessVariance := ProcessNoiseVariance);
    Innovation := MassFlowMeasurement - PredictedEstimate;
    InnovationVariance := PredictedCovariance + MeasurementNoiseVariance;
    EstimatorValid := (PredictedCovariance >= LREAL#0.0)
        AND (PredictedCovariance <= LREAL#1000000.0)
        AND (InnovationVariance > LREAL#0.0)
        AND (InnovationVariance <= LREAL#1000000.0);
END_IF;

IF EstimatorValid THEN
    KalmanGain := Lesson71KalmanGain(
        PredictedCovariance := PredictedCovariance,
        MeasurementVariance := MeasurementNoiseVariance);
    Estimate := PredictedEstimate + KalmanGain * Innovation;
    Covariance := (LREAL#1.0 - KalmanGain) * PredictedCovariance;
    NormalizedInnovationSquared := Innovation * Innovation
        / InnovationVariance;
ELSE
    // Preserve finite state and make the fixed estimator the explicit fallback.
    PredictedEstimate := Estimate;
    PredictedCovariance := Covariance;
    KalmanGain := LREAL#0.0;
    Innovation := LREAL#0.0;
    InnovationVariance := LREAL#0.0;
    NormalizedInnovationSquared := LREAL#0.0;
END_IF;

IF UseKalman AND EstimatorValid THEN
    SelectedEstimate := Estimate;
ELSE
    SelectedEstimate := SimpleEstimate;
END_IF;

IF Enabled THEN
    BeltCommandApplied := LIMIT(
        LREAL#0.0,
        BeltCommandRequested,
        LREAL#20.0
    );
ELSE
    BeltCommandApplied := LREAL#0.0;
END_IF;
PreviousBeltCommand := BeltCommandApplied;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ProcessNoiseVariance AT %ML0 : LREAL;
    P1.MeasurementNoiseVariance AT %ML8 : LREAL;
    P1.BeltCommandRequested AT %ML16 : LREAL;
    P1.PredictedEstimate AT %ML24 : LREAL;
    P1.PredictedCovariance AT %ML32 : LREAL;
    P1.KalmanGain AT %ML40 : LREAL;
    P1.Estimate AT %ML48 : LREAL;
    P1.Covariance AT %ML56 : LREAL;
    P1.SimpleEstimate AT %ML64 : LREAL;
    P1.SelectedEstimate AT %ML72 : LREAL;
    P1.Innovation AT %ML80 : LREAL;
    P1.InnovationVariance AT %ML88 : LREAL;
    P1.NormalizedInnovationSquared AT %ML96 : LREAL;
    P1.Enabled AT %MX120.0 : BOOL;
    P1.UseKalman AT %MX120.1 : BOOL;
    P1.EstimatorValid AT %MX120.2 : BOOL;
    P1.ExperimentEnabled AT %MX120.3 : BOOL;
    P1.SafetyAbort AT %MX120.4 : BOOL;
    P1.InputsValid AT %MX120.5 : BOOL;
    P1.BeltCommandApplied AT %QL0 : LREAL;
    P1.MassFlowTruth AT %IL0 : LREAL;
    P1.MassFlowMeasurement AT %IL8 : LREAL;
END_VAR
END_CONFIGURATION
