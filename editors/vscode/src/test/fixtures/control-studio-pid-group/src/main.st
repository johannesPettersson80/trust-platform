USING Control;



FUNCTION SamePidParameters : BOOL
VAR_INPUT
    Left : Control.PIDParameters;
    Right : Control.PIDParameters;
END_VAR
SamePidParameters :=
    Left.ProportionalGain = Right.ProportionalGain
    AND     Left.IntegralGain = Right.IntegralGain
    AND     Left.DerivativeGain = Right.DerivativeGain
    AND     Left.ProportionalSetpointWeight = Right.ProportionalSetpointWeight
    AND     Left.DerivativeSetpointWeight = Right.DerivativeSetpointWeight
    AND     Left.Bias = Right.Bias
    AND     Left.OutputMinimum = Right.OutputMinimum
    AND     Left.OutputMaximum = Right.OutputMaximum
    AND     Left.RateMinimum = Right.RateMinimum
    AND     Left.RateMaximum = Right.RateMaximum
    AND     Left.TrackingTime = Right.TrackingTime
    AND     Left.AntiWindup = Right.AntiWindup;
END_FUNCTION

PROGRAM Main
VAR
    AutoMode : BOOL := FALSE;
    Controller : Control.PID;
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#1.0,
        IntegralGain := LREAL#0.2,
        DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#1.0,
        DerivativeSetpointWeight := LREAL#0.0,
        Bias := LREAL#0.0,
        OutputMinimum := LREAL#0.0,
        OutputMaximum := LREAL#100.0,
        RateMinimum := LREAL#-25.0,
        RateMaximum := LREAL#25.0,
        TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#Combined
    );
    ConfiguredParameters : Control.PIDParameters;
    ParameterRevision : DINT := 0;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    ManualOutput : LREAL := 0.0;
    Setpoint : LREAL := 20.0;
    ProcessValue : LREAL := 20.0;
    ManipulatedValue : LREAL := 0.0;
    OptionalFeedback : LREAL := 0.0;
    Secondary : Control.PID;
    SecondaryParameters : Control.PIDParameters := (
        ProportionalGain := LREAL#1.0,
        IntegralGain := LREAL#0.2,
        DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#1.0,
        DerivativeSetpointWeight := LREAL#0.0,
        Bias := LREAL#0.0,
        OutputMinimum := LREAL#0.0,
        OutputMaximum := LREAL#100.0,
        RateMinimum := LREAL#-25.0,
        RateMaximum := LREAL#25.0,
        TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#Combined
    );
    SecondaryConfiguredParameters : Control.PIDParameters;
    SecondaryParameterRevision : DINT := 0;
    SecondaryInitialState : Control.PIDInitialState;
    SecondaryInput : Control.PIDInput;
    SecondaryInitialized : BOOL := FALSE;
    SecondaryManualOutput : LREAL := 0.0;
    SecondarySetpoint : LREAL := 20.0;
    SecondaryProcessValue : LREAL := 20.0;
    SecondaryManipulatedValue : LREAL := 0.0;
    SecondaryOptionalFeedback : LREAL := 0.0;
END_VAR


(* The runtime adopts this complete record at the task boundary. Copy it into
   the actual controller before each Update; declaration defaults run only at startup. *)
IF NOT Initialized OR NOT SamePidParameters(Left := Parameters, Right := ConfiguredParameters) THEN
    ConfiguredParameters := Parameters;
    ParameterRevision := ParameterRevision + 1;
END_IF;
Controller.Configure(Parameters := Parameters);
IF NOT Initialized THEN
    IF NOT AutoMode THEN
        Setpoint := ProcessValue;
    END_IF;
    InitialState.Setpoint := Setpoint;
    InitialState.ProcessValue := ProcessValue;
    InitialState.NominalOutput := ManipulatedValue;
    InitialState.SaturatedOutput := ManipulatedValue;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;

Input.Setpoint := Setpoint;
Input.ProcessValue := ProcessValue;
IF AutoMode THEN
    Input.Mode := PIDMode#Auto;
ELSE
    Input.Mode := PIDMode#Manual;
END_IF;
Input.ManualOutput := ManualOutput;
Input.CycleTime := LREAL#0.1;
ManipulatedValue := Controller.Update(Input := Input);



(* The runtime adopts this complete record at the task boundary. Copy it into
   the actual controller before each Update; declaration defaults run only at startup. *)
IF NOT SecondaryInitialized OR NOT SamePidParameters(Left := SecondaryParameters, Right := SecondaryConfiguredParameters) THEN
    SecondaryConfiguredParameters := SecondaryParameters;
    SecondaryParameterRevision := SecondaryParameterRevision + 1;
END_IF;
Secondary.Configure(Parameters := SecondaryParameters);
IF NOT SecondaryInitialized THEN
    IF NOT AutoMode THEN
        SecondarySetpoint := SecondaryProcessValue;
    END_IF;
    SecondaryInitialState.Setpoint := SecondarySetpoint;
    SecondaryInitialState.ProcessValue := SecondaryProcessValue;
    SecondaryInitialState.NominalOutput := SecondaryManipulatedValue;
    SecondaryInitialState.SaturatedOutput := SecondaryManipulatedValue;
    Secondary.Initialize(State := SecondaryInitialState);
    SecondaryInitialized := TRUE;
END_IF;

SecondaryInput.Setpoint := SecondarySetpoint;
SecondaryInput.ProcessValue := SecondaryProcessValue;
IF AutoMode THEN
    SecondaryInput.Mode := PIDMode#Auto;
ELSE
    SecondaryInput.Mode := PIDMode#Manual;
END_IF;
SecondaryInput.ManualOutput := SecondaryManualOutput;
SecondaryInput.CycleTime := LREAL#0.1;
SecondaryManipulatedValue := Secondary.Update(Input := SecondaryInput);
END_PROGRAM



PROGRAM Unselected
VAR
    AutoMode : BOOL := FALSE;
    Controller : Control.PID;
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#1.0,
        IntegralGain := LREAL#0.2,
        DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#1.0,
        DerivativeSetpointWeight := LREAL#0.0,
        Bias := LREAL#0.0,
        OutputMinimum := LREAL#0.0,
        OutputMaximum := LREAL#100.0,
        RateMinimum := LREAL#-25.0,
        RateMaximum := LREAL#25.0,
        TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#Combined
    );
    ConfiguredParameters : Control.PIDParameters;
    ParameterRevision : DINT := 0;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    ManualOutput : LREAL := 0.0;
    Setpoint : LREAL := 20.0;
    ProcessValue : LREAL := 20.0;
    ManipulatedValue : LREAL := 0.0;
    OptionalFeedback : LREAL := 0.0;
END_VAR


(* The runtime adopts this complete record at the task boundary. Copy it into
   the actual controller before each Update; declaration defaults run only at startup. *)
IF NOT Initialized OR NOT SamePidParameters(Left := Parameters, Right := ConfiguredParameters) THEN
    ConfiguredParameters := Parameters;
    ParameterRevision := ParameterRevision + 1;
END_IF;
Controller.Configure(Parameters := Parameters);
IF NOT Initialized THEN
    IF NOT AutoMode THEN
        Setpoint := ProcessValue;
    END_IF;
    InitialState.Setpoint := Setpoint;
    InitialState.ProcessValue := ProcessValue;
    InitialState.NominalOutput := ManipulatedValue;
    InitialState.SaturatedOutput := ManipulatedValue;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;

Input.Setpoint := Setpoint;
Input.ProcessValue := ProcessValue;
IF AutoMode THEN
    Input.Mode := PIDMode#Auto;
ELSE
    Input.Mode := PIDMode#Manual;
END_IF;
Input.ManualOutput := ManualOutput;
Input.CycleTime := LREAL#0.1;
ManipulatedValue := Controller.Update(Input := Input);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
    PROGRAM P3 WITH MainTask : Unselected;
END_RESOURCE
VAR_CONFIG
    P1.Setpoint AT %ML0 : LREAL;
    P1.ManualOutput AT %ML16 : LREAL;
    P1.ManipulatedValue AT %QL0 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
    P1.SecondarySetpoint AT %ML32 : LREAL;
    P1.SecondaryManualOutput AT %ML24 : LREAL;
    P1.SecondaryManipulatedValue AT %QL8 : LREAL;
    P1.SecondaryProcessValue AT %IL8 : LREAL;
    P1.SecondaryOptionalFeedback AT %IL24 : LREAL;
    P3.Setpoint AT %ML40 : LREAL;
    P3.ManualOutput AT %ML48 : LREAL;
    P3.ManipulatedValue AT %QL16 : LREAL;
    P3.ProcessValue AT %IL16 : LREAL;
    P3.OptionalFeedback AT %IL32 : LREAL;
END_VAR
END_CONFIGURATION
