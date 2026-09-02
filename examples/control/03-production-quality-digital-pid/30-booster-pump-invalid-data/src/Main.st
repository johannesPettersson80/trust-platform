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
    InitializeRequest : BOOL := FALSE;
    ApplyConfiguration : BOOL := FALSE;
    SensorQualityGood : BOOL := TRUE;
    DeadlineMissed : BOOL := FALSE;
    AcknowledgeRequest : BOOL := FALSE;
    ResetRequest : BOOL := FALSE;
    EnableAutoRequest : BOOL := FALSE;
    Setpoint : LREAL := 6.0;
    ProcessValue : LREAL := 6.0;
    ActualValvePosition : LREAL := 40.0;
    FeedForward : LREAL := 0.0;
    SensorAgeSeconds : LREAL := 0.0;
    MeasuredCycleTime : LREAL := 0.1;
    CandidateOutputMinimum : LREAL := 0.0;
    CandidateOutputMaximum : LREAL := 100.0;
    CandidateProportionalGain : LREAL := 5.0;
    CandidateIntegralGain : LREAL := 1.0;
    CandidateTrackingTime : LREAL := 1.0;
    CandidateRateLimit : LREAL := 20.0;
    ValveCommand : LREAL := 30.0;
    FaultCode : INT := 0;
    DetectedFaultCode : INT := 0;
    ActiveRevision : INT := 0;
    RecoveryValidCount : INT := 0;
    FaultLatched : BOOL := FALSE;
    FaultAcknowledged : BOOL := FALSE;
    RecoveryReady : BOOL := FALSE;
    RecoveryComplete : BOOL := FALSE;
    ControllerCalledThisScan : BOOL := FALSE;
    ConfiguredThisScan : BOOL := FALSE;
    AutoAuthorized : BOOL := FALSE;
    ControllerReady : BOOL := FALSE;
    NumericRangeValid : BOOL := FALSE;
    MeasurementFresh : BOOL := FALSE;
    TimingValid : BOOL := FALSE;
    CandidateConfigurationValid : BOOL := FALSE;
    ActiveConfigurationValid : BOOL := FALSE;
    RecoveryInputValid : BOOL := FALSE;
END_VAR

ControllerCalledThisScan := FALSE;
ConfiguredThisScan := FALSE;

(* Control Studio writes one reviewed Boolean. The project owns the invalid
   candidate, correction, acknowledgement, three qualification scans, reset,
   renewed Auto, and safe restoration as separate observable scans. *)
IF ExperimentEnabled THEN
    IF NOT ExperimentWasEnabled THEN
        ExperimentCycle := ULINT#0;
    END_IF;
    ExperimentCycle := ExperimentCycle + ULINT#1;
    InitializeRequest := FALSE;
    ApplyConfiguration := FALSE;
    AcknowledgeRequest := FALSE;
    ResetRequest := FALSE;
    EnableAutoRequest := FALSE;
    SensorQualityGood := TRUE;
    SensorAgeSeconds := LREAL#0.0;
    MeasuredCycleTime := LREAL#0.1;
    DeadlineMissed := FALSE;
    IF ExperimentCycle = ULINT#1 THEN
        Setpoint := LREAL#6.0;
        ActualValvePosition := LREAL#40.0;
        CandidateOutputMinimum := LREAL#0.0;
        CandidateOutputMaximum := LREAL#100.0;
        InitializeRequest := TRUE;
    ELSIF ExperimentCycle = ULINT#2 THEN
        CandidateOutputMinimum := LREAL#80.0;
        CandidateOutputMaximum := LREAL#20.0;
        ApplyConfiguration := TRUE;
    ELSIF ExperimentCycle = ULINT#3 THEN
        CandidateOutputMinimum := LREAL#0.0;
        CandidateOutputMaximum := LREAL#100.0;
    ELSIF ExperimentCycle = ULINT#4 THEN
        AcknowledgeRequest := TRUE;
    ELSIF ExperimentCycle = ULINT#8 THEN
        Setpoint := ProcessValue;
        ActualValvePosition := LREAL#30.0;
        ResetRequest := TRUE;
    ELSIF ExperimentCycle >= ULINT#9 THEN
        Setpoint := ProcessValue;
        ActualValvePosition := LREAL#30.0;
        EnableAutoRequest := TRUE;
    END_IF;
ELSIF ExperimentWasEnabled THEN
    ExperimentCycle := ULINT#0;
    InitializeRequest := FALSE;
    ApplyConfiguration := FALSE;
    AcknowledgeRequest := FALSE;
    ResetRequest := FALSE;
    EnableAutoRequest := FALSE;
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
    ActiveParameters.RateMinimum := LREAL#-20.0;
    ActiveParameters.RateMaximum := LREAL#20.0;
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

    ValveCommand := ActualValvePosition;
    FaultCode := 0;
    ActiveRevision := 0;
    RecoveryValidCount := 0;
    FaultLatched := FALSE;
    FaultAcknowledged := FALSE;
    RecoveryReady := FALSE;
    RecoveryComplete := FALSE;
    AutoAuthorized := FALSE;
    ControllerReady := TRUE;
END_IF;

(* Range comparisons reject infinities and also reject NaN because every
   ordered comparison with NaN is false. Invalid values never reach PID math. *)
NumericRangeValid := (Setpoint >= LREAL#0.0) AND (Setpoint <= LREAL#12.0)
    AND (ProcessValue >= LREAL#0.0) AND (ProcessValue <= LREAL#12.0)
    AND (ActualValvePosition >= LREAL#0.0) AND (ActualValvePosition <= LREAL#100.0)
    AND (FeedForward >= LREAL#-100.0) AND (FeedForward <= LREAL#100.0);
MeasurementFresh := SensorQualityGood
    AND (SensorAgeSeconds >= LREAL#0.0)
    AND (SensorAgeSeconds <= LREAL#0.5);
TimingValid := (MeasuredCycleTime > LREAL#0.0)
    AND (MeasuredCycleTime <= LREAL#0.2);
CandidateConfigurationValid :=
    (CandidateProportionalGain >= LREAL#0.0)
    AND (CandidateProportionalGain <= LREAL#100.0)
    AND (CandidateIntegralGain >= LREAL#0.0)
    AND (CandidateIntegralGain <= LREAL#50.0)
    AND (CandidateTrackingTime >= LREAL#0.01)
    AND (CandidateTrackingTime <= LREAL#10.0)
    AND (CandidateOutputMinimum >= LREAL#0.0)
    AND (CandidateOutputMaximum <= LREAL#100.0)
    AND (CandidateOutputMinimum < CandidateOutputMaximum)
    AND (CandidateRateLimit > LREAL#0.0)
    AND (CandidateRateLimit <= LREAL#1000.0);
ActiveConfigurationValid :=
    (ActiveParameters.ProportionalGain >= LREAL#0.0)
    AND (ActiveParameters.ProportionalGain <= LREAL#100.0)
    AND (ActiveParameters.IntegralGain >= LREAL#0.0)
    AND (ActiveParameters.IntegralGain <= LREAL#50.0)
    AND (ActiveParameters.TrackingTime >= LREAL#0.01)
    AND (ActiveParameters.TrackingTime <= LREAL#10.0)
    AND (ActiveParameters.OutputMinimum >= LREAL#0.0)
    AND (ActiveParameters.OutputMaximum <= LREAL#100.0)
    AND (ActiveParameters.OutputMinimum < ActiveParameters.OutputMaximum)
    AND (ActiveParameters.RateMinimum < LREAL#0.0)
    AND (ActiveParameters.RateMaximum > LREAL#0.0);

(* Stable priority makes simultaneous failures reproducible. The first cause
   remains operator-facing until a qualified reset. *)
IF NOT NumericRangeValid THEN
    DetectedFaultCode := 1;
ELSIF NOT MeasurementFresh THEN
    DetectedFaultCode := 2;
ELSIF NOT TimingValid THEN
    DetectedFaultCode := 3;
ELSIF DeadlineMissed THEN
    DetectedFaultCode := 4;
ELSIF ApplyConfiguration AND NOT CandidateConfigurationValid THEN
    DetectedFaultCode := 5;
ELSE
    DetectedFaultCode := 0;
END_IF;

IF (DetectedFaultCode <> 0) AND NOT FaultLatched THEN
    FaultCode := DetectedFaultCode;
    FaultLatched := TRUE;
    FaultAcknowledged := FALSE;
    RecoveryValidCount := 0;
    RecoveryReady := FALSE;
    RecoveryComplete := FALSE;
    AutoAuthorized := FALSE;
END_IF;

IF FaultLatched THEN
    ValveCommand := LREAL#30.0;

    IF AcknowledgeRequest THEN
        (* Acknowledgement records awareness; it never clears the fault. *)
        FaultAcknowledged := TRUE;
    END_IF;

    (* Qualification starts after the acknowledgement scan. This preserves
       three observable valid scans instead of counting the button press as
       process evidence. *)
    RecoveryInputValid := FaultAcknowledged
        AND NOT AcknowledgeRequest
        AND (DetectedFaultCode = 0)
        AND ActiveConfigurationValid;
    IF RecoveryInputValid THEN
        IF RecoveryValidCount < 3 THEN
            RecoveryValidCount := RecoveryValidCount + 1;
        END_IF;
    ELSE
        RecoveryValidCount := 0;
    END_IF;
    RecoveryReady := RecoveryValidCount >= 3;

    IF ResetRequest AND FaultAcknowledged AND RecoveryReady THEN
        (* Rebuild controller histories from present measured state. Automatic
           authority is deliberately left false on the reset scan. *)
        Controller.Configure(Parameters := ActiveParameters);
        ConfiguredThisScan := TRUE;
        InitialState.Setpoint := Setpoint;
        InitialState.SetpointRate := LREAL#0.0;
        InitialState.ProcessValue := ProcessValue;
        InitialState.ProcessValueRate := LREAL#0.0;
        InitialState.FeedForward := FeedForward;
        InitialState.NominalOutput := ActualValvePosition;
        InitialState.SaturatedOutput := ActualValvePosition;
        Controller.Initialize(State := InitialState);
        ValveCommand := ActualValvePosition;
        FaultCode := 0;
        FaultLatched := FALSE;
        FaultAcknowledged := FALSE;
        RecoveryValidCount := 0;
        RecoveryReady := FALSE;
        RecoveryComplete := TRUE;
        AutoAuthorized := FALSE;
    END_IF;
ELSE
    IF ApplyConfiguration AND CandidateConfigurationValid THEN
        CandidateParameters.ProportionalGain := CandidateProportionalGain;
        CandidateParameters.IntegralGain := CandidateIntegralGain;
        CandidateParameters.DerivativeGain := LREAL#0.0;
        CandidateParameters.ProportionalSetpointWeight := LREAL#1.0;
        CandidateParameters.DerivativeSetpointWeight := LREAL#0.0;
        CandidateParameters.Bias := LREAL#0.0;
        CandidateParameters.OutputMinimum := CandidateOutputMinimum;
        CandidateParameters.OutputMaximum := CandidateOutputMaximum;
        CandidateParameters.RateMinimum := -CandidateRateLimit;
        CandidateParameters.RateMaximum := CandidateRateLimit;
        CandidateParameters.TrackingTime := CandidateTrackingTime;
        CandidateParameters.AntiWindup := PIDAntiWindup#Combined;
        ActiveParameters := CandidateParameters;
        Controller.Configure(Parameters := ActiveParameters);
        ActiveRevision := ActiveRevision + 1;
        ConfiguredThisScan := TRUE;
    END_IF;

    IF ControllerReady AND EnableAutoRequest THEN
        Input.Setpoint := Setpoint;
        Input.ProcessValue := ProcessValue;
        Input.FeedForward := FeedForward;
        Input.Mode := PIDMode#Auto;
        Input.CycleTime := MeasuredCycleTime;
        ValveCommand := Controller.Update(Input := Input);
        ControllerCalledThisScan := TRUE;
        AutoAuthorized := TRUE;
        RecoveryComplete := FALSE;
    ELSE
        ValveCommand := ActualValvePosition;
        AutoAuthorized := FALSE;
    END_IF;
END_IF;
ExperimentWasEnabled := ExperimentEnabled;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.InitializeRequest AT %MX64.0 : BOOL;
    P1.ApplyConfiguration AT %MX64.1 : BOOL;
    P1.SensorQualityGood AT %MX64.2 : BOOL;
    P1.DeadlineMissed AT %MX64.3 : BOOL;
    P1.AcknowledgeRequest AT %MX64.4 : BOOL;
    P1.ResetRequest AT %MX64.5 : BOOL;
    P1.EnableAutoRequest AT %MX64.6 : BOOL;
    P1.ExperimentEnabled AT %MX64.7 : BOOL;
    P1.FaultLatched AT %MX65.0 : BOOL;
    P1.FaultAcknowledged AT %MX65.1 : BOOL;
    P1.RecoveryReady AT %MX65.2 : BOOL;
    P1.RecoveryComplete AT %MX65.3 : BOOL;
    P1.ControllerCalledThisScan AT %MX65.4 : BOOL;
    P1.ConfiguredThisScan AT %MX65.5 : BOOL;
    P1.AutoAuthorized AT %MX65.6 : BOOL;
    P1.FaultCode AT %MW66 : INT;
    P1.ActiveRevision AT %MW68 : INT;
    P1.RecoveryValidCount AT %MW70 : INT;
    P1.Setpoint AT %ML0 : LREAL;
    P1.ActualValvePosition AT %ML8 : LREAL;
    P1.FeedForward AT %ML16 : LREAL;
    P1.SensorAgeSeconds AT %ML24 : LREAL;
    P1.MeasuredCycleTime AT %ML32 : LREAL;
    P1.CandidateOutputMinimum AT %ML40 : LREAL;
    P1.CandidateOutputMaximum AT %ML48 : LREAL;
    P1.CandidateProportionalGain AT %ML56 : LREAL;
    P1.CandidateIntegralGain AT %ML64 : LREAL;
    P1.CandidateTrackingTime AT %ML72 : LREAL;
    P1.ValveCommand AT %QL0 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
END_VAR
END_CONFIGURATION
