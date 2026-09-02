USING Control;


PROGRAM Main
VAR
    FeedbackTrain : ReactorTrain;
    IntegratedTrain : ReactorTrain;
    Initialized : BOOL := FALSE;
    OperatingMode : DINT := DINT#1;
    ReactorSetpoint : LREAL := 80.0;
    JacketCommissioningSetpoint : LREAL := 80.0;
    FeedThermalLoad : LREAL := 0.0;
    ManualUtilityDemand : LREAL := 50.0;
    CycleTime : LREAL := 0.1;
    FeedLoadQualityGood : BOOL := TRUE;
    ReactorQualityGood : BOOL := TRUE;
    JacketQualityGood : BOOL := TRUE;
    HeatingAvailable : BOOL := TRUE;
    CoolingAvailable : BOOL := TRUE;
    FeedbackReactorTemperature : LREAL := 80.0;
    FeedbackJacketTemperature : LREAL := 80.0;
    IntegratedReactorTemperature : LREAL := 80.0;
    IntegratedJacketTemperature : LREAL := 80.0;
    FeedbackJacketSetpoint : LREAL := 80.0;
    FeedbackFeedforward : LREAL := 0.0;
    FeedbackNormalDemand : LREAL := 50.0;
    FeedbackProtectionDemand : LREAL := 100.0;
    FeedbackSelectedDemand : LREAL := 50.0;
    FeedbackHeatingRequested : LREAL := 50.0;
    FeedbackCoolingRequested : LREAL := 0.0;
    IntegratedJacketSetpoint : LREAL := 80.0;
    IntegratedFeedforward : LREAL := 0.0;
    IntegratedNormalDemand : LREAL := 50.0;
    IntegratedProtectionDemand : LREAL := 100.0;
    IntegratedSelectedDemand : LREAL := 50.0;
    IntegratedHeatingRequested : LREAL := 50.0;
    IntegratedCoolingRequested : LREAL := 0.0;
    FeedbackHeatingApplied : LREAL := 50.0;
    FeedbackCoolingApplied : LREAL := 0.0;
    FeedbackLoadApplied : LREAL := 0.0;
    IntegratedHeatingApplied : LREAL := 50.0;
    IntegratedCoolingApplied : LREAL := 0.0;
    IntegratedLoadApplied : LREAL := 0.0;
    FeedbackProtectionActive : BOOL := FALSE;
    IntegratedProtectionActive : BOOL := FALSE;
    FeedbackTrackingActive : BOOL := TRUE;
    IntegratedTrackingActive : BOOL := TRUE;
    AuthorityFault : BOOL := FALSE;
    FeedbackAuthority : INT := 1;
    IntegratedAuthority : INT := 1;
    ArchitectureRevision : DINT := DINT#79;
    ControllerAuto : BOOL := FALSE;
END_VAR

IF NOT Initialized THEN
    FeedbackTrain.Configure(
        MeasuredFeedforwardGain := LREAL#0.0,
        InitialReactorTemperature := FeedbackReactorTemperature,
        InitialJacketTemperature := FeedbackJacketTemperature
    );
    IntegratedTrain.Configure(
        MeasuredFeedforwardGain := LREAL#0.525,
        InitialReactorTemperature := IntegratedReactorTemperature,
        InitialJacketTemperature := IntegratedJacketTemperature
    );
    Initialized := TRUE;
END_IF;

FeedbackTrain.Update(
    OperatingMode := OperatingMode,
    ReactorSetpoint := ReactorSetpoint,
    JacketCommissioningSetpoint := JacketCommissioningSetpoint,
    MeasuredReactorTemperature := FeedbackReactorTemperature,
    MeasuredJacketTemperature := FeedbackJacketTemperature,
    FeedThermalLoad := FeedThermalLoad,
    FeedLoadQualityGood := FeedLoadQualityGood,
    ReactorQualityGood := ReactorQualityGood,
    JacketQualityGood := JacketQualityGood,
    HeatingAvailable := HeatingAvailable,
    CoolingAvailable := CoolingAvailable,
    ManualUtilityDemand := ManualUtilityDemand,
    CycleTime := CycleTime
);
IntegratedTrain.Update(
    OperatingMode := OperatingMode,
    ReactorSetpoint := ReactorSetpoint,
    JacketCommissioningSetpoint := JacketCommissioningSetpoint,
    MeasuredReactorTemperature := IntegratedReactorTemperature,
    MeasuredJacketTemperature := IntegratedJacketTemperature,
    FeedThermalLoad := FeedThermalLoad,
    FeedLoadQualityGood := FeedLoadQualityGood,
    ReactorQualityGood := ReactorQualityGood,
    JacketQualityGood := JacketQualityGood,
    HeatingAvailable := HeatingAvailable,
    CoolingAvailable := CoolingAvailable,
    ManualUtilityDemand := ManualUtilityDemand,
    CycleTime := CycleTime
);

FeedbackJacketSetpoint := FeedbackTrain.JacketSetpoint;
FeedbackFeedforward := FeedbackTrain.FeedforwardContribution;
FeedbackNormalDemand := FeedbackTrain.NormalUtilityDemand;
FeedbackProtectionDemand := FeedbackTrain.ProtectionDemand;
FeedbackSelectedDemand := FeedbackTrain.SelectedUtilityDemand;
FeedbackHeatingRequested := FeedbackTrain.HeatingRequested;
FeedbackCoolingRequested := FeedbackTrain.CoolingRequested;
FeedbackHeatingApplied := FeedbackTrain.HeatingApplied;
FeedbackCoolingApplied := FeedbackTrain.CoolingApplied;
FeedbackProtectionActive := FeedbackTrain.ProtectionActive;
FeedbackTrackingActive := FeedbackTrain.TrackingActive;
FeedbackAuthority := FeedbackTrain.ActiveAuthority;

IntegratedJacketSetpoint := IntegratedTrain.JacketSetpoint;
IntegratedFeedforward := IntegratedTrain.FeedforwardContribution;
IntegratedNormalDemand := IntegratedTrain.NormalUtilityDemand;
IntegratedProtectionDemand := IntegratedTrain.ProtectionDemand;
IntegratedSelectedDemand := IntegratedTrain.SelectedUtilityDemand;
IntegratedHeatingRequested := IntegratedTrain.HeatingRequested;
IntegratedCoolingRequested := IntegratedTrain.CoolingRequested;
IntegratedHeatingApplied := IntegratedTrain.HeatingApplied;
IntegratedCoolingApplied := IntegratedTrain.CoolingApplied;
IntegratedProtectionActive := IntegratedTrain.ProtectionActive;
IntegratedTrackingActive := IntegratedTrain.TrackingActive;
IntegratedAuthority := IntegratedTrain.ActiveAuthority;

AuthorityFault := FeedbackTrain.AuthorityFault OR IntegratedTrain.AuthorityFault;
IF AuthorityFault THEN
    (* The two trains share one physical utility station. A loss of authority in
       either candidate therefore makes every exported actuator command safe. *)
    FeedbackHeatingApplied := LREAL#0.0;
    FeedbackCoolingApplied := LREAL#0.0;
    IntegratedHeatingApplied := LREAL#0.0;
    IntegratedCoolingApplied := LREAL#0.0;
END_IF;
FeedbackLoadApplied := FeedThermalLoad;
IntegratedLoadApplied := FeedThermalLoad;
ArchitectureRevision := DINT#79;
(* Control Studio consumes an explicit Boolean mode signal, not this lesson's numeric mode encoding. *)
ControllerAuto := OperatingMode = DINT#2;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.OperatingMode AT %MD192 : DINT;
    P1.ReactorSetpoint AT %ML200 : LREAL;
    P1.JacketCommissioningSetpoint AT %ML208 : LREAL;
    P1.FeedThermalLoad AT %ML216 : LREAL;
    P1.ManualUtilityDemand AT %ML224 : LREAL;
    P1.FeedLoadQualityGood AT %MX240.0 : BOOL;
    P1.ReactorQualityGood AT %MX240.1 : BOOL;
    P1.JacketQualityGood AT %MX240.2 : BOOL;
    P1.HeatingAvailable AT %MX240.3 : BOOL;
    P1.CoolingAvailable AT %MX240.4 : BOOL;
    P1.FeedbackReactorTemperature AT %IL0 : LREAL;
    P1.FeedbackJacketTemperature AT %IL8 : LREAL;
    P1.IntegratedReactorTemperature AT %IL16 : LREAL;
    P1.IntegratedJacketTemperature AT %IL24 : LREAL;
    P1.FeedbackJacketSetpoint AT %ML0 : LREAL;
    P1.FeedbackFeedforward AT %ML8 : LREAL;
    P1.FeedbackSelectedDemand AT %ML32 : LREAL;
    P1.IntegratedJacketSetpoint AT %ML56 : LREAL;
    P1.IntegratedFeedforward AT %ML64 : LREAL;
    P1.IntegratedSelectedDemand AT %ML88 : LREAL;
    P1.FeedbackHeatingApplied AT %QL0 : LREAL;
    P1.FeedbackCoolingApplied AT %QL8 : LREAL;
    P1.FeedbackLoadApplied AT %QL16 : LREAL;
    P1.IntegratedHeatingApplied AT %QL24 : LREAL;
    P1.IntegratedCoolingApplied AT %QL32 : LREAL;
    P1.IntegratedLoadApplied AT %QL40 : LREAL;
    P1.ArchitectureRevision AT %MD120 : DINT;
    P1.ControllerAuto AT %MX124.0 : BOOL;
END_VAR
END_CONFIGURATION
