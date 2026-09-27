FUNCTION Lesson16GravityOutflow : LREAL
VAR_INPUT
    OutletCoefficient : LREAL;
    LevelMetres : LREAL;
END_VAR
Lesson16GravityOutflow := OutletCoefficient * SQRT(MAX(LevelMetres, LREAL#0.0));
END_FUNCTION

FUNCTION Lesson16EquilibriumLevel : LREAL
VAR_INPUT
    InletFlow : LREAL;
    OutletCoefficient : LREAL;
END_VAR
Lesson16EquilibriumLevel :=
    (InletFlow / OutletCoefficient) * (InletFlow / OutletCoefficient);
END_FUNCTION

FUNCTION Lesson16InitialLevelRate : LREAL
VAR_INPUT
    InletFlow : LREAL;
    OutletFlow : LREAL;
    TankArea : LREAL;
END_VAR
Lesson16InitialLevelRate := (InletFlow - OutletFlow) / TankArea;
END_FUNCTION

PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    TankArea : LREAL := 0.5;
    OutletCoefficient : LREAL := 0.02;
    InletFlow : LREAL := 0.01;
    TankLevel : LREAL := 0.25;
    GravityOutflow : LREAL := 0.01;
    NetFlow : LREAL := 0.0;
    InitialLevelRate : LREAL := 0.0;
    PredictedEquilibriumLevel : LREAL := 0.25;
END_VAR

TankArea := LREAL#0.5;
OutletCoefficient := LREAL#0.02;
IF ExperimentEnabled THEN
    InletFlow := LREAL#0.011;
ELSE
    InletFlow := LREAL#0.010;
END_IF;

(* The measured level determines gravity outflow. Net flow then determines
   whether the tank stores or loses liquid. *)
GravityOutflow := Lesson16GravityOutflow(
    OutletCoefficient := OutletCoefficient,
    LevelMetres := TankLevel
);
NetFlow := InletFlow - GravityOutflow;
InitialLevelRate := Lesson16InitialLevelRate(
    InletFlow := InletFlow,
    OutletFlow := GravityOutflow,
    TankArea := TankArea
);
PredictedEquilibriumLevel := Lesson16EquilibriumLevel(
    InletFlow := InletFlow,
    OutletCoefficient := OutletCoefficient
);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#1s, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.InletFlow AT %QL0 : LREAL;
    P1.TankLevel AT %IL0 : LREAL;
    P1.GravityOutflow AT %ML0 : LREAL;
    P1.NetFlow AT %ML8 : LREAL;
    P1.InitialLevelRate AT %ML16 : LREAL;
    P1.PredictedEquilibriumLevel AT %ML24 : LREAL;
    P1.ExperimentEnabled AT %MX80.0 : BOOL;
END_VAR
END_CONFIGURATION
