FUNCTION Lesson12AccumulatedChange : LREAL
VAR_INPUT
    ChangeRate : LREAL;
    ElapsedTime : LREAL;
END_VAR
Lesson12AccumulatedChange := ChangeRate * ElapsedTime;
END_FUNCTION

FUNCTION Lesson12SampledWave : LREAL
VAR_INPUT
    Frequency : LREAL;
    SampleInterval : LREAL;
    SampleNumber : DINT;
    ReverseSign : BOOL;
END_VAR
VAR
    Value : LREAL;
END_VAR
Value := SIN(
    LREAL#2.0 * LREAL#3.141592653589793 * Frequency
    * SampleInterval * DINT_TO_LREAL(SampleNumber)
);
IF ReverseSign THEN
    Value := -Value;
END_IF;
Lesson12SampledWave := Value;
END_FUNCTION

PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    ExperimentCycle : DINT := 0;
    ScriptedElapsedTime : LREAL := 0.1;
    AssumedElapsedTime : LREAL := 0.1;
    ChangeRate : LREAL := 0.5;
    ChangeUsingScriptedTime : LREAL := 0.0;
    ChangeUsingAssumedTime : LREAL := 0.0;
    AccumulatedUsingScriptedTime : LREAL := 0.0;
    AccumulatedUsingAssumedTime : LREAL := 0.0;
    SlowWaveSample : LREAL := 0.0;
    FastWaveSample : LREAL := 0.0;
END_VAR

IF ExperimentEnabled THEN
    IF NOT ExperimentWasEnabled THEN
        ExperimentCycle := 0;
        AccumulatedUsingScriptedTime := LREAL#0.0;
        AccumulatedUsingAssumedTime := LREAL#0.0;
    END_IF;

    IF ExperimentCycle < 20 THEN
        (* Cycle 10 represents the update after one scheduled 100 ms update was
           missed. The process has therefore continued for 200 ms. *)
        IF ExperimentCycle = 10 THEN
            ScriptedElapsedTime := LREAL#0.2;
        ELSE
            ScriptedElapsedTime := LREAL#0.1;
        END_IF;

        ChangeUsingScriptedTime := Lesson12AccumulatedChange(
            ChangeRate := ChangeRate,
            ElapsedTime := ScriptedElapsedTime
        );
        ChangeUsingAssumedTime := Lesson12AccumulatedChange(
            ChangeRate := ChangeRate,
            ElapsedTime := AssumedElapsedTime
        );
        AccumulatedUsingScriptedTime := AccumulatedUsingScriptedTime
            + ChangeUsingScriptedTime;
        AccumulatedUsingAssumedTime := AccumulatedUsingAssumedTime
            + ChangeUsingAssumedTime;

        (* A 2 Hz wave and a sign-reversed 8 Hz wave have equal values when
           both are sampled ten times per second. They differ between samples. *)
        SlowWaveSample := Lesson12SampledWave(
            Frequency := LREAL#2.0,
            SampleInterval := LREAL#0.1,
            SampleNumber := ExperimentCycle,
            ReverseSign := FALSE
        );
        FastWaveSample := Lesson12SampledWave(
            Frequency := LREAL#8.0,
            SampleInterval := LREAL#0.1,
            SampleNumber := ExperimentCycle,
            ReverseSign := TRUE
        );
        ExperimentCycle := ExperimentCycle + 1;
    END_IF;
ELSE
    ExperimentCycle := 0;
    ScriptedElapsedTime := LREAL#0.1;
    ChangeUsingScriptedTime := LREAL#0.0;
    ChangeUsingAssumedTime := LREAL#0.0;
    AccumulatedUsingScriptedTime := LREAL#0.0;
    AccumulatedUsingAssumedTime := LREAL#0.0;
    SlowWaveSample := LREAL#0.0;
    FastWaveSample := LREAL#0.0;
END_IF;
ExperimentWasEnabled := ExperimentEnabled;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ScriptedElapsedTime AT %ML0 : LREAL;
    P1.AssumedElapsedTime AT %ML8 : LREAL;
    P1.ChangeUsingScriptedTime AT %ML16 : LREAL;
    P1.ChangeUsingAssumedTime AT %ML24 : LREAL;
    P1.AccumulatedUsingScriptedTime AT %ML32 : LREAL;
    P1.AccumulatedUsingAssumedTime AT %ML40 : LREAL;
    P1.SlowWaveSample AT %ML48 : LREAL;
    P1.FastWaveSample AT %ML56 : LREAL;
    P1.ExperimentEnabled AT %MX80.0 : BOOL;
END_VAR
END_CONFIGURATION
