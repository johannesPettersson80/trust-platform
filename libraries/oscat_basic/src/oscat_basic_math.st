(* Additional OSCAT BASIC 3.33 mathematical helpers ported for truST. *)

FUNCTION ACOSH : REAL
VAR_INPUT
    X : REAL;
END_VAR
ACOSH := LN(SQRT(X * X - 1.0) + X);
END_FUNCTION

FUNCTION ACOTH : REAL
VAR_INPUT
    X : REAL;
END_VAR
ACOTH := LN((X + 1.0) / (X - 1.0)) * 0.5;
END_FUNCTION

FUNCTION ASINH : REAL
VAR_INPUT
    X : REAL;
END_VAR
ASINH := LN(SQRT(X * X + 1.0) + X);
END_FUNCTION

FUNCTION ATANH : REAL
VAR_INPUT
    X : REAL;
END_VAR
ATANH := LN((1.0 + X) / (1.0 - X)) * 0.5;
END_FUNCTION

FUNCTION CEIL : INT
VAR_INPUT
    X : REAL;
END_VAR
CEIL := REAL_TO_INT(X);
IF INT_TO_REAL(CEIL) < X THEN
    CEIL := CEIL + 1;
END_IF;
END_FUNCTION

FUNCTION CEIL2 : DINT
VAR_INPUT
    X : REAL;
END_VAR
CEIL2 := REAL_TO_DINT(X);
IF DINT_TO_REAL(CEIL2) < X THEN
    CEIL2 := CEIL2 + 1;
END_IF;
END_FUNCTION

FUNCTION CMP : BOOL
VAR_INPUT
    X : REAL;
    Y : REAL;
    N : INT;
END_VAR
VAR
    ScaleFactor : REAL;
    LogValue : REAL;
    DecadeExponent : INT;
    XDigits : DINT;
    YDigits : DINT;
END_VAR
ScaleFactor := ABS(X);
IF ScaleFactor > 0.0 THEN
    LogValue := LOG(ScaleFactor);
    DecadeExponent := FLOOR(X := LogValue) - N + 1;
    ScaleFactor := EXP10(X := INT_TO_REAL(DecadeExponent));
    XDigits := FLOOR2(X := X / ScaleFactor);
    YDigits := FLOOR2(X := Y / ScaleFactor);
    CMP := XDigits = YDigits;
ELSE
    CMP := X = Y;
END_IF;
END_FUNCTION

FUNCTION COSH : REAL
VAR_INPUT
    X : REAL;
END_VAR
VAR
    T : REAL;
END_VAR
T := EXP(X);
COSH := (1.0 / T + T) * 0.5;
END_FUNCTION

FUNCTION D_TRUNC : DINT
VAR_INPUT
    X : REAL;
END_VAR
D_TRUNC := REAL_TO_DINT(X);
IF X > 0.0 THEN
    IF DINT_TO_REAL(D_TRUNC) > X THEN
        D_TRUNC := D_TRUNC - 1;
    END_IF;
ELSE
    IF DINT_TO_REAL(D_TRUNC) < X THEN
        D_TRUNC := D_TRUNC + 1;
    END_IF;
END_IF;
END_FUNCTION

FUNCTION DEC1 : INT
VAR_INPUT
    X : INT;
    N : INT;
END_VAR
IF X = 0 THEN
    DEC1 := N - 1;
ELSE
    DEC1 := X - 1;
END_IF;
END_FUNCTION

FUNCTION EVEN : BOOL
VAR_INPUT
    IN : DINT;
END_VAR
EVEN := (IN MOD 2) = 0;
END_FUNCTION

FUNCTION FLOOR : INT
VAR_INPUT
    X : REAL;
END_VAR
FLOOR := REAL_TO_INT(X);
IF INT_TO_REAL(FLOOR) > X THEN
    FLOOR := FLOOR - 1;
END_IF;
END_FUNCTION

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

FUNCTION FRACT : REAL
VAR_INPUT
    X : REAL;
END_VAR
VAR
    IntegerPart : DINT;
END_VAR
IF ABS(X) < 2.0E9 THEN
    IntegerPart := D_TRUNC(X := X);
    FRACT := X - DINT_TO_REAL(IntegerPart);
ELSE
    FRACT := 0.0;
END_IF;
END_FUNCTION

FUNCTION INC : INT
VAR_INPUT
    X : INT;
    D : INT;
    M : INT;
END_VAR
INC := (X + D + M + 1) MOD (M + 1);
END_FUNCTION

FUNCTION INC1 : INT
VAR_INPUT
    X : INT;
    N : INT;
END_VAR
IF X >= N - 1 THEN
    INC1 := 0;
ELSE
    INC1 := X + 1;
END_IF;
END_FUNCTION

FUNCTION INC2 : INT
VAR_INPUT
    X : INT;
    D : INT;
    L : INT;
    U : INT;
END_VAR
VAR
    Span : INT;
END_VAR
Span := U - L + 1;
INC2 := (X + D - L + Span) MOD Span + L;
END_FUNCTION

FUNCTION INV : REAL
VAR_INPUT
    X : REAL;
END_VAR
IF X <> 0.0 THEN
    INV := 1.0 / X;
ELSE
    INV := 0.0;
END_IF;
END_FUNCTION

FUNCTION MAX3 : REAL
VAR_INPUT
    IN1 : REAL;
    IN2 : REAL;
    IN3 : REAL;
END_VAR
MAX3 := MAX(MAX(IN1, IN2), IN3);
END_FUNCTION

FUNCTION MID3 : REAL
VAR_INPUT
    IN1 : REAL;
    IN2 : REAL;
    IN3 : REAL;
END_VAR
MID3 := IN1 + IN2 + IN3 - MAX3(IN1 := IN1, IN2 := IN2, IN3 := IN3) - MIN3(IN1 := IN1, IN2 := IN2, IN3 := IN3);
END_FUNCTION

FUNCTION MIN3 : REAL
VAR_INPUT
    IN1 : REAL;
    IN2 : REAL;
    IN3 : REAL;
END_VAR
MIN3 := MIN(MIN(IN1, IN2), IN3);
END_FUNCTION

FUNCTION MUL_ADD : REAL
VAR_INPUT
    X : REAL;
    K : REAL;
    O : REAL;
END_VAR
MUL_ADD := X * K + O;
END_FUNCTION

FUNCTION NEGX : REAL
VAR_INPUT
    X : REAL;
END_VAR
NEGX := -X;
END_FUNCTION

FUNCTION SGN : INT
VAR_INPUT
    X : REAL;
END_VAR
IF X > 0.0 THEN
    SGN := 1;
ELSIF X < 0.0 THEN
    SGN := -1;
ELSE
    SGN := 0;
END_IF;
END_FUNCTION

FUNCTION SINH : REAL
VAR_INPUT
    X : REAL;
END_VAR
IF ABS(X) < 2.0E-3 THEN
    SINH := X;
ELSE
    SINH := (EXP(X) - EXP(-X)) * 0.5;
END_IF;
END_FUNCTION

FUNCTION TANH : REAL
VAR_INPUT
    X : REAL;
END_VAR
IF X > 20.0 THEN
    TANH := 1.0;
ELSIF X < -20.0 THEN
    TANH := -1.0;
ELSE
    TANH := 1.0 - 2.0 / (EXP(2.0 * X) + 1.0);
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
