FUNCTION Lesson29RelayHighState : BOOL
VAR_INPUT
    CurrentHighState : BOOL;
    MeasuredTemperature : LREAL;
    CenterTemperature : LREAL;
    HysteresisHalfWidth : LREAL;
END_VAR
IF MeasuredTemperature >= CenterTemperature + HysteresisHalfWidth THEN
    Lesson29RelayHighState := FALSE;
ELSIF MeasuredTemperature <= CenterTemperature - HysteresisHalfWidth THEN
    Lesson29RelayHighState := TRUE;
ELSE
    Lesson29RelayHighState := CurrentHighState;
END_IF;
END_FUNCTION

FUNCTION Lesson29RelayOutput : LREAL
VAR_INPUT
    Bias : LREAL;
    Amplitude : LREAL;
    HighState : BOOL;
END_VAR
IF HighState THEN
    Lesson29RelayOutput := Bias + Amplitude;
ELSE
    Lesson29RelayOutput := Bias - Amplitude;
END_IF;
END_FUNCTION

FUNCTION Lesson29FundamentalRelayAmplitude : LREAL
VAR_INPUT
    RelayAmplitude : LREAL;
END_VAR
Lesson29FundamentalRelayAmplitude := LREAL#4.0 * RelayAmplitude
    / LREAL#3.141592653589793;
END_FUNCTION

FUNCTION Lesson29CriticalGainEstimate : LREAL
VAR_INPUT
    RelayAmplitude : LREAL;
    ProcessAmplitude : LREAL;
END_VAR
Lesson29CriticalGainEstimate := Lesson29FundamentalRelayAmplitude(
    RelayAmplitude := RelayAmplitude
) / ProcessAmplitude;
END_FUNCTION

FUNCTION Lesson29HistoricalProportionalGain : LREAL
VAR_INPUT
    UltimateGain : LREAL;
END_VAR
Lesson29HistoricalProportionalGain := LREAL#0.45 * UltimateGain;
END_FUNCTION

FUNCTION Lesson29HistoricalIntegralTime : LREAL
VAR_INPUT
    UltimatePeriodSeconds : LREAL;
END_VAR
Lesson29HistoricalIntegralTime := UltimatePeriodSeconds / LREAL#1.2;
END_FUNCTION

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
    QualifiedRelayHigh := Lesson29RelayHighState(
        CurrentHighState := QualifiedRelayHigh,
        MeasuredTemperature := QualifiedTemperature,
        CenterTemperature := LREAL#50.0,
        HysteresisHalfWidth := LREAL#0.08
    );
    RejectedRelayHigh := Lesson29RelayHighState(
        CurrentHighState := RejectedRelayHigh,
        MeasuredTemperature := RejectedTemperature,
        CenterTemperature := LREAL#50.0,
        HysteresisHalfWidth := LREAL#0.08
    );

    QualifiedRelayApplied := Lesson29RelayOutput(
        Bias := LREAL#20.0,
        Amplitude := RelayAmplitude,
        HighState := QualifiedRelayHigh
    );
    RejectedRelayApplied := Lesson29RelayOutput(
        Bias := LREAL#20.0,
        Amplitude := LREAL#0.2,
        HighState := RejectedRelayHigh
    );
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
