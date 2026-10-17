(* Bounded-C corpus, retain: retained globals and retained program variables, captured before
   the outputs on every scan when the services keep a retain store (census row S9). The
   program itself makes no call and holds no aggregate. *)
VAR_GLOBAL RETAIN
    counter : DINT := DINT#0;
    setpoint : REAL := REAL#20.0;
END_VAR

VAR_GLOBAL
    ticks : DINT := DINT#0;
END_VAR

PROGRAM Retained
VAR RETAIN
    runs : DINT;
    label : STRING[12] := 'boiler';
END_VAR
ticks := ticks + DINT#1;
counter := counter + DINT#1;
runs := runs + DINT#1;
setpoint := setpoint + REAL#0.5;
END_PROGRAM

CONFIGURATION BoundedC
RESOURCE Controller ON PLC
TASK Scan (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Scan : Retained;
END_RESOURCE
END_CONFIGURATION
