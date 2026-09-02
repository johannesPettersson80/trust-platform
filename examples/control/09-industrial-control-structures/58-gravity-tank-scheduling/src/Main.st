USING Control;

PROGRAM Main
VAR
    SchedulingAvailable : BOOL := TRUE;
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ExperimentElapsed : LREAL := 0.0;
    LevelSetpoint : LREAL := 0.30;
    FixedLevel : LREAL := 0.30;
    InterpolatedLevel : LREAL := 0.30;
    ParallelLevel : LREAL := 0.30;
    FixedPumpRequest : LREAL := 0.027386127875;
    InterpolatedPumpRequest : LREAL := 0.027386127875;
    ParallelPumpRequest : LREAL := 0.027386127875;
    FilteredSchedulingLevel : LREAL := 0.30;
    InterpolationFraction : LREAL := 0.0;
    ActiveKp : LREAL := 0.10;
    ActiveKi : LREAL := 0.00456;
    ParallelRegionValue : LREAL := 0.0;
    ParallelTransitionCount : LREAL := 0.0;
    Controller : GainScheduledTankController;
    AutomaticMode : BOOL := TRUE;
    Initialized : BOOL := FALSE;
END_VAR

IF NOT Initialized THEN
    LevelSetpoint := LREAL#0.30;
    Initialized := TRUE;
END_IF;

(* Control Studio grants only authority. This project owns the complete
   low-middle-high-middle-low sequence and restoration. *)
IF ExperimentEnabled AND InputsValid AND NOT SafetyAbort THEN
    ExperimentElapsed := ExperimentElapsed + LREAL#0.1;
    IF ExperimentElapsed < LREAL#30.0 THEN
        LevelSetpoint := LREAL#0.30;
    ELSIF ExperimentElapsed < LREAL#120.0 THEN
        LevelSetpoint := LREAL#0.55;
    ELSIF ExperimentElapsed < LREAL#240.0 THEN
        LevelSetpoint := LREAL#0.80;
    ELSIF ExperimentElapsed < LREAL#330.0 THEN
        LevelSetpoint := LREAL#0.55;
    ELSE
        LevelSetpoint := LREAL#0.30;
    END_IF;
ELSE
    ExperimentElapsed := LREAL#0.0;
    LevelSetpoint := LREAL#0.30;
END_IF;

Controller(
    SchedulingAvailable := SchedulingAvailable,
    LevelSetpoint := LevelSetpoint,
    FixedLevel := FixedLevel,
    InterpolatedLevel := InterpolatedLevel,
    ParallelLevel := ParallelLevel
);
FixedPumpRequest := Controller.FixedPumpRequest;
InterpolatedPumpRequest := Controller.InterpolatedPumpRequest;
ParallelPumpRequest := Controller.ParallelPumpRequest;
FilteredSchedulingLevel := Controller.FilteredSchedulingLevel;
InterpolationFraction := Controller.InterpolationFraction;
ActiveKp := Controller.ActiveKp;
ActiveKi := Controller.ActiveKi;
ParallelRegionValue := Controller.ParallelRegionValue;
ParallelTransitionCount := Controller.ParallelTransitionCount;
AutomaticMode := TRUE;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.SchedulingAvailable AT %MX89.1 : BOOL;
    P1.ExperimentEnabled AT %MX89.2 : BOOL;
    P1.SafetyAbort AT %MX89.3 : BOOL;
    P1.InputsValid AT %MX89.4 : BOOL;
    P1.AutomaticMode AT %MX272.0 : BOOL;
    P1.LevelSetpoint AT %ML216 : LREAL;
    P1.FilteredSchedulingLevel AT %ML224 : LREAL;
    P1.InterpolationFraction AT %ML232 : LREAL;
    P1.ActiveKp AT %ML240 : LREAL;
    P1.ActiveKi AT %ML248 : LREAL;
    P1.ParallelRegionValue AT %ML256 : LREAL;
    P1.ParallelTransitionCount AT %ML264 : LREAL;
    P1.FixedLevel AT %IL136 : LREAL;
    P1.InterpolatedLevel AT %IL144 : LREAL;
    P1.ParallelLevel AT %IL152 : LREAL;
    P1.FixedPumpRequest AT %QL152 : LREAL;
    P1.InterpolatedPumpRequest AT %QL160 : LREAL;
    P1.ParallelPumpRequest AT %QL168 : LREAL;
END_VAR
END_CONFIGURATION
