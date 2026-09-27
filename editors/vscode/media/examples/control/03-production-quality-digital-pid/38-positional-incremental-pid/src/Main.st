USING Control;

FUNCTION Lesson38PositionalPOutput : LREAL
VAR_INPUT
    Bias : LREAL;
    ProportionalGain : LREAL;
    CurrentError : LREAL;
END_VAR
Lesson38PositionalPOutput := Bias + ProportionalGain * CurrentError;
END_FUNCTION

FUNCTION Lesson38RejectedIncrementalPOutput : LREAL
VAR_INPUT
    PreviousOutput : LREAL;
    ProportionalGain : LREAL;
    CurrentError : LREAL;
    PreviousError : LREAL;
END_VAR
Lesson38RejectedIncrementalPOutput := PreviousOutput
    + ProportionalGain * (CurrentError - PreviousError);
END_FUNCTION

FUNCTION Lesson38BackwardIntegralIncrement : LREAL
VAR_INPUT
    IntegralGain : LREAL;
    SampleInterval : LREAL;
    CurrentError : LREAL;
END_VAR
Lesson38BackwardIntegralIncrement := IntegralGain * SampleInterval * CurrentError;
END_FUNCTION

PROGRAM Main
VAR
    PositionController : Control.PID;
    PositionParameters : Control.PIDParameters;
    PositionInitialState : Control.PIDInitialState;
    PositionInput : Control.PIDInput;
    HeaterController : Control.PID;
    HeaterParameters : Control.PIDParameters;
    HeaterInitialState : Control.PIDInitialState;
    HeaterInput : Control.PIDInput;
    Initialized : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    ExperimentCycle : ULINT := ULINT#0;
    AutoMode : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ManualMode : BOOL := TRUE;
    PositionSetpoint : LREAL := 0.0;
    PositionProportionalGain : LREAL := 4.0;
    HeaterSetpoint : LREAL := 20.0;
    HeaterProportionalGain : LREAL := 4.0;
    HeaterIntegralGain : LREAL := 0.8;
    DiagnosticError : LREAL := 0.5;
    RejectedIncrementalPGain : LREAL := 4.0;
    PreviousDiagnosticError : LREAL := 0.5;
    DiagnosticErrorChange : LREAL := 0.0;
    RejectedIncrementalPOutput : LREAL := 1.0;
    PositionalPOutput : LREAL := 0.0;
    Position : LREAL := 0.0;
    Velocity : LREAL := 0.0;
    HeaterTemperature : LREAL := 20.0;
    PositionTorque : LREAL := 0.0;
    HeaterPower : LREAL := 0.0;
    PositionUsesPositionalForm : BOOL := TRUE;
    HeaterUsesIncrementalForm : BOOL := TRUE;
END_VAR

IF NOT Initialized THEN
    PositionParameters.IntegralGain := LREAL#0.0;
    PositionParameters.DerivativeGain := LREAL#0.0;
    PositionParameters.ProportionalSetpointWeight := LREAL#1.0;
    PositionParameters.DerivativeSetpointWeight := LREAL#0.0;
    PositionParameters.Bias := LREAL#0.0;
    PositionParameters.OutputMinimum := LREAL#-10.0;
    PositionParameters.OutputMaximum := LREAL#10.0;
    PositionParameters.RateMinimum := LREAL#-500.0;
    PositionParameters.RateMaximum := LREAL#500.0;
    PositionParameters.TrackingTime := LREAL#0.2;
    PositionParameters.AntiWindup := PIDAntiWindup#Combined;
    PositionParameters.ProportionalGain := PositionProportionalGain;
    PositionController.Configure(Parameters := PositionParameters);
    PositionInitialState.Setpoint := PositionSetpoint;
    PositionInitialState.ProcessValue := Position;
    PositionController.Initialize(State := PositionInitialState);

    HeaterParameters.IntegralGain := HeaterIntegralGain;
    HeaterParameters.DerivativeGain := LREAL#0.0;
    HeaterParameters.ProportionalSetpointWeight := LREAL#1.0;
    HeaterParameters.DerivativeSetpointWeight := LREAL#0.0;
    HeaterParameters.Bias := LREAL#0.0;
    HeaterParameters.OutputMinimum := LREAL#0.0;
    HeaterParameters.OutputMaximum := LREAL#100.0;
    HeaterParameters.RateMinimum := LREAL#-500.0;
    HeaterParameters.RateMaximum := LREAL#500.0;
    HeaterParameters.TrackingTime := LREAL#1.0;
    HeaterParameters.AntiWindup := PIDAntiWindup#Combined;
    HeaterParameters.ProportionalGain := HeaterProportionalGain;
    HeaterController.Configure(Parameters := HeaterParameters);
    HeaterInitialState.Setpoint := HeaterSetpoint;
    HeaterInitialState.ProcessValue := HeaterTemperature;
    HeaterInitialState.NominalOutput := HeaterPower;
    HeaterInitialState.SaturatedOutput := HeaterPower;
    HeaterController.Initialize(State := HeaterInitialState);

    (* The retained 1 N m value makes the rejected recurrence's arbitrary
       starting state visible. This diagnostic never drives either plant. *)
    RejectedIncrementalPOutput := LREAL#1.0;
    PreviousDiagnosticError := DiagnosticError;
    Initialized := TRUE;
END_IF;

(* Control Studio writes one reviewed authority. The project owns the two
   setpoints, simultaneous gain edit, and safe restoration. The exact
   unchanged-input proof remains in the fresh-state native comparison. *)
IF ExperimentEnabled THEN
    IF NOT ExperimentWasEnabled THEN
        ExperimentCycle := ULINT#0;
        PositionProportionalGain := LREAL#4.0;
        HeaterProportionalGain := LREAL#4.0;
        RejectedIncrementalPGain := LREAL#4.0;
    END_IF;
    ExperimentCycle := ExperimentCycle + ULINT#1;
    PositionSetpoint := LREAL#0.5;
    HeaterSetpoint := LREAL#35.0;
    AutoMode := TRUE;
    IF ExperimentCycle > ULINT#250 THEN
        PositionProportionalGain := LREAL#8.0;
        HeaterProportionalGain := LREAL#8.0;
        RejectedIncrementalPGain := LREAL#8.0;
    END_IF;
ELSIF ExperimentWasEnabled THEN
    ExperimentCycle := ULINT#0;
    PositionSetpoint := LREAL#0.0;
    HeaterSetpoint := LREAL#20.0;
    PositionProportionalGain := LREAL#4.0;
    HeaterProportionalGain := LREAL#4.0;
    RejectedIncrementalPGain := LREAL#4.0;
    AutoMode := FALSE;
END_IF;

(* Reconfigure the shipped controllers so a gain change is observed on the
   same scan instead of being hidden behind lesson-specific controller code. *)
PositionParameters.ProportionalGain := PositionProportionalGain;
PositionController.Configure(Parameters := PositionParameters);
HeaterParameters.ProportionalGain := HeaterProportionalGain;
HeaterParameters.IntegralGain := HeaterIntegralGain;
HeaterController.Configure(Parameters := HeaterParameters);

PositionInput.Setpoint := PositionSetpoint;
PositionInput.ProcessValue := Position;
PositionInput.CycleTime := LREAL#0.02;
HeaterInput.Setpoint := HeaterSetpoint;
HeaterInput.ProcessValue := HeaterTemperature;
HeaterInput.CycleTime := LREAL#0.02;
InputsValid := Position >= LREAL#-4.0 AND Position <= LREAL#4.0
    AND Velocity >= LREAL#-20.0 AND Velocity <= LREAL#20.0
    AND HeaterTemperature >= LREAL#0.0 AND HeaterTemperature <= LREAL#80.0;
IF AutoMode AND InputsValid AND NOT SafetyAbort THEN
    PositionInput.Mode := PIDMode#Auto;
    HeaterInput.Mode := PIDMode#Auto;
    ManualMode := FALSE;
ELSE
    PositionInput.Mode := PIDMode#Manual;
    PositionInput.ManualOutput := LREAL#0.0;
    HeaterInput.Mode := PIDMode#Manual;
    HeaterInput.ManualOutput := LREAL#0.0;
    ManualMode := TRUE;
END_IF;

PositionTorque := PositionController.Update(Input := PositionInput);
HeaterPower := HeaterController.Update(Input := HeaterInput);
PositionUsesPositionalForm := PositionParameters.IntegralGain = LREAL#0.0;
HeaterUsesIncrementalForm := HeaterParameters.IntegralGain <> LREAL#0.0;

(* This deliberately rejected P recurrence changes output only when error
   changes. It demonstrates why it may not be used as the plant controller. *)
DiagnosticErrorChange := DiagnosticError - PreviousDiagnosticError;
RejectedIncrementalPOutput := Lesson38RejectedIncrementalPOutput(
    PreviousOutput := RejectedIncrementalPOutput,
    ProportionalGain := RejectedIncrementalPGain,
    CurrentError := DiagnosticError,
    PreviousError := PreviousDiagnosticError
);
PositionalPOutput := Lesson38PositionalPOutput(
    Bias := LREAL#0.0,
    ProportionalGain := RejectedIncrementalPGain,
    CurrentError := DiagnosticError
);
PreviousDiagnosticError := DiagnosticError;
ExperimentWasEnabled := ExperimentEnabled;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.AutoMode AT %MX120.0 : BOOL;
    P1.ExperimentEnabled AT %MX120.1 : BOOL;
    P1.SafetyAbort AT %MX120.2 : BOOL;
    P1.InputsValid AT %MX120.3 : BOOL;
    P1.ManualMode AT %MX120.4 : BOOL;
    P1.PositionSetpoint AT %ML0 : LREAL;
    P1.PositionProportionalGain AT %ML8 : LREAL;
    P1.HeaterSetpoint AT %ML16 : LREAL;
    P1.HeaterProportionalGain AT %ML24 : LREAL;
    P1.HeaterIntegralGain AT %ML32 : LREAL;
    P1.DiagnosticError AT %ML40 : LREAL;
    P1.RejectedIncrementalPGain AT %ML48 : LREAL;
    P1.RejectedIncrementalPOutput AT %ML56 : LREAL;
    P1.PositionalPOutput AT %ML64 : LREAL;
    P1.PositionUsesPositionalForm AT %MX72.0 : BOOL;
    P1.HeaterUsesIncrementalForm AT %MX72.1 : BOOL;
    P1.PositionTorque AT %QL0 : LREAL;
    P1.HeaterPower AT %QL8 : LREAL;
    P1.Position AT %IL0 : LREAL;
    P1.Velocity AT %IL8 : LREAL;
    P1.HeaterTemperature AT %IL16 : LREAL;
END_VAR
END_CONFIGURATION
