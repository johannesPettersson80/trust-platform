FUNCTION CascadeUpstreamRate : LREAL
VAR_INPUT
    StateM : LREAL;
    FlowDeviationM3PerS : LREAL;
END_VAR
CascadeUpstreamRate := -LREAL#0.04 * StateM + LREAL#2.0 * FlowDeviationM3PerS;
END_FUNCTION

FUNCTION CascadeDownstreamRate : LREAL
VAR_INPUT
    UpstreamStateM : LREAL;
    DownstreamStateM : LREAL;
END_VAR
CascadeDownstreamRate := LREAL#0.04 * UpstreamStateM - LREAL#0.04 * DownstreamStateM;
END_FUNCTION

PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ClockValid : BOOL := FALSE;
    FaultLatched : BOOL := FALSE;
    InitialConditionValid : BOOL := FALSE;
    PredictorReady : BOOL := FALSE;
    WasEnabled : BOOL := FALSE;
    ClockPrimed : BOOL := FALSE;
    NowTime : TIME;
    PreviousTime : TIME;
    StartedAt : TIME;
    State : ARRAY[1..2] OF LREAL;
    NextState : ARRAY[1..2] OF LREAL;
    Model1LevelM : LREAL := 0.25;
    Model2LevelM : LREAL := 0.25;
    Derivative1MPerS : LREAL := 0.0;
    Derivative2MPerS : LREAL := 0.0;
    ModelError1M : LREAL := 0.0;
    ModelError2M : LREAL := 0.0;
    ExperimentElapsedS : LREAL := 0.0;
    RequestedPumpFlow : LREAL := 0.010;
    PumpFlow : LREAL := 0.010;
    FlowDeviationM3PerS : LREAL := 0.0;
    Tank1Level : LREAL := 0.27;
    Tank2Level : LREAL := 0.25;
END_VAR

(* Physical levels are read-only outputs of the coupled nonlinear simulator.
   These two local model states are a separate one-second Euler prediction. *)
NowTime := TIME();
ClockValid := ClockPrimed AND ((NowTime - PreviousTime) = T#1s);
IF SafetyAbort OR NOT InputsValid OR
   NOT ((Tank1Level >= LREAL#0.05) AND (Tank1Level <= LREAL#0.45)) OR
   NOT ((Tank2Level >= LREAL#0.05) AND (Tank2Level <= LREAL#0.45)) OR
   (ClockPrimed AND NOT ClockValid) THEN
    FaultLatched := TRUE;
END_IF;

IF WasEnabled AND NOT ExperimentEnabled AND ((NowTime - StartedAt) < T#120s) THEN
    FaultLatched := TRUE;
END_IF;
IF ExperimentEnabled AND NOT WasEnabled THEN
    StartedAt := NowTime;
    InitialConditionValid := (ABS(Tank1Level - LREAL#0.27) <= LREAL#0.000001) AND
                             (ABS(Tank2Level - LREAL#0.25) <= LREAL#0.000001);
    IF NOT InitialConditionValid THEN FaultLatched := TRUE; END_IF;
    State[1] := Tank1Level - LREAL#0.25;
    State[2] := Tank2Level - LREAL#0.25;
    ExperimentElapsedS := LREAL#0.0;
ELSIF ExperimentEnabled AND NOT FaultLatched THEN
    (* Accept the prediction only after the actual clock confirms one second. *)
    State[1] := NextState[1];
    State[2] := NextState[2];
    ExperimentElapsedS := ExperimentElapsedS + LREAL#1.0;
END_IF;

RequestedPumpFlow := LREAL#0.010;
IF ExperimentEnabled AND (ExperimentElapsedS < LREAL#20.0) THEN
    RequestedPumpFlow := RequestedPumpFlow + FlowDeviationM3PerS;
END_IF;
IF NOT ((RequestedPumpFlow >= LREAL#0.0) AND (RequestedPumpFlow <= LREAL#0.020)) THEN
    FaultLatched := TRUE;
END_IF;
IF FaultLatched THEN
    PumpFlow := LREAL#0.0;
ELSE
    PumpFlow := RequestedPumpFlow;
END_IF;

Model1LevelM := LREAL#0.25 + State[1];
Model2LevelM := LREAL#0.25 + State[2];
ModelError1M := Model1LevelM - Tank1Level;
ModelError2M := Model2LevelM - Tank2Level;
PredictorReady := ExperimentEnabled AND NOT FaultLatched;
IF PredictorReady THEN
    Derivative1MPerS := CascadeUpstreamRate(StateM := State[1], FlowDeviationM3PerS := PumpFlow - LREAL#0.010);
    Derivative2MPerS := CascadeDownstreamRate(UpstreamStateM := State[1], DownstreamStateM := State[2]);
    (* Both next entries use the same old state; never overwrite State[1]
       before calculating the downstream prediction. The horizon is one second. *)
    NextState[1] := State[1] + Derivative1MPerS;
    NextState[2] := State[2] + Derivative2MPerS;
    IF NOT (((LREAL#0.25 + NextState[1]) >= LREAL#0.05) AND ((LREAL#0.25 + NextState[1]) <= LREAL#0.45)) OR
       NOT (((LREAL#0.25 + NextState[2]) >= LREAL#0.05) AND ((LREAL#0.25 + NextState[2]) <= LREAL#0.45)) THEN
        FaultLatched := TRUE;
        PredictorReady := FALSE;
        PumpFlow := LREAL#0.0;
    END_IF;
END_IF;
WasEnabled := ExperimentEnabled;
PreviousTime := NowTime;
ClockPrimed := TRUE;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#1s, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.PumpFlow AT %QL0 : LREAL;
    P1.Tank1Level AT %IL0 : LREAL;
    P1.Tank2Level AT %IL8 : LREAL;
    P1.Model1LevelM AT %ML0 : LREAL;
    P1.Model2LevelM AT %ML8 : LREAL;
    P1.Derivative1MPerS AT %ML16 : LREAL;
    P1.Derivative2MPerS AT %ML24 : LREAL;
    P1.State[1] AT %ML32 : LREAL;
    P1.State[2] AT %ML40 : LREAL;
    P1.ExperimentElapsedS AT %ML48 : LREAL;
    P1.RequestedPumpFlow AT %ML56 : LREAL;
    P1.FlowDeviationM3PerS AT %ML72 : LREAL;
    P1.ExperimentEnabled AT %MX80.0 : BOOL;
    P1.SafetyAbort AT %MX80.1 : BOOL;
    P1.ClockValid AT %MX80.3 : BOOL;
    P1.FaultLatched AT %MX80.4 : BOOL;
    P1.InitialConditionValid AT %MX80.5 : BOOL;
    P1.PredictorReady AT %MX80.6 : BOOL;
    P1.ModelError1M AT %ML88 : LREAL;
    P1.ModelError2M AT %ML96 : LREAL;
END_VAR
END_CONFIGURATION
