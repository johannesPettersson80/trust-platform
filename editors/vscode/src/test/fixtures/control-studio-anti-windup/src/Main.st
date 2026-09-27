PROGRAM Main
VAR
    Kp : LREAL := LREAL#2.0;
    Ki : LREAL := LREAL#0.5;
    Kd : LREAL := LREAL#0.0;
    BackCalculationGain : LREAL := LREAL#0.0;
    Manual : BOOL := TRUE;
    ExperimentEnabled AT %MX64.0 : BOOL := FALSE;
    ExperimentScan : ULINT := ULINT#0;
    SP AT %ML0 : LREAL := LREAL#35.0;
    PV AT %IL0 : LREAL;
    MV AT %QL0 : LREAL;
    RequestedOutput AT %ML8 : LREAL := LREAL#75.0;
    Integral AT %ML16 : LREAL := LREAL#75.0;
    Error : LREAL;
    TrialIntegral : LREAL;
END_VAR

IF ExperimentEnabled THEN
    IF ExperimentScan = ULINT#0 THEN
        (* The approved scenario starts from the declared 35 degree / 75 percent hot reset. *)
        Integral := LREAL#75.0;
    END_IF;
    ExperimentScan := ExperimentScan + ULINT#1;
    Manual := FALSE;
    IF ExperimentScan <= ULINT#19 THEN
        SP := LREAL#50.0;
    ELSE
        SP := LREAL#35.0;
    END_IF;
ELSIF ExperimentScan > ULINT#0 THEN
    (* The project sequence alone owns the saturation scenario and restoration. *)
    Manual := TRUE;
    ExperimentScan := ULINT#0;
    SP := LREAL#35.0;
END_IF;

IF Manual THEN
    RequestedOutput := LREAL#75.0;
    MV := LREAL#75.0;
ELSE
    Error := SP - PV;
    TrialIntegral := Integral + Ki * Error * LREAL#0.1;
    RequestedOutput := Kp * Error + TrialIntegral;
    MV := LIMIT(LREAL#0.0, RequestedOutput, LREAL#100.0);
    Integral := TrialIntegral + LREAL#0.1 * BackCalculationGain * (MV - RequestedOutput);
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Main WITH MainTask : Main;
END_RESOURCE
END_CONFIGURATION
