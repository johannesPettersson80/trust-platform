FUNCTION Lesson14LevelRateMetresPerSecond : LREAL
VAR_INPUT
    InletFlowCubicMetresPerSecond : LREAL;
    OutletFlowCubicMetresPerSecond : LREAL;
    TankAreaSquareMetres : LREAL;
END_VAR
Lesson14LevelRateMetresPerSecond :=
    (InletFlowCubicMetresPerSecond - OutletFlowCubicMetresPerSecond)
        / TankAreaSquareMetres;
END_FUNCTION

FUNCTION Lesson14LevelAfterTime : LREAL
VAR_INPUT
    InitialLevelMetres : LREAL;
    LevelRateMetresPerSecond : LREAL;
    ElapsedSeconds : LREAL;
END_VAR
Lesson14LevelAfterTime := InitialLevelMetres
    + LevelRateMetresPerSecond * ElapsedSeconds;
END_FUNCTION

PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    InletFlow : LREAL := LREAL#0.010;
    OutletFlow : LREAL := LREAL#0.010;
    NetFlow : LREAL := LREAL#0.0;
    TankArea : LREAL := LREAL#0.5;
    PredictedLevelRate : LREAL := LREAL#0.0;
    PredictedLevelAfterTenSeconds : LREAL := LREAL#0.25;
    TankLevel : LREAL := LREAL#0.25;
    NetFlowCommand : LREAL := LREAL#0.0;
END_VAR

OutletFlow := LREAL#0.010;
TankArea := LREAL#0.5;
IF ExperimentEnabled THEN
    InletFlow := LREAL#0.012;
ELSE
    InletFlow := LREAL#0.010;
END_IF;

(* Net flow is positive when more liquid enters than leaves. The integrator
   plant receives that physical imbalance in m3/s. *)
NetFlow := InletFlow - OutletFlow;
NetFlowCommand := NetFlow;
PredictedLevelRate := Lesson14LevelRateMetresPerSecond(
    InletFlowCubicMetresPerSecond := InletFlow,
    OutletFlowCubicMetresPerSecond := OutletFlow,
    TankAreaSquareMetres := TankArea
);
PredictedLevelAfterTenSeconds := Lesson14LevelAfterTime(
    InitialLevelMetres := LREAL#0.25,
    LevelRateMetresPerSecond := PredictedLevelRate,
    ElapsedSeconds := LREAL#10.0
);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#1s, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.InletFlow AT %ML0 : LREAL;
    P1.OutletFlow AT %ML8 : LREAL;
    P1.NetFlow AT %ML16 : LREAL;
    P1.TankArea AT %ML24 : LREAL;
    P1.PredictedLevelRate AT %ML32 : LREAL;
    P1.PredictedLevelAfterTenSeconds AT %ML40 : LREAL;
    P1.ExperimentEnabled AT %MX80.0 : BOOL;
    P1.NetFlowCommand AT %QL0 : LREAL;
    P1.TankLevel AT %IL0 : LREAL;
END_VAR
END_CONFIGURATION
