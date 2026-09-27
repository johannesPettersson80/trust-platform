USING Control;

FUNCTION Lesson52RequiredValveDeviation : LREAL
VAR_INPUT
    JacketDeviationDegC : LREAL;
    UtilityDisturbanceDegC : LREAL;
    ValveToJacketGain : LREAL;
END_VAR
Lesson52RequiredValveDeviation :=
    (JacketDeviationDegC - UtilityDisturbanceDegC) / ValveToJacketGain;
END_FUNCTION

FUNCTION Lesson52CommissioningActive : BOOL
VAR_INPUT
    ExperimentElapsedSeconds : LREAL;
    ExperimentEnabled : BOOL;
    InputsValid : BOOL;
    SafetyAbort : BOOL;
END_VAR
Lesson52CommissioningActive := ExperimentEnabled AND InputsValid
    AND (NOT SafetyAbort) AND (ExperimentElapsedSeconds < LREAL#40.0);
END_FUNCTION

FUNCTION Lesson52JacketTestSetpoint : LREAL
VAR_INPUT
    ExperimentElapsedSeconds : LREAL;
    ExperimentEnabled : BOOL;
    InputsValid : BOOL;
    SafetyAbort : BOOL;
END_VAR
IF ExperimentEnabled AND InputsValid AND (NOT SafetyAbort)
    AND (ExperimentElapsedSeconds >= LREAL#10.0)
    AND (ExperimentElapsedSeconds < LREAL#30.0) THEN
    Lesson52JacketTestSetpoint := LREAL#75.0;
ELSE
    Lesson52JacketTestSetpoint := LREAL#70.0;
END_IF;
END_FUNCTION

FUNCTION Lesson52UtilityDisturbance : LREAL
VAR_INPUT
    ExperimentElapsedSeconds : LREAL;
    ExperimentEnabled : BOOL;
    InputsValid : BOOL;
    SafetyAbort : BOOL;
END_VAR
IF ExperimentEnabled AND InputsValid AND (NOT SafetyAbort)
    AND (ExperimentElapsedSeconds >= LREAL#40.0) THEN
    Lesson52UtilityDisturbance := LREAL#-10.0;
ELSE
    Lesson52UtilityDisturbance := LREAL#0.0;
END_IF;
END_FUNCTION

PROGRAM Main
VAR
    DirectController : Control.PID;
    FastOuterController : Control.PID;
    FastInnerController : Control.PID;
    SlowOuterController : Control.PID;
    SlowInnerController : Control.PID;
    DirectParameters : Control.PIDParameters;
    OuterParameters : Control.PIDParameters;
    FastInnerParameters : Control.PIDParameters;
    SlowInnerParameters : Control.PIDParameters;
    DirectState : Control.PIDInitialState;
    OuterState : Control.PIDInitialState;
    InnerState : Control.PIDInitialState;
    DirectInput : Control.PIDInput;
    FastOuterInput : Control.PIDInput;
    FastInnerInput : Control.PIDInput;
    SlowOuterInput : Control.PIDInput;
    SlowInnerInput : Control.PIDInput;
    Initialized : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ExperimentElapsed : LREAL := 0.0;
    InnerCommissioning : BOOL := FALSE;
    InnerAvailable : BOOL := TRUE;
    ReactorSetpoint : LREAL := 80.0;
    JacketTestSetpoint : LREAL := 70.0;
    UtilityDisturbance : LREAL := 0.0;
    DirectReactorTemperature : LREAL := 80.0;
    DirectJacketTemperature : LREAL := 70.0;
    FastReactorTemperature : LREAL := 80.0;
    FastJacketTemperature : LREAL := 70.0;
    SlowReactorTemperature : LREAL := 80.0;
    SlowJacketTemperature : LREAL := 70.0;
    DirectValveRequest : LREAL := 50.0;
    DirectUtilityInput : LREAL := 0.0;
    FastValveRequest : LREAL := 50.0;
    FastUtilityInput : LREAL := 0.0;
    SlowValveRequest : LREAL := 50.0;
    SlowUtilityInput : LREAL := 0.0;
    FastJacketSetpoint : LREAL := 70.0;
    SlowJacketSetpoint : LREAL := 70.0;
    AutomaticMode : BOOL := TRUE;
END_VAR

IF NOT Initialized THEN
    DirectParameters.ProportionalGain := LREAL#2.0;
    DirectParameters.IntegralGain := LREAL#0.20;
    DirectParameters.DerivativeGain := LREAL#0.0;
    DirectParameters.ProportionalSetpointWeight := LREAL#1.0;
    DirectParameters.DerivativeSetpointWeight := LREAL#0.0;
    DirectParameters.Bias := LREAL#50.0;
    DirectParameters.OutputMinimum := LREAL#0.0;
    DirectParameters.OutputMaximum := LREAL#100.0;
    DirectParameters.RateMinimum := LREAL#-1000.0;
    DirectParameters.RateMaximum := LREAL#1000.0;
    DirectParameters.TrackingTime := LREAL#1.0;
    DirectParameters.AntiWindup := PIDAntiWindup#Combined;

    OuterParameters := DirectParameters;
    OuterParameters.ProportionalGain := LREAL#1.0;
    OuterParameters.IntegralGain := LREAL#0.08;
    OuterParameters.Bias := LREAL#70.0;
    OuterParameters.OutputMinimum := LREAL#60.0;
    OuterParameters.OutputMaximum := LREAL#85.0;

    FastInnerParameters := DirectParameters;
    FastInnerParameters.ProportionalGain := LREAL#2.0;
    FastInnerParameters.IntegralGain := LREAL#1.0;

    SlowInnerParameters := DirectParameters;
    SlowInnerParameters.ProportionalGain := LREAL#0.20;
    SlowInnerParameters.IntegralGain := LREAL#0.02;

    DirectState.Setpoint := ReactorSetpoint;
    DirectState.ProcessValue := DirectReactorTemperature;
    DirectState.NominalOutput := LREAL#50.0;
    DirectState.SaturatedOutput := LREAL#50.0;
    OuterState.Setpoint := ReactorSetpoint;
    OuterState.ProcessValue := FastReactorTemperature;
    OuterState.NominalOutput := LREAL#70.0;
    OuterState.SaturatedOutput := LREAL#70.0;
    InnerState.Setpoint := LREAL#70.0;
    InnerState.ProcessValue := LREAL#70.0;
    InnerState.NominalOutput := LREAL#50.0;
    InnerState.SaturatedOutput := LREAL#50.0;

    DirectController.Configure(Parameters := DirectParameters);
    FastOuterController.Configure(Parameters := OuterParameters);
    SlowOuterController.Configure(Parameters := OuterParameters);
    FastInnerController.Configure(Parameters := FastInnerParameters);
    SlowInnerController.Configure(Parameters := SlowInnerParameters);
    DirectController.Initialize(State := DirectState);
    FastOuterController.Initialize(State := OuterState);
    SlowOuterController.Initialize(State := OuterState);
    FastInnerController.Initialize(State := InnerState);
    SlowInnerController.Initialize(State := InnerState);
    Initialized := TRUE;
END_IF;

IF ExperimentEnabled AND (NOT SafetyAbort) AND InputsValid THEN
    ExperimentElapsed := ExperimentElapsed + LREAL#0.1;
ELSE
    ExperimentElapsed := LREAL#0.0;
END_IF;
InnerCommissioning := Lesson52CommissioningActive(
    ExperimentElapsedSeconds := ExperimentElapsed,
    ExperimentEnabled := ExperimentEnabled,
    InputsValid := InputsValid,
    SafetyAbort := SafetyAbort);
JacketTestSetpoint := Lesson52JacketTestSetpoint(
    ExperimentElapsedSeconds := ExperimentElapsed,
    ExperimentEnabled := ExperimentEnabled,
    InputsValid := InputsValid,
    SafetyAbort := SafetyAbort);
UtilityDisturbance := Lesson52UtilityDisturbance(
    ExperimentElapsedSeconds := ExperimentElapsed,
    ExperimentEnabled := ExperimentEnabled,
    InputsValid := InputsValid,
    SafetyAbort := SafetyAbort);

DirectUtilityInput := UtilityDisturbance;
FastUtilityInput := UtilityDisturbance;
SlowUtilityInput := UtilityDisturbance;

DirectInput.Setpoint := ReactorSetpoint;
DirectInput.ProcessValue := DirectReactorTemperature;
DirectInput.FeedForward := LREAL#0.0;
DirectInput.Mode := PIDMode#Auto;
DirectInput.CycleTime := LREAL#0.1;
DirectValveRequest := DirectController.Update(Input := DirectInput);

FastOuterInput.Setpoint := ReactorSetpoint;
FastOuterInput.ProcessValue := FastReactorTemperature;
FastOuterInput.FeedForward := LREAL#0.0;
FastOuterInput.CycleTime := LREAL#0.1;
SlowOuterInput.Setpoint := ReactorSetpoint;
SlowOuterInput.ProcessValue := SlowReactorTemperature;
SlowOuterInput.FeedForward := LREAL#0.0;
SlowOuterInput.CycleTime := LREAL#0.1;

(* Commission the valve-facing loops first. Tracking keeps each inactive outer
   controller aligned with the jacket setpoint that is actually in authority. *)
IF InnerCommissioning THEN
    FastOuterInput.Mode := PIDMode#Track;
    FastOuterInput.TrackingOutput := JacketTestSetpoint;
    FastJacketSetpoint := FastOuterController.Update(Input := FastOuterInput);
    SlowOuterInput.Mode := PIDMode#Track;
    SlowOuterInput.TrackingOutput := JacketTestSetpoint;
    SlowJacketSetpoint := SlowOuterController.Update(Input := SlowOuterInput);
ELSIF NOT InnerAvailable THEN
    (* Preserve the last achievable secondary setpoint while the inner loop has
       no valve authority. This prevents hidden outer-loop windup. *)
    FastOuterInput.Mode := PIDMode#Track;
    FastOuterInput.Setpoint := FastReactorTemperature;
    FastOuterInput.TrackingOutput := FastJacketSetpoint;
    FastOuterController.Update(Input := FastOuterInput);
    FastJacketSetpoint := FastOuterInput.TrackingOutput;
    SlowOuterInput.Mode := PIDMode#Track;
    SlowOuterInput.Setpoint := SlowReactorTemperature;
    SlowOuterInput.TrackingOutput := SlowJacketSetpoint;
    SlowOuterController.Update(Input := SlowOuterInput);
    SlowJacketSetpoint := SlowOuterInput.TrackingOutput;
ELSE
    FastOuterInput.Mode := PIDMode#Auto;
    FastJacketSetpoint := FastOuterController.Update(Input := FastOuterInput);
    SlowOuterInput.Mode := PIDMode#Auto;
    SlowJacketSetpoint := SlowOuterController.Update(Input := SlowOuterInput);
END_IF;

FastInnerInput.Setpoint := FastJacketSetpoint;
FastInnerInput.ProcessValue := FastJacketTemperature;
FastInnerInput.FeedForward := LREAL#0.0;
FastInnerInput.CycleTime := LREAL#0.1;
SlowInnerInput.Setpoint := SlowJacketSetpoint;
SlowInnerInput.ProcessValue := SlowJacketTemperature;
SlowInnerInput.FeedForward := LREAL#0.0;
SlowInnerInput.CycleTime := LREAL#0.1;

IF InnerAvailable THEN
    FastInnerInput.Mode := PIDMode#Auto;
    FastValveRequest := FastInnerController.Update(Input := FastInnerInput);
    SlowInnerInput.Mode := PIDMode#Auto;
    SlowValveRequest := SlowInnerController.Update(Input := SlowInnerInput);
ELSE
    (* Loss of the secondary loop removes cascade authority without changing
       the last achievable valve request. *)
    FastInnerInput.Mode := PIDMode#Track;
    FastInnerInput.Setpoint := FastJacketTemperature;
    FastInnerInput.TrackingOutput := FastValveRequest;
    FastInnerController.Update(Input := FastInnerInput);
    FastValveRequest := FastInnerInput.TrackingOutput;
    SlowInnerInput.Mode := PIDMode#Track;
    SlowInnerInput.Setpoint := SlowJacketTemperature;
    SlowInnerInput.TrackingOutput := SlowValveRequest;
    SlowInnerController.Update(Input := SlowInnerInput);
    SlowValveRequest := SlowInnerInput.TrackingOutput;
END_IF;
AutomaticMode := TRUE;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.InnerCommissioning AT %MX136.0 : BOOL;
    P1.InnerAvailable AT %MX136.1 : BOOL;
    P1.AutomaticMode AT %MX136.2 : BOOL;
    P1.ExperimentEnabled AT %MX136.3 : BOOL;
    P1.SafetyAbort AT %MX136.4 : BOOL;
    P1.InputsValid AT %MX136.5 : BOOL;
    P1.ReactorSetpoint AT %ML88 : LREAL;
    P1.JacketTestSetpoint AT %ML96 : LREAL;
    P1.UtilityDisturbance AT %ML104 : LREAL;
    P1.FastJacketSetpoint AT %ML112 : LREAL;
    P1.SlowJacketSetpoint AT %ML120 : LREAL;
    P1.DirectReactorTemperature AT %IL0 : LREAL;
    P1.DirectJacketTemperature AT %IL8 : LREAL;
    P1.FastReactorTemperature AT %IL16 : LREAL;
    P1.FastJacketTemperature AT %IL24 : LREAL;
    P1.SlowReactorTemperature AT %IL32 : LREAL;
    P1.SlowJacketTemperature AT %IL40 : LREAL;
    P1.DirectValveRequest AT %QL0 : LREAL;
    P1.DirectUtilityInput AT %QL8 : LREAL;
    P1.FastValveRequest AT %QL16 : LREAL;
    P1.FastUtilityInput AT %QL24 : LREAL;
    P1.SlowValveRequest AT %QL32 : LREAL;
    P1.SlowUtilityInput AT %QL40 : LREAL;
END_VAR
END_CONFIGURATION
