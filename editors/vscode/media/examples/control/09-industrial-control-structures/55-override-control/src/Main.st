USING Control;

FUNCTION Lesson55ControlAuthority : BOOL
VAR_INPUT
    MarginValid : BOOL;
    InputsValid : BOOL;
    SafetyAbort : BOOL;
END_VAR
Lesson55ControlAuthority := MarginValid AND InputsValid AND (NOT SafetyAbort);
END_FUNCTION

FUNCTION Lesson55OperationalAuthority : BOOL
VAR_INPUT
    InputsValid : BOOL;
    SafetyAbort : BOOL;
END_VAR
Lesson55OperationalAuthority := InputsValid AND (NOT SafetyAbort);
END_FUNCTION

FUNCTION Lesson55HighSelect : LREAL
VAR_INPUT
    NormalDemand : LREAL;
    ProtectiveDemand : LREAL;
END_VAR
Lesson55HighSelect := MAX(NormalDemand, ProtectiveDemand);
END_FUNCTION

FUNCTION Lesson55TakeoverRequired : BOOL
VAR_INPUT MarginPercent : LREAL; END_VAR
Lesson55TakeoverRequired := MarginPercent <= LREAL#14.0;
END_FUNCTION

FUNCTION Lesson55ReleasePermitted : BOOL
VAR_INPUT
    MarginPercent : LREAL;
    NormalDemand : LREAL;
    ProtectiveDemand : LREAL;
END_VAR
Lesson55ReleasePermitted := (MarginPercent >= LREAL#16.0)
    AND (NormalDemand >= ProtectiveDemand);
END_FUNCTION

FUNCTION Lesson55ReviewedPressureSetpoint : LREAL
VAR_INPUT
    ExperimentElapsedSeconds : LREAL;
    ExperimentEnabled : BOOL;
    InputsValid : BOOL;
    SafetyAbort : BOOL;
END_VAR
IF ExperimentEnabled AND InputsValid AND (NOT SafetyAbort) THEN
    IF ExperimentElapsedSeconds < LREAL#20.0 THEN
        Lesson55ReviewedPressureSetpoint := LREAL#8.0;
    ELSIF ExperimentElapsedSeconds < LREAL#80.0 THEN
        Lesson55ReviewedPressureSetpoint := LREAL#9.0;
    ELSE
        Lesson55ReviewedPressureSetpoint := LREAL#8.0;
    END_IF;
ELSE
    Lesson55ReviewedPressureSetpoint := LREAL#8.0;
END_IF;
END_FUNCTION

PROGRAM Main
VAR
    BaselineNormalController : Control.PID;
    UntrackedNormalController : Control.PID;
    UntrackedProtectController : Control.PID;
    TrackedNormalController : Control.PID;
    TrackedProtectController : Control.PID;
    NormalParameters : Control.PIDParameters;
    ProtectParameters : Control.PIDParameters;
    BaselineNormalState : Control.PIDInitialState;
    UntrackedNormalState : Control.PIDInitialState;
    UntrackedProtectState : Control.PIDInitialState;
    TrackedNormalState : Control.PIDInitialState;
    TrackedProtectState : Control.PIDInitialState;
    BaselineNormalInput : Control.PIDInput;
    UntrackedNormalInput : Control.PIDInput;
    UntrackedProtectInput : Control.PIDInput;
    TrackedNormalInput : Control.PIDInput;
    TrackedProtectInput : Control.PIDInput;
    Initialized : BOOL := FALSE;
    MarginValid : BOOL := TRUE;
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ExperimentElapsed : LREAL := 0.0;
    PressureSetpoint : LREAL := 8.0;
    MarginControlSetpoint : LREAL := 14.0;
    BaselinePressure : LREAL := 8.0;
    BaselineMargin : LREAL := 20.0;
    UntrackedPressure : LREAL := 8.0;
    UntrackedMargin : LREAL := 20.0;
    TrackedPressure : LREAL := 8.0;
    TrackedMargin : LREAL := 20.0;
    BaselineValveRequest : LREAL := 30.0;
    UntrackedValveRequest : LREAL := 30.0;
    TrackedValveRequest : LREAL := 30.0;
    UntrackedNormalDemand : LREAL := 30.0;
    UntrackedProtectDemand : LREAL := 30.0;
    TrackedNormalDemand : LREAL := 30.0;
    TrackedProtectDemand : LREAL := 30.0;
    UntrackedProtectionActive : BOOL := FALSE;
    TrackedProtectionActive : BOOL := FALSE;
    TrackedProtectionAvailable : BOOL := TRUE;
    OperationalAuthority : BOOL := TRUE;
    AutomaticMode : BOOL := TRUE;
END_VAR

IF NOT Initialized THEN
    PressureSetpoint := LREAL#8.0;
    (* Opening recycle lowers pressure, so the normal controller has negative
       action. Every candidate remains in common recycle-valve percent. *)
    NormalParameters.ProportionalGain := LREAL#-10.0;
    NormalParameters.IntegralGain := LREAL#-1.0;
    NormalParameters.DerivativeGain := LREAL#0.0;
    NormalParameters.ProportionalSetpointWeight := LREAL#1.0;
    NormalParameters.DerivativeSetpointWeight := LREAL#0.0;
    NormalParameters.Bias := LREAL#30.0;
    NormalParameters.OutputMinimum := LREAL#0.0;
    NormalParameters.OutputMaximum := LREAL#100.0;
    NormalParameters.RateMinimum := LREAL#-1000.0;
    NormalParameters.RateMaximum := LREAL#1000.0;
    NormalParameters.TrackingTime := LREAL#1.0;
    NormalParameters.AntiWindup := PIDAntiWindup#Combined;

    ProtectParameters := NormalParameters;
    ProtectParameters.ProportionalGain := LREAL#0.5;
    ProtectParameters.IntegralGain := LREAL#0.2;

    BaselineNormalState.Setpoint := PressureSetpoint;
    BaselineNormalState.ProcessValue := BaselinePressure;
    BaselineNormalState.NominalOutput := LREAL#30.0;
    BaselineNormalState.SaturatedOutput := LREAL#30.0;
    UntrackedNormalState.Setpoint := PressureSetpoint;
    UntrackedNormalState.ProcessValue := UntrackedPressure;
    UntrackedNormalState.NominalOutput := LREAL#30.0;
    UntrackedNormalState.SaturatedOutput := LREAL#30.0;
    TrackedNormalState.Setpoint := PressureSetpoint;
    TrackedNormalState.ProcessValue := TrackedPressure;
    TrackedNormalState.NominalOutput := LREAL#30.0;
    TrackedNormalState.SaturatedOutput := LREAL#30.0;
    UntrackedProtectState.Setpoint := MarginControlSetpoint;
    UntrackedProtectState.ProcessValue := UntrackedMargin;
    UntrackedProtectState.NominalOutput := LREAL#30.0;
    UntrackedProtectState.SaturatedOutput := LREAL#30.0;
    TrackedProtectState.Setpoint := MarginControlSetpoint;
    TrackedProtectState.ProcessValue := TrackedMargin;
    TrackedProtectState.NominalOutput := LREAL#30.0;
    TrackedProtectState.SaturatedOutput := LREAL#30.0;

    BaselineNormalController.Configure(Parameters := NormalParameters);
    UntrackedNormalController.Configure(Parameters := NormalParameters);
    TrackedNormalController.Configure(Parameters := NormalParameters);
    UntrackedProtectController.Configure(Parameters := ProtectParameters);
    TrackedProtectController.Configure(Parameters := ProtectParameters);
    BaselineNormalController.Initialize(State := BaselineNormalState);
    UntrackedNormalController.Initialize(State := UntrackedNormalState);
    TrackedNormalController.Initialize(State := TrackedNormalState);
    UntrackedProtectController.Initialize(State := UntrackedProtectState);
    TrackedProtectController.Initialize(State := TrackedProtectState);
    Initialized := TRUE;
END_IF;

(* Control Studio grants only experiment authority. The project owns the
   pressure-demand cycle and restores the nominal setpoint on every exit. *)
IF ExperimentEnabled AND InputsValid AND NOT SafetyAbort THEN
    ExperimentElapsed := ExperimentElapsed + LREAL#0.1;
ELSE
    ExperimentElapsed := LREAL#0.0;
END_IF;
PressureSetpoint := Lesson55ReviewedPressureSetpoint(
    ExperimentElapsedSeconds := ExperimentElapsed,
    ExperimentEnabled := ExperimentEnabled,
    InputsValid := InputsValid,
    SafetyAbort := SafetyAbort);
OperationalAuthority := Lesson55OperationalAuthority(
    InputsValid := InputsValid, SafetyAbort := SafetyAbort);

BaselineNormalInput.Setpoint := PressureSetpoint;
BaselineNormalInput.ProcessValue := BaselinePressure;
IF OperationalAuthority THEN
    BaselineNormalInput.Mode := PIDMode#Auto;
ELSE
    BaselineNormalInput.Setpoint := BaselinePressure;
    BaselineNormalInput.Mode := PIDMode#Track;
    BaselineNormalInput.TrackingOutput := LREAL#30.0;
END_IF;
BaselineNormalInput.CycleTime := LREAL#0.1;
BaselineValveRequest := BaselineNormalController.Update(Input := BaselineNormalInput);

(* This pair intentionally remains in Auto while unselected, demonstrating
   how stored integral state delays or disturbs selector transitions. *)
UntrackedNormalInput.Setpoint := PressureSetpoint;
UntrackedNormalInput.ProcessValue := UntrackedPressure;
IF OperationalAuthority THEN
    UntrackedNormalInput.Mode := PIDMode#Auto;
ELSE
    UntrackedNormalInput.Setpoint := UntrackedPressure;
    UntrackedNormalInput.Mode := PIDMode#Track;
    UntrackedNormalInput.TrackingOutput := LREAL#30.0;
END_IF;
UntrackedNormalInput.CycleTime := LREAL#0.1;
UntrackedNormalDemand := UntrackedNormalController.Update(Input := UntrackedNormalInput);
UntrackedProtectInput.Setpoint := MarginControlSetpoint;
UntrackedProtectInput.ProcessValue := UntrackedMargin;
IF OperationalAuthority THEN
    UntrackedProtectInput.Mode := PIDMode#Auto;
ELSE
    UntrackedProtectInput.Setpoint := UntrackedMargin;
    UntrackedProtectInput.Mode := PIDMode#Track;
    UntrackedProtectInput.TrackingOutput := LREAL#30.0;
END_IF;
UntrackedProtectInput.CycleTime := LREAL#0.1;
UntrackedProtectDemand := UntrackedProtectController.Update(Input := UntrackedProtectInput);

IF NOT OperationalAuthority THEN
    UntrackedProtectionActive := FALSE;
    UntrackedValveRequest := LREAL#30.0;
ELSE
    IF NOT UntrackedProtectionActive
        AND Lesson55TakeoverRequired(MarginPercent := UntrackedMargin) THEN
        UntrackedProtectionActive := TRUE;
    ELSIF UntrackedProtectionActive AND Lesson55ReleasePermitted(
        MarginPercent := UntrackedMargin,
        NormalDemand := UntrackedNormalDemand,
        ProtectiveDemand := UntrackedProtectDemand) THEN
        UntrackedProtectionActive := FALSE;
    END_IF;
    IF UntrackedProtectionActive THEN
        UntrackedValveRequest := Lesson55HighSelect(
            NormalDemand := UntrackedNormalDemand,
            ProtectiveDemand := UntrackedProtectDemand);
    ELSE
        UntrackedValveRequest := UntrackedNormalDemand;
    END_IF;
END_IF;

TrackedProtectionAvailable := Lesson55ControlAuthority(
    MarginValid := MarginValid,
    InputsValid := InputsValid,
    SafetyAbort := SafetyAbort);
TrackedNormalInput.Setpoint := PressureSetpoint;
TrackedNormalInput.ProcessValue := TrackedPressure;
TrackedNormalInput.CycleTime := LREAL#0.1;
TrackedProtectInput.Setpoint := MarginControlSetpoint;
TrackedProtectInput.ProcessValue := TrackedMargin;
TrackedProtectInput.CycleTime := LREAL#0.1;

IF NOT TrackedProtectionAvailable THEN
    TrackedProtectionActive := FALSE;
    TrackedNormalInput.Setpoint := TrackedPressure;
    TrackedNormalInput.Mode := PIDMode#Track;
    TrackedNormalInput.TrackingOutput := LREAL#30.0;
    TrackedProtectInput.Setpoint := TrackedMargin;
    TrackedProtectInput.Mode := PIDMode#Track;
    TrackedProtectInput.TrackingOutput := LREAL#30.0;
ELSE
    IF TrackedProtectionActive THEN
        TrackedNormalInput.Mode := PIDMode#Track;
        TrackedNormalInput.TrackingOutput := TrackedValveRequest;
        TrackedProtectInput.Mode := PIDMode#Auto;
    ELSE
        TrackedNormalInput.Mode := PIDMode#Auto;
        TrackedProtectInput.Mode := PIDMode#Track;
        TrackedProtectInput.TrackingOutput := TrackedValveRequest;
    END_IF;
END_IF;

TrackedNormalDemand := TrackedNormalController.Update(Input := TrackedNormalInput);
TrackedProtectDemand := TrackedProtectController.Update(Input := TrackedProtectInput);

IF TrackedProtectionAvailable THEN
    IF NOT TrackedProtectionActive
        AND Lesson55TakeoverRequired(MarginPercent := TrackedMargin) THEN
        TrackedProtectionActive := TRUE;
    ELSIF TrackedProtectionActive AND Lesson55ReleasePermitted(
        MarginPercent := TrackedMargin,
        NormalDemand := TrackedNormalDemand,
        ProtectiveDemand := TrackedProtectDemand) THEN
        TrackedProtectionActive := FALSE;
    END_IF;
    IF TrackedProtectionActive THEN
        TrackedValveRequest := Lesson55HighSelect(
            NormalDemand := TrackedNormalDemand,
            ProtectiveDemand := TrackedProtectDemand);
    ELSE
        TrackedValveRequest := TrackedNormalDemand;
    END_IF;
ELSE
    (* Without a valid protected measurement this lesson makes no protection
       claim and holds the separately reviewed simulated fallback position. *)
    TrackedValveRequest := LREAL#30.0;
END_IF;
AutomaticMode := TrackedProtectionAvailable;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.MarginValid AT %MX88.5 : BOOL;
    P1.ExperimentEnabled AT %MX89.1 : BOOL;
    P1.SafetyAbort AT %MX89.2 : BOOL;
    P1.InputsValid AT %MX89.3 : BOOL;
    P1.UntrackedProtectionActive AT %MX88.6 : BOOL;
    P1.TrackedProtectionActive AT %MX88.7 : BOOL;
    P1.TrackedProtectionAvailable AT %MX89.0 : BOOL;
    P1.AutomaticMode AT %MX216.0 : BOOL;
    P1.PressureSetpoint AT %ML192 : LREAL;
    P1.TrackedNormalDemand AT %ML200 : LREAL;
    P1.TrackedProtectDemand AT %ML208 : LREAL;
    P1.BaselinePressure AT %IL88 : LREAL;
    P1.BaselineMargin AT %IL96 : LREAL;
    P1.UntrackedPressure AT %IL104 : LREAL;
    P1.UntrackedMargin AT %IL112 : LREAL;
    P1.TrackedPressure AT %IL120 : LREAL;
    P1.TrackedMargin AT %IL128 : LREAL;
    P1.BaselineValveRequest AT %QL128 : LREAL;
    P1.UntrackedValveRequest AT %QL136 : LREAL;
    P1.TrackedValveRequest AT %QL144 : LREAL;
END_VAR
END_CONFIGURATION
