(* Bounded-C corpus, calls: user functions and standard functions (census rows A2 to A4, C1 and
   C5 to C11). Positional and named arguments, an omitted input with a default, VAR_OUTPUT
   copy-back to variables and to array elements, VAR_IN_OUT, nested user calls, fixed,
   variadic and named standard functions, and SPLIT_TOD with its outputs. *)
VAR_GLOBAL
    ticks : DINT := DINT#0;
END_VAR

FUNCTION Scale : DINT
VAR_INPUT
    value : DINT;
    factor : DINT := DINT#2;
END_VAR
Scale := value * factor;
END_FUNCTION

FUNCTION Digits : BOOL
VAR_INPUT
    value : DINT;
END_VAR
VAR_OUTPUT
    low : DINT;
    high : DINT;
END_VAR
low := value MOD DINT#10;
high := value / DINT#10;
Digits := low < high;
END_FUNCTION

FUNCTION Twice : DINT
VAR_INPUT
    value : DINT;
END_VAR
Twice := Scale(value := value) + Scale(value := value, factor := DINT#3);
END_FUNCTION

FUNCTION Bump : DINT
VAR_IN_OUT
    target : DINT;
END_VAR
target := target + DINT#1;
Bump := target;
END_FUNCTION

PROGRAM Calls
VAR
    a : DINT := DINT#7;
    b : DINT := DINT#0;
    lo : DINT;
    hi : DINT;
    ordered : BOOL;
    parts : ARRAY[0..1] OF DINT;
    root : LREAL;
    picked : INT;
    hour : INT;
    minute : INT;
    second : INT;
    millis : INT;
END_VAR
ticks := ticks + DINT#1;
b := Scale(value := a);
b := Scale(value := a, factor := DINT#4);
ordered := Digits(value := b, low => lo, high => hi);
ordered := Digits(value := b + ticks, low => parts[0], high => parts[1]);
b := Twice(a) + Bump(target := a);
a := DINT#7;
b := MAX(a, b);
b := LIMIT(DINT#0, b, DINT#1000);
b := ABS(b - DINT#600);
root := SQRT(LREAL#2.0) + EXPT(LREAL#2.0, LREAL#3.0);
picked := SEL(G := ordered, IN0 := INT#4, IN1 := INT#7);
SPLIT_TOD(TOD#12:34:56.789, hour, minute, second, millis);
END_PROGRAM

CONFIGURATION BoundedC
RESOURCE Controller ON PLC
TASK Scan (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Scan : Calls;
END_RESOURCE
END_CONFIGURATION
