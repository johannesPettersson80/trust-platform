USING Control;

FUNCTION Lesson56ControlAvailable : BOOL
VAR_INPUT
    SchedulingAvailable : BOOL;
    InputsValid : BOOL;
    SafetyAbort : BOOL;
END_VAR
Lesson56ControlAvailable := SchedulingAvailable AND InputsValid
    AND (NOT SafetyAbort);
END_FUNCTION

FUNCTION Lesson56EquilibriumFlow : LREAL
VAR_INPUT LevelMetres : LREAL; END_VAR
Lesson56EquilibriumFlow := LREAL#0.020 * SQRT(LevelMetres);
END_FUNCTION

FUNCTION Lesson56LocalGain : LREAL
VAR_INPUT LevelMetres : LREAL; END_VAR
Lesson56LocalGain := LREAL#2.0 * SQRT(LevelMetres) / LREAL#0.020;
END_FUNCTION

FUNCTION Lesson56LocalIntegralGain : LREAL
VAR_INPUT LevelMetres : LREAL; END_VAR
(* With A=.5 and lambda=10 s, Kp=A/lambda=.05. *)
Lesson56LocalIntegralGain := LREAL#0.020
    / (LREAL#20.0 * SQRT(LevelMetres));
END_FUNCTION

FUNCTION Lesson56ReviewedLevelSetpoint : LREAL
VAR_INPUT
    ExperimentElapsedSeconds : LREAL;
    ExperimentEnabled : BOOL;
    InputsValid : BOOL;
    SafetyAbort : BOOL;
END_VAR
IF ExperimentEnabled AND InputsValid AND (NOT SafetyAbort) THEN
    IF ExperimentElapsedSeconds < LREAL#30.0 THEN
        Lesson56ReviewedLevelSetpoint := LREAL#0.16;
    ELSIF ExperimentElapsedSeconds < LREAL#120.0 THEN
        Lesson56ReviewedLevelSetpoint := LREAL#0.25;
    ELSIF ExperimentElapsedSeconds < LREAL#240.0 THEN
        Lesson56ReviewedLevelSetpoint := LREAL#0.36;
    ELSIF ExperimentElapsedSeconds < LREAL#330.0 THEN
        Lesson56ReviewedLevelSetpoint := LREAL#0.25;
    ELSE
        Lesson56ReviewedLevelSetpoint := LREAL#0.16;
    END_IF;
ELSE
    Lesson56ReviewedLevelSetpoint := LREAL#0.16;
END_IF;
END_FUNCTION

PROGRAM Main
VAR
    SchedulingAvailable : BOOL := TRUE;
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ExperimentElapsed : LREAL := 0.0;
    LevelSetpoint : LREAL := 0.16;
    FixedLevel : LREAL := 0.16;
    InterpolatedLevel : LREAL := 0.25;
    ParallelLevel : LREAL := 0.36;
    FixedPumpRequest : LREAL := 0.008;
    InterpolatedPumpRequest : LREAL := 0.010;
    ParallelPumpRequest : LREAL := 0.012;
    FilteredSchedulingLevel : LREAL := 0.16;
    InterpolationFraction : LREAL := 0.0;
    ActiveKp : LREAL := 0.05;
    ActiveKi : LREAL := 0.0025;
    ParallelRegionValue : LREAL := 0.0;
    ParallelTransitionCount : LREAL := 0.0;
    Controller : GainScheduledTankController;
    AutomaticMode : BOOL := TRUE;
    ControlAvailable : BOOL := TRUE;
    Initialized : BOOL := FALSE;
END_VAR

IF NOT Initialized THEN
    LevelSetpoint := LREAL#0.16;
    Initialized := TRUE;
END_IF;

(* Control Studio grants only authority. This project owns the complete
   low-middle-high-middle-low sequence and restoration. *)
IF ExperimentEnabled AND InputsValid AND NOT SafetyAbort THEN
    ExperimentElapsed := ExperimentElapsed + LREAL#1.0;
ELSE
    ExperimentElapsed := LREAL#0.0;
END_IF;
LevelSetpoint := Lesson56ReviewedLevelSetpoint(
    ExperimentElapsedSeconds := ExperimentElapsed,
    ExperimentEnabled := ExperimentEnabled,
    InputsValid := InputsValid,
    SafetyAbort := SafetyAbort);
ControlAvailable := Lesson56ControlAvailable(
    SchedulingAvailable := SchedulingAvailable,
    InputsValid := InputsValid,
    SafetyAbort := SafetyAbort);

Controller(
    SchedulingAvailable := ControlAvailable,
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
AutomaticMode := ControlAvailable;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#1s, PRIORITY := 1);
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
