FUNCTION LIST_ADD : BOOL
VAR_INPUT
    SEP : BYTE;
    INS : STRING[LIST_LENGTH];
END_VAR
VAR_IN_OUT
    LIST : STRING[LIST_LENGTH];
END_VAR
VAR
    SepText : STRING[1];
    Candidate : STRING[LIST_LENGTH];
END_VAR

SepText := CHR_TO_STRING(C := SEP);
Candidate := CONCAT(IN1 := LIST, IN2 := CONCAT(IN1 := SepText, IN2 := INS));
IF LEN(Candidate) > LIST_LENGTH THEN
    LIST_ADD := FALSE;
ELSE
    LIST := Candidate;
    LIST_ADD := TRUE;
END_IF;
END_FUNCTION

FUNCTION LIST_CLEAN : BOOL
VAR_INPUT
    SEP : BYTE;
END_VAR
VAR_IN_OUT
    LIST : STRING[LIST_LENGTH];
END_VAR
VAR
    Pos : INT;
    Current : BYTE;
    Result : STRING[LIST_LENGTH];
    LastWasSep : BOOL;
END_VAR

Result := '';
LastWasSep := FALSE;
FOR Pos := INT#1 TO LEN(LIST) DO
    Current := CODE(STR := LIST, POS := Pos);
    IF Current <> SEP OR NOT LastWasSep THEN
        Result := CONCAT(IN1 := Result, IN2 := CHR_TO_STRING(C := Current));
    END_IF;
    LastWasSep := Current = SEP;
END_FOR;

IF LEN(Result) > INT#0 AND CODE(STR := Result, POS := LEN(Result)) = SEP THEN
    Result := LEFT(IN := Result, L := LEN(Result) - INT#1);
END_IF;

LIST := Result;
LIST_CLEAN := TRUE;
END_FUNCTION

FUNCTION LIST_GET : STRING[LIST_LENGTH]
VAR_INPUT
    SEP : BYTE;
    POS : INT;
END_VAR
VAR_IN_OUT
    LIST : STRING[LIST_LENGTH];
END_VAR
VAR
    Index : INT;
    Count : INT;
    Current : BYTE;
    Done : BOOL;
END_VAR

LIST_GET := '';
Index := INT#1;
Count := INT#0;
Done := FALSE;

WHILE Index <= LEN(LIST) AND NOT Done DO
    Current := CODE(STR := LIST, POS := Index);
    IF Current = SEP THEN
        Count := Count + INT#1;
        IF Count > POS THEN
            Done := TRUE;
        END_IF;
    ELSIF Count = POS THEN
        LIST_GET := CONCAT(IN1 := LIST_GET, IN2 := CHR_TO_STRING(C := Current));
    END_IF;
    Index := Index + INT#1;
END_WHILE;
END_FUNCTION

FUNCTION LIST_INSERT : BOOL
VAR_INPUT
    SEP : BYTE;
    POS : INT;
    INS : STRING[LIST_LENGTH];
END_VAR
VAR_IN_OUT
    LIST : STRING[LIST_LENGTH];
END_VAR
VAR
    SepText : STRING[1];
    InsertText : STRING[LIST_LENGTH];
    Prefix : STRING[LIST_LENGTH];
    Suffix : STRING[LIST_LENGTH];
    InsertPos : INT;
    Index : INT;
    Count : INT;
    Done : BOOL;
END_VAR

LIST_INSERT := FALSE;
SepText := CHR_TO_STRING(C := SEP);
InsertText := CONCAT(IN1 := SepText, IN2 := INS);

IF LEN(LIST) + LEN(InsertText) <= LIST_LENGTH THEN
    InsertPos := LEN(LIST) + INT#1;
    IF POS <= INT#1 THEN
        InsertPos := INT#1;
    ELSE
        Index := INT#1;
        Count := INT#0;
        Done := FALSE;
        WHILE Index <= LEN(LIST) AND NOT Done DO
            IF CODE(STR := LIST, POS := Index) = SEP THEN
                Count := Count + INT#1;
                IF Count = POS THEN
                    InsertPos := Index;
                    Done := TRUE;
                END_IF;
            END_IF;
            Index := Index + INT#1;
        END_WHILE;
    END_IF;

    IF InsertPos > INT#1 THEN
        Prefix := LEFT(IN := LIST, L := InsertPos - INT#1);
    ELSE
        Prefix := '';
    END_IF;

    IF InsertPos <= LEN(LIST) THEN
        Suffix := MID(IN := LIST, L := LEN(LIST) - InsertPos + INT#1, P := InsertPos);
    ELSE
        Suffix := '';
    END_IF;

    LIST := CONCAT(IN1 := Prefix, IN2 := CONCAT(IN1 := InsertText, IN2 := Suffix));
    LIST_INSERT := TRUE;
END_IF;
END_FUNCTION

FUNCTION LIST_LEN : INT
VAR_INPUT
    SEP : BYTE;
END_VAR
VAR_IN_OUT
    LIST : STRING[LIST_LENGTH];
END_VAR
VAR
    Pos : INT;
END_VAR

LIST_LEN := INT#0;
FOR Pos := INT#1 TO LEN(LIST) DO
    IF CODE(STR := LIST, POS := Pos) = SEP THEN
        LIST_LEN := LIST_LEN + INT#1;
    END_IF;
END_FOR;
END_FUNCTION

FUNCTION_BLOCK LIST_NEXT
VAR_INPUT
    SEP : BYTE;
    RST : BOOL;
END_VAR
VAR_IN_OUT
    LIST : STRING[LIST_LENGTH];
END_VAR
VAR_OUTPUT
    LEL : STRING[LIST_LENGTH];
    NUL : BOOL;
END_VAR
VAR
    Pos : INT := INT#1;
    Index : INT;
    ListSize : INT;
END_VAR

IF RST THEN
    Pos := INT#1;
END_IF;

LEL := '';
ListSize := LEN(LIST);
IF Pos > ListSize OR ListSize = INT#0 THEN
    NUL := TRUE;
ELSE
    NUL := FALSE;
    Index := Pos + INT#1;
    WHILE Index <= ListSize AND CODE(STR := LIST, POS := Index) <> SEP DO
        LEL := CONCAT(IN1 := LEL, IN2 := MID(IN := LIST, L := INT#1, P := Index));
        Index := Index + INT#1;
    END_WHILE;
    Pos := Index;
END_IF;
END_FUNCTION_BLOCK

FUNCTION LIST_RETRIEVE : STRING[LIST_LENGTH]
VAR_INPUT
    SEP : BYTE;
    POS : INT;
END_VAR
VAR_IN_OUT
    LIST : STRING[LIST_LENGTH];
END_VAR
VAR
    Index : INT;
    Count : INT;
    StartPos : INT;
    EndPos : INT;
    Prefix : STRING[LIST_LENGTH];
    Suffix : STRING[LIST_LENGTH];
END_VAR

LIST_RETRIEVE := '';
StartPos := INT#0;
EndPos := LEN(LIST) + INT#1;

IF POS > INT#0 THEN
    Index := INT#1;
    Count := INT#0;
    WHILE Index <= LEN(LIST) AND EndPos = LEN(LIST) + INT#1 DO
        IF CODE(STR := LIST, POS := Index) = SEP THEN
            Count := Count + INT#1;
            IF Count = POS THEN
                StartPos := Index;
            ELSIF Count = POS + INT#1 AND StartPos > INT#0 THEN
                EndPos := Index;
            END_IF;
        END_IF;
        Index := Index + INT#1;
    END_WHILE;

    IF StartPos > INT#0 THEN
        IF EndPos > StartPos + INT#1 THEN
            LIST_RETRIEVE := MID(IN := LIST, L := EndPos - StartPos - INT#1, P := StartPos + INT#1);
        END_IF;

        IF StartPos > INT#1 THEN
            Prefix := LEFT(IN := LIST, L := StartPos - INT#1);
        ELSE
            Prefix := '';
        END_IF;

        IF EndPos <= LEN(LIST) THEN
            Suffix := MID(IN := LIST, L := LEN(LIST) - EndPos + INT#1, P := EndPos);
        ELSE
            Suffix := '';
        END_IF;

        LIST := CONCAT(IN1 := Prefix, IN2 := Suffix);
    END_IF;
END_IF;
END_FUNCTION

FUNCTION LIST_RETRIEVE_LAST : STRING[LIST_LENGTH]
VAR_INPUT
    SEP : BYTE;
END_VAR
VAR_IN_OUT
    LIST : STRING[LIST_LENGTH];
END_VAR
VAR
    Index : INT;
    LastSep : INT;
END_VAR

LIST_RETRIEVE_LAST := '';
LastSep := INT#0;

FOR Index := INT#1 TO LEN(LIST) DO
    IF CODE(STR := LIST, POS := Index) = SEP THEN
        LastSep := Index;
    END_IF;
END_FOR;

IF LastSep > INT#0 THEN
    IF LastSep < LEN(LIST) THEN
        LIST_RETRIEVE_LAST := MID(IN := LIST, L := LEN(LIST) - LastSep, P := LastSep + INT#1);
    END_IF;

    IF LastSep > INT#1 THEN
        LIST := LEFT(IN := LIST, L := LastSep - INT#1);
    ELSE
        LIST := '';
    END_IF;
END_IF;
END_FUNCTION
