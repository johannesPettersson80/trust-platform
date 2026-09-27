FUNCTION Lesson32ProportionalTorque : LREAL
VAR_INPUT
    ProportionalGain : LREAL;
    Setpoint : LREAL;
    Position : LREAL;
END_VAR
Lesson32ProportionalTorque := ProportionalGain * (Setpoint - Position);
END_FUNCTION

FUNCTION Lesson32NaturalFrequency : LREAL
VAR_INPUT
    ProportionalGain : LREAL;
    Inertia : LREAL;
END_VAR
Lesson32NaturalFrequency := SQRT(ProportionalGain / Inertia);
END_FUNCTION

FUNCTION Lesson32DampingRatio : LREAL
VAR_INPUT
    ViscousDamping : LREAL;
    Inertia : LREAL;
    ProportionalGain : LREAL;
END_VAR
Lesson32DampingRatio := ViscousDamping
    / (LREAL#2.0 * SQRT(Inertia * ProportionalGain));
END_FUNCTION

PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    Setpoint : LREAL := 0.0;
    LowGainPosition : LREAL := 0.0;
    HighGainPosition : LREAL := 0.0;
    LowGainVelocity : LREAL := 0.0;
    HighGainVelocity : LREAL := 0.0;
    LowGainTorque : LREAL := 0.0;
    HighGainTorque : LREAL := 0.0;
END_VAR

IF ExperimentEnabled THEN
    Setpoint := LREAL#1.0;
ELSE
    Setpoint := LREAL#0.0;
END_IF;

(* Proportional position feedback pulls each shaft toward the setpoint.
   The two copies differ only in the strength of that pull. *)
LowGainTorque := Lesson32ProportionalTorque(
    ProportionalGain := LREAL#4.0,
    Setpoint := Setpoint,
    Position := LowGainPosition);
HighGainTorque := Lesson32ProportionalTorque(
    ProportionalGain := LREAL#16.0,
    Setpoint := Setpoint,
    Position := HighGainPosition);
LowGainTorque := MAX(MIN(LowGainTorque, LREAL#20.0), LREAL#-20.0);
HighGainTorque := MAX(MIN(HighGainTorque, LREAL#20.0), LREAL#-20.0);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX112.0 : BOOL;
    P1.Setpoint AT %ML0 : LREAL;
    P1.LowGainTorque AT %QL0 : LREAL;
    P1.HighGainTorque AT %QL8 : LREAL;
    P1.LowGainPosition AT %IL0 : LREAL;
    P1.HighGainPosition AT %IL8 : LREAL;
    P1.LowGainVelocity AT %IL16 : LREAL;
    P1.HighGainVelocity AT %IL24 : LREAL;
END_VAR
END_CONFIGURATION
