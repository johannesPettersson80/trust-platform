INTERFACE ICounter
METHOD Advance : INT
VAR_INPUT
    delta : INT;
END_VAR
END_METHOD
END_INTERFACE

FUNCTION_BLOCK Counter IMPLEMENTS ICounter
VAR_INPUT
    pulse : BOOL R_EDGE;
END_VAR
VAR
    value : INT := INT#3;
    history : ARRAY[1..3] OF INT := [INT#1, INT#2, INT#3];
END_VAR
METHOD PUBLIC Advance : INT
VAR_INPUT
    delta : INT;
END_VAR
VAR
    previous : REF_TO INT := REF(history[delta]);
END_VAR
value := value + delta;
Advance := previous^;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    counter : Counter;
    port : ICounter;
    delay : TON;
    enabled : BOOL := TRUE;
    result : INT;
END_VAR
VAR RETAIN
    activations : DINT;
END_VAR
counter(pulse := enabled);
port := counter;
delay(IN := enabled, PT := T#25ms);
result := port.Advance(DINT_TO_INT(activations MOD DINT#3) + INT#1);
activations := activations + DINT#1;
END_PROGRAM

CONFIGURATION Portability
RESOURCE Controller ON PLC
TASK Periodic (INTERVAL := T#25ms, PRIORITY := 0);
PROGRAM Plant WITH Periodic : Main;
END_RESOURCE
END_CONFIGURATION
