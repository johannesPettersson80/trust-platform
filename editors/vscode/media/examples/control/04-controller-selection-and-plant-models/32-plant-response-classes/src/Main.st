PROGRAM Main
VAR
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

(* One approved normalized step is converted into real actuator units. No
   feedback controller or fitted model is hidden inside this open-loop test. *)
BoundedExperimentInput := MAX(MIN(ExperimentInput, LREAL#1.0), LREAL#0.0);
HeaterCommand := LREAL#20.0 + LREAL#20.0 * BoundedExperimentInput;
TankInflowTrim := LREAL#10.0 * BoundedExperimentInput;
OvenCommand := LREAL#20.0 + LREAL#20.0 * BoundedExperimentInput;
DriveTorque := LREAL#1.0 * BoundedExperimentInput;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
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
