PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    RelayAmplitude : LREAL := 0.0;
    ExperimentElapsed : LREAL := 0.0;
    QualifiedRelayHigh : BOOL := TRUE;
    RejectedRelayHigh : BOOL := TRUE;
    QualifiedRelayApplied : LREAL := 20.0;
    RejectedRelayApplied : LREAL := 20.0;
    QualifiedTemperature : LREAL := 50.0;
    RejectedTemperature : LREAL := 50.0;
END_VAR

InputsValid := QualifiedTemperature >= LREAL#40.0
    AND QualifiedTemperature <= LREAL#60.0
    AND RejectedTemperature >= LREAL#40.0
    AND RejectedTemperature <= LREAL#60.0;
IF ExperimentEnabled AND InputsValid AND NOT SafetyAbort THEN
    RelayAmplitude := LREAL#18.0;
ELSE
    RelayAmplitude := LREAL#0.0;
END_IF;

IF RelayAmplitude <= LREAL#0.0 THEN
    ExperimentElapsed := LREAL#0.0;
    QualifiedRelayHigh := TRUE;
    RejectedRelayHigh := TRUE;
    QualifiedRelayApplied := LREAL#20.0;
    RejectedRelayApplied := LREAL#20.0;
ELSIF ExperimentElapsed >= LREAL#180.0 THEN
    (* Duration is a safety bound, not a switching clock. Both relays restore
       the known operating-point command when that bound expires. *)
    QualifiedRelayApplied := LREAL#20.0;
    RejectedRelayApplied := LREAL#20.0;
ELSE
    ExperimentElapsed := ExperimentElapsed + LREAL#0.1;

    (* Retaining state inside the hysteresis band prevents measurement noise
       from turning the relay into rapid threshold chatter. *)
    IF QualifiedTemperature >= LREAL#50.08 THEN
        QualifiedRelayHigh := FALSE;
    ELSIF QualifiedTemperature <= LREAL#49.92 THEN
        QualifiedRelayHigh := TRUE;
    END_IF;

    IF RejectedTemperature >= LREAL#50.08 THEN
        RejectedRelayHigh := FALSE;
    ELSIF RejectedTemperature <= LREAL#49.92 THEN
        RejectedRelayHigh := TRUE;
    END_IF;

    IF QualifiedRelayHigh THEN
        QualifiedRelayApplied := LREAL#20.0 + RelayAmplitude;
    ELSE
        QualifiedRelayApplied := LREAL#20.0 - RelayAmplitude;
    END_IF;

    IF RejectedRelayHigh THEN
        RejectedRelayApplied := LREAL#20.2;
    ELSE
        RejectedRelayApplied := LREAL#19.8;
    END_IF;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX8.0 : BOOL;
    P1.SafetyAbort AT %MX8.1 : BOOL;
    P1.InputsValid AT %MX8.2 : BOOL;
    P1.RelayAmplitude AT %ML0 : LREAL;
    P1.QualifiedRelayApplied AT %QL0 : LREAL;
    P1.RejectedRelayApplied AT %QL8 : LREAL;
    P1.QualifiedTemperature AT %IL0 : LREAL;
    P1.RejectedTemperature AT %IL8 : LREAL;
END_VAR
END_CONFIGURATION
