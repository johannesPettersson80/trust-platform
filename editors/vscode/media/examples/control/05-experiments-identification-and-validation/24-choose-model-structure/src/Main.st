PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    AppliedPowerPct : LREAL := 75.0;
    WaterTemperatureC : LREAL := 35.0;
    TemperatureC : LREAL := 35.0;
END_VAR
IF ExperimentEnabled THEN AppliedPowerPct := LREAL#85.0;
ELSE AppliedPowerPct := LREAL#75.0;
END_IF;

(* Water and probe temperatures arrive through ordinary PLC inputs. *)
IF TemperatureC <> TemperatureC OR TemperatureC < LREAL#0.0
    OR TemperatureC > LREAL#80.0 THEN SafetyAbort := TRUE;
END_IF;
IF SafetyAbort THEN AppliedPowerPct := LREAL#0.0;
END_IF;
END_PROGRAM
CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.SafetyAbort AT %MX80.1 : BOOL;
    P1.AppliedPowerPct AT %QL0 : LREAL;
    P1.WaterTemperatureC AT %IL0 : LREAL;
    P1.TemperatureC AT %IL8 : LREAL;
    P1.ExperimentEnabled AT %MX80.0 : BOOL;
END_VAR
END_CONFIGURATION
