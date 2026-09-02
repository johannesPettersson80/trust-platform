PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ExperimentInput : LREAL := 0.0;
    BoundedExperimentInput : LREAL := 0.0;
    HeaterCommand : LREAL := 20.0;
    TankInflowTrim : LREAL := 0.0;
    OvenCommand : LREAL := 20.0;
    DriveTorque : LREAL := 0.0;
    HeaterTemperature : LREAL := 50.0;
    TankLevel : LREAL := 50.0;
    OvenTemperature : LREAL := 80.0;
    DrivePosition : LREAL := 0.0;
END_VAR

IF ExperimentEnabled THEN
    ExperimentInput := LREAL#1.0;
ELSIF ExperimentWasEnabled THEN
    ExperimentInput := LREAL#0.0;
END_IF;
InputsValid := HeaterTemperature >= LREAL#0.0
    AND HeaterTemperature <= LREAL#100.0
    AND TankLevel >= LREAL#0.0 AND TankLevel <= LREAL#100.0
    AND OvenTemperature >= LREAL#0.0 AND OvenTemperature <= LREAL#150.0
    AND DrivePosition >= LREAL#-5.0 AND DrivePosition <= LREAL#5.0;
IF SafetyAbort OR NOT InputsValid THEN
    ExperimentInput := LREAL#0.0;
END_IF;

(* One approved normalized step is converted into real actuator units. No
   feedback controller or fitted model is hidden inside this open-loop test. *)
BoundedExperimentInput := MAX(MIN(ExperimentInput, LREAL#1.0), LREAL#0.0);
HeaterCommand := LREAL#20.0 + LREAL#20.0 * BoundedExperimentInput;
TankInflowTrim := LREAL#10.0 * BoundedExperimentInput;
OvenCommand := LREAL#20.0 + LREAL#20.0 * BoundedExperimentInput;
DriveTorque := LREAL#1.0 * BoundedExperimentInput;
ExperimentWasEnabled := ExperimentEnabled;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX40.0 : BOOL;
    P1.SafetyAbort AT %MX40.1 : BOOL;
    P1.InputsValid AT %MX40.2 : BOOL;
    P1.ExperimentInput AT %ML0 : LREAL;
    P1.BoundedExperimentInput AT %ML8 : LREAL;
    P1.HeaterCommand AT %QL0 : LREAL;
    P1.TankInflowTrim AT %QL8 : LREAL;
    P1.OvenCommand AT %QL16 : LREAL;
    P1.DriveTorque AT %QL24 : LREAL;
    P1.HeaterTemperature AT %IL0 : LREAL;
    P1.TankLevel AT %IL8 : LREAL;
    P1.OvenTemperature AT %IL16 : LREAL;
    P1.DrivePosition AT %IL24 : LREAL;
END_VAR
END_CONFIGURATION
