(* OSCAT BASIC 3.33 arithmetic function helpers. *)

FUNCTION F_LIN : REAL
VAR_INPUT
    X : REAL;
    A : REAL;
    B : REAL;
END_VAR
F_LIN := A * X + B;
END_FUNCTION

FUNCTION F_LIN2 : REAL
VAR_INPUT
    X : REAL;
    X1 : REAL;
    Y1 : REAL;
    X2 : REAL;
    Y2 : REAL;
END_VAR
F_LIN2 := (Y2 - Y1) / (X2 - X1) * (X - X1) + Y1;
END_FUNCTION

FUNCTION F_POLY : REAL
VAR_INPUT
    X : REAL;
    C : ARRAY[0..7] OF REAL;
END_VAR
F_POLY := (((((((C[7] * X + C[6]) * X + C[5]) * X + C[4]) * X + C[3]) * X + C[2]) * X + C[1]) * X + C[0]);
END_FUNCTION

FUNCTION F_POWER : REAL
VAR_INPUT
    A : REAL;
    X : REAL;
    N : REAL;
END_VAR
F_POWER := A * EXPT(X, N);
END_FUNCTION

FUNCTION F_QUAD : REAL
VAR_INPUT
    X : REAL;
    A : REAL;
    B : REAL;
    C : REAL;
END_VAR
F_QUAD := (A * X + B) * X + C;
END_FUNCTION

FUNCTION FRMP_B : BYTE
VAR_INPUT
    START : BYTE;
    DIR : BOOL;
    TD : TIME;
    TR : TIME;
END_VAR
VAR
    TdMs : DWORD;
    TrMs : DWORD;
    DeltaValue : DINT;
    StartValue : DINT;
    ResultValue : DINT;
    MaxDeltaValue : DINT;
END_VAR
IF TD < TR THEN
    TdMs := TIME_TO_DWORD(IN := TD);
    TrMs := TIME_TO_DWORD(IN := TR);
    DeltaValue := D_TRUNC(
        X := UDINT_TO_REAL(IN := DWORD_TO_UDINT(IN := TdMs)) * REAL#256.0
            / UDINT_TO_REAL(IN := DWORD_TO_UDINT(IN := TrMs))
    );
    StartValue := BYTE_TO_DINT(IN := START);
    IF DIR THEN
        MaxDeltaValue := DINT#255 - StartValue;
        IF DeltaValue > MaxDeltaValue THEN
            DeltaValue := MaxDeltaValue;
        END_IF;
        ResultValue := StartValue + DeltaValue;
    ELSE
        MaxDeltaValue := StartValue;
        IF DeltaValue > MaxDeltaValue THEN
            DeltaValue := MaxDeltaValue;
        END_IF;
        ResultValue := StartValue - DeltaValue;
    END_IF;
    FRMP_B := DINT_TO_BYTE(IN := ResultValue);
ELSE
    IF DIR THEN
        FRMP_B := BYTE#255;
    ELSE
        FRMP_B := BYTE#0;
    END_IF;
END_IF;
END_FUNCTION

FUNCTION_BLOCK FT_AVG
VAR_INPUT
    IN : REAL;
    E : BOOL := TRUE;
    N : INT := INT#32;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    AVG : REAL;
END_VAR
VAR
    BUFF : DELAY;
    I : INT;
    INIT : BOOL;
    WINDOW : INT;
END_VAR

WINDOW := LIMIT(INT#0, N, INT#32);
IF NOT INIT OR RST THEN
    FOR I := INT#1 TO WINDOW DO
        BUFF(IN := IN, N := WINDOW, RST := FALSE);
    END_FOR;
    AVG := IN;
    INIT := TRUE;
ELSIF E THEN
    BUFF(IN := IN, N := WINDOW, RST := FALSE);
    AVG := AVG + (IN - BUFF.OUT) / INT_TO_REAL(IN := WINDOW);
END_IF;
END_FUNCTION_BLOCK
