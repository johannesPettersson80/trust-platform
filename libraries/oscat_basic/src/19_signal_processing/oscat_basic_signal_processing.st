(* OSCAT BASIC 3.33 signal-processing helpers. *)

FUNCTION BYTE_TO_RANGE : REAL
VAR_INPUT
    X : BYTE;
    low : REAL;
    high : REAL;
END_VAR
BYTE_TO_RANGE := (high - low) * DINT_TO_REAL(BYTE_TO_DINT(X)) / 255.0 + low;
END_FUNCTION

FUNCTION RANGE_TO_BYTE : BYTE
VAR_INPUT
    X : REAL;
    low : REAL;
    high : REAL;
END_VAR
VAR
    LimitedValue : REAL;
    ScaledValue : REAL;
    TruncatedValue : DINT;
END_VAR
LimitedValue := LIMIT(MN := low, IN := X, MX := high);
ScaledValue := (LimitedValue - low) * 255.0 / (high - low);
TruncatedValue := FLOOR2(X := ScaledValue);
RANGE_TO_BYTE := DINT_TO_BYTE(TruncatedValue);
END_FUNCTION

FUNCTION RANGE_TO_WORD : WORD
VAR_INPUT
    X : REAL;
    low : REAL;
    high : REAL;
END_VAR
VAR
    LimitedValue : REAL;
    ScaledValue : REAL;
    TruncatedValue : DINT;
END_VAR
LimitedValue := LIMIT(MN := low, IN := X, MX := high);
ScaledValue := (LimitedValue - low) * 65535.0 / (high - low);
TruncatedValue := FLOOR2(X := ScaledValue);
RANGE_TO_WORD := DINT_TO_WORD(TruncatedValue);
END_FUNCTION

FUNCTION SCALE : REAL
VAR_INPUT
    X : REAL;
    K : REAL;
    O : REAL;
    MX : REAL;
    MN : REAL;
END_VAR
SCALE := LIMIT(MN := MN, IN := X * K + O, MX := MX);
END_FUNCTION

FUNCTION WORD_TO_RANGE : REAL
VAR_INPUT
    X : WORD;
    low : REAL;
    high : REAL;
END_VAR
WORD_TO_RANGE := (high - low) * DINT_TO_REAL(WORD_TO_DINT(X)) / 65535.0 + low;
END_FUNCTION

FUNCTION_BLOCK DELAY
VAR_INPUT
    IN : REAL;
    N : INT;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    OUT : REAL;
END_VAR
VAR
    BUF : ARRAY[0..31] OF REAL;
    I : INT;
    INIT : BOOL;
    STOP : INT;
    WINDOW : INT;
END_VAR

WINDOW := LIMIT(INT#0, N, INT#32);
STOP := WINDOW - INT#1;
IF RST OR NOT INIT THEN
    INIT := TRUE;
    FOR I := INT#0 TO STOP DO
        BUF[I] := IN;
    END_FOR;
    OUT := IN;
    I := INT#0;
ELSIF STOP < INT#0 THEN
    OUT := IN;
ELSE
    OUT := BUF[I];
    BUF[I] := IN;
    I := INC1(X := I, N := WINDOW);
END_IF;
END_FUNCTION_BLOCK
