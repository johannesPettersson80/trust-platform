USING Control;

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
Value := SIN(LREAL#2.0 * LREAL#3.141592653589793 * Frequency
    * SampleInterval * DINT_TO_LREAL(SampleNumber));
IF ReverseSign THEN Value := -Value; END_IF;
Lesson12SampledWave := Value;
END_FUNCTION

PROGRAM Main
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#10.397336997569598,
        IntegralGain := LREAL#2.250325442359752,
        DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#0.5,
        DerivativeSetpointWeight := LREAL#0.0,
        Bias := LREAL#0.0,
        OutputMinimum := LREAL#0.0,
        OutputMaximum := LREAL#100.0,
        RateMinimum := LREAL#-10000.0,
        RateMaximum := LREAL#10000.0,
        TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#BackCalculation
    );
    InitialState : Control.PIDInitialState;
    ObservedState : Control.PIDInitialState;
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
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
    TemperatureC : LREAL := 35.0;
    DeliveredTemperatureC : LREAL := 35.0;
    SetpointC : LREAL := 35.0;
    AppliedPowerPct : LREAL := 75.0;
    RequestedPowerPct : LREAL := 75.0;
    IntegralPct : LREAL := 75.0;
END_VAR

DeliveredTemperatureC := TemperatureC;
IF NOT (TemperatureC >= LREAL#0.0 AND TemperatureC <= LREAL#80.0) THEN
    SafetyAbort := TRUE;
END_IF;
Parameters.ProportionalGain := LREAL#10.397336997569598;
Parameters.IntegralGain := LREAL#2.250325442359752;
Controller.Configure(Parameters := Parameters);
Input.Setpoint := SetpointC;
Input.ProcessValue := TemperatureC;
Input.ManualOutput := LREAL#75.0;
Input.FeedForward := LREAL#0.0;
Input.TrackingOutput := LREAL#75.0;
IF ExperimentEnabled AND NOT SafetyAbort THEN Input.Mode := PIDMode#Auto;
ELSE Input.Mode := PIDMode#Manual; END_IF;

IF NOT Initialized THEN
    InitialState.Setpoint := LREAL#35.0;
    InitialState.ProcessValue := TemperatureC;
    InitialState.NominalOutput := LREAL#75.0;
    InitialState.SaturatedOutput := LREAL#75.0;
    InitialState.SetpointRate := LREAL#0.0;
    InitialState.ProcessValueRate := LREAL#0.0;
    InitialState.FeedForward := LREAL#0.0;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;

IF ExperimentEnabled THEN
    IF NOT ExperimentWasEnabled THEN
        ExperimentCycle := 0;
        AccumulatedUsingScriptedTime := LREAL#0.0;
        AccumulatedUsingAssumedTime := LREAL#0.0;
    END_IF;
    IF ExperimentCycle < 20 THEN
        IF ExperimentCycle = 10 THEN ScriptedElapsedTime := LREAL#0.2;
        ELSE ScriptedElapsedTime := LREAL#0.1; END_IF;
        ChangeUsingScriptedTime := Lesson12AccumulatedChange(
            ChangeRate := ChangeRate, ElapsedTime := ScriptedElapsedTime);
        ChangeUsingAssumedTime := Lesson12AccumulatedChange(
            ChangeRate := ChangeRate, ElapsedTime := AssumedElapsedTime);
        AccumulatedUsingScriptedTime := AccumulatedUsingScriptedTime
            + ChangeUsingScriptedTime;
        AccumulatedUsingAssumedTime := AccumulatedUsingAssumedTime
            + ChangeUsingAssumedTime;
        SlowWaveSample := Lesson12SampledWave(
            Frequency := LREAL#2.0, SampleInterval := LREAL#0.1,
            SampleNumber := ExperimentCycle, ReverseSign := FALSE);
        FastWaveSample := Lesson12SampledWave(
            Frequency := LREAL#8.0, SampleInterval := LREAL#0.1,
            SampleNumber := ExperimentCycle, ReverseSign := TRUE);
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
Input.CycleTime := ScriptedElapsedTime;
IF SafetyAbort THEN
    Input.Mode := PIDMode#Manual;
    Input.ManualOutput := LREAL#0.0;
    Input.TrackingOutput := LREAL#0.0;
END_IF;
AppliedPowerPct := Controller.Update(Input := Input);
ObservedState := Controller.GetState();
RequestedPowerPct := ObservedState.NominalOutput;
IntegralPct := ObservedState.NominalOutput;
IF SafetyAbort THEN AppliedPowerPct := LREAL#0.0; END_IF;
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
    P1.DeliveredTemperatureC AT %ML64 : LREAL;
    P1.RequestedPowerPct AT %ML72 : LREAL;
    P1.IntegralPct AT %ML80 : LREAL;
    P1.ExperimentEnabled AT %MX80.0 : BOOL;
    P1.SafetyAbort AT %MX80.1 : BOOL;
    P1.TemperatureC AT %IL0 : LREAL;
    P1.AppliedPowerPct AT %QL0 : LREAL;
END_VAR
END_CONFIGURATION
