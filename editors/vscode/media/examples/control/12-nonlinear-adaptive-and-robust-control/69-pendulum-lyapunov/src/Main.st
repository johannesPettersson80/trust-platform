PROGRAM Main
VAR
    Enabled : BOOL := FALSE;
    AngleSetpoint : LREAL := 3.141592653589793;
    AbsoluteAngle : LREAL := 3.141592653589793;
    AngleError : LREAL := 0.0;
    AngularRate : LREAL := 0.0;
    TorqueRequested : LREAL := 0.0;
    TorqueApplied : LREAL := 0.0;
    TorqueSaturated : BOOL := FALSE;
    LyapunovValue : LREAL := 0.0;
    LyapunovDerivative : LREAL := 0.0;
    PreviousLyapunovValue : LREAL := 0.0;
    LyapunovDelta : LREAL := 0.0;
    CertificateLevel : LREAL := 0.025;
    EvidenceInitialized : BOOL := FALSE;
    InsideCertifiedSet : BOOL := FALSE;
    CertificateValid : BOOL := FALSE;
    Initialized : BOOL := FALSE;
END_VAR

// Keep the externally selected enable state and setpoint on the first scan.
// This is essential for a safe disabled start on a real actuator.
IF NOT Initialized THEN
    Initialized := TRUE;
END_IF;

AngleError := AbsoluteAngle - AngleSetpoint;
TorqueRequested := -LREAL#4.0 * AngleError - LREAL#2.0 * AngularRate;

IF Enabled THEN
    TorqueApplied := LIMIT(LREAL#-2.0, TorqueRequested, LREAL#2.0);
ELSE
    TorqueApplied := LREAL#0.0;
END_IF;
TorqueSaturated := Enabled AND ABS(TorqueRequested - TorqueApplied) > LREAL#1.0E-12;

// This quadratic value is the local certificate, not a second plant model.
LyapunovValue := LREAL#0.5 * (
    (LREAL#4.0 / LREAL#3.0) * AngleError * AngleError
    + (LREAL#1.0 / LREAL#3.0) * AngleError * AngularRate
    + (LREAL#1.0 / LREAL#3.0) * AngularRate * AngularRate
);

// The sine remainder keeps the derivative tied to the nonlinear plant.
LyapunovDerivative := -LREAL#0.5 * (
    AngleError * AngleError + AngularRate * AngularRate
) + (AngleError / LREAL#6.0 + AngularRate / LREAL#3.0)
    * (SIN(AngleError) - AngleError);

IF EvidenceInitialized THEN
    LyapunovDelta := LyapunovValue - PreviousLyapunovValue;
ELSE
    LyapunovDelta := LREAL#0.0;
END_IF;

InsideCertifiedSet := LyapunovValue <= CertificateLevel;
CertificateValid := Enabled
    AND InsideCertifiedSet
    AND NOT TorqueSaturated
    AND ((LyapunovValue <= LREAL#1.0E-12)
        OR (LyapunovDerivative < LREAL#0.0
            AND ((NOT EvidenceInitialized) OR (LyapunovDelta <= LREAL#1.0E-12))));

// Retain the current value only after all current-sample checks are complete.
PreviousLyapunovValue := LyapunovValue;
EvidenceInitialized := TRUE;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.AngleSetpoint AT %ML0 : LREAL;
    P1.AngleError AT %ML8 : LREAL;
    P1.TorqueRequested AT %ML16 : LREAL;
    P1.LyapunovValue AT %ML24 : LREAL;
    P1.LyapunovDerivative AT %ML32 : LREAL;
    P1.PreviousLyapunovValue AT %ML40 : LREAL;
    P1.LyapunovDelta AT %ML48 : LREAL;
    P1.CertificateLevel AT %ML56 : LREAL;
    P1.Enabled AT %MX64.0 : BOOL;
    P1.TorqueSaturated AT %MX64.1 : BOOL;
    P1.EvidenceInitialized AT %MX64.2 : BOOL;
    P1.InsideCertifiedSet AT %MX64.3 : BOOL;
    P1.CertificateValid AT %MX64.4 : BOOL;
    P1.TorqueApplied AT %QL0 : LREAL;
    P1.AbsoluteAngle AT %IL0 : LREAL;
    P1.AngularRate AT %IL8 : LREAL;
END_VAR
END_CONFIGURATION
