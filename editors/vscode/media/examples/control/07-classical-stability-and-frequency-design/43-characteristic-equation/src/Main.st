FUNCTION Lesson43RouthCubicS1Element : LREAL
VAR_INPUT
    CubicCoefficient : LREAL;
    QuadraticCoefficient : LREAL;
    LinearCoefficient : LREAL;
    ConstantCoefficient : LREAL;
END_VAR
Lesson43RouthCubicS1Element :=
    (QuadraticCoefficient * LinearCoefficient
        - CubicCoefficient * ConstantCoefficient)
    / QuadraticCoefficient;
END_FUNCTION

FUNCTION Lesson43ProportionalRequest : LREAL
VAR_INPUT
    Gain : LREAL;
    Setpoint : LREAL;
    Position : LREAL;
END_VAR
Lesson43ProportionalRequest := Gain * (Setpoint - Position);
END_FUNCTION

FUNCTION Lesson43ForceClamp : LREAL
VAR_INPUT
    RequestedForce : LREAL;
END_VAR
Lesson43ForceClamp := MAX(MIN(RequestedForce, LREAL#100.0), LREAL#-100.0);
END_FUNCTION

PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    StableGain : LREAL := 4.0;
    MarginalGain : LREAL := 6.0;
    UnstableGain : LREAL := 8.0;
    StableSetpoint : LREAL := 0.0;
    MarginalSetpoint : LREAL := 0.0;
    UnstableSetpoint : LREAL := 0.0;
    StableRequested : LREAL := 0.0;
    MarginalRequested : LREAL := 0.0;
    UnstableRequested : LREAL := 0.0;
    StableApplied : LREAL := 0.0;
    MarginalApplied : LREAL := 0.0;
    UnstableApplied : LREAL := 0.0;
    StablePosition : LREAL := 0.1;
    MarginalPosition : LREAL := 0.1;
    UnstablePosition : LREAL := 0.1;
    StableAborted : BOOL := FALSE;
    MarginalAborted : BOOL := FALSE;
    UnstableAborted : BOOL := FALSE;
END_VAR

(* Global validity rejects corrupt measurements. Each comparison path owns its
   own two-metre motion abort below, so the deliberately unstable copy cannot
   erase the stable and marginal evidence. *)
InputsValid := (StablePosition = StablePosition)
    AND (MarginalPosition = MarginalPosition)
    AND (UnstablePosition = UnstablePosition)
    AND ABS(StablePosition) < LREAL#1000000.0
    AND ABS(MarginalPosition) < LREAL#1000000.0
    AND ABS(UnstablePosition) < LREAL#1000000.0;

IF NOT ExperimentEnabled OR SafetyAbort OR NOT InputsValid THEN
    StableRequested := LREAL#0.0;
    MarginalRequested := LREAL#0.0;
    UnstableRequested := LREAL#0.0;
    StableApplied := LREAL#0.0;
    MarginalApplied := LREAL#0.0;
    UnstableApplied := LREAL#0.0;
    StableAborted := FALSE;
    MarginalAborted := FALSE;
    UnstableAborted := FALSE;
ELSE
    (* These explicit paths differ only in gain, so the comparison corresponds
       directly to the three predictions made before the run. *)
    StableRequested := Lesson43ProportionalRequest(Gain := StableGain,
        Setpoint := StableSetpoint, Position := StablePosition);
    MarginalRequested := Lesson43ProportionalRequest(Gain := MarginalGain,
        Setpoint := MarginalSetpoint, Position := MarginalPosition);
    UnstableRequested := Lesson43ProportionalRequest(Gain := UnstableGain,
        Setpoint := UnstableSetpoint, Position := UnstablePosition);

    IF ABS(StablePosition) >= LREAL#2.0 OR ABS(StableRequested) > LREAL#100.0 THEN
        StableAborted := TRUE;
    END_IF;
    IF ABS(MarginalPosition) >= LREAL#2.0 OR ABS(MarginalRequested) > LREAL#100.0 THEN
        MarginalAborted := TRUE;
    END_IF;
    IF ABS(UnstablePosition) >= LREAL#2.0 OR ABS(UnstableRequested) > LREAL#100.0 THEN
        UnstableAborted := TRUE;
    END_IF;

    IF StableAborted THEN StableApplied := LREAL#0.0;
    ELSE StableApplied := Lesson43ForceClamp(RequestedForce := StableRequested); END_IF;
    IF MarginalAborted THEN MarginalApplied := LREAL#0.0;
    ELSE MarginalApplied := Lesson43ForceClamp(RequestedForce := MarginalRequested); END_IF;
    IF UnstableAborted THEN UnstableApplied := LREAL#0.0;
    ELSE UnstableApplied := Lesson43ForceClamp(RequestedForce := UnstableRequested); END_IF;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX96.0 : BOOL;
    P1.SafetyAbort AT %MX96.1 : BOOL;
    P1.InputsValid AT %MX96.2 : BOOL;
    P1.StableRequested AT %ML0 : LREAL;
    P1.MarginalRequested AT %ML8 : LREAL;
    P1.UnstableRequested AT %ML16 : LREAL;
    P1.StableSetpoint AT %ML24 : LREAL;
    P1.MarginalSetpoint AT %ML32 : LREAL;
    P1.UnstableSetpoint AT %ML40 : LREAL;
    P1.StableApplied AT %QL0 : LREAL;
    P1.MarginalApplied AT %QL8 : LREAL;
    P1.UnstableApplied AT %QL16 : LREAL;
    P1.StablePosition AT %IL0 : LREAL;
    P1.MarginalPosition AT %IL8 : LREAL;
    P1.UnstablePosition AT %IL16 : LREAL;
    P1.StableAborted AT %MX97.0 : BOOL;
    P1.MarginalAborted AT %MX97.1 : BOOL;
    P1.UnstableAborted AT %MX97.2 : BOOL;
END_VAR
END_CONFIGURATION
