USING Control;

(* ReactorTrain coordinates the cascade, feedforward, split-range, override, and tracking controllers used by the jacketed reactor. It computes requested valve commands while Main owns plant I/O and operating-state authority. *)
FUNCTION_BLOCK ReactorTrain
VAR
    OuterController : Control.PID;
    InnerController : Control.PID;
    OuterInput : Control.PIDInput;
    InnerInput : Control.PIDInput;
    FeedforwardGain : LREAL := 0.0;
    ProtectionLatched : BOOL := FALSE;
    LastSelectedDemand : LREAL := 50.0;
    LastJacketSetpoint : LREAL := 80.0;
    Configured : BOOL := FALSE;
END_VAR
VAR_OUTPUT
    ReactorTemperature : LREAL := 80.0;
    JacketTemperature : LREAL := 80.0;
    JacketSetpoint : LREAL := 80.0;
    OuterFeedbackOutput : LREAL := 80.0;
    FeedforwardContribution : LREAL := 0.0;
    NormalUtilityDemand : LREAL := 50.0;
    ProtectionDemand : LREAL := 100.0;
    SelectedUtilityDemand : LREAL := 50.0;
    HeatingRequested : LREAL := 50.0;
    CoolingRequested : LREAL := 0.0;
    HeatingApplied : LREAL := 50.0;
    CoolingApplied : LREAL := 0.0;
    ProtectionActive : BOOL := FALSE;
    TrackingActive : BOOL := TRUE;
    ActiveAuthority : INT := 1;
    AuthorityFault : BOOL := FALSE;
END_VAR

METHOD PUBLIC Configure
VAR_INPUT
    MeasuredFeedforwardGain : LREAL;
    InitialReactorTemperature : LREAL;
    InitialJacketTemperature : LREAL;
END_VAR
VAR
    OuterParameters : Control.PIDParameters;
    InnerParameters : Control.PIDParameters;
    OuterState : Control.PIDInitialState;
    InnerState : Control.PIDInitialState;
END_VAR

FeedforwardGain := MeasuredFeedforwardGain;

OuterParameters.ProportionalGain := LREAL#1.0;
OuterParameters.IntegralGain := LREAL#0.08;
OuterParameters.DerivativeGain := LREAL#0.0;
OuterParameters.ProportionalSetpointWeight := LREAL#1.0;
OuterParameters.DerivativeSetpointWeight := LREAL#0.0;
OuterParameters.Bias := LREAL#80.0;
OuterParameters.OutputMinimum := LREAL#70.0;
OuterParameters.OutputMaximum := LREAL#90.0;
OuterParameters.RateMinimum := LREAL#-10.0;
OuterParameters.RateMaximum := LREAL#10.0;
OuterParameters.TrackingTime := LREAL#1.0;
OuterParameters.AntiWindup := PIDAntiWindup#Combined;

(* The PI zero cancels the qualified local jacket pole at -0.5 1/s. *)
InnerParameters.ProportionalGain := LREAL#10.0;
InnerParameters.IntegralGain := LREAL#5.0;
InnerParameters.DerivativeGain := LREAL#0.0;
InnerParameters.ProportionalSetpointWeight := LREAL#1.0;
InnerParameters.DerivativeSetpointWeight := LREAL#0.0;
InnerParameters.Bias := LREAL#50.0;
InnerParameters.OutputMinimum := LREAL#-100.0;
InnerParameters.OutputMaximum := LREAL#100.0;
InnerParameters.RateMinimum := LREAL#-50.0;
InnerParameters.RateMaximum := LREAL#50.0;
InnerParameters.TrackingTime := LREAL#0.5;
InnerParameters.AntiWindup := PIDAntiWindup#Combined;

OuterState.Setpoint := InitialReactorTemperature;
OuterState.ProcessValue := InitialReactorTemperature;
OuterState.NominalOutput := InitialJacketTemperature;
OuterState.SaturatedOutput := InitialJacketTemperature;
InnerState.Setpoint := InitialJacketTemperature;
InnerState.ProcessValue := InitialJacketTemperature;
InnerState.NominalOutput := LREAL#50.0;
InnerState.SaturatedOutput := LREAL#50.0;
OuterController.Configure(Parameters := OuterParameters);
InnerController.Configure(Parameters := InnerParameters);
OuterController.Initialize(State := OuterState);
InnerController.Initialize(State := InnerState);

ReactorTemperature := InitialReactorTemperature;
JacketTemperature := InitialJacketTemperature;
JacketSetpoint := InitialJacketTemperature;
LastJacketSetpoint := InitialJacketTemperature;
SelectedUtilityDemand := LREAL#50.0;
LastSelectedDemand := LREAL#50.0;
Configured := TRUE;
END_METHOD

METHOD PUBLIC Update : BOOL
VAR_INPUT
    OperatingMode : DINT;
    ReactorSetpoint : LREAL;
    JacketCommissioningSetpoint : LREAL;
    MeasuredReactorTemperature : LREAL;
    MeasuredJacketTemperature : LREAL;
    FeedThermalLoad : LREAL;
    FeedLoadQualityGood : BOOL;
    ReactorQualityGood : BOOL;
    JacketQualityGood : BOOL;
    HeatingAvailable : BOOL;
    CoolingAvailable : BOOL;
    ManualUtilityDemand : LREAL;
    CycleTime : LREAL;
END_VAR
VAR
    CandidateDemand : LREAL;
    SafeRequested : BOOL;
END_VAR

IF NOT Configured THEN
    Update := FALSE;
ELSE
    ReactorTemperature := MeasuredReactorTemperature;
    JacketTemperature := MeasuredJacketTemperature;
    AuthorityFault := (NOT ReactorQualityGood) OR (NOT JacketQualityGood)
        OR ((NOT HeatingAvailable) AND (NOT CoolingAvailable));
    FeedforwardContribution := LREAL#0.0;
    IF (OperatingMode = DINT#2) AND FeedLoadQualityGood THEN
        FeedforwardContribution := FeedforwardGain * FeedThermalLoad;
    END_IF;

    ProtectionDemand := LIMIT(
        LREAL#-100.0,
        LREAL#50.0 + LREAL#50.0 * (LREAL#84.0 - ReactorTemperature),
        LREAL#100.0
    );
    IF (OperatingMode = DINT#2) AND ReactorQualityGood THEN
        IF (NOT ProtectionLatched) AND (ReactorTemperature >= LREAL#83.5) THEN
            ProtectionLatched := TRUE;
        END_IF;
    ELSE
        ProtectionLatched := FALSE;
    END_IF;

    OuterInput.Setpoint := ReactorSetpoint;
    OuterInput.ProcessValue := ReactorTemperature;
    OuterInput.FeedForward := FeedforwardContribution;
    OuterInput.ManualOutput := LastJacketSetpoint;
    OuterInput.TrackingOutput := LastJacketSetpoint;
    OuterInput.CycleTime := CycleTime;
    InnerInput.Setpoint := LastJacketSetpoint;
    InnerInput.ProcessValue := JacketTemperature;
    InnerInput.FeedForward := LREAL#0.0;
    InnerInput.ManualOutput := LastSelectedDemand;
    InnerInput.TrackingOutput := LastSelectedDemand;
    InnerInput.CycleTime := CycleTime;

    TrackingActive := FALSE;
    SafeRequested := AuthorityFault OR (OperatingMode = DINT#0);
    IF SafeRequested THEN
        OuterInput.Setpoint := ReactorTemperature;
        OuterInput.FeedForward := LREAL#0.0;
        OuterInput.Mode := PIDMode#Track;
        OuterController.Update(Input := OuterInput);
        InnerInput.Setpoint := JacketTemperature;
        InnerInput.Mode := PIDMode#Track;
        InnerInput.TrackingOutput := LREAL#0.0;
        InnerController.Update(Input := InnerInput);
        JacketSetpoint := JacketTemperature;
        NormalUtilityDemand := LREAL#0.0;
        CandidateDemand := LREAL#0.0;
        ActiveAuthority := 0;
        TrackingActive := TRUE;
    ELSIF OperatingMode = DINT#1 THEN
        (* The utility-facing jacket loop is commissioned while the slower
           reactor loop tracks the jacket setpoint that is actually in use. *)
        OuterInput.Setpoint := ReactorTemperature;
        OuterInput.FeedForward := LREAL#0.0;
        OuterInput.Mode := PIDMode#Track;
        OuterInput.TrackingOutput := JacketCommissioningSetpoint;
        OuterController.Update(Input := OuterInput);
        JacketSetpoint := JacketCommissioningSetpoint;
        InnerInput.Setpoint := JacketSetpoint;
        InnerInput.Mode := PIDMode#Auto;
        NormalUtilityDemand := InnerController.Update(Input := InnerInput);
        CandidateDemand := NormalUtilityDemand;
        ActiveAuthority := 1;
        TrackingActive := TRUE;
    ELSIF OperatingMode = DINT#2 THEN
        IF ProtectionLatched THEN
            OuterInput.Setpoint := ReactorTemperature;
            OuterInput.FeedForward := LREAL#0.0;
            OuterInput.Mode := PIDMode#Track;
            OuterInput.TrackingOutput := LastJacketSetpoint;
            OuterController.Update(Input := OuterInput);
            JacketSetpoint := LastJacketSetpoint;
            InnerInput.Setpoint := JacketTemperature;
            InnerInput.Mode := PIDMode#Track;
            InnerInput.TrackingOutput := LastSelectedDemand;
            NormalUtilityDemand := InnerController.Update(Input := InnerInput);
            (* Lower signed demand always means less heating or more cooling. *)
            CandidateDemand := MIN(NormalUtilityDemand, ProtectionDemand);
            ActiveAuthority := 3;
            TrackingActive := TRUE;
        ELSE
            OuterInput.Mode := PIDMode#Auto;
            OuterFeedbackOutput := OuterController.Update(Input := OuterInput);
            JacketSetpoint := OuterFeedbackOutput;
            InnerInput.Setpoint := JacketSetpoint;
            InnerInput.Mode := PIDMode#Auto;
            NormalUtilityDemand := InnerController.Update(Input := InnerInput);
            CandidateDemand := NormalUtilityDemand;
            ActiveAuthority := 2;
        END_IF;
    ELSIF OperatingMode = DINT#3 THEN
        OuterInput.Setpoint := ReactorTemperature;
        OuterInput.FeedForward := LREAL#0.0;
        OuterInput.Mode := PIDMode#Track;
        OuterInput.TrackingOutput := LastJacketSetpoint;
        OuterController.Update(Input := OuterInput);
        InnerInput.Setpoint := JacketTemperature;
        InnerInput.Mode := PIDMode#Track;
        InnerInput.TrackingOutput := ManualUtilityDemand;
        NormalUtilityDemand := InnerController.Update(Input := InnerInput);
        JacketSetpoint := LastJacketSetpoint;
        CandidateDemand := ManualUtilityDemand;
        ActiveAuthority := 4;
        TrackingActive := TRUE;
    ELSE
        OuterInput.Setpoint := ReactorTemperature;
        OuterInput.FeedForward := LREAL#0.0;
        OuterInput.Mode := PIDMode#Track;
        OuterInput.TrackingOutput := LREAL#70.0;
        OuterController.Update(Input := OuterInput);
        JacketSetpoint := LREAL#70.0;
        InnerInput.Setpoint := JacketSetpoint;
        InnerInput.Mode := PIDMode#Auto;
        NormalUtilityDemand := InnerController.Update(Input := InnerInput);
        IF ReactorTemperature <= LREAL#75.0 THEN
            CandidateDemand := LREAL#0.0;
        ELSE
            CandidateDemand := NormalUtilityDemand;
        END_IF;
        ActiveAuthority := 5;
        TrackingActive := TRUE;
    END_IF;

    IF ProtectionLatched AND (ReactorTemperature <= LREAL#82.5)
        AND (NormalUtilityDemand <= ProtectionDemand) THEN
        ProtectionLatched := FALSE;
    END_IF;
    ProtectionActive := ProtectionLatched;

    IF SafeRequested THEN
        SelectedUtilityDemand := LREAL#0.0;
    ELSE
        SelectedUtilityDemand := LIMIT(
            LastSelectedDemand - LREAL#5.0,
            CandidateDemand,
            LastSelectedDemand + LREAL#5.0
        );
    END_IF;
    SelectedUtilityDemand := LIMIT(
        LREAL#-100.0,
        SelectedUtilityDemand,
        LREAL#100.0
    );

    HeatingRequested := LREAL#0.0;
    CoolingRequested := LREAL#0.0;
    IF SelectedUtilityDemand >= LREAL#0.0 THEN
        HeatingRequested := SelectedUtilityDemand;
    ELSE
        CoolingRequested := -SelectedUtilityDemand;
    END_IF;

    IF (HeatingRequested > LREAL#0.0) AND (NOT HeatingAvailable) THEN
        AuthorityFault := TRUE;
    END_IF;
    IF (CoolingRequested > LREAL#0.0) AND (NOT CoolingAvailable) THEN
        AuthorityFault := TRUE;
    END_IF;
    IF AuthorityFault THEN
        HeatingApplied := LREAL#0.0;
        CoolingApplied := LREAL#0.0;
        ActiveAuthority := 0;
    ELSE
        (* The characterized split-range map is mutually exclusive by
           construction; applied feedback remains separate for real hardware. *)
        HeatingApplied := HeatingRequested;
        CoolingApplied := CoolingRequested;
    END_IF;

    LastSelectedDemand := HeatingApplied - CoolingApplied;
    LastJacketSetpoint := JacketSetpoint;
    Update := NOT AuthorityFault;
END_IF;
END_METHOD
END_FUNCTION_BLOCK
