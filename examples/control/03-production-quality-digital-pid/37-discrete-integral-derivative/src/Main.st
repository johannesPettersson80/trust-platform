USING Control;

PROGRAM Main
VAR
    Controller : Lesson37DiscretePID;
    Filter : Control.LowPass;
    FilterParameters : Control.LowPassParameters;
    Initialized : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    Setpoint : LREAL := 0.0;
    RawPosition : LREAL := 0.0;
    FilteredPosition : LREAL := 0.0;
    Velocity : LREAL := 0.0;
    MotorTorque : LREAL := 0.0;
    SampleInterval : LREAL := LREAL#0.02;
    ProportionalTorque : LREAL := 0.0;
    IntegralTorque : LREAL := 0.0;
    TrialIntegralTorque : LREAL := 0.0;
    DerivativeTorque : LREAL := 0.0;
    RequestedTorque : LREAL := 0.0;
    AppliedTorque : LREAL := 0.0;
    PreviousPosition : LREAL := 0.0;
END_VAR

IF NOT Initialized THEN
    FilterParameters.TimeConstant := LREAL#0.05;
    FilterParameters.Order := FilterOrder#FirstOrder;
    Filter.Configure(Parameters := FilterParameters);
    Filter.Initialize(Input := RawPosition);
    Initialized := TRUE;
END_IF;

IF ExperimentEnabled THEN
    Setpoint := LREAL#1.0;
ELSE
    Setpoint := LREAL#0.0;
END_IF;

FilteredPosition := Filter.Update(
    Input := RawPosition,
    CycleTime := SampleInterval);
(* The elapsed time used by both stored-state updates is explicit. *)
MotorTorque := Controller.Update(
    Setpoint := Setpoint,
    FilteredPosition := FilteredPosition,
    ElapsedSeconds := SampleInterval);
ProportionalTorque := Controller.ProportionalTorque;
IntegralTorque := Controller.IntegralTorque;
TrialIntegralTorque := Controller.TrialIntegralTorque;
DerivativeTorque := Controller.DerivativeTorque;
RequestedTorque := Controller.RequestedTorque;
AppliedTorque := Controller.AppliedTorque;
PreviousPosition := Controller.PreviousPositionOutput;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX128.0 : BOOL;
    P1.Setpoint AT %ML0 : LREAL;
    P1.FilteredPosition AT %ML8 : LREAL;
    P1.RawPosition AT %IL0 : LREAL;
    P1.Velocity AT %IL8 : LREAL;
    P1.MotorTorque AT %QL0 : LREAL;
    P1.ProportionalTorque AT %ML24 : LREAL;
    P1.IntegralTorque AT %ML32 : LREAL;
    P1.TrialIntegralTorque AT %ML40 : LREAL;
    P1.DerivativeTorque AT %ML48 : LREAL;
    P1.RequestedTorque AT %ML56 : LREAL;
    P1.AppliedTorque AT %ML64 : LREAL;
    P1.PreviousPosition AT %ML72 : LREAL;
    P1.SampleInterval AT %ML80 : LREAL;
END_VAR
END_CONFIGURATION
