(* The local rates of the two level deviations in m/s, the two rows of x' = Ax + Bv around 0.25 m and 0.010 m³/s. *)
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

(* Lesson 74's two-tank state-space model. Initialize takes the state from measured levels; each call advances it by one explicit
   1 s step with the flow deviation applied during that second. Both rates come from the old states before either state
   is stored. The model levels are 0.25 m plus the states. *)
FUNCTION_BLOCK CascadeModel
VAR_INPUT
    FlowDeviationM3PerS : LREAL;
END_VAR
VAR_OUTPUT
    Tank1M : LREAL := 0.25;
    Tank2M : LREAL := 0.25;
    Rate1MPerS : LREAL := 0.0;
    Rate2MPerS : LREAL := 0.0;
END_VAR
VAR
    State : ARRAY[1..2] OF LREAL;
END_VAR
METHOD PUBLIC Initialize
VAR_INPUT
    MeasuredTank1M : LREAL;
    MeasuredTank2M : LREAL;
END_VAR
State[1] := MeasuredTank1M - LREAL#0.25;
State[2] := MeasuredTank2M - LREAL#0.25;
Tank1M := MeasuredTank1M;
Tank2M := MeasuredTank2M;
END_METHOD
Rate1MPerS := CascadeUpstreamRate(StateM := State[1], FlowDeviationM3PerS := FlowDeviationM3PerS);
Rate2MPerS := CascadeDownstreamRate(UpstreamStateM := State[1], DownstreamStateM := State[2]);
State[1] := State[1] + Rate1MPerS;
State[2] := State[2] + Rate2MPerS;
Tank1M := LREAL#0.25 + State[1];
Tank2M := LREAL#0.25 + State[2];
END_FUNCTION_BLOCK

(* The pulse pair: +0.001 m³/s in the first 20 updates, −0.001 m³/s in the next 11, the whole updates nearest the
   10.97 s that return the model's tank 1 to rest, then nominal flow. ElapsedS counts whole updates from the start. *)
FUNCTION SteeringFlowDeviationM3PerS : LREAL
VAR_INPUT
    ElapsedS : LREAL;
END_VAR
SteeringFlowDeviationM3PerS := LREAL#0.0;
IF ElapsedS < LREAL#19.5 THEN
    SteeringFlowDeviationM3PerS := LREAL#0.001;
ELSIF ElapsedS < LREAL#30.5 THEN
    SteeringFlowDeviationM3PerS := LREAL#-0.001;
END_IF;
END_FUNCTION

(* How long the flow must stay lowered by a after PulseSeconds raised by a for the model's tank 1 to return to rest:
   x1 = 50a·(1 − e^(−0.04·t1)) after the first pulse, then x1 = −50a + (x1 + 50a)·e^(−0.04·t), which is zero when
   e^(−0.04·t2) = 1/(2 − e^(−0.04·t1)). The pulse size a cancels. *)
FUNCTION CounterPulseSeconds : LREAL
VAR_INPUT
    PulseSeconds : LREAL;
END_VAR
CounterPulseSeconds := LN(LREAL#2.0 - EXP(-LREAL#0.04 * PulseSeconds)) / LREAL#0.04;
END_FUNCTION

PROGRAM Main
VAR
    (* One tank cascade with the model beside it. Control Studio's project experiment steers the cascade with the pulse
       pair to tank 1 at rest and tank 2 raised, then leaves the pump at 0.010 m³/s. *)
    Model : CascadeModel;
    PumpM3PerS : LREAL := 0.010;
    Tank1M : LREAL := 0.25;
    Tank2M : LREAL := 0.25;
    Model1M : LREAL := 0.25;
    Model2M : LREAL := 0.25;
    ElapsedS : LREAL := 0.0;
    ExperimentEnabled : BOOL := FALSE;
    WasEnabled : BOOL := FALSE;
    ClockPrimed : BOOL := FALSE;
    ClockValid : BOOL := TRUE;
    FaultLatched : BOOL := FALSE;
    NowTime : TIME;
    PreviousTime : TIME;
END_VAR
(* As in Lesson 74: the model advances one second per update, so an interval outside 1 s ± 50 ms, or a level outside
   0.05–0.45 m, latches zero flow. *)
NowTime := TIME();
ClockValid := NOT ClockPrimed OR ((NowTime - PreviousTime) >= T#950ms AND (NowTime - PreviousTime) <= T#1050ms);
IF NOT ClockValid OR Tank1M < LREAL#0.05 OR Tank1M > LREAL#0.45 OR Tank2M < LREAL#0.05 OR Tank2M > LREAL#0.45 THEN
    FaultLatched := TRUE;
END_IF;
(* The model starts from the measured levels; each later update advances it with the flow of the second that has
   just passed. *)
IF ExperimentEnabled AND NOT WasEnabled THEN
    Model.Initialize(MeasuredTank1M := Tank1M, MeasuredTank2M := Tank2M);
    ElapsedS := LREAL#0.0;
ELSIF ExperimentEnabled AND NOT FaultLatched THEN
    Model(FlowDeviationM3PerS := PumpM3PerS - LREAL#0.010);
    ElapsedS := ElapsedS + LREAL#1.0;
END_IF;
PumpM3PerS := LREAL#0.010;
IF ExperimentEnabled THEN
    PumpM3PerS := LREAL#0.010 + SteeringFlowDeviationM3PerS(ElapsedS := ElapsedS);
END_IF;
IF FaultLatched THEN
    PumpM3PerS := LREAL#0.0;
END_IF;
Model1M := Model.Tank1M;
Model2M := Model.Tank2M;
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
    P1.PumpM3PerS AT %QL0 : LREAL;
    P1.Tank1M AT %IL0 : LREAL;
    P1.Tank2M AT %IL8 : LREAL;
    P1.Model1M AT %ML0 : LREAL;
    P1.Model2M AT %ML8 : LREAL;
    P1.ExperimentEnabled AT %MX16.0 : BOOL;
    P1.ClockValid AT %MX16.1 : BOOL;
    P1.FaultLatched AT %MX16.2 : BOOL;
END_VAR
END_CONFIGURATION
