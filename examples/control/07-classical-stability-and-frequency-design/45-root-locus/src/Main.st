(* Position control with fixed velocity braking: u = Kp·(r − θ) − Kd·ω from the measured angle θ and velocity ω,
   within the drive's ±20 N·m. *)
FUNCTION BrakedPositionTorque : LREAL
VAR_INPUT
    ProportionalGain : LREAL;
    VelocityGain : LREAL;
    Target : LREAL;
    Angle : LREAL;
    Velocity : LREAL;
END_VAR
BrakedPositionTorque := ProportionalGain * (Target - Angle) - VelocityGain * Velocity;
END_FUNCTION

PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ManualMode : BOOL := TRUE;
    (* The gain chosen from the root locus and the alternative it is compared with; Kd stays 7 for both. *)
    Kp : LREAL := 16.0;
    AlternativeKp : LREAL := 12.0;
    Kd : LREAL := 7.0;
    Setpoint : LREAL := 0.0;
    Angle : LREAL := 0.0;
    Velocity : LREAL := 0.0;
    RequestedTorque : LREAL := 0.0;
    AppliedTorque : LREAL := 0.0;
    AlternativeAngle : LREAL := 0.0;
    AlternativeVelocity : LREAL := 0.0;
    AlternativeTorque : LREAL := 0.0;
END_VAR

(* The simulation supplies measured angles and velocities. Control Studio calculates the root locus; this program
   only applies the chosen gains. Restart the simulator to reset both shafts for each run. *)
IF ABS(Angle) > LREAL#2.0 OR ABS(Velocity) > LREAL#10.0 OR Angle <> Angle OR Velocity <> Velocity
    OR ABS(AlternativeAngle) > LREAL#2.0 OR ABS(AlternativeVelocity) > LREAL#10.0
    OR AlternativeAngle <> AlternativeAngle OR AlternativeVelocity <> AlternativeVelocity
    OR Kp < LREAL#0.0 OR Kp > LREAL#20.0 OR Kp <> Kp
    OR AlternativeKp < LREAL#0.0 OR AlternativeKp > LREAL#20.0 OR AlternativeKp <> AlternativeKp
    OR Kd <> LREAL#7.0 THEN
    SafetyAbort := TRUE;
END_IF;
ManualMode := (NOT ExperimentEnabled) OR SafetyAbort OR (NOT InputsValid);
IF ManualMode THEN
    Setpoint := LREAL#0.0;
    RequestedTorque := LREAL#0.0;
    AppliedTorque := LREAL#0.0;
    AlternativeTorque := LREAL#0.0;
ELSE
    Setpoint := LREAL#1.0;
    RequestedTorque := BrakedPositionTorque(ProportionalGain := Kp, VelocityGain := Kd, Target := Setpoint,
        Angle := Angle, Velocity := Velocity);
    AppliedTorque := LIMIT(LREAL#-20.0, RequestedTorque, LREAL#20.0);
    AlternativeTorque := LIMIT(LREAL#-20.0, BrakedPositionTorque(ProportionalGain := AlternativeKp, VelocityGain := Kd,
        Target := Setpoint, Angle := AlternativeAngle, Velocity := AlternativeVelocity), LREAL#20.0);
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
    P1.AlternativeTorque AT %QL8 : LREAL;
    P1.Angle AT %IL0 : LREAL;
    P1.Velocity AT %IL8 : LREAL;
    P1.AlternativeAngle AT %IL16 : LREAL;
    P1.AlternativeVelocity AT %IL24 : LREAL;
END_VAR
END_CONFIGURATION
