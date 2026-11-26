// A part counter that keeps its total over restarts: each run counts one part, as a sensor would.
// TotalParts is RETAIN, so a warm restart and a stop keep it, and ShiftParts starts over at every
// start of the program.
PROGRAM PartCounter
VAR RETAIN
    TotalParts : UDINT;
END_VAR
VAR
    ShiftParts : UDINT;
END_VAR
TotalParts := TotalParts + 1;
ShiftParts := ShiftParts + 1;
END_PROGRAM
