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

FUNCTION_BLOCK FT_MIN_MAX
VAR_INPUT
    IN : REAL;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    MX : REAL;
    MN : REAL;
END_VAR
VAR
    INIT : BOOL;
END_VAR

IF RST OR NOT INIT THEN
    MN := IN;
    MX := IN;
    INIT := TRUE;
ELSIF IN < MN THEN
    MN := IN;
ELSIF IN > MX THEN
    MX := IN;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FT_RMP
VAR_INPUT
    RMP : BOOL := TRUE;
    IN : REAL;
    KR : REAL;
    KF : REAL;
END_VAR
VAR_OUTPUT
    OUT : REAL;
    BUSY : BOOL;
    UD : BOOL;
END_VAR
VAR
    TxMs : DINT;
    LastMs : DINT;
    DeltaMs : DINT;
    INIT : BOOL;
END_VAR

TxMs := DWORD_TO_DINT(T_PLC_MS());
IF NOT INIT THEN
    INIT := TRUE;
    LastMs := TxMs;
    OUT := IN;
END_IF;
DeltaMs := TxMs - LastMs;

IF NOT RMP THEN
    OUT := IN;
    BUSY := FALSE;
ELSIF OUT > IN THEN
    OUT := OUT - DINT_TO_REAL(DeltaMs) * REAL#0.001 * KF;
    OUT := MAX(IN, OUT);
ELSIF OUT < IN THEN
    OUT := OUT + DINT_TO_REAL(DeltaMs) * REAL#0.001 * KR;
    OUT := MIN(IN, OUT);
END_IF;

IF OUT < IN THEN
    BUSY := TRUE;
    UD := TRUE;
ELSIF OUT > IN THEN
    BUSY := TRUE;
    UD := FALSE;
ELSE
    BUSY := FALSE;
END_IF;
LastMs := TxMs;
END_FUNCTION_BLOCK

FUNCTION LINEAR_INT : REAL
VAR_INPUT
    X : REAL;
    XY : ARRAY[1..20, 0..1] OF REAL;
    PTS : INT;
END_VAR
VAR
    I : INT;
    LocalPts : INT;
END_VAR

LocalPts := MIN(PTS, INT#20);
I := INT#2;
WHILE (I < LocalPts) AND (XY[I, 0] < X) DO
    I := I + 1;
END_WHILE;
LINEAR_INT := ((XY[I, 1] - XY[I - 1, 1]) * X - XY[I, 1] * XY[I - 1, 0] + XY[I - 1, 1] * XY[I, 0]) / (XY[I, 0] - XY[I - 1, 0]);
END_FUNCTION

FUNCTION POLYNOM_INT : REAL
VAR_INPUT
    X : REAL;
    XY : ARRAY[1..5, 0..1] OF REAL;
    PTS : INT;
END_VAR
VAR
    I : INT;
    J : INT;
    K : INT;
    Stop : INT;
    LocalPts : INT;
    Work : ARRAY[1..5, 0..1] OF REAL;
END_VAR

LocalPts := MIN(PTS, INT#5);
FOR I := INT#1 TO LocalPts DO
    FOR K := INT#0 TO INT#1 DO
        Work[I, K] := XY[I, K];
    END_FOR;
END_FOR;
FOR I := INT#1 TO LocalPts DO
    Stop := I + 1;
    FOR J := LocalPts TO Stop BY -1 DO
        Work[J, 1] := (Work[J, 1] - Work[J - 1, 1]) / (Work[J, 0] - Work[J - I, 0]);
    END_FOR;
END_FOR;

POLYNOM_INT := REAL#0.0;
FOR I := LocalPts TO INT#1 BY -1 DO
    POLYNOM_INT := POLYNOM_INT * (X - Work[I, 0]) + Work[I, 1];
END_FOR;
END_FUNCTION
