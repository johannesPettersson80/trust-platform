FUNCTION Lesson68BearingReference : LREAL
VAR_INPUT
    ExperimentElapsedSeconds : LREAL;
    ControlAuthority : BOOL;
END_VAR
IF ControlAuthority AND (ExperimentElapsedSeconds >= LREAL#80.0) AND
   (ExperimentElapsedSeconds < LREAL#160.0) THEN
    Lesson68BearingReference := LREAL#0.1;
ELSE
    Lesson68BearingReference := LREAL#0.0;
END_IF;
END_FUNCTION

FUNCTION Lesson68BearingCurrentRequest : LREAL
VAR_INPUT
    Reference : LREAL;
    Displacement : LREAL;
    Velocity : LREAL;
END_VAR
Lesson68BearingCurrentRequest := LREAL#12.0 * Reference
    - LREAL#16.0 * Displacement
    - LREAL#7.0 * Velocity;
END_FUNCTION

FUNCTION Lesson68MotorTorqueRequest : LREAL
VAR_INPUT
    Reference : LREAL;
    EstimatedSpeed : LREAL;
    EstimatedLoad : LREAL;
END_VAR
Lesson68MotorTorqueRequest := LREAL#3.0 * Reference
    - LREAL#2.0 * EstimatedSpeed
    + EstimatedLoad;
END_FUNCTION

PROGRAM Main
VAR
    BearingEnabled : BOOL := TRUE;
    MotorEnabled : BOOL := TRUE;
    MotorObserverEnabled : BOOL := TRUE;
    BearingArchitecture : INT := 1;
    BearingSetpoint : LREAL := 0.0;
    MotorSetpoint : LREAL := 0.0;
    BearingCurrentRequested : LREAL := 0.0;
    BearingCurrentApplied : LREAL := 0.0;
    MotorTorqueRequested : LREAL := 0.0;
    MotorTorqueApplied : LREAL := 0.0;
    EstimatedMotorSpeed : LREAL := 0.0;
    EstimatedLoadTorque : LREAL := 0.0;
    MotorInnovation : LREAL := 0.0;
    PreviousMotorTorque : LREAL := 0.0;
    BearingDisplacement : LREAL := 0.0;
    BearingVelocity : LREAL := 0.0;
    MotorSpeed : LREAL := 0.0;
    MotorLoadTruth : LREAL := 0.0;
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ExperimentElapsed : LREAL := 0.0;
END_VAR

(* Control Studio grants one Boolean authority. The project owns the reviewed
   sequence and restores both actuators when authority is removed. Preserve
   the learner's feedback selections for separate comparison runs. *)
IF ExperimentEnabled AND InputsValid AND NOT SafetyAbort THEN
    ExperimentElapsed := ExperimentElapsed + LREAL#0.1;
    BearingEnabled := TRUE;
    MotorEnabled := TRUE;
    MotorSetpoint := LREAL#1.0;
    BearingSetpoint := Lesson68BearingReference(
        ExperimentElapsedSeconds := ExperimentElapsed,
        ControlAuthority := TRUE);
ELSE
    ExperimentElapsed := LREAL#0.0;
    BearingEnabled := FALSE;
    MotorEnabled := FALSE;
    BearingSetpoint := LREAL#0.0;
    MotorSetpoint := LREAL#0.0;
    EstimatedMotorSpeed := LREAL#0.0;
    EstimatedLoadTorque := LREAL#0.0;
    MotorInnovation := LREAL#0.0;
END_IF;

IF BearingEnabled THEN
    IF BearingArchitecture = 1 THEN
        BearingCurrentRequested := Lesson68BearingCurrentRequest(
            Reference := BearingSetpoint,
            Displacement := BearingDisplacement,
            Velocity := BearingVelocity);
    ELSE
        BearingCurrentRequested := LREAL#5.0
            * (BearingSetpoint - BearingDisplacement);
    END_IF;
    BearingCurrentApplied := LIMIT(
        LREAL#-2.0,
        BearingCurrentRequested,
        LREAL#2.0
    );
ELSE
    BearingCurrentRequested := LREAL#0.0;
    BearingCurrentApplied := LREAL#0.0;
END_IF;

IF MotorEnabled THEN
    IF MotorObserverEnabled THEN
        MotorInnovation := MotorSpeed - EstimatedMotorSpeed;
        // The observer predicts with torque applied during the preceding scan.
        EstimatedMotorSpeed := EstimatedMotorSpeed + LREAL#0.1 * (
            -EstimatedMotorSpeed - EstimatedLoadTorque + PreviousMotorTorque
            + LREAL#8.0 * MotorInnovation
        );
        // Positive physical load reduces speed, so its innovation gain is negative.
        EstimatedLoadTorque := EstimatedLoadTorque
            + LREAL#0.1 * (LREAL#-20.0 * MotorInnovation);
    ELSE
        EstimatedMotorSpeed := MotorSpeed;
        EstimatedLoadTorque := LREAL#0.0;
        MotorInnovation := LREAL#0.0;
    END_IF;

    MotorTorqueRequested := Lesson68MotorTorqueRequest(
        Reference := MotorSetpoint,
        EstimatedSpeed := EstimatedMotorSpeed,
        EstimatedLoad := EstimatedLoadTorque);
    MotorTorqueApplied := LIMIT(
        LREAL#-3.0,
        MotorTorqueRequested,
        LREAL#3.0
    );
    PreviousMotorTorque := MotorTorqueApplied;
ELSE
    MotorTorqueRequested := LREAL#0.0;
    MotorTorqueApplied := LREAL#0.0;
    PreviousMotorTorque := LREAL#0.0;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.BearingArchitecture AT %MW0 : INT;
    P1.BearingSetpoint AT %ML8 : LREAL;
    P1.MotorSetpoint AT %ML16 : LREAL;
    P1.BearingCurrentRequested AT %ML24 : LREAL;
    P1.MotorTorqueRequested AT %ML32 : LREAL;
    P1.EstimatedMotorSpeed AT %ML40 : LREAL;
    P1.EstimatedLoadTorque AT %ML48 : LREAL;
    P1.MotorInnovation AT %ML56 : LREAL;
    P1.BearingEnabled AT %MX120.0 : BOOL;
    P1.MotorEnabled AT %MX120.1 : BOOL;
    P1.MotorObserverEnabled AT %MX120.2 : BOOL;
    P1.ExperimentEnabled AT %MX120.3 : BOOL;
    P1.SafetyAbort AT %MX120.4 : BOOL;
    P1.InputsValid AT %MX120.5 : BOOL;
    P1.BearingCurrentApplied AT %QL0 : LREAL;
    P1.MotorTorqueApplied AT %QL16 : LREAL;
    P1.BearingDisplacement AT %IL0 : LREAL;
    P1.BearingVelocity AT %IL8 : LREAL;
    P1.MotorSpeed AT %IL16 : LREAL;
    P1.MotorLoadTruth AT %IL24 : LREAL;
END_VAR
END_CONFIGURATION
