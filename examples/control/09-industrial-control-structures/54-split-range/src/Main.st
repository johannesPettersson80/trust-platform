USING Control;

FUNCTION Lesson54ControlAuthority : BOOL
VAR_INPUT
    AutomaticAvailable : BOOL;
    InputsValid : BOOL;
    SafetyAbort : BOOL;
END_VAR
Lesson54ControlAuthority := AutomaticAvailable AND InputsValid
    AND (NOT SafetyAbort);
END_FUNCTION

FUNCTION Lesson54DeadZoneHeating : LREAL
VAR_INPUT Demand : LREAL; END_VAR
Lesson54DeadZoneHeating := MAX(Demand - LREAL#4.0, LREAL#0.0);
END_FUNCTION

FUNCTION Lesson54DeadZoneCooling : LREAL
VAR_INPUT Demand : LREAL; END_VAR
Lesson54DeadZoneCooling := MAX(-Demand - LREAL#4.0, LREAL#0.0);
END_FUNCTION

FUNCTION Lesson54OverlapHeating : LREAL
VAR_INPUT Demand : LREAL; END_VAR
Lesson54OverlapHeating := MAX(Demand + LREAL#4.0, LREAL#0.0);
END_FUNCTION

FUNCTION Lesson54OverlapCooling : LREAL
VAR_INPUT Demand : LREAL; END_VAR
Lesson54OverlapCooling := MAX(LREAL#4.0 - Demand, LREAL#0.0);
END_FUNCTION

FUNCTION Lesson54CharacterizedHeating : LREAL
VAR_INPUT Demand : LREAL; END_VAR
Lesson54CharacterizedHeating := MAX(Demand, LREAL#0.0);
END_FUNCTION

FUNCTION Lesson54CharacterizedCooling : LREAL
VAR_INPUT Demand : LREAL; END_VAR
Lesson54CharacterizedCooling :=
    MAX(-Demand * LREAL#0.6666666666666667, LREAL#0.0);
END_FUNCTION

FUNCTION Lesson54ReviewedSetpoint : LREAL
VAR_INPUT
    ExperimentElapsedSeconds : LREAL;
    ExperimentEnabled : BOOL;
    InputsValid : BOOL;
    SafetyAbort : BOOL;
END_VAR
IF ExperimentEnabled AND InputsValid AND (NOT SafetyAbort) THEN
    IF ExperimentElapsedSeconds < LREAL#20.0 THEN
        Lesson54ReviewedSetpoint := LREAL#80.0;
    ELSIF ExperimentElapsedSeconds < LREAL#80.0 THEN
        Lesson54ReviewedSetpoint := LREAL#84.0;
    ELSIF ExperimentElapsedSeconds < LREAL#140.0 THEN
        Lesson54ReviewedSetpoint := LREAL#76.0;
    ELSE
        Lesson54ReviewedSetpoint := LREAL#80.0;
    END_IF;
ELSE
    Lesson54ReviewedSetpoint := LREAL#80.0;
END_IF;
END_FUNCTION

PROGRAM Main
VAR
    DeadZoneController : Control.PID;
    OverlapController : Control.PID;
    CharacterizedController : Control.PID;
    Parameters : Control.PIDParameters;
    DeadZoneState : Control.PIDInitialState;
    OverlapState : Control.PIDInitialState;
    CharacterizedState : Control.PIDInitialState;
    DeadZoneInput : Control.PIDInput;
    OverlapInput : Control.PIDInput;
    CharacterizedInput : Control.PIDInput;
    Initialized : BOOL := FALSE;
    AutomaticAvailable : BOOL := TRUE;
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ExperimentElapsed : LREAL := 0.0;
    Setpoint : LREAL := 80.0;
    DeadZoneTemperature : LREAL := 80.0;
    OverlapTemperature : LREAL := 80.0;
    CharacterizedTemperature : LREAL := 80.0;
    DeadZoneDemand : LREAL := 0.0;
    OverlapDemand : LREAL := 0.0;
    CharacterizedDemand : LREAL := 0.0;
    DeadZoneHeatingRequest : LREAL := 0.0;
    DeadZoneCoolingRequest : LREAL := 0.0;
    OverlapHeatingRequest : LREAL := 0.0;
    OverlapCoolingRequest : LREAL := 0.0;
    CharacterizedHeatingRequest : LREAL := 0.0;
    CharacterizedCoolingRequest : LREAL := 0.0;
    ControlAuthority : BOOL := TRUE;
END_VAR

IF NOT Initialized THEN
    Setpoint := LREAL#80.0;
    Parameters.ProportionalGain := LREAL#2.0;
    Parameters.IntegralGain := LREAL#0.20;
    Parameters.DerivativeGain := LREAL#0.0;
    Parameters.ProportionalSetpointWeight := LREAL#1.0;
    Parameters.DerivativeSetpointWeight := LREAL#0.0;
    Parameters.Bias := LREAL#0.0;
    Parameters.OutputMinimum := LREAL#-100.0;
    Parameters.OutputMaximum := LREAL#100.0;
    Parameters.RateMinimum := LREAL#-1000.0;
    Parameters.RateMaximum := LREAL#1000.0;
    Parameters.TrackingTime := LREAL#1.0;
    Parameters.AntiWindup := PIDAntiWindup#Combined;

    DeadZoneState.Setpoint := Setpoint;
    DeadZoneState.ProcessValue := DeadZoneTemperature;
    DeadZoneState.NominalOutput := LREAL#0.0;
    DeadZoneState.SaturatedOutput := LREAL#0.0;
    OverlapState.Setpoint := Setpoint;
    OverlapState.ProcessValue := OverlapTemperature;
    OverlapState.NominalOutput := LREAL#0.0;
    OverlapState.SaturatedOutput := LREAL#0.0;
    CharacterizedState.Setpoint := Setpoint;
    CharacterizedState.ProcessValue := CharacterizedTemperature;
    CharacterizedState.NominalOutput := LREAL#0.0;
    CharacterizedState.SaturatedOutput := LREAL#0.0;
    DeadZoneController.Configure(Parameters := Parameters);
    OverlapController.Configure(Parameters := Parameters);
    CharacterizedController.Configure(Parameters := Parameters);
    DeadZoneController.Initialize(State := DeadZoneState);
    OverlapController.Initialize(State := OverlapState);
    CharacterizedController.Initialize(State := CharacterizedState);
    Initialized := TRUE;
END_IF;

(* Control Studio only grants experiment authority. The runnable project owns
   the complete, reproducible setpoint sequence and safe restoration. *)
IF ExperimentEnabled AND InputsValid AND NOT SafetyAbort THEN
    ExperimentElapsed := ExperimentElapsed + LREAL#0.1;
ELSE
    ExperimentElapsed := LREAL#0.0;
END_IF;
Setpoint := Lesson54ReviewedSetpoint(
    ExperimentElapsedSeconds := ExperimentElapsed,
    ExperimentEnabled := ExperimentEnabled,
    InputsValid := InputsValid,
    SafetyAbort := SafetyAbort);
ControlAuthority := Lesson54ControlAuthority(
    AutomaticAvailable := AutomaticAvailable,
    InputsValid := InputsValid,
    SafetyAbort := SafetyAbort);

DeadZoneInput.Setpoint := Setpoint;
DeadZoneInput.ProcessValue := DeadZoneTemperature;
DeadZoneInput.FeedForward := LREAL#0.0;
DeadZoneInput.CycleTime := LREAL#0.1;
OverlapInput.Setpoint := Setpoint;
OverlapInput.ProcessValue := OverlapTemperature;
OverlapInput.FeedForward := LREAL#0.0;
OverlapInput.CycleTime := LREAL#0.1;
CharacterizedInput.Setpoint := Setpoint;
CharacterizedInput.ProcessValue := CharacterizedTemperature;
CharacterizedInput.FeedForward := LREAL#0.0;
CharacterizedInput.CycleTime := LREAL#0.1;

IF ControlAuthority THEN
    DeadZoneInput.Mode := PIDMode#Auto;
    OverlapInput.Mode := PIDMode#Auto;
    CharacterizedInput.Mode := PIDMode#Auto;
ELSE
    (* Align error and output while authority is absent so no controller term
       can reopen either utility during the tracking scan. *)
    DeadZoneInput.Setpoint := DeadZoneTemperature;
    DeadZoneInput.Mode := PIDMode#Track;
    DeadZoneInput.TrackingOutput := LREAL#0.0;
    OverlapInput.Setpoint := OverlapTemperature;
    OverlapInput.Mode := PIDMode#Track;
    OverlapInput.TrackingOutput := LREAL#0.0;
    CharacterizedInput.Setpoint := CharacterizedTemperature;
    CharacterizedInput.Mode := PIDMode#Track;
    CharacterizedInput.TrackingOutput := LREAL#0.0;
END_IF;

DeadZoneDemand := DeadZoneController.Update(Input := DeadZoneInput);
OverlapDemand := OverlapController.Update(Input := OverlapInput);
CharacterizedDemand := CharacterizedController.Update(Input := CharacterizedInput);

IF ControlAuthority THEN
    (* Four demand points are deliberately unavailable around neutral. *)
    DeadZoneHeatingRequest := Lesson54DeadZoneHeating(Demand := DeadZoneDemand);
    DeadZoneCoolingRequest := Lesson54DeadZoneCooling(Demand := DeadZoneDemand);

    (* Four points of overlap keep both opposing utilities open near neutral. *)
    OverlapHeatingRequest := Lesson54OverlapHeating(Demand := OverlapDemand);
    OverlapCoolingRequest := Lesson54OverlapCooling(Demand := OverlapDemand);

    CharacterizedHeatingRequest := Lesson54CharacterizedHeating(
        Demand := CharacterizedDemand);
    CharacterizedCoolingRequest := Lesson54CharacterizedCooling(
        Demand := CharacterizedDemand);
ELSE
    DeadZoneHeatingRequest := LREAL#0.0;
    DeadZoneCoolingRequest := LREAL#0.0;
    OverlapHeatingRequest := LREAL#0.0;
    OverlapCoolingRequest := LREAL#0.0;
    CharacterizedHeatingRequest := LREAL#0.0;
    CharacterizedCoolingRequest := LREAL#0.0;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.AutomaticAvailable AT %MX88.4 : BOOL;
    P1.ExperimentEnabled AT %MX88.5 : BOOL;
    P1.SafetyAbort AT %MX88.6 : BOOL;
    P1.InputsValid AT %MX88.7 : BOOL;
    P1.Setpoint AT %ML160 : LREAL;
    P1.DeadZoneDemand AT %ML168 : LREAL;
    P1.OverlapDemand AT %ML176 : LREAL;
    P1.CharacterizedDemand AT %ML184 : LREAL;
    P1.DeadZoneTemperature AT %IL64 : LREAL;
    P1.OverlapTemperature AT %IL72 : LREAL;
    P1.CharacterizedTemperature AT %IL80 : LREAL;
    P1.DeadZoneHeatingRequest AT %QL80 : LREAL;
    P1.DeadZoneCoolingRequest AT %QL88 : LREAL;
    P1.OverlapHeatingRequest AT %QL96 : LREAL;
    P1.OverlapCoolingRequest AT %QL104 : LREAL;
    P1.CharacterizedHeatingRequest AT %QL112 : LREAL;
    P1.CharacterizedCoolingRequest AT %QL120 : LREAL;
END_VAR
END_CONFIGURATION
