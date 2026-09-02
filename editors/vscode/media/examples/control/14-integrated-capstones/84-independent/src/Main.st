USING Control;

PROGRAM Main
VAR
    Enabled : BOOL := FALSE;
    OperatingMode : DINT := DINT#1;
    PressureQualityGood : BOOL := TRUE;
    DemandQualityGood : BOOL := TRUE;
    DeliveryQualityGood : BOOL := TRUE;
    CompressorAvailable : BOOL := TRUE;
    PressureSetpoint : LREAL := 7.0;
    MeasuredDemand : LREAL := 50.0;
    ManualFeedbackRequest : LREAL := 0.0;
    ManualIntegratedRequest : LREAL := 0.0;
    CycleTime : LREAL := 1.0;
    FeedbackPressure : LREAL := 7.0;
    IntegratedPressure : LREAL := 7.0;
    FeedbackActualDelivery : LREAL := 50.0;
    IntegratedActualDelivery : LREAL := 50.0;
    Initialized : BOOL := FALSE;
    FeedforwardQualified : BOOL := TRUE;
    TrackingActive : BOOL := TRUE;
    FaultActive : BOOL := FALSE;
    TimingValid : BOOL := TRUE;
    FeedbackRequested : LREAL := 50.0;
    IntegratedFeedback : LREAL := 50.0;
    IntegratedFeedforward : LREAL := 0.0;
    IntegratedRequested : LREAL := 50.0;
    FeedbackApplied : LREAL := 0.0;
    IntegratedApplied : LREAL := 0.0;
    PreviousFeedbackApplied : LREAL := 50.0;
    PreviousIntegratedApplied : LREAL := 50.0;
    FeedbackPlantDemand : LREAL := 50.0;
    IntegratedPlantDemand : LREAL := 50.0;
    ControllerRevision : DINT := DINT#84;
    Controller : CompressedAirController;
    StartupInitialized : BOOL := FALSE;
END_VAR

// Start the simulated plant in its reviewed automatic production state.
IF NOT StartupInitialized THEN
    Enabled := TRUE;
    OperatingMode := DINT#1;
    PressureSetpoint := LREAL#7.0;
    StartupInitialized := TRUE;
END_IF;

Controller(
    Enabled := Enabled,
    OperatingMode := OperatingMode,
    PressureQualityGood := PressureQualityGood,
    DemandQualityGood := DemandQualityGood,
    DeliveryQualityGood := DeliveryQualityGood,
    CompressorAvailable := CompressorAvailable,
    PressureSetpoint := PressureSetpoint,
    MeasuredDemand := MeasuredDemand,
    ManualFeedbackRequest := ManualFeedbackRequest,
    ManualIntegratedRequest := ManualIntegratedRequest,
    CycleTime := CycleTime,
    FeedbackPressure := FeedbackPressure,
    IntegratedPressure := IntegratedPressure,
    FeedbackActualDelivery := FeedbackActualDelivery,
    IntegratedActualDelivery := IntegratedActualDelivery
);
Initialized := Controller.Initialized;
FeedforwardQualified := Controller.FeedforwardQualified;
TrackingActive := Controller.TrackingActive;
FaultActive := Controller.FaultActive;
TimingValid := Controller.TimingValid;
FeedbackRequested := Controller.FeedbackRequested;
IntegratedFeedback := Controller.IntegratedFeedback;
IntegratedFeedforward := Controller.IntegratedFeedforward;
IntegratedRequested := Controller.IntegratedRequested;
FeedbackApplied := Controller.FeedbackApplied;
IntegratedApplied := Controller.IntegratedApplied;
PreviousFeedbackApplied := Controller.PreviousFeedbackApplied;
PreviousIntegratedApplied := Controller.PreviousIntegratedApplied;
FeedbackPlantDemand := Controller.FeedbackPlantDemand;
IntegratedPlantDemand := Controller.IntegratedPlantDemand;
ControllerRevision := Controller.ControllerRevision;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#1s, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.OperatingMode AT %MD0 : DINT;
    P1.Enabled AT %MX8.0 : BOOL;
    P1.PressureQualityGood AT %MX8.1 : BOOL;
    P1.DemandQualityGood AT %MX8.2 : BOOL;
    P1.DeliveryQualityGood AT %MX8.3 : BOOL;
    P1.CompressorAvailable AT %MX8.4 : BOOL;
    P1.FeedforwardQualified AT %MX8.5 : BOOL;
    P1.TrackingActive AT %MX8.6 : BOOL;
    P1.FaultActive AT %MX8.7 : BOOL;
    P1.Initialized AT %MX9.0 : BOOL;
    P1.TimingValid AT %MX9.1 : BOOL;
    P1.PressureSetpoint AT %ML16 : LREAL;
    P1.MeasuredDemand AT %ML24 : LREAL;
    P1.ManualFeedbackRequest AT %ML32 : LREAL;
    P1.ManualIntegratedRequest AT %ML40 : LREAL;
    P1.FeedbackRequested AT %ML48 : LREAL;
    P1.IntegratedFeedback AT %ML56 : LREAL;
    P1.IntegratedFeedforward AT %ML64 : LREAL;
    P1.IntegratedRequested AT %ML72 : LREAL;
    P1.PreviousFeedbackApplied AT %ML80 : LREAL;
    P1.PreviousIntegratedApplied AT %ML88 : LREAL;
    P1.ControllerRevision AT %MD96 : DINT;
    P1.CycleTime AT %ML104 : LREAL;
    P1.FeedbackApplied AT %QL0 : LREAL;
    P1.FeedbackPlantDemand AT %QL8 : LREAL;
    P1.IntegratedApplied AT %QL16 : LREAL;
    P1.IntegratedPlantDemand AT %QL24 : LREAL;
    P1.FeedbackPressure AT %IL0 : LREAL;
    P1.FeedbackActualDelivery AT %IL8 : LREAL;
    P1.IntegratedPressure AT %IL16 : LREAL;
    P1.IntegratedActualDelivery AT %IL24 : LREAL;
END_VAR
END_CONFIGURATION
