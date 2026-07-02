FUNCTION _BUFFER_CLEAR : BOOL
VAR_IN_OUT
    PT : ARRAY[*] OF BYTE;
END_VAR
VAR_INPUT
    SIZE : UINT;
END_VAR
VAR
    Index : INT;
    Limit : INT;
END_VAR

_BUFFER_CLEAR := TRUE;
Limit := UINT_TO_INT(SIZE);
IF Limit > INT#0 THEN
    FOR Index := INT#0 TO Limit - INT#1 DO
        PT[Index] := BYTE#0;
    END_FOR;
END_IF;
END_FUNCTION

FUNCTION _BUFFER_INIT : BOOL
VAR_IN_OUT
    PT : ARRAY[*] OF BYTE;
END_VAR
VAR_INPUT
    SIZE : UINT;
    INIT : BYTE;
END_VAR
VAR
    Index : INT;
    Limit : INT;
END_VAR

_BUFFER_INIT := TRUE;
Limit := UINT_TO_INT(SIZE);
IF Limit > INT#0 THEN
    FOR Index := INT#0 TO Limit - INT#1 DO
        PT[Index] := INIT;
    END_FOR;
END_IF;
END_FUNCTION

FUNCTION _STRING_TO_BUFFER : INT
VAR_IN_OUT
    PT : ARRAY[*] OF BYTE;
END_VAR
VAR_INPUT
    STR : STRING[STRING_LENGTH];
    POS : INT;
    SIZE : UINT;
END_VAR
VAR
    EndPos : INT;
    Index : INT;
END_VAR

_STRING_TO_BUFFER := POS;
IF POS >= INT#0 THEN
    EndPos := MIN(POS + LEN(STR), UINT_TO_INT(SIZE));
    IF EndPos > POS THEN
        FOR Index := POS TO EndPos - INT#1 DO
            PT[Index] := CODE(STR := STR, POS := Index - POS + INT#1);
        END_FOR;
    END_IF;
    _STRING_TO_BUFFER := EndPos;
END_IF;
END_FUNCTION

FUNCTION _BUFFER_INSERT : INT
VAR_IN_OUT
    PT : ARRAY[*] OF BYTE;
END_VAR
VAR_INPUT
    STR : STRING[STRING_LENGTH];
    POS : INT;
    SIZE : UINT;
END_VAR
VAR
    EndPos : INT;
    Limit : INT;
    CopyLen : INT;
    Index : INT;
    WritePos : INT;
END_VAR

_BUFFER_INSERT := POS;
Limit := UINT_TO_INT(SIZE);
CopyLen := LEN(STR);
IF POS >= INT#0 AND POS < Limit AND CopyLen > INT#0 THEN
    EndPos := MIN(POS + CopyLen, Limit);
    IF EndPos < Limit THEN
        FOR Index := Limit - INT#1 TO EndPos BY -INT#1 DO
            PT[Index] := PT[Index - CopyLen];
        END_FOR;
    END_IF;
    WritePos := POS;
    WHILE WritePos < EndPos DO
        PT[WritePos] := CODE(STR := STR, POS := WritePos - POS + INT#1);
        WritePos := WritePos + INT#1;
    END_WHILE;
    _BUFFER_INSERT := EndPos;
ELSIF POS >= INT#0 THEN
    _BUFFER_INSERT := MIN(POS, Limit);
END_IF;
END_FUNCTION

FUNCTION _BUFFER_UPPERCASE : BOOL
VAR_IN_OUT
    PT : ARRAY[*] OF BYTE;
END_VAR
VAR_INPUT
    SIZE : INT;
END_VAR
VAR
    Index : INT;
END_VAR

_BUFFER_UPPERCASE := TRUE;
IF SIZE > INT#0 THEN
    FOR Index := INT#0 TO SIZE - INT#1 DO
        PT[Index] := TO_UPPER(IN := PT[Index]);
    END_FOR;
END_IF;
END_FUNCTION

FUNCTION BUFFER_COMP : INT
VAR_IN_OUT
    PT1 : ARRAY[*] OF BYTE;
    PT2 : ARRAY[*] OF BYTE;
END_VAR
VAR_INPUT
    SIZE1 : INT;
    SIZE2 : INT;
    START : INT;
END_VAR
VAR
    I : INT;
    J : INT;
    EndPos : INT;
    Match : BOOL;
END_VAR

BUFFER_COMP := INT#-1;
IF SIZE2 > INT#0 AND SIZE2 <= SIZE1 THEN
    EndPos := SIZE1 - SIZE2;
    FOR I := START TO EndPos DO
        IF BUFFER_COMP = INT#-1 AND PT1[I] = PT2[0] THEN
            Match := TRUE;
            J := INT#1;
            WHILE J < SIZE2 DO
                IF Match AND PT2[J] <> PT1[I + J] THEN
                    Match := FALSE;
                END_IF;
                J := J + INT#1;
            END_WHILE;
            IF Match THEN
                BUFFER_COMP := I;
            END_IF;
        END_IF;
    END_FOR;
END_IF;
END_FUNCTION

FUNCTION BUFFER_SEARCH : INT
VAR_IN_OUT
    PT : ARRAY[*] OF BYTE;
END_VAR
VAR_INPUT
    SIZE : INT;
    STR : STRING[STRING_LENGTH];
    POS : INT;
    IGN : BOOL;
END_VAR
VAR
    I : INT;
    K : INT;
    EndPos : INT;
    Match : BOOL;
    Current : BYTE;
END_VAR

BUFFER_SEARCH := INT#-1;
IF LEN(STR) > INT#0 AND SIZE > INT#0 AND POS >= INT#0 THEN
    EndPos := MIN(SIZE - LEN(STR), SIZE - INT#1);
    FOR I := POS TO EndPos DO
        IF BUFFER_SEARCH = INT#-1 THEN
            Match := TRUE;
            K := INT#0;
            WHILE K < LEN(STR) DO
                IF IGN THEN
                    Current := TO_UPPER(IN := PT[I + K]);
                ELSE
                    Current := PT[I + K];
                END_IF;
                IF Match AND CODE(STR := STR, POS := K + INT#1) <> Current THEN
                    Match := FALSE;
                END_IF;
                K := K + INT#1;
            END_WHILE;
            IF Match THEN
                BUFFER_SEARCH := I;
            END_IF;
        END_IF;
    END_FOR;
END_IF;
END_FUNCTION

FUNCTION BUFFER_TO_STRING : STRING[STRING_LENGTH]
VAR_IN_OUT
    PT : ARRAY[*] OF BYTE;
END_VAR
VAR_INPUT
    SIZE : UINT;
    START : UINT;
    STOP : UINT;
END_VAR
VAR
    Sta : UINT;
    Stp : UINT;
    Index : UINT;
    Count : INT;
END_VAR

BUFFER_TO_STRING := '';
IF SIZE > UINT#0 THEN
    Sta := MIN(START, SIZE - UINT#1);
    Stp := MIN(STOP, SIZE - UINT#1);
    IF Stp >= Sta THEN
        Index := Sta;
        Count := INT#0;
        WHILE Index <= Stp AND Count < STRING_LENGTH DO
            BUFFER_TO_STRING := CONCAT(IN1 := BUFFER_TO_STRING, IN2 := CHR_TO_STRING(C := PT[UINT_TO_INT(Index)]));
            Index := Index + UINT#1;
            Count := Count + INT#1;
        END_WHILE;
    END_IF;
END_IF;
END_FUNCTION
