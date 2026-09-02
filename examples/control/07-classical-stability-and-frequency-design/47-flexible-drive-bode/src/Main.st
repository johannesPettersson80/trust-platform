PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ExperimentElapsed : LREAL := 0.0;
    CycleTime : LREAL := 0.01;
    ProbeAmplitude : LREAL := 1.0;
    LowFrequency : LREAL := 0.5;
    ResonantFrequency : LREAL := 2.0;
    HighFrequency : LREAL := 8.0;
    LowRequested : LREAL := 0.0;
    ResonantRequested : LREAL := 0.0;
    HighRequested : LREAL := 0.0;
    LowApplied : LREAL := 0.0;
    ResonantApplied : LREAL := 0.0;
    HighApplied : LREAL := 0.0;
    LowSpeed : LREAL := 1000.0;
    ResonantSpeed : LREAL := 1000.0;
    HighSpeed : LREAL := 1000.0;
END_VAR

IF (NOT ExperimentEnabled) OR SafetyAbort OR (NOT InputsValid) THEN
    ExperimentElapsed := LREAL#0.0;
    LowRequested := LREAL#0.0;
    ResonantRequested := LREAL#0.0;
    HighRequested := LREAL#0.0;
ELSE
    ExperimentElapsed := ExperimentElapsed + CycleTime;
    (* Equal-amplitude probes make frequency the only changed condition. *)
    LowRequested := ProbeAmplitude * SIN(LowFrequency * ExperimentElapsed);
    ResonantRequested := ProbeAmplitude * SIN(ResonantFrequency * ExperimentElapsed);
    HighRequested := ProbeAmplitude * SIN(HighFrequency * ExperimentElapsed);
END_IF;

LowApplied := MAX(MIN(LowRequested, LREAL#100.0), LREAL#-100.0);
ResonantApplied := MAX(MIN(ResonantRequested, LREAL#100.0), LREAL#-100.0);
HighApplied := MAX(MIN(HighRequested, LREAL#100.0), LREAL#-100.0);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX96.0 : BOOL;
    P1.SafetyAbort AT %MX96.1 : BOOL;
    P1.InputsValid AT %MX96.2 : BOOL;
    P1.LowRequested AT %ML0 : LREAL;
    P1.ResonantRequested AT %ML8 : LREAL;
    P1.HighRequested AT %ML16 : LREAL;
    P1.LowApplied AT %QL0 : LREAL;
    P1.ResonantApplied AT %QL8 : LREAL;
    P1.HighApplied AT %QL16 : LREAL;
    P1.LowSpeed AT %IL0 : LREAL;
    P1.ResonantSpeed AT %IL8 : LREAL;
    P1.HighSpeed AT %IL16 : LREAL;
END_VAR
END_CONFIGURATION
