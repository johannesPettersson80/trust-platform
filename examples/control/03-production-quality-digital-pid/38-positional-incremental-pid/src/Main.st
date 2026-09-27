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

FUNCTION Lesson38IncrementalPidOutput : LREAL
VAR_INPUT
    PreviousRequest : LREAL;
    CurrentError : LREAL;
    PreviousError : LREAL;
    CurrentVelocity : LREAL;
    PreviousVelocity : LREAL;
    ProportionalGain : LREAL;
    IntegralGain : LREAL;
    DerivativeGain : LREAL;
    SampleInterval : LREAL;
    PreviousTrackingCorrection : LREAL;
END_VAR
Lesson38IncrementalPidOutput := PreviousRequest
    + ProportionalGain * (CurrentError - PreviousError)
    + IntegralGain * CurrentError * SampleInterval
    - DerivativeGain * (CurrentVelocity - PreviousVelocity)
    + PreviousTrackingCorrection;
END_FUNCTION

PROGRAM Main
VAR
    PositionController : Control.PID;
    PositionParameters : Control.PIDParameters;
    PositionInitialState : Control.PIDInitialState;
    PositionInput : Control.PIDInput;
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
    TrialKind : INT := 0; (* 0 nominal, 1 clipping, 2 gain revision. *)
    DiagnosticError : LREAL := 0.5;
    RejectedIncrementalPGain : LREAL := 4.0;
    PreviousDiagnosticError : LREAL := 0.5;
    DiagnosticErrorChange : LREAL := 0.0;
    RejectedIncrementalPOutput : LREAL := 1.0;
    PositionalPOutput : LREAL := 0.0;
    Position : LREAL := 0.0;
    Velocity : LREAL := 0.0;
    PositionTorque : LREAL := 0.0;
    IncrementalRequest : LREAL := 0.0;
    IncrementalApplied : LREAL := 0.0;
    PreviousIncrementalRequest : LREAL := 0.0;
    PreviousError : LREAL := 0.0;
    PreviousVelocity : LREAL := 0.0;
    TrackingCorrection : LREAL := 0.0;
    SampleInterval : LREAL := LREAL#0.02;
    PositionError : LREAL := 0.0;
    PositionUsesPositionalForm : BOOL := TRUE;
    IncrementalUsesRetainedRequest : BOOL := TRUE;
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

    (* The retained 1 N m value makes the rejected recurrence's arbitrary
       starting state visible. This diagnostic never drives either plant. *)
    RejectedIncrementalPOutput := LREAL#1.0;
    PreviousDiagnosticError := DiagnosticError;
    Initialized := TRUE;
END_IF;

(* Control Studio writes one reviewed authority. The project owns the
   timestamped position record and the incremental shadow state. *)
IF ExperimentEnabled THEN
    IF NOT ExperimentWasEnabled THEN
        ExperimentCycle := ULINT#0;
        PositionProportionalGain := LREAL#4.0;
        RejectedIncrementalPGain := LREAL#4.0;
        IF TrialKind = 1 THEN
            PreviousIncrementalRequest := LREAL#25.0;
        ELSE
            PreviousIncrementalRequest := LREAL#0.0;
        END_IF;
    END_IF;
    ExperimentCycle := ExperimentCycle + ULINT#1;
    PositionSetpoint := LREAL#0.5;
    AutoMode := TRUE;
    IF TrialKind = 2 AND ExperimentCycle > ULINT#250 THEN
        PositionProportionalGain := LREAL#8.0;
        RejectedIncrementalPGain := LREAL#8.0;
    END_IF;
ELSIF ExperimentWasEnabled THEN
    ExperimentCycle := ULINT#0;
    PositionSetpoint := LREAL#0.0;
    PositionProportionalGain := LREAL#4.0;
    RejectedIncrementalPGain := LREAL#4.0;
    AutoMode := FALSE;
END_IF;

(* Reconfigure the shipped controllers so a gain change is observed on the
   same scan instead of being hidden behind lesson-specific controller code. *)
PositionParameters.ProportionalGain := PositionProportionalGain;
PositionController.Configure(Parameters := PositionParameters);

PositionInput.Setpoint := PositionSetpoint;
PositionInput.ProcessValue := Position;
PositionInput.CycleTime := SampleInterval;
InputsValid := Position >= LREAL#-4.0 AND Position <= LREAL#4.0
    AND Velocity >= LREAL#-20.0 AND Velocity <= LREAL#20.0;
IF AutoMode AND InputsValid AND NOT SafetyAbort THEN
    PositionInput.Mode := PIDMode#Auto;
    ManualMode := FALSE;
ELSE
    PositionInput.Mode := PIDMode#Manual;
    PositionInput.ManualOutput := LREAL#0.0;
    ManualMode := TRUE;
END_IF;

PositionTorque := PositionController.Update(Input := PositionInput);
PositionUsesPositionalForm := PositionParameters.IntegralGain = LREAL#0.0;
IncrementalUsesRetainedRequest := TRUE;

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
PositionError := PositionSetpoint - Position;
IncrementalRequest := Lesson38IncrementalPidOutput(
    PreviousRequest := PreviousIncrementalRequest,
    CurrentError := PositionError,
    PreviousError := PreviousError,
    CurrentVelocity := Velocity,
    PreviousVelocity := PreviousVelocity,
    ProportionalGain := PositionProportionalGain,
    IntegralGain := LREAL#0.0,
    DerivativeGain := LREAL#0.0,
    SampleInterval := SampleInterval,
    PreviousTrackingCorrection := TrackingCorrection
);
IncrementalApplied := MAX(MIN(IncrementalRequest, LREAL#20.0), LREAL#-20.0);
PreviousIncrementalRequest := IncrementalRequest;
PreviousError := PositionError;
PreviousVelocity := Velocity;
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
    P1.TrialKind AT %MW80 : INT;
    P1.IncrementalRequest AT %ML16 : LREAL;
    P1.IncrementalApplied AT %ML20 : LREAL;
    P1.PreviousIncrementalRequest AT %ML24 : LREAL;
    P1.PositionError AT %ML32 : LREAL;
    P1.PreviousError AT %ML40 : LREAL;
    P1.PreviousVelocity AT %ML48 : LREAL;
    P1.SampleInterval AT %ML80 : LREAL;
    P1.DiagnosticError AT %ML40 : LREAL;
    P1.RejectedIncrementalPGain AT %ML48 : LREAL;
    P1.RejectedIncrementalPOutput AT %ML56 : LREAL;
    P1.PositionalPOutput AT %ML64 : LREAL;
    P1.PositionUsesPositionalForm AT %MX72.0 : BOOL;
    P1.IncrementalUsesRetainedRequest AT %MX72.1 : BOOL;
    P1.PositionTorque AT %QL0 : LREAL;
    P1.Position AT %IL0 : LREAL;
    P1.Velocity AT %IL8 : LREAL;
END_VAR
END_CONFIGURATION
