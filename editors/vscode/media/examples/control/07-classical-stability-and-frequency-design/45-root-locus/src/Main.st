PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ManualMode : BOOL := TRUE;
    Kp : LREAL := 16.0;
    Kd : LREAL := 7.0;
    Setpoint : LREAL := 0.0;
    Angle : LREAL := 0.0;
    Velocity : LREAL := 0.0;
    RequestedTorque : LREAL := 0.0;
    AppliedTorque : LREAL := 0.0;
END_VAR

(* The simulation supplies measured angle and velocity. Control Studio finds
   the root locus; this program only applies its selected proportional gain.
   Kd remains 7. Restart the simulator to reset angle and velocity for each run. *)
IF ABS(Angle) > LREAL#2.0 OR ABS(Velocity) > LREAL#10.0
    OR Angle <> Angle OR Velocity <> Velocity
    OR Kp < LREAL#0.0 OR Kp > LREAL#20.0 OR Kp <> Kp
    OR Kd <> LREAL#7.0 THEN
    SafetyAbort := TRUE;
END_IF;
ManualMode := (NOT ExperimentEnabled) OR SafetyAbort OR (NOT InputsValid);
IF ManualMode THEN
    Setpoint := LREAL#0.0;
    RequestedTorque := LREAL#0.0;
    AppliedTorque := LREAL#0.0;
ELSE
    Setpoint := LREAL#1.0;
    RequestedTorque := Kp * (Setpoint - Angle) - Kd * Velocity;
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
