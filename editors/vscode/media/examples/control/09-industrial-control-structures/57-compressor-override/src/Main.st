USING Control;

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

BaselineNormalInput.Setpoint := PressureSetpoint;
BaselineNormalInput.ProcessValue := BaselinePressure;
BaselineNormalInput.Mode := PIDMode#Auto;
BaselineNormalInput.CycleTime := LREAL#0.1;
BaselineValveRequest := BaselineNormalController.Update(Input := BaselineNormalInput);

(* This pair intentionally remains in Auto while unselected, demonstrating
   how stored integral state delays or disturbs selector transitions. *)
UntrackedNormalInput.Setpoint := PressureSetpoint;
UntrackedNormalInput.ProcessValue := UntrackedPressure;
UntrackedNormalInput.Mode := PIDMode#Auto;
UntrackedNormalInput.CycleTime := LREAL#0.1;
UntrackedNormalDemand := UntrackedNormalController.Update(Input := UntrackedNormalInput);
UntrackedProtectInput.Setpoint := MarginControlSetpoint;
UntrackedProtectInput.ProcessValue := UntrackedMargin;
UntrackedProtectInput.Mode := PIDMode#Auto;
UntrackedProtectInput.CycleTime := LREAL#0.1;
UntrackedProtectDemand := UntrackedProtectController.Update(Input := UntrackedProtectInput);

IF NOT UntrackedProtectionActive AND UntrackedMargin <= LREAL#14.0 THEN
    UntrackedProtectionActive := TRUE;
ELSIF UntrackedProtectionActive AND UntrackedMargin >= LREAL#16.0
    AND UntrackedNormalDemand >= UntrackedProtectDemand THEN
    UntrackedProtectionActive := FALSE;
END_IF;
IF UntrackedProtectionActive THEN
    UntrackedValveRequest := MAX(UntrackedNormalDemand, UntrackedProtectDemand);
ELSE
    UntrackedValveRequest := UntrackedNormalDemand;
END_IF;

TrackedProtectionAvailable := MarginValid;
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
    IF NOT TrackedProtectionActive AND TrackedMargin <= LREAL#14.0 THEN
        TrackedProtectionActive := TRUE;
    ELSIF TrackedProtectionActive AND TrackedMargin >= LREAL#16.0
        AND TrackedNormalDemand >= TrackedProtectDemand THEN
        TrackedProtectionActive := FALSE;
    END_IF;
    IF TrackedProtectionActive THEN
        TrackedValveRequest := MAX(TrackedNormalDemand, TrackedProtectDemand);
    ELSE
        TrackedValveRequest := TrackedNormalDemand;
    END_IF;
ELSE
    (* Without a valid protected measurement this lesson makes no protection
       claim and holds the separately reviewed simulated fallback position. *)
    TrackedValveRequest := LREAL#30.0;
END_IF;
AutomaticMode := TRUE;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.MarginValid AT %MX88.5 : BOOL;
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
