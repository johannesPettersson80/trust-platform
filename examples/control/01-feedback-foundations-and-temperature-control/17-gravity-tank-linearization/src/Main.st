FUNCTION Lesson17LocalGain : LREAL
VAR_INPUT
    OutletCoefficient : LREAL;
    OperatingLevel : LREAL;
END_VAR
Lesson17LocalGain := LREAL#2.0 * SQRT(OperatingLevel) / OutletCoefficient;
END_FUNCTION

FUNCTION Lesson17TimeConstant : LREAL
VAR_INPUT
    TankArea : LREAL;
    OutletCoefficient : LREAL;
    OperatingLevel : LREAL;
END_VAR
Lesson17TimeConstant := LREAL#2.0 * TankArea * SQRT(OperatingLevel)
    / OutletCoefficient;
END_FUNCTION

FUNCTION Lesson17LocalLevelNext : LREAL
VAR_INPUT
    CurrentLevel : LREAL;
    OperatingLevel : LREAL;
    FlowDeviation : LREAL;
    LocalGain : LREAL;
    TimeConstant : LREAL;
    ElapsedTime : LREAL;
END_VAR
Lesson17LocalLevelNext := CurrentLevel + ElapsedTime
    * (LocalGain * FlowDeviation - (CurrentLevel - OperatingLevel))
    / TimeConstant;
END_FUNCTION

FUNCTION Lesson17NonlinearEquilibrium : LREAL
VAR_INPUT
    InletFlow : LREAL;
    OutletCoefficient : LREAL;
END_VAR
Lesson17NonlinearEquilibrium :=
    (InletFlow / OutletCoefficient) * (InletFlow / OutletCoefficient);
END_FUNCTION

PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    TankArea : LREAL := 0.5;
    OutletCoefficient : LREAL := 0.02;
    OperatingLevel : LREAL := 0.25;
    OperatingFlow : LREAL := 0.01;
    SmallFlowDeviation : LREAL := 0.0;
    LargeFlowDeviation : LREAL := 0.0;
    SmallInletFlow : LREAL := 0.01;
    LargeInletFlow : LREAL := 0.01;
    SmallPhysicalLevel : LREAL := 0.25;
    LargePhysicalLevel : LREAL := 0.25;
    SmallLocalLevel : LREAL := 0.25;
    LargeLocalLevel : LREAL := 0.25;
    LocalGain : LREAL := 50.0;
    TimeConstant : LREAL := 25.0;
    SmallPhysicalEquilibrium : LREAL := 0.25;
    LargePhysicalEquilibrium : LREAL := 0.25;
END_VAR

TankArea := LREAL#0.5;
OutletCoefficient := LREAL#0.02;
OperatingLevel := LREAL#0.25;
OperatingFlow := LREAL#0.01;
LocalGain := Lesson17LocalGain(
    OutletCoefficient := OutletCoefficient,
    OperatingLevel := OperatingLevel
);
TimeConstant := Lesson17TimeConstant(
    TankArea := TankArea,
    OutletCoefficient := OutletCoefficient,
    OperatingLevel := OperatingLevel
);

IF ExperimentEnabled THEN
    SmallFlowDeviation := LREAL#0.001;
    LargeFlowDeviation := LREAL#0.005;
ELSE
    SmallFlowDeviation := LREAL#0.0;
    LargeFlowDeviation := LREAL#0.0;
    SmallLocalLevel := OperatingLevel;
    LargeLocalLevel := OperatingLevel;
END_IF;

SmallInletFlow := OperatingFlow + SmallFlowDeviation;
LargeInletFlow := OperatingFlow + LargeFlowDeviation;

(* Both tangent models use the same slope calculated at 0.25 m. Their
   difference from the nonlinear tanks reveals the local validity limit. *)
SmallLocalLevel := Lesson17LocalLevelNext(
    CurrentLevel := SmallLocalLevel,
    OperatingLevel := OperatingLevel,
    FlowDeviation := SmallFlowDeviation,
    LocalGain := LocalGain,
    TimeConstant := TimeConstant,
    ElapsedTime := LREAL#0.1
);
LargeLocalLevel := Lesson17LocalLevelNext(
    CurrentLevel := LargeLocalLevel,
    OperatingLevel := OperatingLevel,
    FlowDeviation := LargeFlowDeviation,
    LocalGain := LocalGain,
    TimeConstant := TimeConstant,
    ElapsedTime := LREAL#0.1
);
SmallPhysicalEquilibrium := Lesson17NonlinearEquilibrium(
    InletFlow := SmallInletFlow,
    OutletCoefficient := OutletCoefficient
);
LargePhysicalEquilibrium := Lesson17NonlinearEquilibrium(
    InletFlow := LargeInletFlow,
    OutletCoefficient := OutletCoefficient
);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#1s, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.SmallInletFlow AT %QL0 : LREAL;
    P1.LargeInletFlow AT %QL8 : LREAL;
    P1.SmallPhysicalLevel AT %IL0 : LREAL;
    P1.LargePhysicalLevel AT %IL8 : LREAL;
    P1.SmallLocalLevel AT %ML0 : LREAL;
    P1.LargeLocalLevel AT %ML8 : LREAL;
    P1.LocalGain AT %ML16 : LREAL;
    P1.TimeConstant AT %ML24 : LREAL;
    P1.SmallPhysicalEquilibrium AT %ML32 : LREAL;
    P1.LargePhysicalEquilibrium AT %ML40 : LREAL;
    P1.ExperimentEnabled AT %MX80.0 : BOOL;
END_VAR
END_CONFIGURATION
