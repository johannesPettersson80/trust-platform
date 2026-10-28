(* Create the fixed experiment command. The function has no measured-temperature
   input, so the measurement cannot change its OFF or ON decision. *)
FUNCTION ColdStorageManualDemand : LREAL
VAR_INPUT
    Enabled : BOOL;
    InputsValid : BOOL;
    SafetyAbort : BOOL;
    ExperimentScan : ULINT;
END_VAR

IF Enabled AND InputsValid AND NOT SafetyAbort
    AND ExperimentScan <= ULINT#200
THEN
    ColdStorageManualDemand := LREAL#100.0;
ELSE
    ColdStorageManualDemand := LREAL#0.0;
END_IF;
END_FUNCTION

PROGRAM Main
VAR
    (* Desired room temperature in degrees Celsius. *)
    Setpoint : LREAL := 2.0;
    (* Compressor state requested by this program: 0.0 is OFF, 100.0 is ON. *)
    ManualDemand : LREAL := 0.0;
    (* Room temperature reported by the sensor in degrees Celsius. *)
    ProcessValue : LREAL := 4.0;
    (* Compressor state sent to the simulated refrigeration system. *)
    ManipulatedValue : LREAL := 0.0;
    DoorOpenIndication : LREAL := 0.0;
    EffectiveSurroundings : LREAL := 4.0;
    (* Independent simulated heat-load input; 4 degC with the door closed,
       6 degC with it open. This is not a compressor command. *)
    SurroundingsCommand : LREAL := 4.0;
    DoorCommand : LREAL := 0.0;
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ExperimentScan : ULINT := ULINT#0;
    ExperimentElapsedSeconds : LREAL := 0.0;
END_VAR

(* These are program-owned observations, not learner selections. Publish the
   fixed reference and compute validity each scan, including the first scan
   after the process image is connected. Comparisons also reject NaN/infinity. *)
Setpoint := LREAL#2.0;
InputsValid := ProcessValue >= LREAL#-10.0 AND ProcessValue <= LREAL#10.0;

(* Both the cooling interval and door interval begin at experiment enablement,
   so reading the instructions with the simulator running cannot consume them. *)
IF ExperimentEnabled AND InputsValid AND NOT SafetyAbort THEN
    ExperimentScan := ExperimentScan + ULINT#1;
    ExperimentElapsedSeconds := ULINT_TO_LREAL(ExperimentScan) * LREAL#0.1;
ELSE
    ExperimentScan := ULINT#0;
    ExperimentElapsedSeconds := LREAL#0.0;
END_IF;

DoorCommand := LREAL#0.0;
SurroundingsCommand := LREAL#4.0;
IF ExperimentEnabled AND InputsValid AND NOT SafetyAbort
    AND ExperimentScan >= ULINT#600 AND ExperimentScan < ULINT#900
THEN
    DoorCommand := LREAL#1.0;
    SurroundingsCommand := LREAL#6.0;
END_IF;

ManualDemand := ColdStorageManualDemand(
    Enabled := ExperimentEnabled,
    InputsValid := InputsValid,
    SafetyAbort := SafetyAbort,
    ExperimentScan := ExperimentScan
);

(* Apply the requested compressor state without changing it. *)
ManipulatedValue := ManualDemand;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    (* Run Main once every 100 milliseconds, which is 0.1 second. *)
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.Setpoint AT %ML0 : LREAL;
    P1.ManualDemand AT %ML8 : LREAL;
    P1.ManipulatedValue AT %QL0 : LREAL;
    P1.SurroundingsCommand AT %QL8 : LREAL;
    P1.DoorCommand AT %QL16 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
    P1.DoorOpenIndication AT %IL8 : LREAL;
    P1.EffectiveSurroundings AT %IL16 : LREAL;
    P1.ExperimentEnabled AT %MX24.0 : BOOL;
    P1.SafetyAbort AT %MX24.1 : BOOL;
    P1.InputsValid AT %MX24.2 : BOOL;
    P1.ExperimentElapsedSeconds AT %ML32 : LREAL;
END_VAR
END_CONFIGURATION
