PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ManualMode : BOOL := TRUE;
    Kp : LREAL := 1.0;
    Setpoint : LREAL := 0.0;
    Angle : LREAL := 0.0;
    Velocity : LREAL := 0.0;
    RequestedTorque : LREAL := 0.0;
    AppliedTorque : LREAL := 0.0;
END_VAR

(* Control Studio performs the continuous-time Routh analysis. This project
   only applies the selected proportional shaft controller at a 20 ms sample. *)
IF ABS(Angle) >= LREAL#0.2 OR ABS(Velocity) >= LREAL#2.0
    OR Angle <> Angle OR Velocity <> Velocity
    OR Kp < LREAL#-1.0 OR Kp > LREAL#4.0 OR Kp <> Kp THEN
    SafetyAbort := TRUE;
END_IF;
ManualMode := (NOT ExperimentEnabled) OR SafetyAbort OR (NOT InputsValid);
IF ManualMode THEN
    Setpoint := LREAL#0.0;
    RequestedTorque := LREAL#0.0;
    AppliedTorque := LREAL#0.0;
ELSE
    Setpoint := LREAL#0.01;
    RequestedTorque := Kp * (Setpoint - Angle);
    AppliedTorque := MAX(MIN(RequestedTorque, LREAL#20.0), LREAL#-20.0);
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX96.0 : BOOL;
    P1.SafetyAbort AT %MX96.1 : BOOL;
    P1.ManualMode AT %MX96.3 : BOOL;
    P1.RequestedTorque AT %ML0 : LREAL;
    P1.Setpoint AT %ML16 : LREAL;
    P1.AppliedTorque AT %QL0 : LREAL;
    P1.Angle AT %IL0 : LREAL;
    P1.Velocity AT %IL8 : LREAL;
END_VAR
END_CONFIGURATION
