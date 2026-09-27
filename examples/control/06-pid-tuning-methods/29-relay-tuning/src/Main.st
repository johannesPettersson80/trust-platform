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
    RelayAmplitude : LREAL := 5.0; (* Percentage points; choose 5..10 before starting. *)
    ActiveAmplitude : LREAL := 0.0;
    ExperimentElapsed : LREAL := 0.0;
    QualifiedRelayHigh : BOOL := TRUE;
    QualifiedRelayApplied : LREAL := 75.0;
    QualifiedTemperature : LREAL := 35.0;
    Started : BOOL := FALSE;
    Finished : BOOL := FALSE;
    Ticks : UINT := 0;
    PreviousHigh : BOOL := TRUE;
    CycleStartS : LREAL := 0.0;
    CycleUpperC : LREAL := 35.0;
    CycleLowerC : LREAL := 35.0;
    CompletedCycles : LREAL := 0.0;
    RetainedCycles : LREAL := 0.0;
    LastAmplitudeC : LREAL := 0.0;
    LastPeriodS : LREAL := 0.0;
    SumAmplitudeC : LREAL := 0.0;
    SumPeriodS : LREAL := 0.0;
    MeanAmplitudeC : LREAL := 0.0;
    MeanPeriodS : LREAL := 0.0;
    MinimumAmplitudeC : LREAL := 0.0;
    MaximumAmplitudeC : LREAL := 0.0;
    MinimumPeriodS : LREAL := 0.0;
    MaximumPeriodS : LREAL := 0.0;
    CandidateValid : BOOL := FALSE;
    CandidateKu : LREAL := 0.0;
    CandidateKp : LREAL := 0.0;
    CandidateTi : LREAL := 0.0;
    CandidateKi : LREAL := 0.0;
END_VAR

(* Ordinary PLC input comes from the native heater. A failed comparison also
   rejects NaN; the latched abort requires a full runtime/plant restart. *)
InputsValid := QualifiedTemperature >= LREAL#33.0
    AND QualifiedTemperature <= LREAL#37.0
    AND RelayAmplitude >= LREAL#5.0 AND RelayAmplitude <= LREAL#10.0;
IF Started AND RelayAmplitude <> ActiveAmplitude THEN InputsValid := FALSE; END_IF;
IF NOT InputsValid THEN SafetyAbort := TRUE; END_IF;

QualifiedRelayApplied := LREAL#75.0;
IF SafetyAbort THEN
    QualifiedRelayApplied := LREAL#0.0;
    CandidateValid := FALSE;
    Finished := TRUE;
ELSIF ExperimentEnabled AND NOT Finished AND Ticks < 600 THEN
    IF NOT Started THEN
        Started := TRUE;
        ActiveAmplitude := RelayAmplitude;
        QualifiedRelayHigh := TRUE;
        CycleUpperC := QualifiedTemperature;
        CycleLowerC := QualifiedTemperature;
    END_IF;
    ExperimentElapsed := UINT_TO_LREAL(Ticks) * LREAL#0.1;
    PreviousHigh := QualifiedRelayHigh;
    QualifiedRelayHigh := Lesson29RelayHighState(
        CurrentHighState := QualifiedRelayHigh,
        MeasuredTemperature := QualifiedTemperature,
        CenterTemperature := LREAL#35.0,
        HysteresisHalfWidth := LREAL#0.1
    );
    IF QualifiedTemperature > CycleUpperC THEN CycleUpperC := QualifiedTemperature; END_IF;
    IF QualifiedTemperature < CycleLowerC THEN CycleLowerC := QualifiedTemperature; END_IF;

    (* A low-to-high crossing ends a complete high/low cycle. The initial
       partial/transient response is included among the two discarded cycles. *)
    IF QualifiedRelayHigh AND NOT PreviousHigh THEN
        CompletedCycles := CompletedCycles + LREAL#1.0;
        LastAmplitudeC := (CycleUpperC - CycleLowerC) / LREAL#2.0;
        LastPeriodS := ExperimentElapsed - CycleStartS;
        IF CompletedCycles > LREAL#2.0 THEN
            RetainedCycles := RetainedCycles + LREAL#1.0;
            SumAmplitudeC := SumAmplitudeC + LastAmplitudeC;
            SumPeriodS := SumPeriodS + LastPeriodS;
            IF RetainedCycles = LREAL#1.0 THEN
                MinimumAmplitudeC := LastAmplitudeC;
                MaximumAmplitudeC := LastAmplitudeC;
                MinimumPeriodS := LastPeriodS;
                MaximumPeriodS := LastPeriodS;
            END_IF;
            IF LastAmplitudeC < MinimumAmplitudeC THEN MinimumAmplitudeC := LastAmplitudeC; END_IF;
            IF LastAmplitudeC > MaximumAmplitudeC THEN MaximumAmplitudeC := LastAmplitudeC; END_IF;
            IF LastPeriodS < MinimumPeriodS THEN MinimumPeriodS := LastPeriodS; END_IF;
            IF LastPeriodS > MaximumPeriodS THEN MaximumPeriodS := LastPeriodS; END_IF;
            MeanAmplitudeC := SumAmplitudeC / RetainedCycles;
            MeanPeriodS := SumPeriodS / RetainedCycles;
        END_IF;
        CycleStartS := ExperimentElapsed;
        CycleUpperC := QualifiedTemperature;
        CycleLowerC := QualifiedTemperature;
    END_IF;
    QualifiedRelayApplied := Lesson29RelayOutput(
        Bias := LREAL#75.0, Amplitude := RelayAmplitude, HighState := QualifiedRelayHigh
    );
    Ticks := Ticks + 1;
ELSIF Started AND NOT Finished THEN
    (* Normal completion retains the measured scalars. No candidate is applied.
       A new amplitude or PI trial requires a full 35°C/75% plant reset. *)
    Finished := TRUE;
    ExperimentElapsed := UINT_TO_LREAL(Ticks) * LREAL#0.1;
    CandidateValid := Ticks = 600 AND RetainedCycles >= LREAL#3.0
        AND MeanAmplitudeC > LREAL#0.0 AND MeanPeriodS > LREAL#0.0
        AND MinimumAmplitudeC >= LREAL#0.9 * MeanAmplitudeC
        AND MaximumAmplitudeC <= LREAL#1.1 * MeanAmplitudeC
        AND MinimumPeriodS >= LREAL#0.9 * MeanPeriodS
        AND MaximumPeriodS <= LREAL#1.1 * MeanPeriodS;
    IF CandidateValid THEN
        CandidateKu := Lesson29CriticalGainEstimate(
            RelayAmplitude := RelayAmplitude, ProcessAmplitude := MeanAmplitudeC
        );
        CandidateKp := Lesson29HistoricalProportionalGain(UltimateGain := CandidateKu);
        CandidateTi := Lesson29HistoricalIntegralTime(UltimatePeriodSeconds := MeanPeriodS);
        CandidateKi := CandidateKp / CandidateTi;
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
    P1.QualifiedRelayHigh AT %MX8.3 : BOOL;
    P1.CandidateValid AT %MX8.4 : BOOL;
    P1.QualifiedRelayApplied AT %QL0 : LREAL;
    P1.QualifiedTemperature AT %IL0 : LREAL;
    P1.CompletedCycles AT %ML32 : LREAL;
    P1.RetainedCycles AT %ML40 : LREAL;
    P1.LastAmplitudeC AT %ML48 : LREAL;
    P1.LastPeriodS AT %ML56 : LREAL;
    P1.MeanAmplitudeC AT %ML64 : LREAL;
    P1.MeanPeriodS AT %ML72 : LREAL;
    P1.CandidateKp AT %ML80 : LREAL;
    P1.CandidateKi AT %ML88 : LREAL;
END_VAR
END_CONFIGURATION
