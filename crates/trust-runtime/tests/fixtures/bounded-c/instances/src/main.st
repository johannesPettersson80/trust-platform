(* Bounded-C corpus, instances: a class instance declared in a function body (census row N5).
   Every call of Window3 creates the instance with its inherited parent part, runs its
   methods and retires it with the frame. *)
VAR_GLOBAL
    ticks : DINT := DINT#0;
END_VAR

CLASS Accumulator
VAR PUBLIC
    sum : DINT := DINT#0;
END_VAR
METHOD PUBLIC Add : DINT
VAR_INPUT
    value : DINT;
END_VAR
sum := sum + value;
Add := sum;
END_METHOD
END_CLASS

CLASS Window EXTENDS Accumulator
VAR PUBLIC
    width : DINT := DINT#3;
END_VAR
END_CLASS

FUNCTION Window3 : DINT
VAR_INPUT
    seed : DINT;
END_VAR
VAR
    scratch : Window;
    index : DINT;
END_VAR
FOR index := DINT#1 TO scratch.width DO
    Window3 := scratch.Add(seed + index);
END_FOR;
END_FUNCTION

PROGRAM Instances
VAR
    result : DINT;
END_VAR
ticks := ticks + DINT#1;
result := Window3(DINT#2) + Window3(seed := DINT#5);
END_PROGRAM

CONFIGURATION BoundedC
RESOURCE Controller ON PLC
TASK Scan (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Scan : Instances;
END_RESOURCE
END_CONFIGURATION
