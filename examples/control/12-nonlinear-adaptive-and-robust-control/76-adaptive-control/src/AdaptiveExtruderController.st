USING Control;

(* AdaptiveExtruderController estimates the local screw-speed-to-pressure gain and
   bias while regulating die pressure. Adaptation is allowed only with explicit
   probe approval, healthy informative data, and an unsaturated actuator; the
   block otherwise freezes its estimate and reports the reason. *)
FUNCTION_BLOCK AdaptiveExtruderController
VAR_INPUT
    RequestedArchitecture : INT;
    ResinGrade : INT;
    Enabled : BOOL;
    AdaptationRequested : BOOL;
    DataHealthy : BOOL;
    ProbeEnabled : BOOL;
    PressureSetpoint : LREAL;
    DiePressure : LREAL;
    InitialEstimatedGain : LREAL := 1.0;
    InitialEstimatedBias : LREAL := 0.0;
END_VAR
VAR_OUTPUT
    ActiveArchitecture : INT;
    AdaptationActive : BOOL;
    EstimateFrozen : BOOL;
    PersistentExcitation : BOOL;
    EstimateQualified : BOOL;
    FallbackActive : BOOL;
    ProjectionActive : BOOL;
    OutputSaturated : BOOL;
    EstimatorInitialized : BOOL;
    EstimatedGain : LREAL := 1.0;
    EstimatedBias : LREAL := 0.0;
    Covariance11 : LREAL := 10.0;
    Covariance12 : LREAL := 0.0;
    Covariance22 : LREAL := 10.0;
    Information11 : LREAL := 0.0;
    Information12 : LREAL := 0.0;
    Information22 : LREAL := 0.0;
    CovarianceTrace : LREAL := 20.0;
    InformationDeterminant : LREAL := 0.0;
    Innovation : LREAL := 0.0;
    ActiveProportionalGain : LREAL := 1.666666666666667;
    ActiveIntegralGain : LREAL := 0.333333333333333;
    Probe : LREAL := 0.0;
    ScrewSpeedRequested : LREAL := 0.0;
    ScrewSpeedApplied : LREAL := 0.0;
    InformativeUpdateCount : DINT := 0;
    FrozenReason : INT := 0;
END_VAR
VAR
    PreviousPressure : LREAL := 40.0;
    PreviousApplied : LREAL := 40.0;
    PreviousSaturated : BOOL := FALSE;
    ProbeTimer : LREAL := 0.0;
    ProbeSign : LREAL := 1.0;
    Phi1 : LREAL := 0.0;
    Observation : LREAL := 0.0;
    PredictedObservation : LREAL := 0.0;
    PTimesPhi1 : LREAL := 0.0;
    PTimesPhi2 : LREAL := 0.0;
    Denominator : LREAL := 1.0;
    EstimatorGain1 : LREAL := 0.0;
    EstimatorGain2 : LREAL := 0.0;
    CandidateGain : LREAL := 1.0;
    CandidateBiasScaled : LREAL := 0.0;
    BiasScaled : LREAL := 0.0;
    NextCovariance11 : LREAL := 10.0;
    NextCovariance12 : LREAL := 0.0;
    NextCovariance22 : LREAL := 10.0;
    DesignGain : LREAL := 1.0;
    Controller : PID;
    Parameters : PIDParameters;
    InitialState : PIDInitialState;
    Input : PIDInput;
    ControllerInitialized : BOOL := FALSE;
END_VAR

ProjectionActive := FALSE;
IF NOT EstimatorInitialized THEN
    (* Commissioning estimates cross the program boundary only once. The block
       owns their bounded evolution after initialization. *)
    EstimatedGain := InitialEstimatedGain;
    EstimatedBias := InitialEstimatedBias;
END_IF;
IF EstimatedGain < LREAL#0.5 THEN
    EstimatedGain := LREAL#0.5;
    ProjectionActive := TRUE;
ELSIF EstimatedGain > LREAL#1.5 THEN
    EstimatedGain := LREAL#1.5;
    ProjectionActive := TRUE;
END_IF;
IF EstimatedBias < LREAL#-20.0 THEN
    EstimatedBias := LREAL#-20.0;
    ProjectionActive := TRUE;
ELSIF EstimatedBias > LREAL#20.0 THEN
    EstimatedBias := LREAL#20.0;
    ProjectionActive := TRUE;
END_IF;

IF NOT EstimatorInitialized THEN
    // Start uncertain and unqualified; controller authority remains scheduled.
    Covariance11 := LREAL#10.0;
    Covariance12 := LREAL#0.0;
    Covariance22 := LREAL#10.0;
    Information11 := LREAL#0.0;
    Information12 := LREAL#0.0;
    Information22 := LREAL#0.0;
    PreviousPressure := DiePressure;
    PreviousApplied := LREAL#40.0;
    PreviousSaturated := FALSE;
    EstimatorInitialized := TRUE;
    AdaptationActive := FALSE;
    Innovation := LREAL#0.0;
ELSE
    Phi1 := PreviousApplied / LREAL#100.0;
    IF Enabled AND AdaptationRequested AND DataHealthy AND NOT PreviousSaturated THEN
        Information11 := LREAL#0.98 * Information11 + Phi1 * Phi1;
        Information12 := LREAL#0.98 * Information12 + Phi1;
        Information22 := LREAL#0.98 * Information22 + LREAL#1.0;
    END_IF;
    InformationDeterminant := Information11 * Information22
        - Information12 * Information12;
    PersistentExcitation := InformationDeterminant >= LREAL#0.10;
    AdaptationActive := Enabled
        AND AdaptationRequested
        AND DataHealthy
        AND NOT PreviousSaturated
        AND PersistentExcitation;
    // Informative data do not authorize an experiment. The probe gate is kept
    // separate so removing approval visibly and immediately freezes learning.
    IF NOT ProbeEnabled THEN
        AdaptationActive := FALSE;
    END_IF;
    Innovation := LREAL#0.0;

    IF AdaptationActive THEN
        Observation := (PreviousPressure
            + LREAL#5.0 * (DiePressure - PreviousPressure) / LREAL#0.1)
            / LREAL#100.0;
        BiasScaled := EstimatedBias / LREAL#100.0;
        PredictedObservation := EstimatedGain * Phi1 + BiasScaled;
        Innovation := Observation - PredictedObservation;
        PTimesPhi1 := Covariance11 * Phi1 + Covariance12;
        PTimesPhi2 := Covariance12 * Phi1 + Covariance22;
        Denominator := LREAL#0.98 + Phi1 * PTimesPhi1 + PTimesPhi2;
        EstimatorGain1 := PTimesPhi1 / Denominator;
        EstimatorGain2 := PTimesPhi2 / Denominator;
        CandidateGain := EstimatedGain + EstimatorGain1 * Innovation;
        CandidateBiasScaled := BiasScaled + EstimatorGain2 * Innovation;

        NextCovariance11 := (Covariance11 - EstimatorGain1 * PTimesPhi1)
            / LREAL#0.98;
        NextCovariance12 := (Covariance12 - EstimatorGain1 * PTimesPhi2)
            / LREAL#0.98;
        NextCovariance22 := (Covariance22 - EstimatorGain2 * PTimesPhi2)
            / LREAL#0.98;
        Covariance11 := NextCovariance11;
        Covariance12 := NextCovariance12;
        Covariance22 := NextCovariance22;

        IF CandidateGain < LREAL#0.5 THEN
            EstimatedGain := LREAL#0.5;
            ProjectionActive := TRUE;
        ELSIF CandidateGain > LREAL#1.5 THEN
            EstimatedGain := LREAL#1.5;
            ProjectionActive := TRUE;
        ELSE
            EstimatedGain := CandidateGain;
        END_IF;
        IF CandidateBiasScaled < LREAL#-0.2 THEN
            EstimatedBias := LREAL#-20.0;
            ProjectionActive := TRUE;
        ELSIF CandidateBiasScaled > LREAL#0.2 THEN
            EstimatedBias := LREAL#20.0;
            ProjectionActive := TRUE;
        ELSE
            EstimatedBias := LREAL#100.0 * CandidateBiasScaled;
        END_IF;
        InformativeUpdateCount := InformativeUpdateCount + DINT#1;
    END_IF;
END_IF;

CovarianceTrace := Covariance11 + Covariance22;
InformationDeterminant := Information11 * Information22
    - Information12 * Information12;
PersistentExcitation := InformationDeterminant >= LREAL#0.10;
EstimateQualified := DataHealthy
    AND (InformativeUpdateCount >= DINT#20)
    AND (CovarianceTrace <= LREAL#10.0)
    AND (EstimatedGain > LREAL#0.5)
    AND (EstimatedGain < LREAL#1.5)
    AND (EstimatedBias > LREAL#-20.0)
    AND (EstimatedBias < LREAL#20.0);
EstimateFrozen := AdaptationRequested AND NOT AdaptationActive;

IF NOT AdaptationRequested THEN
    FrozenReason := INT#1;
ELSIF NOT Enabled THEN
    FrozenReason := INT#2;
ELSIF NOT DataHealthy THEN
    FrozenReason := INT#3;
ELSIF PreviousSaturated THEN
    FrozenReason := INT#4;
ELSIF NOT ProbeEnabled THEN
    FrozenReason := INT#5;
ELSIF NOT PersistentExcitation THEN
    FrozenReason := INT#6;
ELSE
    FrozenReason := INT#0;
END_IF;

FallbackActive := (RequestedArchitecture = INT#2) AND NOT EstimateQualified;
IF RequestedArchitecture = INT#0 THEN
    ActiveArchitecture := INT#0;
ELSIF RequestedArchitecture = INT#1 THEN
    ActiveArchitecture := INT#1;
ELSIF EstimateQualified THEN
    ActiveArchitecture := INT#2;
ELSE
    ActiveArchitecture := INT#1;
END_IF;

IF ProbeEnabled AND Enabled AND DataHealthy THEN
    ProbeTimer := ProbeTimer + LREAL#0.1;
    IF ProbeTimer >= LREAL#2.0 THEN
        ProbeTimer := ProbeTimer - LREAL#2.0;
        ProbeSign := -ProbeSign;
    END_IF;
    Probe := LREAL#10.0 * ProbeSign;
ELSE
    ProbeTimer := LREAL#0.0;
    ProbeSign := LREAL#1.0;
    Probe := LREAL#0.0;
END_IF;

IF ActiveArchitecture = INT#0 THEN
    DesignGain := LREAL#1.0;
ELSIF ActiveArchitecture = INT#1 THEN
    DesignGain := Lesson76RecipeGain(ResinGrade := ResinGrade);
ELSE
    DesignGain := EstimatedGain;
END_IF;
ActiveProportionalGain := Lesson76ProportionalGain(DesignGain := DesignGain);
ActiveIntegralGain := Lesson76IntegralGain(DesignGain := DesignGain);

Parameters.ProportionalGain := ActiveProportionalGain;
Parameters.IntegralGain := ActiveIntegralGain;
Parameters.DerivativeGain := LREAL#0.0;
Parameters.ProportionalSetpointWeight := LREAL#1.0;
Parameters.DerivativeSetpointWeight := LREAL#0.0;
Parameters.Bias := LREAL#0.0;
Parameters.OutputMinimum := LREAL#0.0;
Parameters.OutputMaximum := LREAL#100.0;
Parameters.RateMinimum := LREAL#-1000.0;
Parameters.RateMaximum := LREAL#1000.0;
Parameters.TrackingTime := LREAL#0.5;
Parameters.AntiWindup := PIDAntiWindup#Combined;
Controller.Configure(Parameters := Parameters);

Input.Setpoint := PressureSetpoint;
Input.ProcessValue := DiePressure;
Input.FeedForward := Probe;
Input.CycleTime := LREAL#0.1;
IF Enabled AND DataHealthy THEN
    Input.Mode := PIDMode#Auto;
ELSE
    Input.Mode := PIDMode#Manual;
    Input.ManualOutput := LREAL#0.0;
END_IF;

IF NOT ControllerInitialized THEN
    InitialState.Setpoint := PressureSetpoint;
    InitialState.ProcessValue := DiePressure;
    InitialState.ProcessValueRate := LREAL#0.0;
    InitialState.FeedForward := Probe;
    InitialState.NominalOutput := LREAL#40.0;
    InitialState.SaturatedOutput := LREAL#40.0;
    Controller.Initialize(State := InitialState);
    ControllerInitialized := TRUE;
END_IF;

ScrewSpeedRequested := Controller.Update(Input := Input);
ScrewSpeedApplied := LIMIT(LREAL#0.0, ScrewSpeedRequested, LREAL#100.0);
OutputSaturated := Enabled AND DataHealthy
    AND ((ScrewSpeedApplied <= LREAL#1.0E-12
            AND PressureSetpoint < DiePressure)
        OR (ScrewSpeedApplied >= LREAL#99.999999999999
            AND PressureSetpoint > DiePressure));

PreviousPressure := DiePressure;
PreviousApplied := ScrewSpeedApplied;
PreviousSaturated := OutputSaturated;
END_FUNCTION_BLOCK
