FUNCTION Lesson33PdTorque : LREAL
VAR_INPUT
    ProportionalGain : LREAL;
    DerivativeGain : LREAL;
    Setpoint : LREAL;
    Position : LREAL;
    Velocity : LREAL;
END_VAR
Lesson33PdTorque := ProportionalGain * (Setpoint - Position)
    - DerivativeGain * Velocity;
END_FUNCTION

FUNCTION Lesson33CriticalDerivativeGain : LREAL
VAR_INPUT
    Inertia : LREAL;
    ProportionalGain : LREAL;
    PhysicalDamping : LREAL;
END_VAR
Lesson33CriticalDerivativeGain := LREAL#2.0
    * SQRT(Inertia * ProportionalGain) - PhysicalDamping;
END_FUNCTION

PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    Setpoint : LREAL := 0.0;
    ProportionalPosition : LREAL := 0.0;
    DampedPosition : LREAL := 0.0;
    ProportionalVelocity : LREAL := 0.0;
    DampedVelocity : LREAL := 0.0;
    ProportionalTorque : LREAL := 0.0;
    DampedTorque : LREAL := 0.0;
END_VAR

IF ExperimentEnabled THEN
    Setpoint := LREAL#1.0;
ELSE
    Setpoint := LREAL#0.0;
END_IF;

(* Both shafts keep Kp = 16. The second adds Kd = 7, so only
   velocity feedback explains the change in oscillation. *)
ProportionalTorque := Lesson33PdTorque(
    ProportionalGain := LREAL#16.0,
    DerivativeGain := LREAL#0.0,
    Setpoint := Setpoint,
    Position := ProportionalPosition,
    Velocity := ProportionalVelocity);
DampedTorque := Lesson33PdTorque(
    ProportionalGain := LREAL#16.0,
    DerivativeGain := LREAL#7.0,
    Setpoint := Setpoint,
    Position := DampedPosition,
    Velocity := DampedVelocity);
ProportionalTorque := MAX(MIN(ProportionalTorque, LREAL#20.0), LREAL#-20.0);
DampedTorque := MAX(MIN(DampedTorque, LREAL#20.0), LREAL#-20.0);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX120.0 : BOOL;
    P1.Setpoint AT %ML0 : LREAL;
    P1.ProportionalTorque AT %QL0 : LREAL;
    P1.DampedTorque AT %QL8 : LREAL;
    P1.ProportionalPosition AT %IL0 : LREAL;
    P1.DampedPosition AT %IL8 : LREAL;
    P1.ProportionalVelocity AT %IL16 : LREAL;
    P1.DampedVelocity AT %IL24 : LREAL;
END_VAR
END_CONFIGURATION
