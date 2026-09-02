USING Control;

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
    ELSIF ExperimentCycle = ULINT#3 THEN
        Setpoint := ProcessValue;
        FeedForward := LREAL#0.0;
        ActualValvePosition := LREAL#40.0;
        RateLimitPerSecond := LREAL#20.0;
        InitializeRequest := TRUE;
    ELSE
        Setpoint := ProcessValue;
        FeedForward := LREAL#20.0;
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

        IF UnsafeSequentialUpdate THEN
            (* Failure demonstration only: one new field mixed with the old
               revision proves why production configuration must be atomic. *)
            ActiveParameters.ProportionalGain := CandidateParameters.ProportionalGain;
            MixedConfigurationObserved := TRUE;
        ELSE
            (* One record assignment followed by one Configure call gives the
               controller either the complete old revision or complete new one. *)
            ActiveParameters := CandidateParameters;
            ActiveRevision := ActiveRevision + 1;
            MixedConfigurationObserved := FALSE;
        END_IF;
        Controller.Configure(Parameters := ActiveParameters);
        ConfiguredThisScan := TRUE;
        ActiveProportionalGain := ActiveParameters.ProportionalGain;
        ActiveIntegralGain := ActiveParameters.IntegralGain;
        ActiveTrackingTime := ActiveParameters.TrackingTime;
    END_IF;

    Input.Setpoint := Setpoint;
    Input.ProcessValue := ProcessValue;
    Input.FeedForward := FeedForward;
    Input.Mode := PIDMode#Auto;
    Input.CycleTime := LREAL#0.1;

    RateLowerBound := MAX(
        ActiveParameters.OutputMinimum,
        PreviousAppliedOutput + Input.CycleTime * ActiveParameters.RateMinimum
    );
    RateUpperBound := MIN(
        ActiveParameters.OutputMaximum,
        PreviousAppliedOutput + Input.CycleTime * ActiveParameters.RateMaximum
    );
    (* This independent diagnostic accumulates the raw PI/feedforward increments
       before constraints. It exposes the unconstrained target for teaching but
       never participates in the plant-command path; Control.PID is authoritative. *)
    UnconstrainedTargetDiagnostic := UnconstrainedTargetDiagnostic
        + ActiveParameters.ProportionalGain
            * ((Setpoint - PreviousSetpoint) - (ProcessValue - PreviousProcessValue))
        + ActiveParameters.IntegralGain * Input.CycleTime * (Setpoint - ProcessValue)
        + (FeedForward - PreviousFeedForward);
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
    P1.InitializeRequest AT %MX56.0 : BOOL;
    P1.ApplyConfiguration AT %MX56.1 : BOOL;
    P1.UnsafeSequentialUpdate AT %MX56.2 : BOOL;
    P1.ControllerReady AT %MX56.3 : BOOL;
    P1.ConfiguredThisScan AT %MX56.4 : BOOL;
    P1.MixedConfigurationObserved AT %MX56.5 : BOOL;
    P1.RateLimited AT %MX56.6 : BOOL;
    P1.AmplitudeLimited AT %MX56.7 : BOOL;
    P1.ExperimentEnabled AT %MX57.0 : BOOL;
    P1.SafetyAbort AT %MX57.1 : BOOL;
    P1.InputsValid AT %MX57.2 : BOOL;
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
