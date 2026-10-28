PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    HasStarted : BOOL := FALSE;
    Finished : BOOL := FALSE;
    TrialElapsedMs : DINT := 0;
    TemperatureC : LREAL := 35.0;
    AppliedPowerPct : LREAL := 0.0;
END_VAR

(* This is a prepared open-loop record, not a PI response and not an ST
   equilibrium calculation. The simulator supplies the 35°C / 75% history. *)
IF TemperatureC <> TemperatureC OR TemperatureC < LREAL#33.0
    OR TemperatureC > LREAL#39.0 THEN SafetyAbort := TRUE;
END_IF;
IF SafetyAbort OR Finished THEN
    AppliedPowerPct := LREAL#0.0;
ELSIF NOT ExperimentEnabled THEN
    IF HasStarted THEN
        Finished := TRUE;
        AppliedPowerPct := LREAL#0.0;
    ELSE
        AppliedPowerPct := LREAL#75.0;
    END_IF;
ELSIF TrialElapsedMs >= 30000 THEN
    Finished := TRUE;
    AppliedPowerPct := LREAL#0.0;
ELSE
    HasStarted := TRUE;
    AppliedPowerPct := LREAL#85.0;
    TrialElapsedMs := TrialElapsedMs + 100;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX80.0 : BOOL;
    P1.SafetyAbort AT %MX80.1 : BOOL;
    P1.AppliedPowerPct AT %QL0 : LREAL;
    P1.TemperatureC AT %IL0 : LREAL;
END_VAR
END_CONFIGURATION
