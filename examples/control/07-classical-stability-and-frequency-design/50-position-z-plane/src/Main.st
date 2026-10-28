(* Exact update of the shaft J = B = 1 over one period h with the torque u held: ω' = a·ω + d·u and
   θ' = θ + b·ω + c·u, with a = e^(−h), b = d = 1 − e^(−h) and c = h − 1 + e^(−h). *)
FUNCTION HeldVelocityFactor : LREAL
VAR_INPUT
    PeriodS : LREAL;
END_VAR
HeldVelocityFactor := EXP(-PeriodS);
END_FUNCTION

FUNCTION HeldVelocityGain : LREAL
VAR_INPUT
    PeriodS : LREAL;
END_VAR
HeldVelocityGain := LREAL#1.0 - EXP(-PeriodS);
END_FUNCTION

FUNCTION HeldAngleGain : LREAL
VAR_INPUT
    PeriodS : LREAL;
END_VAR
HeldAngleGain := PeriodS - LREAL#1.0 + EXP(-PeriodS);
END_FUNCTION

(* The characteristic polynomial of the sampled loop with one update of computation delay,
   z·(z − 1)·(z − a) + Kd·d·(z − 1) + Kp·(c·(z − a) + b·d), at the multiplier z. *)
FUNCTION SampledLoopPolynomial : LREAL
VAR_INPUT
    PeriodS : LREAL;
    ProportionalGain : LREAL;
    VelocityGain : LREAL;
    Z : LREAL;
END_VAR
VAR
    A : LREAL;
    B : LREAL;
    C : LREAL;
END_VAR
A := HeldVelocityFactor(PeriodS := PeriodS);
B := HeldVelocityGain(PeriodS := PeriodS);
C := HeldAngleGain(PeriodS := PeriodS);
SampledLoopPolynomial := Z * (Z - LREAL#1.0) * (Z - A) + VelocityGain * B * (Z - LREAL#1.0)
    + ProportionalGain * (C * (Z - A) + B * B);
END_FUNCTION

(* PD on the measured angle and velocity, u = Kp·(r − θ) − Kd·ω, with one update of computation delay: at each update
   the torque calculated at the previous update is applied and the new one waits, limited to ±20 N·m. Outside ±0.2 rad
   or ±2 rad/s the block latches its abort, clears the waiting torque and brakes the shaft with −Kd·ω. *)
FUNCTION_BLOCK DelayedPd
VAR_INPUT
    Enable : BOOL;
    Update : BOOL;
    TargetRad : LREAL;
    AngleRad : LREAL;
    VelocityRadS : LREAL;
END_VAR
VAR_OUTPUT
    TorqueNm : LREAL := 0.0;
    PendingNm : LREAL := 0.0;
    Abort : BOOL := FALSE;
END_VAR
VAR
    ProportionalGain : LREAL := 16.0;
    VelocityGain : LREAL := 7.0;
END_VAR
IF NOT Enable THEN
    Abort := FALSE;
    PendingNm := LREAL#0.0;
    TorqueNm := LREAL#0.0;
ELSE
    Abort := Abort OR ABS(AngleRad) > LREAL#0.2 OR ABS(VelocityRadS) > LREAL#2.0;
    IF Abort THEN
        PendingNm := LREAL#0.0;
        TorqueNm := LIMIT(LREAL#-20.0, -VelocityGain * VelocityRadS, LREAL#20.0);
    ELSIF Update THEN
        TorqueNm := PendingNm;
        PendingNm := LIMIT(LREAL#-20.0, ProportionalGain * (TargetRad - AngleRad) - VelocityGain * VelocityRadS, LREAL#20.0);
    END_IF;
END_IF;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    (* Two copies of the shaft under the same delayed PD, Kp = 16 N·m/rad and Kd = 7 N·m·s/rad: the first updated
       every 0.02 s, at every 20 ms scan, the second every 0.6 s, at every 30th scan. *)
    FastController : DelayedPd;
    SlowController : DelayedPd;
    FastAngleRad : LREAL := 0.0;
    FastVelocityRadS : LREAL := 0.0;
    FastTorqueNm : LREAL := 0.0;
    FastPendingNm : LREAL := 0.0;
    SlowAngleRad : LREAL := 0.0;
    SlowVelocityRadS : LREAL := 0.0;
    SlowTorqueNm : LREAL := 0.0;
    SlowPendingNm : LREAL := 0.0;
    SlowScan : INT := 0;
    TargetRad : LREAL := 0.0;
    FastAbort : BOOL := FALSE;
    SlowAbort : BOOL := FALSE;
    (* Control Studio's project experiment steps the target of both shafts from 0 to 0.01 rad. *)
    ExperimentEnabled : BOOL := FALSE;
END_VAR

IF ExperimentEnabled THEN
    TargetRad := LREAL#0.01;
ELSE
    TargetRad := LREAL#0.0;
    SlowScan := 0;
END_IF;
FastController(Enable := ExperimentEnabled, Update := TRUE, TargetRad := TargetRad, AngleRad := FastAngleRad,
    VelocityRadS := FastVelocityRadS);
SlowController(Enable := ExperimentEnabled, Update := ExperimentEnabled AND SlowScan = 0, TargetRad := TargetRad,
    AngleRad := SlowAngleRad, VelocityRadS := SlowVelocityRadS);
IF ExperimentEnabled THEN
    SlowScan := (SlowScan + 1) MOD 30;
END_IF;
FastTorqueNm := FastController.TorqueNm;
FastPendingNm := FastController.PendingNm;
FastAbort := FastController.Abort;
SlowTorqueNm := SlowController.TorqueNm;
SlowPendingNm := SlowController.PendingNm;
SlowAbort := SlowController.Abort;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.FastTorqueNm AT %QL0 : LREAL;
    P1.FastAngleRad AT %IL0 : LREAL;
    P1.FastVelocityRadS AT %IL8 : LREAL;
    P1.SlowTorqueNm AT %QL8 : LREAL;
    P1.SlowAngleRad AT %IL16 : LREAL;
    P1.SlowVelocityRadS AT %IL24 : LREAL;
    P1.TargetRad AT %ML0 : LREAL;
    P1.FastPendingNm AT %ML8 : LREAL;
    P1.SlowPendingNm AT %ML16 : LREAL;
    P1.ExperimentEnabled AT %MX24.0 : BOOL;
    P1.FastAbort AT %MX24.1 : BOOL;
    P1.SlowAbort AT %MX24.2 : BOOL;
END_VAR
END_CONFIGURATION
