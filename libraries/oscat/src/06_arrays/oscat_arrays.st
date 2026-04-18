(* OSCAT BASIC 3.33 Chapter 6 Arrays helpers. *)

FUNCTION _ARRAY_ABS : BOOL
VAR_INPUT
    PT : POINTER TO ARRAY[*] OF REAL;
    SIZE : UINT;
END_VAR
VAR
    Count : INT;
    I : INT;
END_VAR
Count := UINT_TO_INT(SIZE) / 4;
FOR I := 0 TO Count - 1 DO
    PT^[I] := ABS(PT^[I]);
END_FOR;
_ARRAY_ABS := TRUE;
END_FUNCTION

FUNCTION _ARRAY_ADD : BOOL
VAR_INPUT
    PT : POINTER TO ARRAY[*] OF REAL;
    SIZE : UINT;
    X : REAL;
END_VAR
VAR
    Count : INT;
    I : INT;
END_VAR
Count := UINT_TO_INT(SIZE) / 4;
FOR I := 0 TO Count - 1 DO
    PT^[I] := PT^[I] + X;
END_FOR;
_ARRAY_ADD := TRUE;
END_FUNCTION

FUNCTION _ARRAY_INIT : BOOL
VAR_INPUT
    PT : POINTER TO ARRAY[*] OF REAL;
    SIZE : UINT;
    INIT : REAL;
END_VAR
VAR
    Count : INT;
    I : INT;
END_VAR
Count := UINT_TO_INT(SIZE) / 4;
FOR I := 0 TO Count - 1 DO
    PT^[I] := INIT;
END_FOR;
_ARRAY_INIT := TRUE;
END_FUNCTION

FUNCTION _ARRAY_MEDIAN : REAL
VAR_INPUT
    PT : POINTER TO ARRAY[*] OF REAL;
    SIZE : UINT;
END_VAR
VAR
    Count : INT;
    Mid : INT;
END_VAR
_ARRAY_SORT(PT := PT, SIZE := SIZE);
Count := UINT_TO_INT(SIZE) / 4;
IF Count <= 0 THEN
    _ARRAY_MEDIAN := 0.0;
ELSIF (Count MOD 2) = 1 THEN
    Mid := Count / 2;
    _ARRAY_MEDIAN := PT^[Mid];
ELSE
    Mid := Count / 2;
    _ARRAY_MEDIAN := (PT^[Mid - 1] + PT^[Mid]) * 0.5;
END_IF;
END_FUNCTION

FUNCTION _ARRAY_MUL : BOOL
VAR_INPUT
    PT : POINTER TO ARRAY[*] OF REAL;
    SIZE : UINT;
    X : REAL;
END_VAR
VAR
    Count : INT;
    I : INT;
END_VAR
Count := UINT_TO_INT(SIZE) / 4;
FOR I := 0 TO Count - 1 DO
    PT^[I] := PT^[I] * X;
END_FOR;
_ARRAY_MUL := TRUE;
END_FUNCTION

FUNCTION _ARRAY_SHUFFLE : BOOL
VAR_INPUT
    PT : POINTER TO ARRAY[*] OF REAL;
    SIZE : UINT;
END_VAR
VAR
    Temp : REAL;
    Pos : INT;
    I : INT;
    Stop : INT;
END_VAR
Stop := UINT_TO_INT(SIZE) / 4 - 1;
FOR I := 0 TO Stop DO
    Pos := RDM2(LAST := I + Pos, LOW := I, HIGH := Stop);
    Temp := PT^[I];
    PT^[I] := PT^[Pos];
    PT^[Pos] := Temp;
END_FOR;
_ARRAY_SHUFFLE := TRUE;
END_FUNCTION

FUNCTION _ARRAY_SORT : BOOL
VAR_INPUT
    PT : POINTER TO ARRAY[*] OF REAL;
    SIZE : UINT;
END_VAR
VAR
    Count : INT;
    I : INT;
    J : INT;
    Value : REAL;
END_VAR
Count := UINT_TO_INT(SIZE) / 4;
FOR I := 1 TO Count - 1 DO
    Value := PT^[I];
    J := I - 1;
    WHILE J >= 0 DO
        IF PT^[J] > Value THEN
            PT^[J + 1] := PT^[J];
            J := J - 1;
        ELSE
            EXIT;
        END_IF;
    END_WHILE;
    PT^[J + 1] := Value;
END_FOR;
_ARRAY_SORT := TRUE;
END_FUNCTION

FUNCTION ARRAY_AVG : REAL
VAR_INPUT
    PT : POINTER TO ARRAY[*] OF REAL;
    SIZE : UINT;
END_VAR
VAR
    Count : INT;
    I : INT;
END_VAR
Count := UINT_TO_INT(SIZE) / 4;
IF Count <= 0 THEN
    ARRAY_AVG := 0.0;
ELSE
    ARRAY_AVG := PT^[0];
    FOR I := 1 TO Count - 1 DO
        ARRAY_AVG := ARRAY_AVG + PT^[I];
    END_FOR;
    ARRAY_AVG := ARRAY_AVG / INT_TO_REAL(Count);
END_IF;
END_FUNCTION

FUNCTION ARRAY_GAV : REAL
VAR_INPUT
    PT : POINTER TO ARRAY[*] OF REAL;
    SIZE : UINT;
END_VAR
VAR
    Count : INT;
    I : INT;
END_VAR
Count := UINT_TO_INT(SIZE) / 4;
ARRAY_GAV := 1.0;
FOR I := 0 TO Count - 1 DO
    IF PT^[I] > 0.0 THEN
        ARRAY_GAV := ARRAY_GAV * PT^[I];
    ELSE
        ARRAY_GAV := 0.0;
        EXIT;
    END_IF;
END_FOR;
IF ARRAY_GAV > 0.0 THEN
    ARRAY_GAV := SQRTN(X := ARRAY_GAV, N := Count);
END_IF;
END_FUNCTION

FUNCTION ARRAY_HAV : REAL
VAR_INPUT
    PT : POINTER TO ARRAY[*] OF REAL;
    SIZE : UINT;
END_VAR
VAR
    Count : INT;
    I : INT;
END_VAR
Count := UINT_TO_INT(SIZE) / 4;
FOR I := 0 TO Count - 1 DO
    IF PT^[I] <> 0.0 THEN
        ARRAY_HAV := ARRAY_HAV + 1.0 / PT^[I];
    ELSE
        ARRAY_HAV := 0.0;
        EXIT;
    END_IF;
END_FOR;
IF ARRAY_HAV <> 0.0 THEN
    ARRAY_HAV := INT_TO_REAL(Count) / ARRAY_HAV;
END_IF;
END_FUNCTION

FUNCTION ARRAY_MAX : REAL
VAR_INPUT
    PT : POINTER TO ARRAY[*] OF REAL;
    SIZE : UINT;
END_VAR
VAR
    Count : INT;
    I : INT;
END_VAR
Count := UINT_TO_INT(SIZE) / 4;
IF Count <= 0 THEN
    ARRAY_MAX := 0.0;
ELSE
    ARRAY_MAX := PT^[0];
    FOR I := 1 TO Count - 1 DO
        IF PT^[I] > ARRAY_MAX THEN
            ARRAY_MAX := PT^[I];
        END_IF;
    END_FOR;
END_IF;
END_FUNCTION

FUNCTION ARRAY_MIN : REAL
VAR_INPUT
    PT : POINTER TO ARRAY[*] OF REAL;
    SIZE : UINT;
END_VAR
VAR
    Count : INT;
    I : INT;
END_VAR
Count := UINT_TO_INT(SIZE) / 4;
IF Count <= 0 THEN
    ARRAY_MIN := 0.0;
ELSE
    ARRAY_MIN := PT^[0];
    FOR I := 1 TO Count - 1 DO
        IF PT^[I] < ARRAY_MIN THEN
            ARRAY_MIN := PT^[I];
        END_IF;
    END_FOR;
END_IF;
END_FUNCTION

FUNCTION ARRAY_SDV : REAL
VAR_INPUT
    PT : POINTER TO ARRAY[*] OF REAL;
    SIZE : UINT;
END_VAR
ARRAY_SDV := SQRT(ARRAY_VAR(PT := PT, SIZE := SIZE));
END_FUNCTION

FUNCTION ARRAY_SPR : REAL
VAR_INPUT
    PT : POINTER TO ARRAY[*] OF REAL;
    SIZE : UINT;
END_VAR
ARRAY_SPR := ARRAY_MAX(PT := PT, SIZE := SIZE) - ARRAY_MIN(PT := PT, SIZE := SIZE);
END_FUNCTION

FUNCTION ARRAY_SUM : REAL
VAR_INPUT
    PT : POINTER TO ARRAY[*] OF REAL;
    SIZE : UINT;
END_VAR
VAR
    Count : INT;
    I : INT;
END_VAR
Count := UINT_TO_INT(SIZE) / 4;
IF Count <= 0 THEN
    ARRAY_SUM := 0.0;
ELSE
    ARRAY_SUM := PT^[0];
    FOR I := 1 TO Count - 1 DO
        ARRAY_SUM := ARRAY_SUM + PT^[I];
    END_FOR;
END_IF;
END_FUNCTION

FUNCTION ARRAY_TREND : REAL
VAR_INPUT
    PT : POINTER TO ARRAY[*] OF REAL;
    SIZE : UINT;
END_VAR
VAR
    Count : INT;
    Stop : INT;
    Half : INT;
    StartSecond : INT;
    I : INT;
    Acc : REAL;
END_VAR
Count := UINT_TO_INT(SIZE) / 4;
IF Count <= 0 THEN
    ARRAY_TREND := 0.0;
ELSE
    Stop := Count - 1;
    Half := Stop / 2;
    FOR I := 0 TO Half DO
        Acc := Acc - PT^[I];
    END_FOR;
    IF (Stop MOD 2) = 0 THEN
        StartSecond := Half;
    ELSE
        StartSecond := Half + 1;
    END_IF;
    FOR I := StartSecond TO Stop DO
        Acc := Acc + PT^[I];
    END_FOR;
    ARRAY_TREND := Acc / INT_TO_REAL(Half + 1);
END_IF;
END_FUNCTION

FUNCTION ARRAY_VAR : REAL
VAR_INPUT
    PT : POINTER TO ARRAY[*] OF REAL;
    SIZE : UINT;
END_VAR
VAR
    Count : INT;
    Avg : REAL;
    I : INT;
END_VAR
Count := UINT_TO_INT(SIZE) / 4;
IF Count <= 1 THEN
    ARRAY_VAR := 0.0;
ELSE
    Avg := ARRAY_AVG(PT := PT, SIZE := SIZE);
    FOR I := 0 TO Count - 1 DO
        ARRAY_VAR := ARRAY_VAR + (PT^[I] - Avg) * (PT^[I] - Avg);
    END_FOR;
    ARRAY_VAR := ARRAY_VAR / INT_TO_REAL(Count - 1);
END_IF;
END_FUNCTION

FUNCTION IS_SORTED : BOOL
VAR_INPUT
    PT : POINTER TO ARRAY[*] OF REAL;
    SIZE : UINT;
END_VAR
VAR
    Count : INT;
    I : INT;
END_VAR
Count := UINT_TO_INT(SIZE) / 4;
FOR I := 0 TO Count - 2 DO
    IF PT^[I] > PT^[I + 1] THEN
        IS_SORTED := FALSE;
        EXIT;
    END_IF;
END_FOR;
IF Count > 0 THEN
    IS_SORTED := TRUE;
END_IF;
END_FUNCTION
