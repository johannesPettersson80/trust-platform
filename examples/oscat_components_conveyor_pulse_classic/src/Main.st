PROGRAM Main
VAR
    BeltPulse : GEN_PULSE;
    ConveyorEnabled : BOOL := FALSE;
    PulseCommand : BOOL;
END_VAR

BeltPulse(ENQ := ConveyorEnabled, PTH := T#500ms, PTL := T#500ms);
PulseCommand := BeltPulse.Q;
END_PROGRAM
