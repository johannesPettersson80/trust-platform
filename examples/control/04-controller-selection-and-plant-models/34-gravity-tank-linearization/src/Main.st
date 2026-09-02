PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    TankAreaSquareMetres : LREAL := 0.5;
    OutletCoefficient : LREAL := 0.02;
    SampleTimeSeconds : LREAL := 0.1;
    FlowDeviation : LREAL := 0.0;

    LowOperatingLevelMetres : LREAL := 0.25;
    MiddleOperatingLevelMetres : LREAL := 0.50;
    HighOperatingLevelMetres : LREAL := 0.81;

    LowEquilibriumFlow : LREAL := 0.0;
    MiddleEquilibriumFlow : LREAL := 0.0;
    HighEquilibriumFlow : LREAL := 0.0;
    LowGain : LREAL := 0.0;
    MiddleGain : LREAL := 0.0;
    HighGain : LREAL := 0.0;
    LowTimeConstant : LREAL := 0.0;
    MiddleTimeConstant : LREAL := 0.0;
    HighTimeConstant : LREAL := 0.0;
    LowPole : LREAL := 0.0;
    MiddlePole : LREAL := 0.0;
    HighPole : LREAL := 0.0;

    LowLocalDeviationMetres : LREAL := 0.0;
    MiddleLocalDeviationMetres : LREAL := 0.0;
    HighLocalDeviationMetres : LREAL := 0.0;
    LowLocalLevelCentimetres : LREAL := 25.0;
    MiddleLocalLevelCentimetres : LREAL := 50.0;
    HighLocalLevelCentimetres : LREAL := 81.0;

    LowInletFlow : LREAL := 0.01;
    MiddleInletFlow : LREAL := 0.014142135623730952;
    HighInletFlow : LREAL := 0.018;
    LowPhysicalLevel : LREAL := 0.25;
    MiddlePhysicalLevel : LREAL := 0.50;
    HighPhysicalLevel : LREAL := 0.81;
END_VAR

IF ExperimentEnabled THEN
    FlowDeviation := LREAL#0.001;
ELSIF ExperimentWasEnabled THEN
    FlowDeviation := LREAL#0.0;
END_IF;

InputsValid := LowPhysicalLevel >= LREAL#0.0
    AND LowPhysicalLevel <= LREAL#1.0
    AND MiddlePhysicalLevel >= LREAL#0.0
    AND MiddlePhysicalLevel <= LREAL#1.0
    AND HighPhysicalLevel >= LREAL#0.0
    AND HighPhysicalLevel <= LREAL#1.2;
IF SafetyAbort OR NOT InputsValid THEN
    FlowDeviation := LREAL#0.0;
END_IF;

(* Derive each tangent model from the same gravity-outflow balance. *)
LowEquilibriumFlow := OutletCoefficient * SQRT(LowOperatingLevelMetres);
MiddleEquilibriumFlow := OutletCoefficient * SQRT(MiddleOperatingLevelMetres);
HighEquilibriumFlow := OutletCoefficient * SQRT(HighOperatingLevelMetres);

LowGain := LREAL#2.0 * SQRT(LowOperatingLevelMetres) / OutletCoefficient;
MiddleGain := LREAL#2.0 * SQRT(MiddleOperatingLevelMetres) / OutletCoefficient;
HighGain := LREAL#2.0 * SQRT(HighOperatingLevelMetres) / OutletCoefficient;

LowTimeConstant :=
    LREAL#2.0 * TankAreaSquareMetres * SQRT(LowOperatingLevelMetres)
    / OutletCoefficient;
MiddleTimeConstant :=
    LREAL#2.0 * TankAreaSquareMetres * SQRT(MiddleOperatingLevelMetres)
    / OutletCoefficient;
HighTimeConstant :=
    LREAL#2.0 * TankAreaSquareMetres * SQRT(HighOperatingLevelMetres)
    / OutletCoefficient;

LowPole := -LREAL#1.0 / LowTimeConstant;
MiddlePole := -LREAL#1.0 / MiddleTimeConstant;
HighPole := -LREAL#1.0 / HighTimeConstant;

LowInletFlow := LowEquilibriumFlow + FlowDeviation;
MiddleInletFlow := MiddleEquilibriumFlow + FlowDeviation;
HighInletFlow := HighEquilibriumFlow + FlowDeviation;

(* Forward Euler exposes the local first-order differential equations. *)
LowLocalDeviationMetres := LowLocalDeviationMetres
    + SampleTimeSeconds
    * ((-LowLocalDeviationMetres + LowGain * FlowDeviation) / LowTimeConstant);
MiddleLocalDeviationMetres := MiddleLocalDeviationMetres
    + SampleTimeSeconds
    * ((-MiddleLocalDeviationMetres + MiddleGain * FlowDeviation)
        / MiddleTimeConstant);
HighLocalDeviationMetres := HighLocalDeviationMetres
    + SampleTimeSeconds
    * ((-HighLocalDeviationMetres + HighGain * FlowDeviation) / HighTimeConstant);

LowLocalLevelCentimetres :=
    (LowOperatingLevelMetres + LowLocalDeviationMetres) * LREAL#100.0;
MiddleLocalLevelCentimetres :=
    (MiddleOperatingLevelMetres + MiddleLocalDeviationMetres) * LREAL#100.0;
HighLocalLevelCentimetres :=
    (HighOperatingLevelMetres + HighLocalDeviationMetres) * LREAL#100.0;
ExperimentWasEnabled := ExperimentEnabled;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX136.0 : BOOL;
    P1.SafetyAbort AT %MX136.1 : BOOL;
    P1.InputsValid AT %MX136.2 : BOOL;
    P1.FlowDeviation AT %ML0 : LREAL;
    P1.LowInletFlow AT %QL0 : LREAL;
    P1.MiddleInletFlow AT %QL8 : LREAL;
    P1.HighInletFlow AT %QL16 : LREAL;
    P1.LowPhysicalLevel AT %IL0 : LREAL;
    P1.MiddlePhysicalLevel AT %IL8 : LREAL;
    P1.HighPhysicalLevel AT %IL16 : LREAL;
    P1.LowEquilibriumFlow AT %ML16 : LREAL;
    P1.MiddleEquilibriumFlow AT %ML24 : LREAL;
    P1.HighEquilibriumFlow AT %ML32 : LREAL;
    P1.LowGain AT %ML40 : LREAL;
    P1.MiddleGain AT %ML48 : LREAL;
    P1.HighGain AT %ML56 : LREAL;
    P1.LowTimeConstant AT %ML64 : LREAL;
    P1.MiddleTimeConstant AT %ML72 : LREAL;
    P1.HighTimeConstant AT %ML80 : LREAL;
    P1.LowPole AT %ML88 : LREAL;
    P1.MiddlePole AT %ML96 : LREAL;
    P1.HighPole AT %ML104 : LREAL;
    P1.LowLocalLevelCentimetres AT %ML112 : LREAL;
    P1.MiddleLocalLevelCentimetres AT %ML120 : LREAL;
    P1.HighLocalLevelCentimetres AT %ML128 : LREAL;
END_VAR
END_CONFIGURATION
