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
    Initialized : BOOL := FALSE;
END_VAR

// Declaration defaults provide a safe standalone start. Do not overwrite mapped
// Control Studio choices on the first scan.
IF NOT Initialized THEN
    Initialized := TRUE;
END_IF;

IF BearingEnabled THEN
    IF BearingArchitecture = 1 THEN
        BearingCurrentRequested := LREAL#12.0 * BearingSetpoint
            - LREAL#16.0 * BearingDisplacement
            - LREAL#7.0 * BearingVelocity;
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

    MotorTorqueRequested := LREAL#3.0 * MotorSetpoint
        - LREAL#2.0 * EstimatedMotorSpeed
        + EstimatedLoadTorque;
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
    P1.BearingCurrentApplied AT %QL0 : LREAL;
    P1.MotorTorqueApplied AT %QL16 : LREAL;
    P1.BearingDisplacement AT %IL0 : LREAL;
    P1.BearingVelocity AT %IL8 : LREAL;
    P1.MotorSpeed AT %IL16 : LREAL;
    P1.MotorLoadTruth AT %IL24 : LREAL;
END_VAR
END_CONFIGURATION
