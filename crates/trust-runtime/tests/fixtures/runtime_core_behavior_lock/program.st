TYPE
    Phase : (IDLE, RUNNING, DONE);
    Payload : STRUCT
        count : DINT;
        flag : BOOL;
    END_STRUCT;
END_TYPE

FUNCTION_BLOCK Bump
VAR_INPUT
    IN : DINT;
END_VAR
VAR_OUTPUT
    OUT : DINT;
END_VAR
OUT := IN + DINT#1;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    in0 AT %IX0.0 : BOOL;
    out0 AT %QX0.0 : BOOL;
    out_word AT %QW2 : WORD;
    phase : Phase := Phase#IDLE;
    payload : Payload;
    samples : ARRAY[0..2] OF DINT;
    fb : Bump;
    ref_count : REF_TO DINT;
END_VAR
VAR RETAIN
    retained_count : DINT := DINT#5;
END_VAR

ref_count := REF(payload.count);

IF in0 THEN
    phase := RUNNING;
    samples[0] := DINT#7;
    samples[1] := samples[0] + DINT#2;
    samples[2] := samples[1] + DINT#3;
    ref_count^ := samples[2];
    payload.flag := TRUE;
    fb(IN := payload.count);
    retained_count := retained_count + fb.OUT;
    out0 := payload.flag;
    out_word := WORD#16#1234;
ELSE
    phase := IDLE;
    payload.flag := FALSE;
    out0 := FALSE;
    out_word := WORD#0;
END_IF
END_PROGRAM
