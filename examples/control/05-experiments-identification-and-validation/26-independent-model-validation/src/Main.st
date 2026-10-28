PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    TrialKind : INT := 0; (* 0:90% command; 1:65% command; 2:30kW extraction. *)
    AppliedPowerPct : LREAL := 75.0;
    DisturbanceKw : LREAL := 0.0;
    WaterTemperatureC : LREAL := 35.0;
    TemperatureC : LREAL := 35.0;
END_VAR
AppliedPowerPct := LREAL#75.0;
DisturbanceKw := LREAL#0.0;
IF ExperimentEnabled THEN
    CASE TrialKind OF
        0: AppliedPowerPct := LREAL#90.0;
        1: AppliedPowerPct := LREAL#65.0;
        2: DisturbanceKw := LREAL#30.0;
    END_CASE;
END_IF;
(* The native plant supplies water and probe temperatures through PLC inputs.
   This program commands the independent experiment; it does not simulate the
   process or calculate a fitted-model prediction. *)
IF TemperatureC <> TemperatureC OR TemperatureC < LREAL#31.0
    OR TemperatureC > LREAL#39.0 THEN SafetyAbort := TRUE;
END_IF;
IF SafetyAbort THEN AppliedPowerPct := LREAL#0.0;
    DisturbanceKw := LREAL#0.0;
END_IF;
END_PROGRAM
CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.SafetyAbort AT %MX64.1 : BOOL;
    P1.ExperimentEnabled AT %MX64.0 : BOOL;
    P1.AppliedPowerPct AT %QL0 : LREAL;
    P1.DisturbanceKw AT %QL8 : LREAL;
    P1.TemperatureC AT %IL8 : LREAL;
    P1.WaterTemperatureC AT %IL0 : LREAL;
END_VAR
END_CONFIGURATION
