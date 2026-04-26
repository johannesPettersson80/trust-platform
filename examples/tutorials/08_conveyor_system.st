PROGRAM ConveyorSystem
VAR
    StartCmd : BOOL := FALSE;
    StopCmd : BOOL := FALSE;
    JamSensor : BOOL := FALSE;
    PartAtEntry : BOOL := FALSE;
    PartAtExit : BOOL := FALSE;
    ConveyorRun : BOOL := FALSE;
    RejectGate : BOOL := FALSE;
    PartCounter : DINT := 0;
END_VAR

IF StopCmd OR JamSensor THEN
    ConveyorRun := FALSE;
ELSIF StartCmd THEN
    ConveyorRun := TRUE;
END_IF;

IF ConveyorRun AND PartAtEntry THEN
    RejectGate := FALSE;
END_IF;

IF ConveyorRun AND PartAtExit THEN
    PartCounter := PartCounter + 1;
END_IF;

IF JamSensor THEN
    RejectGate := TRUE;
END_IF;
END_PROGRAM
