USING Control;

PROGRAM Main
VAR
    OperatingMode : DINT := DINT#1;
    AutoMode : BOOL := TRUE;
    ArchitectureRequested : INT := INT#0;
    ArchitectureActive : INT := INT#0;
    ArchitectureQualified : BOOL := TRUE;
    QualityGood : BOOL := TRUE;
    RefluxAvailable : BOOL := TRUE;
    ReboilerAvailable : BOOL := TRUE;
    OverheadSetpoint : LREAL := 0.0;
    BottomsSetpoint : LREAL := 0.0;
    FeedDisturbanceRequested : LREAL := 0.0;
    ManualReflux : LREAL := 0.0;
    ManualReboiler : LREAL := 0.0;
    OverheadProcessValue : LREAL := 0.0;
    BottomsProcessValue : LREAL := 0.0;
    VirtualOverheadCorrection : LREAL := 0.0;
    VirtualBottomsCorrection : LREAL := 0.0;
    RefluxRequested : LREAL := 0.0;
    ReboilerRequested : LREAL := 0.0;
    CommonScale : LREAL := 1.0;
    RefluxApplied : LREAL := 0.0;
    ReboilerApplied : LREAL := 0.0;
    FeedDisturbanceApplied : LREAL := 0.0;
    StaticQualified : BOOL := FALSE;
    Saturated : BOOL := FALSE;
    TrackingActive : BOOL := TRUE;
    FaultActive : BOOL := FALSE;
    ControllerRevision : DINT := DINT#82;
    Controller : DistillationController;
    StartupInitialized : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ExperimentScan : DINT := DINT#0;
    ExperimentElapsedSeconds : LREAL := 0.0;
END_VAR

IF NOT StartupInitialized THEN
    OperatingMode := DINT#1;
    OverheadSetpoint := LREAL#0.0;
    BottomsSetpoint := LREAL#0.0;
    StartupInitialized := TRUE;
END_IF;

(* Control Studio owns only this Boolean authority.  The project owns the
   complete restored architecture comparison and abnormal sequence. *)
IF ExperimentEnabled AND InputsValid AND NOT SafetyAbort THEN
    ExperimentScan := ExperimentScan + DINT#1;
    ExperimentElapsedSeconds := ExperimentElapsedSeconds + LREAL#0.1;
    OperatingMode := DINT#1;
    QualityGood := TRUE;
    RefluxAvailable := TRUE;
    ReboilerAvailable := TRUE;
    FeedDisturbanceRequested := LREAL#0.0;
    ManualReflux := LREAL#0.0;
    ManualReboiler := LREAL#0.0;
    IF ExperimentScan <= DINT#600 THEN
        ArchitectureRequested := INT#0;
        OverheadSetpoint := LREAL#1.0;
        BottomsSetpoint := LREAL#0.0;
    ELSIF ExperimentScan <= DINT#1200 THEN
        ArchitectureRequested := INT#0;
        OverheadSetpoint := LREAL#0.0;
        BottomsSetpoint := LREAL#1.0;
    ELSIF ExperimentScan <= DINT#2400 THEN
        ArchitectureRequested := INT#1;
        OverheadSetpoint := LREAL#1.0;
        BottomsSetpoint := LREAL#1.0;
    ELSIF ExperimentScan <= DINT#3600 THEN
        ArchitectureRequested := INT#2;
        OverheadSetpoint := LREAL#1.0;
        BottomsSetpoint := LREAL#1.0;
    ELSIF ExperimentScan <= DINT#4800 THEN
        ArchitectureRequested := INT#1;
        OverheadSetpoint := LREAL#0.0;
        BottomsSetpoint := LREAL#0.0;
        FeedDisturbanceRequested := LREAL#1.0;
    ELSIF ExperimentScan <= DINT#6000 THEN
        ArchitectureRequested := INT#2;
        OverheadSetpoint := LREAL#2.0;
        BottomsSetpoint := LREAL#2.0;
    ELSIF ExperimentScan <= DINT#6300 THEN
        ArchitectureRequested := INT#1;
        QualityGood := FALSE;
        OverheadSetpoint := LREAL#0.0;
        BottomsSetpoint := LREAL#0.0;
    ELSIF ExperimentScan <= DINT#6600 THEN
        ArchitectureRequested := INT#1;
        RefluxAvailable := FALSE;
        OverheadSetpoint := LREAL#0.0;
        BottomsSetpoint := LREAL#0.0;
    ELSIF ExperimentScan <= DINT#6900 THEN
        OperatingMode := DINT#0;
        ManualReflux := LREAL#0.0;
        ManualReboiler := LREAL#0.0;
        OverheadSetpoint := LREAL#0.0;
        BottomsSetpoint := LREAL#0.0;
    ELSE
        OperatingMode := DINT#2;
        OverheadSetpoint := LREAL#0.0;
        BottomsSetpoint := LREAL#0.0;
    END_IF;
ELSE
    OperatingMode := DINT#3;
    ArchitectureRequested := INT#0;
    QualityGood := TRUE;
    RefluxAvailable := TRUE;
    ReboilerAvailable := TRUE;
    OverheadSetpoint := LREAL#0.0;
    BottomsSetpoint := LREAL#0.0;
    FeedDisturbanceRequested := LREAL#0.0;
    ManualReflux := LREAL#0.0;
    ManualReboiler := LREAL#0.0;
    ExperimentScan := DINT#0;
    ExperimentElapsedSeconds := LREAL#0.0;
END_IF;

Controller(
    OperatingMode := OperatingMode,
    ArchitectureRequested := ArchitectureRequested,
    QualityGood := QualityGood,
    RefluxAvailable := RefluxAvailable,
    ReboilerAvailable := ReboilerAvailable,
    OverheadSetpoint := OverheadSetpoint,
    BottomsSetpoint := BottomsSetpoint,
    FeedDisturbanceRequested := FeedDisturbanceRequested,
    ManualReflux := ManualReflux,
    ManualReboiler := ManualReboiler,
    OverheadProcessValue := OverheadProcessValue,
    BottomsProcessValue := BottomsProcessValue
);
ArchitectureActive := Controller.ArchitectureActive;
ArchitectureQualified := Controller.ArchitectureQualified;
VirtualOverheadCorrection := Controller.VirtualOverheadCorrection;
VirtualBottomsCorrection := Controller.VirtualBottomsCorrection;
RefluxRequested := Controller.RefluxRequested;
ReboilerRequested := Controller.ReboilerRequested;
CommonScale := Controller.CommonScale;
RefluxApplied := Controller.RefluxApplied;
ReboilerApplied := Controller.ReboilerApplied;
FeedDisturbanceApplied := Controller.FeedDisturbanceApplied;
StaticQualified := Controller.StaticQualified;
Saturated := Controller.Saturated;
TrackingActive := Controller.TrackingActive;
FaultActive := Controller.FaultActive;
AutoMode := OperatingMode = DINT#1;
ControllerRevision := Controller.ControllerRevision;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.OperatingMode AT %MD16 : DINT;
    P1.ArchitectureRequested AT %MW20 : INT;
    P1.ArchitectureActive AT %MW22 : INT;
    P1.QualityGood AT %MX24.0 : BOOL;
    P1.RefluxAvailable AT %MX24.1 : BOOL;
    P1.ReboilerAvailable AT %MX24.2 : BOOL;
    P1.ArchitectureQualified AT %MX24.3 : BOOL;
    P1.OverheadSetpoint AT %ML32 : LREAL;
    P1.BottomsSetpoint AT %ML40 : LREAL;
    P1.FeedDisturbanceRequested AT %ML48 : LREAL;
    P1.VirtualOverheadCorrection AT %ML56 : LREAL;
    P1.VirtualBottomsCorrection AT %ML64 : LREAL;
    P1.RefluxRequested AT %ML72 : LREAL;
    P1.ReboilerRequested AT %ML80 : LREAL;
    P1.CommonScale AT %ML88 : LREAL;
    P1.ManualReflux AT %ML96 : LREAL;
    P1.ManualReboiler AT %ML104 : LREAL;
    P1.StaticQualified AT %MX24.4 : BOOL;
    P1.Saturated AT %MX24.5 : BOOL;
    P1.TrackingActive AT %MX24.6 : BOOL;
    P1.FaultActive AT %MX24.7 : BOOL;
    P1.ControllerRevision AT %MD112 : DINT;
    P1.AutoMode AT %MX116.0 : BOOL;
    P1.ExperimentEnabled AT %MX116.1 : BOOL;
    P1.SafetyAbort AT %MX116.2 : BOOL;
    P1.InputsValid AT %MX116.3 : BOOL;
    P1.ExperimentElapsedSeconds AT %ML120 : LREAL;
    P1.RefluxApplied AT %QL0 : LREAL;
    P1.ReboilerApplied AT %QL8 : LREAL;
    P1.FeedDisturbanceApplied AT %QL16 : LREAL;
    P1.OverheadProcessValue AT %IL0 : LREAL;
    P1.BottomsProcessValue AT %IL8 : LREAL;
END_VAR
END_CONFIGURATION
