FUNCTION Lesson22NaturalFrequencyHz : LREAL
VAR_INPUT
    NaturalFrequencyRadiansPerSecond : LREAL;
END_VAR
Lesson22NaturalFrequencyHz := NaturalFrequencyRadiansPerSecond
    / LREAL#6.283185307179586;
END_FUNCTION

FUNCTION Lesson22Multisine : LREAL
VAR_INPUT
    ElapsedSeconds : LREAL;
END_VAR
Lesson22Multisine := (
    SIN(LREAL#6.283185307179586 * LREAL#0.4 * ElapsedSeconds)
    + SIN(LREAL#6.283185307179586 * LREAL#1.0 * ElapsedSeconds)
    + SIN(LREAL#6.283185307179586 * LREAL#1.6 * ElapsedSeconds)
) / LREAL#3.0;
END_FUNCTION

FUNCTION Lesson22PatternAllowed : BOOL
VAR_INPUT
    PlantSelection : INT;
    PatternSelection : INT;
END_VAR
Lesson22PatternAllowed := ((PlantSelection = 0)
    AND ((PatternSelection = 1) OR (PatternSelection = 2)
        OR (PatternSelection = 3)))
    OR ((PlantSelection = 1)
    AND ((PatternSelection = 4) OR (PatternSelection = 5)));
END_FUNCTION

PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    ElapsedSeconds : LREAL := 0.0;
    RequestedTorque : LREAL := 0.0;
    AppliedTorque : LREAL := 0.0;
    DrivePosition : LREAL := 0.0;
    NaturalFrequencyHz : LREAL := 0.0;
END_VAR

NaturalFrequencyHz := Lesson22NaturalFrequencyHz(
    NaturalFrequencyRadiansPerSecond := LREAL#6.0
);

IF ExperimentEnabled THEN
    ElapsedSeconds := ElapsedSeconds + LREAL#0.1;
    IF (ElapsedSeconds >= LREAL#5.0) AND (ElapsedSeconds < LREAL#25.0) THEN
        (* Three frequencies test below, near, and above the natural frequency. *)
        RequestedTorque := LREAL#0.1 * Lesson22Multisine(
            ElapsedSeconds := ElapsedSeconds - LREAL#5.0
        );
    ELSE
        RequestedTorque := LREAL#0.0;
    END_IF;
ELSE
    ElapsedSeconds := LREAL#0.0;
    RequestedTorque := LREAL#0.0;
END_IF;

AppliedTorque := RequestedTorque;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.AppliedTorque AT %QL8 : LREAL;
    P1.DrivePosition AT %IL8 : LREAL;
    P1.RequestedTorque AT %ML16 : LREAL;
    P1.ElapsedSeconds AT %ML24 : LREAL;
    P1.NaturalFrequencyHz AT %ML32 : LREAL;
    P1.ExperimentEnabled AT %MX80.0 : BOOL;
END_VAR
END_CONFIGURATION
