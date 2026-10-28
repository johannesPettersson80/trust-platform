(* The energy function of the upright arm, V = ½Jω² + ½Kp·φ² + G(cos φ − 1), in J, with J = 1 kg·m², Kp = 16 N·m/rad
   and G = 4 N·m; φ is the angle from upright in rad and ω the speed in rad/s. *)
FUNCTION ArmEnergyJ : LREAL
VAR_INPUT
    AngleRad : LREAL;
    SpeedRadPerS : LREAL;
END_VAR
ArmEnergyJ := LREAL#0.5 * SpeedRadPerS * SpeedRadPerS + LREAL#8.0 * AngleRad * AngleRad + LREAL#4.0 * (COS(AngleRad) - LREAL#1.0);
END_FUNCTION

(* The tested law: τ = −Kp·φ − Kd·ω with Kp = 16 N·m/rad and Kd = 7 N·m·s/rad. *)
FUNCTION PdTorqueNm : LREAL
VAR_INPUT
    AngleRad : LREAL;
    SpeedRadPerS : LREAL;
END_VAR
PdTorqueNm := LREAL#-16.0 * AngleRad - LREAL#7.0 * SpeedRadPerS;
END_FUNCTION

(* The hold before the release: the same gains toward the start angle, plus −G·sin φ, which cancels gravity, so that the
   arm comes to rest exactly at the start angle. *)
FUNCTION HoldTorqueNm : LREAL
VAR_INPUT
    AngleRad : LREAL;
    SpeedRadPerS : LREAL;
    TargetRad : LREAL;
END_VAR
HoldTorqueNm := LREAL#16.0 * (TargetRad - AngleRad) - LREAL#7.0 * SpeedRadPerS - LREAL#4.0 * SIN(AngleRad);
END_FUNCTION

PROGRAM Main
VAR
    (* InitialAngleRad, the start angle from upright, is a Control Studio experiment setting. The project experiment holds
       the arm at it for 3 s, then releases it to the tested law for 8 s. The joint measures its angle from hanging down:
       φ = θ − π. InsideCertificate reports V ≤ 0.1 J without saturation, the region where the energy argument holds. An
       angle beyond 0.3 rad or a speed beyond 2 rad/s from upright, or an update interval outside 20 ms ± 5 ms, latches zero
       torque. *)
    InitialAngleRad : LREAL := 0.1;
    JointAngleRad : LREAL := 3.141592653589793;
    SpeedRadPerS : LREAL := 0.0;
    TorqueNm : LREAL := 0.0;
    RequestedTorqueNm : LREAL := 0.0;
    AngleRad : LREAL := 0.0;
    EnergyJ : LREAL := 0.0;
    ElapsedS : LREAL := 0.0;
    Updates : DINT := 0;
    ExperimentEnabled : BOOL := FALSE;
    WasEnabled : BOOL := FALSE;
    Released : BOOL := FALSE;
    InsideCertificate : BOOL := FALSE;
    Saturated : BOOL := FALSE;
    FaultLatched : BOOL := FALSE;
    ClockPrimed : BOOL := FALSE;
    ClockValid : BOOL := TRUE;
    NowTime : TIME;
    PreviousTime : TIME;
END_VAR
AngleRad := JointAngleRad - LREAL#3.141592653589793;
NowTime := TIME();
ClockValid := NOT ClockPrimed OR ((NowTime - PreviousTime) >= T#15ms AND (NowTime - PreviousTime) <= T#25ms);
IF NOT ClockValid OR ABS(AngleRad) > LREAL#0.3 OR ABS(SpeedRadPerS) > LREAL#2.0 THEN
    FaultLatched := TRUE;
END_IF;
IF ExperimentEnabled AND NOT WasEnabled THEN
    Updates := 0;
ELSIF ExperimentEnabled THEN
    Updates := Updates + 1;
END_IF;
ElapsedS := DINT_TO_LREAL(Updates) * LREAL#0.02;
Released := ExperimentEnabled AND Updates >= 150;
IF ExperimentEnabled AND NOT Released THEN
    RequestedTorqueNm := HoldTorqueNm(AngleRad := AngleRad, SpeedRadPerS := SpeedRadPerS, TargetRad := InitialAngleRad);
ELSE
    RequestedTorqueNm := PdTorqueNm(AngleRad := AngleRad, SpeedRadPerS := SpeedRadPerS);
END_IF;
TorqueNm := MIN(MAX(RequestedTorqueNm, LREAL#-20.0), LREAL#20.0);
Saturated := TorqueNm <> RequestedTorqueNm;
IF FaultLatched THEN
    TorqueNm := LREAL#0.0;
END_IF;
EnergyJ := ArmEnergyJ(AngleRad := AngleRad, SpeedRadPerS := SpeedRadPerS);
InsideCertificate := EnergyJ <= LREAL#0.1 AND NOT Saturated;
WasEnabled := ExperimentEnabled;
PreviousTime := NowTime;
ClockPrimed := TRUE;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.JointAngleRad AT %IL0 : LREAL;
    P1.SpeedRadPerS AT %IL8 : LREAL;
    P1.TorqueNm AT %QL0 : LREAL;
    P1.AngleRad AT %ML0 : LREAL;
    P1.EnergyJ AT %ML8 : LREAL;
    P1.ExperimentEnabled AT %MX24.0 : BOOL;
    P1.Released AT %MX24.1 : BOOL;
    P1.InsideCertificate AT %MX24.2 : BOOL;
    P1.Saturated AT %MX24.3 : BOOL;
    P1.FaultLatched AT %MX24.4 : BOOL;
    P1.ClockValid AT %MX24.5 : BOOL;
END_VAR
END_CONFIGURATION
