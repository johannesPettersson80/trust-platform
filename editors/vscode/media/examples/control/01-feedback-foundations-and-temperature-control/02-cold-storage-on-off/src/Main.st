FUNCTION ColdStorageCoolingRequest : BOOL
VAR_INPUT
    Enabled : BOOL;
    ProcessValue : LREAL;
    Setpoint : LREAL;
END_VAR

ColdStorageCoolingRequest := Enabled AND ProcessValue >= Setpoint;
END_FUNCTION

PROGRAM Main
VAR
    AutoMode : BOOL := FALSE;
    CoolingEnabled : BOOL := FALSE;
    RequestedCooling : BOOL;
    Setpoint : LREAL := 2.0;
    ProcessValue : LREAL := 4.0;
    ManipulatedValue : LREAL := 0.0;
    DoorOpenIndication : LREAL := 0.0;
    EffectiveSurroundings : LREAL := 4.0;
    SwitchCount : DINT := DINT#0;
    ExperimentEnabled : BOOL := FALSE;
    ExperimentScan : ULINT := ULINT#0;
    DoorCommand : LREAL := 0.0;
    SurroundingsCommand : LREAL := 4.0;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
END_VAR

(* Publish the fixed lesson target after the addressed process image is read. *)
Setpoint := LREAL#2.0;
(* This basic lesson has no automatic temperature-range trip. The learner
   checks the range and aborts as described in the README. Do not rely on an
   initial value in addressed memory to permit the switching experiment. *)
InputsValid := TRUE;
AutoMode := ExperimentEnabled AND InputsValid AND NOT SafetyAbort;

(* The same physical door experiment as Lesson1 starts with control authority,
   never while the learner is still preparing the simulator. *)
IF AutoMode THEN ExperimentScan := ExperimentScan + ULINT#1;
ELSE ExperimentScan := ULINT#0;
END_IF;
DoorCommand := LREAL#0.0;
SurroundingsCommand := LREAL#4.0;
IF AutoMode AND ExperimentScan >= ULINT#601 AND ExperimentScan <= ULINT#900 THEN
    DoorCommand := LREAL#1.0;
    SurroundingsCommand := LREAL#6.0;
END_IF;


(* This lesson intentionally uses one threshold. The resulting chatter is
   the observed failure that motivates hysteresis and dwell in Lesson 3. *)
RequestedCooling := ColdStorageCoolingRequest(
    Enabled := AutoMode,
    ProcessValue := ProcessValue,
    Setpoint := Setpoint
);

IF RequestedCooling <> CoolingEnabled THEN
    CoolingEnabled := RequestedCooling;
    SwitchCount := SwitchCount + DINT#1;
END_IF;

IF CoolingEnabled THEN
    ManipulatedValue := LREAL#100.0;
ELSE
    ManipulatedValue := LREAL#0.0;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.Setpoint AT %ML0 : LREAL;
    P1.ExperimentEnabled AT %MX16.2 : BOOL;
    P1.SafetyAbort AT %MX16.3 : BOOL;
    P1.InputsValid AT %MX16.4 : BOOL;
    P1.CoolingEnabled AT %MX16.1 : BOOL;
    P1.AutoMode AT %MX16.0 : BOOL;
    P1.SwitchCount AT %MD24 : DINT;
    P1.ManipulatedValue AT %QL0 : LREAL;
    P1.SurroundingsCommand AT %QL8 : LREAL;
    P1.DoorCommand AT %QL16 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
    P1.DoorOpenIndication AT %IL8 : LREAL;
    P1.EffectiveSurroundings AT %IL16 : LREAL;
END_VAR
END_CONFIGURATION
