(* Bounded-C corpus, blocks: function-block calls (census rows A3, A5, C1 to C4, C7, C8 and C11).
   A user function block with an R_EDGE input, a defaulted input and outputs copied back to a
   variable and to an array element, and the built-in TON, CTU and R_TRIG blocks. *)
VAR_GLOBAL
    ticks : DINT := DINT#0;
END_VAR

FUNCTION_BLOCK Pulse
VAR_INPUT
    trigger : BOOL R_EDGE;
    increment : INT := INT#1;
END_VAR
VAR_OUTPUT
    count : INT;
    active : BOOL;
END_VAR
IF trigger THEN
    count := count + increment;
END_IF;
active := trigger;
END_FUNCTION_BLOCK

PROGRAM Blocks
VAR
    pulse : Pulse;
    delay : TON;
    counter : CTU;
    edge : R_TRIG;
    toggle : BOOL := FALSE;
    total : INT;
    seen : BOOL;
    elapsed : TIME;
    counts : ARRAY[0..1] OF INT;
    slot : INT := INT#1;
END_VAR
ticks := ticks + DINT#1;
toggle := NOT toggle;
pulse(trigger := toggle, increment := INT#2, count => total, active => seen);
pulse(trigger := toggle, count => counts[slot]);
delay(IN := toggle, PT := T#20ms);
elapsed := delay.ET;
counter(CU := toggle, R := FALSE, PV := INT#1000);
edge(CLK := toggle);
seen := edge.Q OR counter.Q OR delay.Q;
END_PROGRAM

CONFIGURATION BoundedC
RESOURCE Controller ON PLC
TASK Scan (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Scan : Blocks;
END_RESOURCE
END_CONFIGURATION
