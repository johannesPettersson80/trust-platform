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

(* One loop type serves all four loops. Each program passes its own cycle time in seconds. *)
FUNCTION_BLOCK MatrixLoop
VAR_INPUT
    CycleSeconds : LREAL;
    Setpoint : LREAL;
    ProcessValue : LREAL;
    ManualOutput : LREAL;
    AutoMode : BOOL;
END_VAR
VAR_OUTPUT
    ManipulatedValue : LREAL;
END_VAR
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
END_VAR

(* The runtime adopts this complete record at the task boundary. Copy it into
   the actual controller before each Update; declaration defaults run only at startup. *)
IF NOT Initialized OR NOT SamePidParameters(Left := Parameters, Right := ConfiguredParameters) THEN
    ConfiguredParameters := Parameters;
    ParameterRevision := ParameterRevision + 1;
END_IF;
Controller.Configure(Parameters := Parameters);
IF NOT Initialized THEN
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
Input.CycleTime := CycleSeconds;
ManipulatedValue := Controller.Update(Input := Input);
END_FUNCTION_BLOCK

(* FA and FU run on the 10 ms task, which equals the resource cycle. FA writes its output to %QL0 directly. *)
PROGRAM AddressedFast
VAR
    Regulator : MatrixLoop;
    Setpoint : LREAL := 20.0;
    ManualOutput : LREAL := 0.0;
    AutoMode : BOOL := FALSE;
    ProcessValue : LREAL := 20.0;
    ManipulatedValue : LREAL := 0.0;
END_VAR
Regulator(CycleSeconds := LREAL#0.01, Setpoint := Setpoint, ProcessValue := ProcessValue,
          ManualOutput := ManualOutput, AutoMode := AutoMode);
ManipulatedValue := Regulator.ManipulatedValue;
(* In Auto the manual output follows the output, so Auto -> Manual starts where Auto left off (Specification 35, A6.4). *)
IF AutoMode THEN
    ManualOutput := ManipulatedValue;
END_IF;
END_PROGRAM

PROGRAM CommandedFast
VAR
    Regulator : MatrixLoop;
    Setpoint : LREAL := 20.0;
    ManualOutput : LREAL := 0.0;
    AutoMode : BOOL := FALSE;
    ProcessValue : LREAL := 20.0;
    ManipulatedValue : LREAL := 0.0;
    Command : LREAL := 0.0;
END_VAR
Regulator(CycleSeconds := LREAL#0.01, Setpoint := Setpoint, ProcessValue := ProcessValue,
          ManualOutput := ManualOutput, AutoMode := AutoMode);
ManipulatedValue := Regulator.ManipulatedValue;
(* The output has no address: the program copies it to Command, which the plant reads at %QL8. *)
Command := ManipulatedValue;
IF AutoMode THEN
    ManualOutput := ManipulatedValue;
END_IF;
END_PROGRAM

(* SA and SU run on the 100 ms task, ten times the resource cycle. *)
PROGRAM AddressedSlow
VAR
    Regulator : MatrixLoop;
    Setpoint : LREAL := 20.0;
    ManualOutput : LREAL := 0.0;
    AutoMode : BOOL := FALSE;
    ProcessValue : LREAL := 20.0;
    ManipulatedValue : LREAL := 0.0;
END_VAR
Regulator(CycleSeconds := LREAL#0.1, Setpoint := Setpoint, ProcessValue := ProcessValue,
          ManualOutput := ManualOutput, AutoMode := AutoMode);
ManipulatedValue := Regulator.ManipulatedValue;
IF AutoMode THEN
    ManualOutput := ManipulatedValue;
END_IF;
END_PROGRAM

PROGRAM CommandedSlow
VAR
    Regulator : MatrixLoop;
    Setpoint : LREAL := 20.0;
    ManualOutput : LREAL := 0.0;
    AutoMode : BOOL := FALSE;
    ProcessValue : LREAL := 20.0;
    ManipulatedValue : LREAL := 0.0;
    Command : LREAL := 0.0;
END_VAR
Regulator(CycleSeconds := LREAL#0.1, Setpoint := Setpoint, ProcessValue := ProcessValue,
          ManualOutput := ManualOutput, AutoMode := AutoMode);
ManipulatedValue := Regulator.ManipulatedValue;
Command := ManipulatedValue;
IF AutoMode THEN
    ManualOutput := ManipulatedValue;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK FastTask(INTERVAL := T#10ms, PRIORITY := 1);
    TASK SlowTask(INTERVAL := T#100ms, PRIORITY := 2);
    PROGRAM FA WITH FastTask : AddressedFast;
    PROGRAM FU WITH FastTask : CommandedFast;
    PROGRAM SA WITH SlowTask : AddressedSlow;
    PROGRAM SU WITH SlowTask : CommandedSlow;
END_RESOURCE
VAR_CONFIG
    FA.Setpoint AT %ML0 : LREAL;
    FA.ManualOutput AT %ML8 : LREAL;
    FA.ManipulatedValue AT %QL0 : LREAL;
    FA.ProcessValue AT %IL0 : LREAL;
    FU.Setpoint AT %ML16 : LREAL;
    FU.ManualOutput AT %ML24 : LREAL;
    FU.Command AT %QL8 : LREAL;
    FU.ProcessValue AT %IL8 : LREAL;
    SA.Setpoint AT %ML32 : LREAL;
    SA.ManualOutput AT %ML40 : LREAL;
    SA.ManipulatedValue AT %QL16 : LREAL;
    SA.ProcessValue AT %IL16 : LREAL;
    SU.Setpoint AT %ML48 : LREAL;
    SU.ManualOutput AT %ML56 : LREAL;
    SU.Command AT %QL24 : LREAL;
    SU.ProcessValue AT %IL24 : LREAL;
END_VAR
END_CONFIGURATION
