USING Control;

PROGRAM Main
VAR
    RequestedArchitecture : INT := 0;
    ActiveArchitecture : INT := 0;
    ResinGrade : INT := 0;
    Enabled : BOOL := FALSE;
    AdaptationRequested : BOOL := FALSE;
    DataHealthy : BOOL := TRUE;
    ProbeEnabled : BOOL := FALSE;
    AdaptationActive : BOOL := FALSE;
    EstimateFrozen : BOOL := TRUE;
    PersistentExcitation : BOOL := FALSE;
    EstimateQualified : BOOL := FALSE;
    FallbackActive : BOOL := FALSE;
    ProjectionActive : BOOL := FALSE;
    OutputSaturated : BOOL := FALSE;
    EstimatorInitialized : BOOL := FALSE;
    PressureSetpoint : LREAL := 40.0;
    DiePressure : LREAL := 40.0;
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
    Controller : AdaptiveExtruderController;
    StartupInitialized : BOOL := FALSE;
END_VAR

// Start at the declared production operating point before testing adaptation.
IF NOT StartupInitialized THEN
    Enabled := TRUE;
    PressureSetpoint := LREAL#40.0;
    StartupInitialized := TRUE;
END_IF;

Controller(
    RequestedArchitecture := RequestedArchitecture,
    ResinGrade := ResinGrade,
    Enabled := Enabled,
    AdaptationRequested := AdaptationRequested,
    DataHealthy := DataHealthy,
    ProbeEnabled := ProbeEnabled,
    PressureSetpoint := PressureSetpoint,
    DiePressure := DiePressure,
    InitialEstimatedGain := EstimatedGain,
    InitialEstimatedBias := EstimatedBias
);
ActiveArchitecture := Controller.ActiveArchitecture;
AdaptationActive := Controller.AdaptationActive;
EstimateFrozen := Controller.EstimateFrozen;
PersistentExcitation := Controller.PersistentExcitation;
EstimateQualified := Controller.EstimateQualified;
FallbackActive := Controller.FallbackActive;
ProjectionActive := Controller.ProjectionActive;
OutputSaturated := Controller.OutputSaturated;
EstimatorInitialized := Controller.EstimatorInitialized;
EstimatedGain := Controller.EstimatedGain;
EstimatedBias := Controller.EstimatedBias;
Covariance11 := Controller.Covariance11;
Covariance12 := Controller.Covariance12;
Covariance22 := Controller.Covariance22;
Information11 := Controller.Information11;
Information12 := Controller.Information12;
Information22 := Controller.Information22;
CovarianceTrace := Controller.CovarianceTrace;
InformationDeterminant := Controller.InformationDeterminant;
Innovation := Controller.Innovation;
ActiveProportionalGain := Controller.ActiveProportionalGain;
ActiveIntegralGain := Controller.ActiveIntegralGain;
Probe := Controller.Probe;
ScrewSpeedRequested := Controller.ScrewSpeedRequested;
ScrewSpeedApplied := Controller.ScrewSpeedApplied;
InformativeUpdateCount := Controller.InformativeUpdateCount;
FrozenReason := Controller.FrozenReason;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.RequestedArchitecture AT %MW0 : INT;
    P1.ActiveArchitecture AT %MW2 : INT;
    P1.ResinGrade AT %MW4 : INT;
    P1.Enabled AT %MX6.0 : BOOL;
    P1.AdaptationRequested AT %MX6.1 : BOOL;
    P1.DataHealthy AT %MX6.2 : BOOL;
    P1.ProbeEnabled AT %MX6.3 : BOOL;
    P1.AdaptationActive AT %MX7.0 : BOOL;
    P1.EstimateFrozen AT %MX7.1 : BOOL;
    P1.PersistentExcitation AT %MX7.2 : BOOL;
    P1.EstimateQualified AT %MX7.3 : BOOL;
    P1.FallbackActive AT %MX7.4 : BOOL;
    P1.ProjectionActive AT %MX7.5 : BOOL;
    P1.OutputSaturated AT %MX7.6 : BOOL;
    P1.EstimatorInitialized AT %MX7.7 : BOOL;
    P1.PressureSetpoint AT %ML8 : LREAL;
    P1.EstimatedGain AT %ML16 : LREAL;
    P1.EstimatedBias AT %ML24 : LREAL;
    P1.Covariance11 AT %ML32 : LREAL;
    P1.Covariance12 AT %ML40 : LREAL;
    P1.Covariance22 AT %ML48 : LREAL;
    P1.Information11 AT %ML56 : LREAL;
    P1.Information12 AT %ML64 : LREAL;
    P1.Information22 AT %ML72 : LREAL;
    P1.CovarianceTrace AT %ML80 : LREAL;
    P1.InformationDeterminant AT %ML88 : LREAL;
    P1.Innovation AT %ML96 : LREAL;
    P1.ActiveProportionalGain AT %ML104 : LREAL;
    P1.ActiveIntegralGain AT %ML112 : LREAL;
    P1.Probe AT %ML120 : LREAL;
    P1.ScrewSpeedRequested AT %ML128 : LREAL;
    P1.InformativeUpdateCount AT %MD136 : DINT;
    P1.FrozenReason AT %MW140 : INT;
    P1.ScrewSpeedApplied AT %QL0 : LREAL;
    P1.DiePressure AT %IL0 : LREAL;
END_VAR
END_CONFIGURATION
