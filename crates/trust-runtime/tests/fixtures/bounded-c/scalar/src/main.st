(* Bounded-C corpus, scalar: arithmetic, logic, comparison, TIME and control flow on scalar
   globals and program variables, with no call, process image, text or aggregate. Every
   variable is read on every scan, so no later scan reaches a name or a stack depth for the
   first time. *)
VAR_GLOBAL
    ticks : DINT := DINT#0;
    total : LINT := LINT#0;
    level : LREAL := LREAL#0.0;
    gain : REAL := REAL#1.5;
    mask : WORD := WORD#16#00FF;
    elapsed : TIME := T#0ms;
END_VAR

PROGRAM Scalars
VAR
    phase : INT := INT#0;
    index : DINT := DINT#0;
    sum : DINT := DINT#0;
    ratio : REAL := REAL#0.0;
    ready : BOOL := FALSE;
END_VAR
ticks := ticks + DINT#1;
sum := DINT#0;
FOR index := DINT#1 TO DINT#8 DO
    sum := sum + (index * DINT#3) - (index MOD DINT#2);
END_FOR;
total := total + LINT#7;
level := level * LREAL#0.5 + LREAL#2.0;
ratio := gain * REAL#2.0 / REAL#3.0;
mask := mask XOR WORD#16#0F0F;
elapsed := elapsed + T#10ms;
ready := ((sum > DINT#50) AND NOT ready) OR (phase = INT#2);
CASE phase OF
    0: phase := INT#1;
    1: phase := INT#2;
ELSE
    phase := INT#0;
END_CASE;
IF (ticks MOD DINT#5) = DINT#0 THEN
    sum := sum - DINT#1;
END_IF;
END_PROGRAM

CONFIGURATION BoundedC
RESOURCE Controller ON PLC
TASK Scan (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Scan : Scalars;
END_RESOURCE
END_CONFIGURATION
