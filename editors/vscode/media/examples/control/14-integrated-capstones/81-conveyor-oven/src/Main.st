USING Control;


PROGRAM Main
VAR
    OrdinaryController : OvenController;
    FeedforwardController : OvenController;
    IntegratedController : OvenController;
    Initialized : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ExperimentScan : DINT := DINT#0;
    ExperimentElapsed : LREAL := 0.0;
    OperatingMode : DINT := DINT#1;
    AutoMode : BOOL := TRUE;
    TemperatureSetpoint : LREAL := 20.0;
    UpstreamTemperatureDeviation : LREAL := 0.0;
    ManualHeaterPower : LREAL := 0.0;
    CycleTime : LREAL := 0.1;
    UpstreamQualityGood : BOOL := TRUE;
    TemperatureQualityGood : BOOL := TRUE;
    ModelValid : BOOL := TRUE;
    HeaterAvailable : BOOL := TRUE;
    OrdinaryMeasuredTemperature : LREAL := 20.0;
    FeedforwardMeasuredTemperature : LREAL := 20.0;
    IntegratedMeasuredTemperature : LREAL := 20.0;
    OrdinaryRequestedHeater : LREAL := 0.0;
    FeedforwardRequestedHeater : LREAL := 0.0;
    IntegratedRequestedHeater : LREAL := 0.0;
    OrdinaryAppliedHeater : LREAL := 0.0;
    FeedforwardAppliedHeater : LREAL := 0.0;
    IntegratedAppliedHeater : LREAL := 0.0;
    OrdinaryThermalInput : LREAL := 0.0;
    FeedforwardThermalInput : LREAL := 0.0;
    IntegratedThermalInput : LREAL := 0.0;
    FeedforwardContribution : LREAL := 0.0;
    IntegratedFeedforwardContribution : LREAL := 0.0;
    IntegratedPredictedTemperature : LREAL := 20.0;
    IntegratedDelayFreeModel : LREAL := 20.0;
    IntegratedDelayedModel : LREAL := 20.0;
    IntegratedModelCorrection : LREAL := 0.0;
    PredictorHistoryComplete : BOOL := FALSE;
    PredictorActive : BOOL := FALSE;
    PredictorFallback : BOOL := FALSE;
    IntegratedTrackingActive : BOOL := TRUE;
    IntegratedFaultActive : BOOL := FALSE;
    ControllerRevision : DINT := DINT#81;
END_VAR

IF NOT Initialized THEN
    OperatingMode := DINT#1;
    TemperatureSetpoint := LREAL#20.0;
    OrdinaryController.Configure(
        UseFeedforward := FALSE,
        UsePrediction := FALSE,
        InitialTemperature := OrdinaryMeasuredTemperature
    );
    FeedforwardController.Configure(
        UseFeedforward := TRUE,
        UsePrediction := FALSE,
        InitialTemperature := FeedforwardMeasuredTemperature
    );
    IntegratedController.Configure(
        UseFeedforward := TRUE,
        UsePrediction := TRUE,
        InitialTemperature := IntegratedMeasuredTemperature
    );
    Initialized := TRUE;
END_IF;

IF ExperimentEnabled AND InputsValid AND NOT SafetyAbort THEN
    IF NOT ExperimentWasEnabled THEN
        ExperimentScan := DINT#0;
        ExperimentElapsed := LREAL#0.0;
    END_IF;
    ExperimentScan := ExperimentScan + DINT#1;
    ExperimentElapsed := ExperimentElapsed + LREAL#0.1;
    OperatingMode := DINT#1;
    UpstreamQualityGood := TRUE;
    TemperatureQualityGood := TRUE;
    ModelValid := TRUE;
    HeaterAvailable := TRUE;
    IF ExperimentScan <= DINT#100 THEN
        TemperatureSetpoint := LREAL#20.0;
    ELSE
        TemperatureSetpoint := LREAL#35.0;
    END_IF;
    IF ExperimentScan <= DINT#540 THEN
        UpstreamTemperatureDeviation := LREAL#0.0;
    ELSE
        UpstreamTemperatureDeviation := LREAL#-2.0;
    END_IF;
ELSIF ExperimentWasEnabled THEN
    ExperimentScan := DINT#0;
    ExperimentElapsed := LREAL#0.0;
    OperatingMode := DINT#0;
    TemperatureSetpoint := LREAL#20.0;
    UpstreamTemperatureDeviation := LREAL#0.0;
    UpstreamQualityGood := TRUE;
    TemperatureQualityGood := TRUE;
    ModelValid := TRUE;
    HeaterAvailable := TRUE;
END_IF;

OrdinaryController.Update(
    OperatingMode := OperatingMode,
    RequestedTemperature := TemperatureSetpoint,
    TemperatureMeasurement := OrdinaryMeasuredTemperature,
    UpstreamTemperatureDeviation := UpstreamTemperatureDeviation,
    UpstreamQualityGood := UpstreamQualityGood,
    TemperatureQualityGood := TemperatureQualityGood,
    ModelValid := ModelValid,
    HeaterAvailable := HeaterAvailable,
    ManualHeaterPower := ManualHeaterPower,
    CycleTime := CycleTime
);
FeedforwardController.Update(
    OperatingMode := OperatingMode,
    RequestedTemperature := TemperatureSetpoint,
    TemperatureMeasurement := FeedforwardMeasuredTemperature,
    UpstreamTemperatureDeviation := UpstreamTemperatureDeviation,
    UpstreamQualityGood := UpstreamQualityGood,
    TemperatureQualityGood := TemperatureQualityGood,
    ModelValid := ModelValid,
    HeaterAvailable := HeaterAvailable,
    ManualHeaterPower := ManualHeaterPower,
    CycleTime := CycleTime
);
IntegratedController.Update(
    OperatingMode := OperatingMode,
    RequestedTemperature := TemperatureSetpoint,
    TemperatureMeasurement := IntegratedMeasuredTemperature,
    UpstreamTemperatureDeviation := UpstreamTemperatureDeviation,
    UpstreamQualityGood := UpstreamQualityGood,
    TemperatureQualityGood := TemperatureQualityGood,
    ModelValid := ModelValid,
    HeaterAvailable := HeaterAvailable,
    ManualHeaterPower := ManualHeaterPower,
    CycleTime := CycleTime
);

OrdinaryRequestedHeater := OrdinaryController.RequestedHeaterPower;
FeedforwardRequestedHeater := FeedforwardController.RequestedHeaterPower;
IntegratedRequestedHeater := IntegratedController.RequestedHeaterPower;
OrdinaryAppliedHeater := OrdinaryController.AppliedHeaterPower;
FeedforwardAppliedHeater := FeedforwardController.AppliedHeaterPower;
IntegratedAppliedHeater := IntegratedController.AppliedHeaterPower;
(* The upstream load is plant truth, not controller authority. Both it and
   compensating heater action traverse the same delayed thermal path. *)
OrdinaryThermalInput := OrdinaryAppliedHeater
    + LREAL#1.25 * UpstreamTemperatureDeviation;
FeedforwardThermalInput := FeedforwardAppliedHeater
    + LREAL#1.25 * UpstreamTemperatureDeviation;
IntegratedThermalInput := IntegratedAppliedHeater
    + LREAL#1.25 * UpstreamTemperatureDeviation;
FeedforwardContribution := FeedforwardController.UpstreamCompensation;
IntegratedFeedforwardContribution := IntegratedController.UpstreamCompensation;
IntegratedPredictedTemperature := IntegratedController.PredictedTemperature;
IntegratedDelayFreeModel := IntegratedController.DelayFreeModel;
IntegratedDelayedModel := IntegratedController.DelayedModel;
IntegratedModelCorrection := IntegratedController.ModelCorrection;
PredictorHistoryComplete := IntegratedController.HistoryComplete;
PredictorActive := IntegratedController.PredictionActive;
PredictorFallback := IntegratedController.PredictionFallback;
IntegratedTrackingActive := IntegratedController.TrackingActive;
IntegratedFaultActive := IntegratedController.FaultActive;
AutoMode := OperatingMode = DINT#1;
ControllerRevision := DINT#81;
ExperimentWasEnabled := ExperimentEnabled;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.OperatingMode AT %MD192 : DINT;
    P1.TemperatureSetpoint AT %ML200 : LREAL;
    P1.UpstreamTemperatureDeviation AT %ML208 : LREAL;
    P1.ManualHeaterPower AT %ML216 : LREAL;
    P1.ExperimentElapsed AT %ML224 : LREAL;
    P1.UpstreamQualityGood AT %MX240.0 : BOOL;
    P1.TemperatureQualityGood AT %MX240.1 : BOOL;
    P1.ModelValid AT %MX240.2 : BOOL;
    P1.HeaterAvailable AT %MX240.3 : BOOL;
    P1.ExperimentEnabled AT %MX240.4 : BOOL;
    P1.SafetyAbort AT %MX240.5 : BOOL;
    P1.InputsValid AT %MX240.6 : BOOL;
    P1.OrdinaryMeasuredTemperature AT %IL0 : LREAL;
    P1.FeedforwardMeasuredTemperature AT %IL8 : LREAL;
    P1.IntegratedMeasuredTemperature AT %IL16 : LREAL;
    P1.OrdinaryRequestedHeater AT %ML0 : LREAL;
    P1.FeedforwardRequestedHeater AT %ML8 : LREAL;
    P1.IntegratedRequestedHeater AT %ML16 : LREAL;
    P1.FeedforwardContribution AT %ML24 : LREAL;
    P1.IntegratedFeedforwardContribution AT %ML32 : LREAL;
    P1.IntegratedPredictedTemperature AT %ML40 : LREAL;
    P1.IntegratedDelayFreeModel AT %ML48 : LREAL;
    P1.IntegratedDelayedModel AT %ML56 : LREAL;
    P1.IntegratedModelCorrection AT %ML64 : LREAL;
    P1.OrdinaryAppliedHeater AT %QL0 : LREAL;
    P1.FeedforwardAppliedHeater AT %QL8 : LREAL;
    P1.IntegratedAppliedHeater AT %QL16 : LREAL;
    P1.OrdinaryThermalInput AT %QL24 : LREAL;
    P1.FeedforwardThermalInput AT %QL32 : LREAL;
    P1.IntegratedThermalInput AT %QL40 : LREAL;
    P1.PredictorHistoryComplete AT %MX72.0 : BOOL;
    P1.PredictorActive AT %MX72.1 : BOOL;
    P1.PredictorFallback AT %MX72.2 : BOOL;
    P1.IntegratedTrackingActive AT %MX72.3 : BOOL;
    P1.IntegratedFaultActive AT %MX72.4 : BOOL;
    P1.ControllerRevision AT %MD76 : DINT;
    P1.AutoMode AT %MX80.0 : BOOL;
END_VAR
END_CONFIGURATION
