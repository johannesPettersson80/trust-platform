USING Control;


PROGRAM Main
VAR
    FixedTank : TankController;
    ScheduledTank : TankController;
    Initialized : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ExperimentScan : DINT := DINT#0;
    ExperimentElapsed : LREAL := 0.0;
    OperatingMode : DINT := DINT#1;
    AutoMode : BOOL := TRUE;
    LevelSetpoint : LREAL := 0.30;
    ManualPumpFlow : LREAL := 0.027386127875;
    CycleTime : LREAL := 0.1;
    LevelQualityGood : BOOL := TRUE;
    SchedulingAvailable : BOOL := TRUE;
    PumpAvailable : BOOL := TRUE;
    FixedMeasuredLevel : LREAL := 0.30;
    ScheduledMeasuredLevel : LREAL := 0.30;
    FixedRequestedPump : LREAL := 0.027386127875;
    ScheduledRequestedPump : LREAL := 0.027386127875;
    FixedAppliedPump : LREAL := 0.027386127875;
    ScheduledAppliedPump : LREAL := 0.027386127875;
    ScheduledFilteredLevel : LREAL := 0.30;
    ScheduledFraction : LREAL := 0.0;
    ScheduledActiveKp : LREAL := 0.100;
    ScheduledActiveKi : LREAL := 0.00456;
    FixedTrackingActive : BOOL := TRUE;
    ScheduledTrackingActive : BOOL := TRUE;
    ScheduledScheduleValid : BOOL := TRUE;
    ScheduledScheduleClamped : BOOL := FALSE;
    FixedFaultActive : BOOL := FALSE;
    ScheduledFaultActive : BOOL := FALSE;
    ControllerRevision : DINT := DINT#80;
END_VAR

IF NOT Initialized THEN
    OperatingMode := DINT#1;
    LevelSetpoint := LREAL#0.30;
    FixedTank.Configure(
        ScheduledControl := FALSE,
        InitialLevel := FixedMeasuredLevel,
        InitialPumpFlow := LREAL#0.027386127875
    );
    ScheduledTank.Configure(
        ScheduledControl := TRUE,
        InitialLevel := ScheduledMeasuredLevel,
        InitialPumpFlow := LREAL#0.027386127875
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
    LevelQualityGood := TRUE;
    SchedulingAvailable := TRUE;
    PumpAvailable := TRUE;
    IF ExperimentScan <= DINT#300 THEN
        LevelSetpoint := LREAL#0.30;
    ELSIF ExperimentScan <= DINT#1200 THEN
        LevelSetpoint := LREAL#0.55;
    ELSIF ExperimentScan <= DINT#2400 THEN
        LevelSetpoint := LREAL#0.80;
    ELSIF ExperimentScan <= DINT#3300 THEN
        LevelSetpoint := LREAL#0.55;
    ELSE
        LevelSetpoint := LREAL#0.30;
    END_IF;
ELSIF ExperimentWasEnabled THEN
    ExperimentScan := DINT#0;
    ExperimentElapsed := LREAL#0.0;
    OperatingMode := DINT#0;
    LevelSetpoint := LREAL#0.30;
    LevelQualityGood := TRUE;
    SchedulingAvailable := TRUE;
    PumpAvailable := TRUE;
END_IF;

FixedTank.Update(
    OperatingMode := OperatingMode,
    RequestedLevel := LevelSetpoint,
    LevelMeasurement := FixedMeasuredLevel,
    LevelQualityGood := LevelQualityGood,
    SchedulingAvailable := SchedulingAvailable,
    PumpAvailable := PumpAvailable,
    ManualPumpFlow := ManualPumpFlow,
    CycleTime := CycleTime
);
ScheduledTank.Update(
    OperatingMode := OperatingMode,
    RequestedLevel := LevelSetpoint,
    LevelMeasurement := ScheduledMeasuredLevel,
    LevelQualityGood := LevelQualityGood,
    SchedulingAvailable := SchedulingAvailable,
    PumpAvailable := PumpAvailable,
    ManualPumpFlow := ManualPumpFlow,
    CycleTime := CycleTime
);

FixedRequestedPump := FixedTank.RequestedPumpFlow;
ScheduledRequestedPump := ScheduledTank.RequestedPumpFlow;
FixedAppliedPump := FixedTank.AppliedPumpFlow;
ScheduledAppliedPump := ScheduledTank.AppliedPumpFlow;
ScheduledFilteredLevel := ScheduledTank.FilteredSchedulingLevel;
ScheduledFraction := ScheduledTank.InterpolationFraction;
ScheduledActiveKp := ScheduledTank.ActiveKp;
ScheduledActiveKi := ScheduledTank.ActiveKi;
FixedTrackingActive := FixedTank.TrackingActive;
ScheduledTrackingActive := ScheduledTank.TrackingActive;
ScheduledScheduleValid := ScheduledTank.ScheduleValid;
ScheduledScheduleClamped := ScheduledTank.ScheduleClamped;
FixedFaultActive := FixedTank.FaultActive;
ScheduledFaultActive := ScheduledTank.FaultActive;
AutoMode := OperatingMode = DINT#1;
ControllerRevision := DINT#80;
ExperimentWasEnabled := ExperimentEnabled;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.OperatingMode AT %MD192 : DINT;
    P1.LevelSetpoint AT %ML200 : LREAL;
    P1.ManualPumpFlow AT %ML208 : LREAL;
    P1.ExperimentElapsed AT %ML216 : LREAL;
    P1.LevelQualityGood AT %MX240.0 : BOOL;
    P1.SchedulingAvailable AT %MX240.1 : BOOL;
    P1.PumpAvailable AT %MX240.2 : BOOL;
    P1.ExperimentEnabled AT %MX240.3 : BOOL;
    P1.SafetyAbort AT %MX240.4 : BOOL;
    P1.InputsValid AT %MX240.5 : BOOL;
    P1.FixedMeasuredLevel AT %IL0 : LREAL;
    P1.ScheduledMeasuredLevel AT %IL8 : LREAL;
    P1.FixedRequestedPump AT %ML0 : LREAL;
    P1.ScheduledRequestedPump AT %ML8 : LREAL;
    P1.ScheduledFilteredLevel AT %ML16 : LREAL;
    P1.ScheduledFraction AT %ML24 : LREAL;
    P1.ScheduledActiveKp AT %ML32 : LREAL;
    P1.ScheduledActiveKi AT %ML40 : LREAL;
    P1.FixedAppliedPump AT %QL0 : LREAL;
    P1.ScheduledAppliedPump AT %QL8 : LREAL;
    P1.ScheduledScheduleValid AT %MX48.2 : BOOL;
    P1.ScheduledScheduleClamped AT %MX48.3 : BOOL;
    P1.ControllerRevision AT %MD52 : DINT;
    P1.AutoMode AT %MX56.0 : BOOL;
END_VAR
END_CONFIGURATION
