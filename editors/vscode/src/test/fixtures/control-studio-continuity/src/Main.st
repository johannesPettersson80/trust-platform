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
    AutoMode : BOOL := FALSE;
    ManualOutput : LREAL := 40.0;
    Setpoint : LREAL := 80.0;
    ProcessValue : LREAL := 80.0;
    ManipulatedValue : LREAL := 40.0;
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
END_RESOURCE
VAR_CONFIG
    P1.Setpoint AT %ML0 : LREAL;
    P1.ManualOutput AT %ML16 : LREAL;
    P1.ManipulatedValue AT %QL0 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
    P1.OptionalFeedback AT %IL8 : LREAL;
END_VAR
END_CONFIGURATION
