(* Bounded-C corpus, methods: methods and interfaces (census rows A3, C1, C2, C5, I10 and I12).
   Method calls on a function block with positional and named inputs, interface assignments
   from a function block and from a class with their lifetime checks, calls through the
   interface, and an interface assignment attempt. *)
VAR_GLOBAL
    ticks : DINT := DINT#0;
END_VAR

INTERFACE IGauge
METHOD Measure : INT
END_METHOD
END_INTERFACE

FUNCTION_BLOCK Tank IMPLEMENTS IGauge
VAR
    level : INT := INT#5;
END_VAR
METHOD PUBLIC Measure : INT
Measure := level;
END_METHOD
METHOD PUBLIC Raise : INT
VAR_INPUT
    delta : INT;
END_VAR
level := level + delta;
IF level > INT#100 THEN
    level := INT#0;
END_IF;
Raise := level;
END_METHOD
END_FUNCTION_BLOCK

CLASS Probe IMPLEMENTS IGauge
VAR PUBLIC
    sample : INT := INT#9;
END_VAR
METHOD PUBLIC Measure : INT
Measure := sample;
END_METHOD
END_CLASS

PROGRAM Methods
VAR
    tank : Tank;
    probe : Probe;
    port : IGauge;
    other : IGauge;
    level : INT;
    total : INT;
END_VAR
ticks := ticks + DINT#1;
level := tank.Raise(INT#3);
level := tank.Raise(delta := INT#1);
port := tank;
total := port.Measure();
port := probe;
total := total + port.Measure();
other ?= tank;
total := total + other.Measure();
END_PROGRAM

CONFIGURATION BoundedC
RESOURCE Controller ON PLC
TASK Scan (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Scan : Methods;
END_RESOURCE
END_CONFIGURATION
