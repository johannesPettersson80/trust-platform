PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ExperimentInput : LREAL := 0.0;
    BoundedExperiment : LREAL := 0.0;
    BearingExperimentEnabled : BOOL := FALSE;

    HeaterCommand : LREAL := 0.0;
    TankNetFlow : LREAL := 0.0;
    DriveTorque : LREAL := 0.0;
    CraneForce : LREAL := 0.0;
    BearingForce : LREAL := 0.0;
    BoilerFeedwater : LREAL := 0.0;

    HeaterTemperature : LREAL := 20.0;
    TankLevel : LREAL := 50.0;
    DrivePosition : LREAL := 0.0;
    CranePayloadPosition : LREAL := 0.0;
    BearingDisplacement : LREAL := 0.0;
    BoilerDrumLevel : LREAL := 0.0;
END_VAR

IF ExperimentEnabled OR BearingExperimentEnabled THEN
    ExperimentInput := LREAL#1.0;
ELSE
    ExperimentInput := LREAL#0.0;
END_IF;

InputsValid := HeaterTemperature >= LREAL#0.0
    AND HeaterTemperature <= LREAL#100.0
    AND TankLevel >= LREAL#0.0 AND TankLevel <= LREAL#100.0
    AND DrivePosition >= LREAL#-5.0 AND DrivePosition <= LREAL#5.0
    AND CranePayloadPosition >= LREAL#-2.0
    AND CranePayloadPosition <= LREAL#2.0
    AND BearingDisplacement >= LREAL#-0.01
    AND BearingDisplacement <= LREAL#0.01
    AND BoilerDrumLevel >= LREAL#-2.0 AND BoilerDrumLevel <= LREAL#2.0;
IF SafetyAbort OR NOT InputsValid THEN
    ExperimentInput := LREAL#0.0;
    BearingExperimentEnabled := FALSE;
END_IF;

IF ExperimentInput < LREAL#0.0 THEN
    BoundedExperiment := LREAL#0.0;
ELSIF ExperimentInput > LREAL#1.0 THEN
    BoundedExperiment := LREAL#1.0;
ELSE
    BoundedExperiment := ExperimentInput;
END_IF;

(* One normalized step becomes a physical, documented input for each plant. *)
HeaterCommand := LREAL#20.0 * BoundedExperiment;
TankNetFlow := LREAL#1.0 * BoundedExperiment;
DriveTorque := LREAL#1.0 * BoundedExperiment;
CraneForce := LREAL#1.0 * BoundedExperiment;
BoilerFeedwater := LREAL#1.0 * BoundedExperiment;

(* The unstable bearing is excited only in its separate abort-bounded run. *)
IF BearingExperimentEnabled THEN
    BearingForce := LREAL#0.0001 * BoundedExperiment;
ELSE
    BearingForce := LREAL#0.0;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX8.1 : BOOL;
    P1.SafetyAbort AT %MX8.2 : BOOL;
    P1.InputsValid AT %MX8.3 : BOOL;
    P1.ExperimentInput AT %ML0 : LREAL;
    P1.BearingExperimentEnabled AT %MX8.0 : BOOL;
    P1.HeaterCommand AT %QL0 : LREAL;
    P1.TankNetFlow AT %QL8 : LREAL;
    P1.DriveTorque AT %QL16 : LREAL;
    P1.CraneForce AT %QL24 : LREAL;
    P1.BearingForce AT %QL32 : LREAL;
    P1.BoilerFeedwater AT %QL40 : LREAL;
    P1.HeaterTemperature AT %IL0 : LREAL;
    P1.TankLevel AT %IL8 : LREAL;
    P1.DrivePosition AT %IL16 : LREAL;
    P1.CranePayloadPosition AT %IL24 : LREAL;
    P1.BearingDisplacement AT %IL32 : LREAL;
    P1.BoilerDrumLevel AT %IL40 : LREAL;
END_VAR
END_CONFIGURATION
