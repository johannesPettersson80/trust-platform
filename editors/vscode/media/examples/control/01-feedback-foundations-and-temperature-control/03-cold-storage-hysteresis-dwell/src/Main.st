FUNCTION ColdStorageHysteresisRequest : BOOL
VAR_INPUT
    CurrentRequest : BOOL;
    ProcessValue : LREAL;
    Setpoint : LREAL;
    HysteresisWidth : LREAL;
END_VAR

ColdStorageHysteresisRequest := CurrentRequest;
IF ProcessValue >= Setpoint + HysteresisWidth * LREAL#0.5 THEN
    ColdStorageHysteresisRequest := TRUE;
ELSIF ProcessValue <= Setpoint - HysteresisWidth * LREAL#0.5 THEN
    ColdStorageHysteresisRequest := FALSE;
END_IF;
END_FUNCTION

FUNCTION ColdStorageDwellComplete : BOOL
VAR_INPUT
    CoolingEnabled : BOOL;
    DwellElapsed : LREAL;
    MinimumOnTime : LREAL;
    MinimumOffTime : LREAL;
END_VAR

ColdStorageDwellComplete :=
    (CoolingEnabled AND DwellElapsed >= MinimumOnTime - LREAL#1.0e-9)
    OR ((NOT CoolingEnabled) AND DwellElapsed >= MinimumOffTime - LREAL#1.0e-9);
END_FUNCTION

PROGRAM Main
VAR
    AutoMode : BOOL := FALSE;
    Initialized : BOOL;
    RequestedCooling : BOOL := FALSE;
    CoolingEnabled : BOOL := FALSE;
    Setpoint : LREAL := 2.0;
    HysteresisWidth : LREAL := 1.0;
    MinimumOnTime : LREAL := 8.0;
    MinimumOffTime : LREAL := 8.0;
    CycleTime : LREAL := 0.1;
    ProcessValue : LREAL := 4.0;
    ManipulatedValue : LREAL := 0.0;
    DoorOpenIndication : LREAL := 0.0;
    EffectiveSurroundings : LREAL := 4.0;
    DwellElapsed : LREAL;
    DwellInhibit : BOOL := FALSE;
    SwitchCount : DINT := DINT#0;
    ExperimentEnabled : BOOL := FALSE;
    ExperimentScan : ULINT := ULINT#0;
    DoorCommand : LREAL := 0.0;
    SurroundingsCommand : LREAL := 4.0;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
END_VAR
(* These are fixed experiment settings, not live tuning selections. Publish
   them after the addressed process image is read on every update. *)
Setpoint := LREAL#2.0;
HysteresisWidth := LREAL#1.0;
MinimumOnTime := LREAL#8.0;
MinimumOffTime := LREAL#8.0;
(* Temperature-range abort remains the learner's check in this lesson. *)
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




(* Qualifying the initial dwell allows the first safe demand immediately;
   later scans retain elapsed time so minimum on/off durations are enforceable. *)
IF NOT Initialized THEN
    DwellElapsed := MAX(MinimumOnTime, MinimumOffTime);
    Initialized := TRUE;
END_IF;

IF AutoMode THEN
    DwellElapsed := DwellElapsed + CycleTime;
    DwellInhibit := FALSE;
    (* Inside the band, the requested state is retained. This state memory is
       what prevents measurement crossings near one threshold from chattering. *)
    RequestedCooling := ColdStorageHysteresisRequest(
        CurrentRequest := RequestedCooling,
        ProcessValue := ProcessValue,
        Setpoint := Setpoint,
        HysteresisWidth := HysteresisWidth
    );

    IF RequestedCooling <> CoolingEnabled THEN
        IF ColdStorageDwellComplete(
            CoolingEnabled := CoolingEnabled,
            DwellElapsed := DwellElapsed,
            MinimumOnTime := MinimumOnTime,
            MinimumOffTime := MinimumOffTime
        ) THEN
            CoolingEnabled := RequestedCooling;
            DwellElapsed := LREAL#0.0;
            SwitchCount := SwitchCount + DINT#1;
        ELSE
            (* Requested and applied states intentionally differ while the
               equipment-protection dwell interval is still active. *)
            DwellInhibit := TRUE;
        END_IF;
    END_IF;
ELSE
    RequestedCooling := FALSE;
    DwellInhibit := FALSE;
    IF CoolingEnabled THEN
        CoolingEnabled := FALSE;
        DwellElapsed := LREAL#0.0;
        SwitchCount := SwitchCount + DINT#1;
    ELSE
        DwellElapsed := DwellElapsed + CycleTime;
    END_IF;
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
    P1.HysteresisWidth AT %ML8 : LREAL;
    P1.MinimumOnTime AT %ML16 : LREAL;
    P1.MinimumOffTime AT %ML24 : LREAL;
    P1.DwellElapsed AT %ML32 : LREAL;
    P1.SwitchCount AT %MD40 : DINT;
    P1.AutoMode AT %MX44.0 : BOOL;
    P1.RequestedCooling AT %MX44.1 : BOOL;
    P1.CoolingEnabled AT %MX44.2 : BOOL;
    P1.DwellInhibit AT %MX44.3 : BOOL;
    P1.ManipulatedValue AT %QL0 : LREAL;
    P1.SurroundingsCommand AT %QL8 : LREAL;
    P1.DoorCommand AT %QL16 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
    P1.DoorOpenIndication AT %IL8 : LREAL;
    P1.EffectiveSurroundings AT %IL16 : LREAL;
    P1.ExperimentEnabled AT %MX44.4 : BOOL;
    P1.SafetyAbort AT %MX44.5 : BOOL;
    P1.InputsValid AT %MX44.6 : BOOL;
END_VAR
END_CONFIGURATION
