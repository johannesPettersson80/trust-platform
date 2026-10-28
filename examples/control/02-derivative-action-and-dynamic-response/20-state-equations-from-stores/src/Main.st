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

(* The nonlinear balances the model approximates: 0.5·dh1/dt = q − 0.020·√h1 and 0.5·dh2/dt = 0.020·√h1 − 0.020·√h2. *)
FUNCTION UpstreamLevelRate : LREAL
VAR_INPUT
    LevelM : LREAL;
    InflowM3PerS : LREAL;
END_VAR
UpstreamLevelRate := (InflowM3PerS - LREAL#0.020 * SQRT(LevelM)) / LREAL#0.5;
END_FUNCTION

FUNCTION DownstreamLevelRate : LREAL
VAR_INPUT
    UpstreamLevelM : LREAL;
    DownstreamLevelM : LREAL;
END_VAR
DownstreamLevelRate := (LREAL#0.020 * SQRT(UpstreamLevelM) - LREAL#0.020 * SQRT(DownstreamLevelM)) / LREAL#0.5;
END_FUNCTION

(* The two-tank state-space model. Initialize takes the state from measured levels; each call advances it by one explicit
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

PROGRAM Main
VAR
    (* Two copies of the tank cascade, each with the model beside it. Control Studio's project experiment raises the pump
       of the first by 0.001 m³/s and lowers the second by 0.001 m³/s for 20 s, then returns both to 0.010 m³/s. *)
    RaiseModel : CascadeModel;
    LowerModel : CascadeModel;
    RaisePumpM3PerS : LREAL := 0.010;
    RaiseTank1M : LREAL := 0.25;
    RaiseTank2M : LREAL := 0.25;
    LowerPumpM3PerS : LREAL := 0.010;
    LowerTank1M : LREAL := 0.25;
    LowerTank2M : LREAL := 0.25;
    RaiseModel1M : LREAL := 0.25;
    RaiseModel2M : LREAL := 0.25;
    LowerModel1M : LREAL := 0.25;
    LowerModel2M : LREAL := 0.25;
    ElapsedS : LREAL := 0.0;
    ExperimentEnabled : BOOL := FALSE;
    WasEnabled : BOOL := FALSE;
    ClockPrimed : BOOL := FALSE;
    ClockValid : BOOL := TRUE;
    FaultLatched : BOOL := FALSE;
    NowTime : TIME;
    PreviousTime : TIME;
END_VAR
(* The model advances one second per update, so the clock must show one second (±50 ms) between updates; a wrong
   interval or a level outside 0.05–0.45 m latches zero flow. *)
NowTime := TIME();
ClockValid := NOT ClockPrimed OR ((NowTime - PreviousTime) >= T#950ms AND (NowTime - PreviousTime) <= T#1050ms);
IF NOT ClockValid
    OR RaiseTank1M < LREAL#0.05 OR RaiseTank1M > LREAL#0.45 OR RaiseTank2M < LREAL#0.05 OR RaiseTank2M > LREAL#0.45
    OR LowerTank1M < LREAL#0.05 OR LowerTank1M > LREAL#0.45 OR LowerTank2M < LREAL#0.05 OR LowerTank2M > LREAL#0.45 THEN
    FaultLatched := TRUE;
END_IF;
(* The models start from the measured levels; each later update advances them with the flow deviation of the second that
   has just passed, so model and measured levels belong to the same instant. *)
IF ExperimentEnabled AND NOT WasEnabled THEN
    RaiseModel.Initialize(MeasuredTank1M := RaiseTank1M, MeasuredTank2M := RaiseTank2M);
    LowerModel.Initialize(MeasuredTank1M := LowerTank1M, MeasuredTank2M := LowerTank2M);
    ElapsedS := LREAL#0.0;
ELSIF ExperimentEnabled AND NOT FaultLatched THEN
    RaiseModel(FlowDeviationM3PerS := RaisePumpM3PerS - LREAL#0.010);
    LowerModel(FlowDeviationM3PerS := LowerPumpM3PerS - LREAL#0.010);
    ElapsedS := ElapsedS + LREAL#1.0;
END_IF;
RaisePumpM3PerS := LREAL#0.010;
LowerPumpM3PerS := LREAL#0.010;
IF ExperimentEnabled AND ElapsedS < LREAL#19.5 THEN
    RaisePumpM3PerS := LREAL#0.011;
    LowerPumpM3PerS := LREAL#0.009;
END_IF;
IF FaultLatched THEN
    RaisePumpM3PerS := LREAL#0.0;
    LowerPumpM3PerS := LREAL#0.0;
END_IF;
RaiseModel1M := RaiseModel.Tank1M;
RaiseModel2M := RaiseModel.Tank2M;
LowerModel1M := LowerModel.Tank1M;
LowerModel2M := LowerModel.Tank2M;
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
    P1.RaisePumpM3PerS AT %QL0 : LREAL;
    P1.RaiseTank1M AT %IL0 : LREAL;
    P1.RaiseTank2M AT %IL8 : LREAL;
    P1.LowerPumpM3PerS AT %QL8 : LREAL;
    P1.LowerTank1M AT %IL16 : LREAL;
    P1.LowerTank2M AT %IL24 : LREAL;
    P1.RaiseModel1M AT %ML0 : LREAL;
    P1.RaiseModel2M AT %ML8 : LREAL;
    P1.LowerModel1M AT %ML16 : LREAL;
    P1.LowerModel2M AT %ML24 : LREAL;
    P1.ExperimentEnabled AT %MX32.0 : BOOL;
    P1.ClockValid AT %MX32.1 : BOOL;
    P1.FaultLatched AT %MX32.2 : BOOL;
END_VAR
END_CONFIGURATION
