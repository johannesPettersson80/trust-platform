FUNCTION Lesson23ProcessGain : LREAL
VAR_INPUT
    InitialInput : LREAL;
    FinalInput : LREAL;
    InitialOutput : LREAL;
    FinalOutput : LREAL;
END_VAR
VAR_OUTPUT
    ENO : BOOL;
END_VAR
IF FinalInput = InitialInput THEN
    ENO := FALSE;
    Lesson23ProcessGain := LREAL#0.0;
ELSE
    Lesson23ProcessGain := (FinalOutput - InitialOutput)
        / (FinalInput - InitialInput);
END_IF;
END_FUNCTION

FUNCTION Lesson23TimeConstant : LREAL
VAR_INPUT
    StepTime : LREAL;
    ResponseStartTime : LREAL;
    SixtyThreePercentTime : LREAL;
END_VAR
Lesson23TimeConstant := SixtyThreePercentTime - ResponseStartTime;
END_FUNCTION

FUNCTION Lesson23Delay : LREAL
VAR_INPUT
    StepTime : LREAL;
    ResponseStartTime : LREAL;
END_VAR
Lesson23Delay := ResponseStartTime - StepTime;
END_FUNCTION

PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    AppliedPowerPct : LREAL := 75.0;
    TemperatureC : LREAL := 35.0;
END_VAR

(* Record this applied command and the measured temperature in Control Studio.
   The functions above support the lesson's arithmetic exercise; no invented
   measurements are used to publish a precomputed identification result. *)
IF ExperimentEnabled THEN
    AppliedPowerPct := LREAL#85.0;
ELSE
    AppliedPowerPct := LREAL#75.0;
END_IF;
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
    P1.TemperatureC AT %IL0 : LREAL;
    P1.ExperimentEnabled AT %MX80.0 : BOOL;
END_VAR
END_CONFIGURATION
