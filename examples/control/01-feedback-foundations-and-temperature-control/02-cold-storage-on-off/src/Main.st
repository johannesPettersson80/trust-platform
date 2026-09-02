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
END_VAR

IF AutoMode THEN
    (* This lesson intentionally uses one threshold. The resulting chatter is
       the observed failure that motivates hysteresis and dwell in Lesson 3. *)
    RequestedCooling := ProcessValue >= Setpoint;
ELSE
    RequestedCooling := FALSE;
END_IF;

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
    P1.AutoMode AT %MX16.0 : BOOL;
    P1.CoolingEnabled AT %MX16.1 : BOOL;
    P1.SwitchCount AT %MD24 : DINT;
    P1.ManipulatedValue AT %QL0 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
    P1.DoorOpenIndication AT %IL8 : LREAL;
    P1.EffectiveSurroundings AT %IL16 : LREAL;
END_VAR
END_CONFIGURATION
