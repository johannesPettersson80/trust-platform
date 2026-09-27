USING Control;

FUNCTION Lesson41Finite : BOOL
VAR_INPUT
    Value : LREAL;
END_VAR
(* Ordered comparisons with NaN are false; the magnitude bound also rejects
   infinities while retaining the PID specification's finite arithmetic range. *)
Lesson41Finite := (Value = Value) AND (ABS(Value) <= LREAL#1.0e100);
END_FUNCTION

FUNCTION Lesson41PidParametersValid : BOOL
VAR_INPUT
    Parameters : Control.PIDParameters;
END_VAR
Lesson41PidParametersValid :=
    Lesson41Finite(Value := Parameters.ProportionalGain)
    AND Lesson41Finite(Value := Parameters.IntegralGain)
    AND Lesson41Finite(Value := Parameters.DerivativeGain)
    AND Lesson41Finite(Value := Parameters.ProportionalSetpointWeight)
    AND Lesson41Finite(Value := Parameters.DerivativeSetpointWeight)
    AND Lesson41Finite(Value := Parameters.Bias)
    AND Lesson41Finite(Value := Parameters.OutputMinimum)
    AND Lesson41Finite(Value := Parameters.OutputMaximum)
    AND Lesson41Finite(Value := Parameters.RateMinimum)
    AND Lesson41Finite(Value := Parameters.RateMaximum)
    AND Lesson41Finite(Value := Parameters.TrackingTime)
    AND Parameters.ProportionalSetpointWeight >= LREAL#0.0
    AND Parameters.ProportionalSetpointWeight <= LREAL#1.0
    AND Parameters.OutputMinimum >= LREAL#0.0
    AND Parameters.OutputMinimum <= Parameters.OutputMaximum
    AND Parameters.OutputMaximum <= LREAL#100.0
    AND Parameters.RateMinimum <= LREAL#0.0
    AND Parameters.RateMaximum >= LREAL#0.0
    AND ((Parameters.IntegralGain = LREAL#0.0)
        OR (Parameters.TrackingTime > LREAL#0.0));
END_FUNCTION

FUNCTION Lesson41PiFeedforwardIncrement : LREAL
VAR_INPUT
    ProportionalGain : LREAL;
    IntegralGain : LREAL;
    CycleTime : LREAL;
    CurrentError : LREAL;
    PreviousError : LREAL;
    FeedForwardChange : LREAL;
END_VAR
Lesson41PiFeedforwardIncrement := ProportionalGain
    * (CurrentError - PreviousError)
    + IntegralGain * CycleTime * CurrentError
    + FeedForwardChange;
END_FUNCTION

FUNCTION Lesson41RateLowerBound : LREAL
VAR_INPUT
    PreviousAppliedOutput : LREAL;
    RateMinimum : LREAL;
    CycleTime : LREAL;
    OutputMinimum : LREAL;
END_VAR
Lesson41RateLowerBound := MAX(
    OutputMinimum,
    PreviousAppliedOutput + CycleTime * RateMinimum
);
END_FUNCTION

FUNCTION Lesson41RateUpperBound : LREAL
VAR_INPUT
    PreviousAppliedOutput : LREAL;
    RateMaximum : LREAL;
    CycleTime : LREAL;
    OutputMaximum : LREAL;
END_VAR
Lesson41RateUpperBound := MIN(
    OutputMaximum,
    PreviousAppliedOutput + CycleTime * RateMaximum
);
END_FUNCTION

PROGRAM Main
VAR
    Controller : Control.PID;
    CandidateParameters : Control.PIDParameters;
    ActiveParameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    ExperimentCycle : ULINT := ULINT#0;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    InitializeRequest : BOOL := FALSE;
    ApplyConfiguration : BOOL := FALSE;
    UnsafeSequentialUpdate : BOOL := FALSE;
    Setpoint : LREAL := 6.0;
    ProcessValue : LREAL := 6.0;
    ActualValvePosition : LREAL := 40.0;
    CandidateProportionalGain : LREAL := 15.0;
    CandidateIntegralGain : LREAL := 3.0;
    CandidateTrackingTime : LREAL := 0.5;
    CandidateAccepted : BOOL := FALSE;
    FeedForward : LREAL := 0.0;
    RateLimitPerSecond : LREAL := 20.0;
    ValveCommand : LREAL := 30.0;
    ActiveProportionalGain : LREAL := 5.0;
    ActiveIntegralGain : LREAL := 1.0;
    ActiveTrackingTime : LREAL := 1.0;
    RejectedExternalSum : LREAL := 40.0;
    UnconstrainedTargetDiagnostic : LREAL := 40.0;
    PreviousSetpoint : LREAL := 6.0;
    PreviousProcessValue : LREAL := 6.0;
    PreviousFeedForward : LREAL := 0.0;
    PreviousAppliedOutput : LREAL := 40.0;
    RateLowerBound : LREAL := 38.0;
    RateUpperBound : LREAL := 42.0;
    ActiveRevision : INT := 0;
    ControllerReady : BOOL := FALSE;
    ConfiguredThisScan : BOOL := FALSE;
    MixedConfigurationObserved : BOOL := FALSE;
    RateLimited : BOOL := FALSE;
    AmplitudeLimited : BOOL := FALSE;
END_VAR

ConfiguredThisScan := FALSE;

(* One reviewed authority executes only valid production paths: an atomic
   configuration update, then a fresh aligned internal-feedforward case. *)
IF ExperimentEnabled THEN
    IF NOT ExperimentWasEnabled THEN
        ExperimentCycle := ULINT#0;
    END_IF;
    ExperimentCycle := ExperimentCycle + ULINT#1;
    InitializeRequest := FALSE;
    ApplyConfiguration := FALSE;
    UnsafeSequentialUpdate := FALSE;
    IF ExperimentCycle = ULINT#1 THEN
        Setpoint := LREAL#6.0;
        FeedForward := LREAL#0.0;
        ActualValvePosition := LREAL#40.0;
        RateLimitPerSecond := LREAL#1000.0;
        InitializeRequest := TRUE;
    ELSIF ExperimentCycle = ULINT#2 THEN
        Setpoint := LREAL#7.0;
        ApplyConfiguration := TRUE;
    ELSE
        Setpoint := LREAL#7.0;
        FeedForward := LREAL#0.0;
    END_IF;
ELSIF ExperimentWasEnabled THEN
    ExperimentCycle := ULINT#0;
    InitializeRequest := FALSE;
    ApplyConfiguration := FALSE;
    UnsafeSequentialUpdate := FALSE;
    FeedForward := LREAL#0.0;
    Setpoint := LREAL#6.0;
    ControllerReady := FALSE;
    ValveCommand := LREAL#30.0;
END_IF;

InputsValid := ProcessValue >= LREAL#0.0 AND ProcessValue <= LREAL#12.0
    AND FeedForward >= LREAL#-100.0 AND FeedForward <= LREAL#100.0;
IF SafetyAbort OR NOT InputsValid THEN
    ControllerReady := FALSE;
    ValveCommand := LREAL#30.0;
END_IF;

IF InitializeRequest THEN
    ActiveParameters.ProportionalGain := LREAL#5.0;
    ActiveParameters.IntegralGain := LREAL#1.0;
    ActiveParameters.DerivativeGain := LREAL#0.0;
    ActiveParameters.ProportionalSetpointWeight := LREAL#1.0;
    ActiveParameters.DerivativeSetpointWeight := LREAL#0.0;
    ActiveParameters.Bias := LREAL#0.0;
    ActiveParameters.OutputMinimum := LREAL#0.0;
    ActiveParameters.OutputMaximum := LREAL#100.0;
    ActiveParameters.RateMinimum := -RateLimitPerSecond;
    ActiveParameters.RateMaximum := RateLimitPerSecond;
    ActiveParameters.TrackingTime := LREAL#1.0;
    ActiveParameters.AntiWindup := PIDAntiWindup#Combined;
    Controller.Configure(Parameters := ActiveParameters);

    InitialState.Setpoint := Setpoint;
    InitialState.SetpointRate := LREAL#0.0;
    InitialState.ProcessValue := ProcessValue;
    InitialState.ProcessValueRate := LREAL#0.0;
    InitialState.FeedForward := FeedForward;
    InitialState.NominalOutput := ActualValvePosition;
    InitialState.SaturatedOutput := ActualValvePosition;
    Controller.Initialize(State := InitialState);

    ActiveProportionalGain := ActiveParameters.ProportionalGain;
    ActiveIntegralGain := ActiveParameters.IntegralGain;
    ActiveTrackingTime := ActiveParameters.TrackingTime;
    PreviousSetpoint := Setpoint;
    PreviousProcessValue := ProcessValue;
    PreviousFeedForward := FeedForward;
    PreviousAppliedOutput := ActualValvePosition;
    UnconstrainedTargetDiagnostic := ActualValvePosition;
    ValveCommand := ActualValvePosition;
    ActiveRevision := 0;
    MixedConfigurationObserved := FALSE;
    ControllerReady := TRUE;
END_IF;

IF ControllerReady THEN
    IF ApplyConfiguration THEN
        CandidateParameters.ProportionalGain := CandidateProportionalGain;
        CandidateParameters.IntegralGain := CandidateIntegralGain;
        CandidateParameters.DerivativeGain := LREAL#0.0;
        CandidateParameters.ProportionalSetpointWeight := LREAL#1.0;
        CandidateParameters.DerivativeSetpointWeight := LREAL#0.0;
        CandidateParameters.Bias := LREAL#0.0;
        CandidateParameters.OutputMinimum := LREAL#0.0;
        CandidateParameters.OutputMaximum := LREAL#100.0;
        CandidateParameters.RateMinimum := -RateLimitPerSecond;
        CandidateParameters.RateMaximum := RateLimitPerSecond;
        CandidateParameters.TrackingTime := CandidateTrackingTime;
        CandidateParameters.AntiWindup := PIDAntiWindup#Combined;

        CandidateAccepted := Lesson41PidParametersValid(
            Parameters := CandidateParameters);
        IF CandidateAccepted AND UnsafeSequentialUpdate THEN
            (* Failure demonstration only: one new field mixed with the old
               revision proves why production configuration must be atomic. *)
            ActiveParameters.ProportionalGain := CandidateParameters.ProportionalGain;
            MixedConfigurationObserved := TRUE;
        ELSIF CandidateAccepted THEN
            (* One record assignment followed by one Configure call gives the
               controller either the complete old revision or complete new one. *)
            ActiveParameters := CandidateParameters;
            ActiveRevision := ActiveRevision + 1;
            MixedConfigurationObserved := FALSE;
        END_IF;
        IF CandidateAccepted THEN
            Controller.Configure(Parameters := ActiveParameters);
            ConfiguredThisScan := TRUE;
        END_IF;
        ActiveProportionalGain := ActiveParameters.ProportionalGain;
        ActiveIntegralGain := ActiveParameters.IntegralGain;
        ActiveTrackingTime := ActiveParameters.TrackingTime;
    END_IF;

    Input.Setpoint := Setpoint;
    Input.ProcessValue := ProcessValue;
    Input.FeedForward := FeedForward;
    Input.Mode := PIDMode#Auto;
    Input.CycleTime := LREAL#0.1;

    RateLowerBound := Lesson41RateLowerBound(
        PreviousAppliedOutput := PreviousAppliedOutput,
        RateMinimum := ActiveParameters.RateMinimum,
        CycleTime := Input.CycleTime,
        OutputMinimum := ActiveParameters.OutputMinimum
    );
    RateUpperBound := Lesson41RateUpperBound(
        PreviousAppliedOutput := PreviousAppliedOutput,
        RateMaximum := ActiveParameters.RateMaximum,
        CycleTime := Input.CycleTime,
        OutputMaximum := ActiveParameters.OutputMaximum
    );
    (* This independent diagnostic accumulates the raw PI/feedforward increments
       before constraints. It exposes the unconstrained target for teaching but
       never participates in the plant-command path; Control.PID is authoritative. *)
    UnconstrainedTargetDiagnostic := UnconstrainedTargetDiagnostic
        + Lesson41PiFeedforwardIncrement(
            ProportionalGain := ActiveParameters.ProportionalGain,
            IntegralGain := ActiveParameters.IntegralGain,
            CycleTime := Input.CycleTime,
            CurrentError := Setpoint - ProcessValue,
            PreviousError := PreviousSetpoint - PreviousProcessValue,
            FeedForwardChange := FeedForward - PreviousFeedForward
        );
    AmplitudeLimited := (UnconstrainedTargetDiagnostic < ActiveParameters.OutputMinimum)
        OR (UnconstrainedTargetDiagnostic > ActiveParameters.OutputMaximum);
    RateLimited := (UnconstrainedTargetDiagnostic < RateLowerBound)
        OR (UnconstrainedTargetDiagnostic > RateUpperBound);

    (* Feedforward enters the controller before anti-windup and both limits.
       ValveCommand is the only value connected to the simulated actuator. *)
    ValveCommand := Controller.Update(Input := Input);
    RejectedExternalSum := ActualValvePosition + FeedForward;

    PreviousSetpoint := Setpoint;
    PreviousProcessValue := ProcessValue;
    PreviousFeedForward := FeedForward;
    PreviousAppliedOutput := ValveCommand;
END_IF;
ExperimentWasEnabled := ExperimentEnabled;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.InitializeRequest AT %MX120.0 : BOOL;
    P1.ApplyConfiguration AT %MX120.1 : BOOL;
    P1.UnsafeSequentialUpdate AT %MX120.2 : BOOL;
    P1.ControllerReady AT %MX120.3 : BOOL;
    P1.ConfiguredThisScan AT %MX120.4 : BOOL;
    P1.MixedConfigurationObserved AT %MX120.5 : BOOL;
    P1.RateLimited AT %MX120.6 : BOOL;
    P1.AmplitudeLimited AT %MX120.7 : BOOL;
    P1.CandidateAccepted AT %MX121.3 : BOOL;
    P1.ExperimentEnabled AT %MX121.0 : BOOL;
    P1.SafetyAbort AT %MX121.1 : BOOL;
    P1.InputsValid AT %MX121.2 : BOOL;
    P1.ActiveRevision AT %MW112 : INT;
    P1.Setpoint AT %ML0 : LREAL;
    P1.ActualValvePosition AT %ML8 : LREAL;
    P1.CandidateProportionalGain AT %ML16 : LREAL;
    P1.CandidateIntegralGain AT %ML24 : LREAL;
    P1.CandidateTrackingTime AT %ML32 : LREAL;
    P1.FeedForward AT %ML40 : LREAL;
    P1.RateLimitPerSecond AT %ML48 : LREAL;
    P1.ActiveProportionalGain AT %ML56 : LREAL;
    P1.ActiveIntegralGain AT %ML64 : LREAL;
    P1.ActiveTrackingTime AT %ML72 : LREAL;
    P1.UnconstrainedTargetDiagnostic AT %ML80 : LREAL;
    P1.RejectedExternalSum AT %ML88 : LREAL;
    P1.RateLowerBound AT %ML96 : LREAL;
    P1.RateUpperBound AT %ML104 : LREAL;
    P1.ValveCommand AT %QL0 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
END_VAR
END_CONFIGURATION
