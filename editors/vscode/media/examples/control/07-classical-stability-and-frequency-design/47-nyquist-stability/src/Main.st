FUNCTION Lesson47GainCrossover : LREAL
VAR_INPUT
    ControllerGain : LREAL;
    PlantGain : LREAL;
    TimeConstantSeconds : LREAL;
END_VAR
Lesson47GainCrossover := SQRT(
    ControllerGain * ControllerGain * PlantGain * PlantGain - LREAL#1.0
) / TimeConstantSeconds;
END_FUNCTION

FUNCTION Lesson47CrossoverPhase : LREAL
VAR_INPUT
    AngularFrequency : LREAL;
    TimeConstantSeconds : LREAL;
    DelaySeconds : LREAL;
END_VAR
Lesson47CrossoverPhase := -ATAN(TimeConstantSeconds * AngularFrequency)
    - DelaySeconds * AngularFrequency;
END_FUNCTION

FUNCTION Lesson47ProportionalEquilibrium : LREAL
VAR_INPUT
    BaselineTemperature : LREAL;
    Setpoint : LREAL;
    ControllerGain : LREAL;
    PlantGain : LREAL;
END_VAR
Lesson47ProportionalEquilibrium :=
    (BaselineTemperature
        + ControllerGain * PlantGain * Setpoint)
    / (LREAL#1.0 + ControllerGain * PlantGain);
END_FUNCTION

FUNCTION Lesson47ClosedLoopUnstablePoleCount : INT
VAR_INPUT
    OpenLoopUnstablePoles : INT;
    CounterClockwiseEncirclements : INT;
END_VAR
Lesson47ClosedLoopUnstablePoleCount := OpenLoopUnstablePoles
    - CounterClockwiseEncirclements;
END_FUNCTION

PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ProportionalGain : LREAL := 2.0;
    HoldingCommand : LREAL := 50.0;
    Setpoint : LREAL := 101.0;
    ShortDelayTemperature : LREAL := 100.0;
    MediumDelayTemperature : LREAL := 100.0;
    LongDelayTemperature : LREAL := 100.0;
    ShortDelayRequested : LREAL := 50.0;
    MediumDelayRequested : LREAL := 50.0;
    LongDelayRequested : LREAL := 50.0;
    ShortDelayApplied : LREAL := 50.0;
    MediumDelayApplied : LREAL := 50.0;
    LongDelayApplied : LREAL := 50.0;
    ShortDelayAborted : BOOL := FALSE;
    MediumDelayAborted : BOOL := FALSE;
    LongDelayAborted : BOOL := FALSE;
END_VAR

(* The PLC runs only bounded time-domain validation. Nyquist contours remain
   retained evidence, so the expected frequency-domain verdict is not embedded
   in the controller being tested. *)
IF (NOT ExperimentEnabled) OR SafetyAbort OR (NOT InputsValid) THEN
    Setpoint := LREAL#100.0;
    ShortDelayRequested := HoldingCommand;
    MediumDelayRequested := HoldingCommand;
    LongDelayRequested := HoldingCommand;
    ShortDelayApplied := HoldingCommand;
    MediumDelayApplied := HoldingCommand;
    LongDelayApplied := HoldingCommand;
    ShortDelayAborted := FALSE;
    MediumDelayAborted := FALSE;
    LongDelayAborted := FALSE;
ELSE
    Setpoint := LREAL#101.0;
    ShortDelayRequested := HoldingCommand
        + ProportionalGain * (Setpoint - ShortDelayTemperature);
    MediumDelayRequested := HoldingCommand
        + ProportionalGain * (Setpoint - MediumDelayTemperature);
    LongDelayRequested := HoldingCommand
        + ProportionalGain * (Setpoint - LongDelayTemperature);

    ShortDelayAborted := ShortDelayAborted
        OR (ShortDelayTemperature < LREAL#90.0)
        OR (ShortDelayTemperature > LREAL#110.0);
    MediumDelayAborted := MediumDelayAborted
        OR (MediumDelayTemperature < LREAL#90.0)
        OR (MediumDelayTemperature > LREAL#110.0);
    LongDelayAborted := LongDelayAborted
        OR (LongDelayTemperature < LREAL#90.0)
        OR (LongDelayTemperature > LREAL#110.0);

    (* An abort restores HoldingCommand to the reviewed equilibrium instead of
       continuing a potentially unstable correction. *)
    IF ShortDelayAborted THEN
        ShortDelayApplied := HoldingCommand;
    ELSE
        ShortDelayApplied := MAX(MIN(ShortDelayRequested, LREAL#100.0), LREAL#0.0);
    END_IF;
    IF MediumDelayAborted THEN
        MediumDelayApplied := HoldingCommand;
    ELSE
        MediumDelayApplied := MAX(MIN(MediumDelayRequested, LREAL#100.0), LREAL#0.0);
    END_IF;
    IF LongDelayAborted THEN
        LongDelayApplied := HoldingCommand;
    ELSE
        LongDelayApplied := MAX(MIN(LongDelayRequested, LREAL#100.0), LREAL#0.0);
    END_IF;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX96.0 : BOOL;
    P1.SafetyAbort AT %MX96.1 : BOOL;
    P1.InputsValid AT %MX96.2 : BOOL;
    P1.ShortDelayAborted AT %MX97.0 : BOOL;
    P1.MediumDelayAborted AT %MX97.1 : BOOL;
    P1.LongDelayAborted AT %MX97.2 : BOOL;
    P1.ShortDelayRequested AT %ML0 : LREAL;
    P1.MediumDelayRequested AT %ML8 : LREAL;
    P1.LongDelayRequested AT %ML16 : LREAL;
    P1.Setpoint AT %ML24 : LREAL;
    P1.ShortDelayApplied AT %QL0 : LREAL;
    P1.MediumDelayApplied AT %QL8 : LREAL;
    P1.LongDelayApplied AT %QL16 : LREAL;
    P1.ShortDelayTemperature AT %IL0 : LREAL;
    P1.MediumDelayTemperature AT %IL8 : LREAL;
    P1.LongDelayTemperature AT %IL16 : LREAL;
END_VAR
END_CONFIGURATION
