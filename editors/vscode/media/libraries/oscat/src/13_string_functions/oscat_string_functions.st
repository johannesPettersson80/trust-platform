FUNCTION MONTH_TO_STRING : STRING[10]
VAR_INPUT
    MTH : INT;
    LANG : INT;
    LX : INT;
END_VAR
VAR
    Ly : INT;
END_VAR

IF LANG = INT#0 THEN
    Ly := LANGUAGE.DEFAULT;
ELSIF LANG > LANGUAGE.LMAX THEN
    Ly := LANGUAGE.LMAX;
ELSE
    Ly := LANG;
END_IF;

MONTH_TO_STRING := '';

IF MTH >= INT#1 AND MTH <= INT#12 THEN
    CASE LX OF
        INT#0:
            MONTH_TO_STRING := LANGUAGE.MONTHS[Ly, MTH];
        INT#3:
            MONTH_TO_STRING := LANGUAGE.MONTHS3[Ly, MTH];
    END_CASE;
END_IF;
END_FUNCTION

FUNCTION WEEKDAY_TO_STRING : STRING[10]
VAR_INPUT
    WDAY : INT;
    LANG : INT;
    LX : INT;
END_VAR
VAR
    Ly : INT;
END_VAR

IF LANG = INT#0 THEN
    Ly := LANGUAGE.DEFAULT;
ELSIF LANG > LANGUAGE.LMAX THEN
    Ly := LANGUAGE.LMAX;
ELSE
    Ly := LANG;
END_IF;

WEEKDAY_TO_STRING := '';

IF WDAY >= INT#1 AND WDAY <= INT#7 THEN
    CASE LX OF
        INT#0:
            WEEKDAY_TO_STRING := LANGUAGE.WEEKDAYS[Ly, WDAY];
        INT#2:
            WEEKDAY_TO_STRING := LANGUAGE.WEEKDAYS2[Ly, WDAY];
    END_CASE;
END_IF;
END_FUNCTION

FUNCTION CHR_TO_STRING : STRING[1]
VAR_INPUT
    C : BYTE;
END_VAR

CHR_TO_STRING := CHAR_TO_STRING(BYTE_TO_CHAR(C));
END_FUNCTION

FUNCTION FILL : STRING[STRING_LENGTH]
VAR_INPUT
    C : BYTE;
    L : INT;
END_VAR
VAR
    I : INT;
    FillChar : STRING[1];
    EffectiveLength : INT;
END_VAR

FillChar := CHR_TO_STRING(C);
EffectiveLength := LIMIT(INT#0, L, STRING_LENGTH);
FILL := '';
FOR I := INT#1 TO EffectiveLength DO
    FILL := CONCAT(IN1 := FILL, IN2 := FillChar);
END_FOR;
END_FUNCTION

FUNCTION FIX : STRING[STRING_LENGTH]
VAR_INPUT
    STR : STRING[STRING_LENGTH];
    L : INT;
    C : BYTE;
    M : INT;
END_VAR
VAR
    PadCount : INT;
    LeftPad : INT;
    RightPad : INT;
    FillText : STRING[STRING_LENGTH];
    EffectiveLength : INT;
END_VAR

EffectiveLength := LIMIT(INT#0, L, STRING_LENGTH);
PadCount := EffectiveLength - LEN(STR);

IF PadCount <= INT#0 THEN
    IF M = INT#1 THEN
        FIX := RIGHT(IN := STR, L := EffectiveLength);
    ELSE
        FIX := LEFT(IN := STR, L := EffectiveLength);
    END_IF;
ELSIF M = INT#1 THEN
    FillText := FILL(C := C, L := PadCount);
    FIX := CONCAT(IN1 := FillText, IN2 := STR);
ELSIF M = INT#2 THEN
    LeftPad := PadCount / INT#2;
    RightPad := PadCount - LeftPad;
    FillText := FILL(C := C, L := RightPad);
    FIX := CONCAT(IN1 := STR, IN2 := FillText);
    FillText := FILL(C := C, L := LeftPad);
    FIX := CONCAT(IN1 := FillText, IN2 := FIX);
ELSE
    FillText := FILL(C := C, L := PadCount);
    FIX := CONCAT(IN1 := STR, IN2 := FillText);
END_IF;
END_FUNCTION

FUNCTION REAL_TO_STRF : STRING[20]
VAR_INPUT
    IN : REAL;
    N : INT;
    DECIMAL : STRING[1];
END_VAR
VAR
    RoundedMagnitude : REAL;
    I : INT;
    Digits : INT;
END_VAR

Digits := LIMIT(INT#0, N, INT#7);
RoundedMagnitude := ABS(IN) * EXP10(X := INT_TO_REAL(Digits));
REAL_TO_STRF := DINT_TO_STRING(REAL_TO_DINT(RoundedMagnitude));

FOR I := LEN(REAL_TO_STRF) TO Digits DO
    REAL_TO_STRF := CONCAT(IN1 := '0', IN2 := REAL_TO_STRF);
END_FOR;

IF Digits > INT#0 THEN
    REAL_TO_STRF := INSERT(IN1 := REAL_TO_STRF, IN2 := DECIMAL, P := LEN(REAL_TO_STRF) - Digits);
END_IF;

IF IN < REAL#0.0 THEN
    REAL_TO_STRF := CONCAT(IN1 := '-', IN2 := REAL_TO_STRF);
END_IF;
END_FUNCTION

FUNCTION DWORD_TO_STRF : STRING[20]
VAR_INPUT
    IN : DWORD;
    N : INT;
END_VAR

DWORD_TO_STRF := FIX(
    STR := DWORD_TO_STRING(IN),
    L := LIMIT(INT#0, N, INT#20),
    C := BYTE#48,
    M := INT#1
);
END_FUNCTION

FUNCTION TRIME : STRING[STRING_LENGTH]
VAR_INPUT
    STR : STRING[STRING_LENGTH];
END_VAR

TRIME := STR;
WHILE LEN(TRIME) > INT#0 AND LEFT(IN := TRIME, L := INT#1) = ' ' DO
    TRIME := DELETE(IN := TRIME, L := INT#1, P := INT#1);
END_WHILE;
WHILE LEN(TRIME) > INT#0 AND RIGHT(IN := TRIME, L := INT#1) = ' ' DO
    TRIME := DELETE(IN := TRIME, L := INT#1, P := LEN(TRIME));
END_WHILE;
END_FUNCTION

FUNCTION TRIM : STRING[STRING_LENGTH]
VAR_INPUT
    STR : STRING[STRING_LENGTH];
END_VAR
VAR
    Pos : INT;
END_VAR

TRIM := STR;
REPEAT
    Pos := FIND(IN1 := TRIM, IN2 := ' ');
    IF Pos > INT#0 THEN
        TRIM := REPLACE(IN1 := TRIM, IN2 := '', L := INT#1, P := Pos);
    END_IF;
UNTIL Pos = INT#0
END_REPEAT;
END_FUNCTION

FUNCTION TRIM1 : STRING[STRING_LENGTH]
VAR_INPUT
    STR : STRING[STRING_LENGTH];
END_VAR
VAR
    Pos : INT;
END_VAR

TRIM1 := TRIME(STR := STR);
REPEAT
    Pos := FIND(IN1 := TRIM1, IN2 := '  ');
    IF Pos > INT#0 THEN
        TRIM1 := REPLACE(IN1 := TRIM1, IN2 := ' ', L := INT#2, P := Pos);
    END_IF;
UNTIL Pos = INT#0
END_REPEAT;
END_FUNCTION

FUNCTION TO_LOWER : BYTE
VAR_INPUT
    IN : BYTE;
END_VAR

IF IN > BYTE#64 AND IN < BYTE#91 THEN
    TO_LOWER := IN OR BYTE#16#20;
ELSE
    TO_LOWER := IN;
END_IF;
END_FUNCTION

FUNCTION TO_UPPER : BYTE
VAR_INPUT
    IN : BYTE;
END_VAR

IF IN > BYTE#96 AND IN < BYTE#123 THEN
    TO_UPPER := IN AND BYTE#16#DF;
ELSE
    TO_UPPER := IN;
END_IF;
END_FUNCTION

FUNCTION LOWERCASE : STRING[STRING_LENGTH]
VAR_INPUT
    STR : STRING[STRING_LENGTH];
END_VAR
VAR
    Pos : INT;
    CurrentChar : STRING[1];
    ConvertedChar : STRING[1];
END_VAR

LOWERCASE := '';
FOR Pos := INT#1 TO LEN(STR) DO
    CurrentChar := MID(IN := STR, L := INT#1, P := Pos);
    CASE CurrentChar OF
        'A': ConvertedChar := 'a';
        'B': ConvertedChar := 'b';
        'C': ConvertedChar := 'c';
        'D': ConvertedChar := 'd';
        'E': ConvertedChar := 'e';
        'F': ConvertedChar := 'f';
        'G': ConvertedChar := 'g';
        'H': ConvertedChar := 'h';
        'I': ConvertedChar := 'i';
        'J': ConvertedChar := 'j';
        'K': ConvertedChar := 'k';
        'L': ConvertedChar := 'l';
        'M': ConvertedChar := 'm';
        'N': ConvertedChar := 'n';
        'O': ConvertedChar := 'o';
        'P': ConvertedChar := 'p';
        'Q': ConvertedChar := 'q';
        'R': ConvertedChar := 'r';
        'S': ConvertedChar := 's';
        'T': ConvertedChar := 't';
        'U': ConvertedChar := 'u';
        'V': ConvertedChar := 'v';
        'W': ConvertedChar := 'w';
        'X': ConvertedChar := 'x';
        'Y': ConvertedChar := 'y';
        'Z': ConvertedChar := 'z';
    ELSE
        ConvertedChar := CurrentChar;
    END_CASE;
    LOWERCASE := CONCAT(IN1 := LOWERCASE, IN2 := ConvertedChar);
END_FOR;
END_FUNCTION

FUNCTION UPPERCASE : STRING[STRING_LENGTH]
VAR_INPUT
    STR : STRING[STRING_LENGTH];
END_VAR
VAR
    Pos : INT;
    CurrentChar : STRING[1];
    ConvertedChar : STRING[1];
END_VAR

UPPERCASE := '';
FOR Pos := INT#1 TO LEN(STR) DO
    CurrentChar := MID(IN := STR, L := INT#1, P := Pos);
    CASE CurrentChar OF
        'a': ConvertedChar := 'A';
        'b': ConvertedChar := 'B';
        'c': ConvertedChar := 'C';
        'd': ConvertedChar := 'D';
        'e': ConvertedChar := 'E';
        'f': ConvertedChar := 'F';
        'g': ConvertedChar := 'G';
        'h': ConvertedChar := 'H';
        'i': ConvertedChar := 'I';
        'j': ConvertedChar := 'J';
        'k': ConvertedChar := 'K';
        'l': ConvertedChar := 'L';
        'm': ConvertedChar := 'M';
        'n': ConvertedChar := 'N';
        'o': ConvertedChar := 'O';
        'p': ConvertedChar := 'P';
        'q': ConvertedChar := 'Q';
        'r': ConvertedChar := 'R';
        's': ConvertedChar := 'S';
        't': ConvertedChar := 'T';
        'u': ConvertedChar := 'U';
        'v': ConvertedChar := 'V';
        'w': ConvertedChar := 'W';
        'x': ConvertedChar := 'X';
        'y': ConvertedChar := 'Y';
        'z': ConvertedChar := 'Z';
    ELSE
        ConvertedChar := CurrentChar;
    END_CASE;
    UPPERCASE := CONCAT(IN1 := UPPERCASE, IN2 := ConvertedChar);
END_FOR;
END_FUNCTION

FUNCTION ISC_ALPHA : BOOL
VAR_INPUT
    IN : BYTE;
END_VAR

ISC_ALPHA := (IN > BYTE#64 AND IN < BYTE#91) OR (IN > BYTE#96 AND IN < BYTE#123);
END_FUNCTION

FUNCTION ISC_CTRL : BOOL
VAR_INPUT
    IN : BYTE;
END_VAR

ISC_CTRL := IN < BYTE#32 OR IN = BYTE#127;
END_FUNCTION

FUNCTION ISC_HEX : BOOL
VAR_INPUT
    IN : BYTE;
END_VAR

ISC_HEX := (IN > BYTE#47 AND IN < BYTE#58)
    OR (IN > BYTE#64 AND IN < BYTE#71)
    OR (IN > BYTE#96 AND IN < BYTE#103);
END_FUNCTION

FUNCTION ISC_LOWER : BOOL
VAR_INPUT
    IN : BYTE;
END_VAR

ISC_LOWER := IN > BYTE#96 AND IN < BYTE#123;
END_FUNCTION

FUNCTION ISC_NUM : BOOL
VAR_INPUT
    IN : BYTE;
END_VAR

ISC_NUM := IN > BYTE#47 AND IN < BYTE#58;
END_FUNCTION

FUNCTION ISC_UPPER : BOOL
VAR_INPUT
    IN : BYTE;
END_VAR

ISC_UPPER := IN > BYTE#64 AND IN < BYTE#91;
END_FUNCTION

FUNCTION IS_ALNUM : BOOL
VAR_INPUT
    STR : STRING[STRING_LENGTH];
END_VAR
VAR
    Pos : INT;
    CurrentChar : STRING[1];
END_VAR

IS_ALNUM := LEN(STR) > INT#0;
FOR Pos := INT#1 TO LEN(STR) DO
    IF IS_ALNUM THEN
        CurrentChar := MID(IN := STR, L := INT#1, P := Pos);
        IF FIND(
            IN1 := 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789',
            IN2 := CurrentChar
        ) = INT#0 THEN
            IS_ALNUM := FALSE;
        END_IF;
    END_IF;
END_FOR;
END_FUNCTION

FUNCTION IS_ALPHA : BOOL
VAR_INPUT
    STR : STRING[STRING_LENGTH];
END_VAR
VAR
    Pos : INT;
    CurrentChar : STRING[1];
END_VAR

IS_ALPHA := LEN(STR) > INT#0;
FOR Pos := INT#1 TO LEN(STR) DO
    IF IS_ALPHA THEN
        CurrentChar := MID(IN := STR, L := INT#1, P := Pos);
        IF FIND(
            IN1 := 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz',
            IN2 := CurrentChar
        ) = INT#0 THEN
            IS_ALPHA := FALSE;
        END_IF;
    END_IF;
END_FOR;
END_FUNCTION

FUNCTION IS_CC : BOOL
VAR_INPUT
    STR : STRING[STRING_LENGTH];
    CMP : STRING[STRING_LENGTH];
END_VAR
VAR
    Pos : INT;
END_VAR

IS_CC := LEN(STR) > INT#0;
FOR Pos := INT#1 TO LEN(STR) DO
    IF IS_CC THEN
        IF FIND(IN1 := CMP, IN2 := MID(IN := STR, L := INT#1, P := Pos)) = INT#0 THEN
            IS_CC := FALSE;
        END_IF;
    END_IF;
END_FOR;
END_FUNCTION

FUNCTION IS_CTRL : BOOL
VAR_INPUT
    STR : STRING[STRING_LENGTH];
END_VAR
VAR
    Pos : INT;
    CurrentChar : CHAR;
    SpaceChar : CHAR;
    DeleteChar : CHAR;
END_VAR

SpaceChar := STRING_TO_CHAR(IN := CHR_TO_STRING(C := BYTE#32));
DeleteChar := STRING_TO_CHAR(IN := CHR_TO_STRING(C := BYTE#127));
IS_CTRL := LEN(STR) > INT#0;
FOR Pos := INT#1 TO LEN(STR) DO
    IF IS_CTRL THEN
        CurrentChar := STRING_TO_CHAR(IN := MID(IN := STR, L := INT#1, P := Pos));
        IF CurrentChar >= SpaceChar AND CurrentChar <> DeleteChar THEN
            IS_CTRL := FALSE;
        END_IF;
    END_IF;
END_FOR;
END_FUNCTION

FUNCTION IS_HEX : BOOL
VAR_INPUT
    STR : STRING[STRING_LENGTH];
END_VAR
VAR
    Pos : INT;
    CurrentChar : STRING[1];
END_VAR

IS_HEX := LEN(STR) > INT#0;
FOR Pos := INT#1 TO LEN(STR) DO
    IF IS_HEX THEN
        CurrentChar := MID(IN := STR, L := INT#1, P := Pos);
        IF FIND(IN1 := '0123456789ABCDEFabcdef', IN2 := CurrentChar) = INT#0 THEN
            IS_HEX := FALSE;
        END_IF;
    END_IF;
END_FOR;
END_FUNCTION

FUNCTION IS_LOWER : BOOL
VAR_INPUT
    STR : STRING[STRING_LENGTH];
END_VAR
VAR
    Pos : INT;
    CurrentChar : STRING[1];
END_VAR

IS_LOWER := LEN(STR) > INT#0;
FOR Pos := INT#1 TO LEN(STR) DO
    IF IS_LOWER THEN
        CurrentChar := MID(IN := STR, L := INT#1, P := Pos);
        IF FIND(IN1 := 'abcdefghijklmnopqrstuvwxyz', IN2 := CurrentChar) = INT#0 THEN
            IS_LOWER := FALSE;
        END_IF;
    END_IF;
END_FOR;
END_FUNCTION

FUNCTION IS_NCC : BOOL
VAR_INPUT
    STR : STRING[STRING_LENGTH];
    CMP : STRING[STRING_LENGTH];
END_VAR
VAR
    Pos : INT;
END_VAR

IS_NCC := TRUE;
FOR Pos := INT#1 TO LEN(STR) DO
    IF IS_NCC THEN
        IF FIND(IN1 := CMP, IN2 := MID(IN := STR, L := INT#1, P := Pos)) > INT#0 THEN
            IS_NCC := FALSE;
        END_IF;
    END_IF;
END_FOR;
END_FUNCTION

FUNCTION IS_NUM : BOOL
VAR_INPUT
    STR : STRING[STRING_LENGTH];
END_VAR
VAR
    Pos : INT;
    CurrentChar : STRING[1];
END_VAR

IS_NUM := LEN(STR) > INT#0;
FOR Pos := INT#1 TO LEN(STR) DO
    IF IS_NUM THEN
        CurrentChar := MID(IN := STR, L := INT#1, P := Pos);
        IF FIND(IN1 := '0123456789', IN2 := CurrentChar) = INT#0 THEN
            IS_NUM := FALSE;
        END_IF;
    END_IF;
END_FOR;
END_FUNCTION

FUNCTION IS_UPPER : BOOL
VAR_INPUT
    STR : STRING[STRING_LENGTH];
END_VAR
VAR
    Pos : INT;
    CurrentChar : STRING[1];
END_VAR

IS_UPPER := LEN(STR) > INT#0;
FOR Pos := INT#1 TO LEN(STR) DO
    IF IS_UPPER THEN
        CurrentChar := MID(IN := STR, L := INT#1, P := Pos);
        IF FIND(IN1 := 'ABCDEFGHIJKLMNOPQRSTUVWXYZ', IN2 := CurrentChar) = INT#0 THEN
            IS_UPPER := FALSE;
        END_IF;
    END_IF;
END_FOR;
END_FUNCTION

FUNCTION FIND_CHAR : INT
VAR_INPUT
    STR : STRING[STRING_LENGTH];
    POS : INT;
END_VAR
VAR
    I : INT;
    StartPos : INT;
    CurrentChar : STRING[1];
END_VAR

StartPos := POS;
IF StartPos < INT#1 THEN
    StartPos := INT#1;
END_IF;
FIND_CHAR := INT#0;
FOR I := StartPos TO LEN(STR) DO
    IF FIND_CHAR = INT#0 THEN
        CurrentChar := MID(IN := STR, L := INT#1, P := I);
        IF NOT IS_CTRL(STR := CurrentChar) THEN
            FIND_CHAR := I;
        END_IF;
    END_IF;
END_FOR;
END_FUNCTION

FUNCTION FIND_CTRL : INT
VAR_INPUT
    STR : STRING[STRING_LENGTH];
    POS : INT;
END_VAR
VAR
    I : INT;
    StartPos : INT;
    CurrentChar : STRING[1];
END_VAR

StartPos := POS;
IF StartPos < INT#1 THEN
    StartPos := INT#1;
END_IF;
FIND_CTRL := INT#0;
FOR I := StartPos TO LEN(STR) DO
    IF FIND_CTRL = INT#0 THEN
        CurrentChar := MID(IN := STR, L := INT#1, P := I);
        IF IS_CTRL(STR := CurrentChar) THEN
            FIND_CTRL := I;
        END_IF;
    END_IF;
END_FOR;
END_FUNCTION

FUNCTION FIND_NONUM : INT
VAR_INPUT
    STR : STRING[STRING_LENGTH];
    POS : INT;
END_VAR
VAR
    I : INT;
    StartPos : INT;
    CurrentChar : STRING[1];
END_VAR

StartPos := POS;
IF StartPos < INT#1 THEN
    StartPos := INT#1;
END_IF;
FIND_NONUM := INT#0;
FOR I := StartPos TO LEN(STR) DO
    IF FIND_NONUM = INT#0 THEN
        CurrentChar := MID(IN := STR, L := INT#1, P := I);
        IF FIND(IN1 := '0123456789.', IN2 := CurrentChar) = INT#0 THEN
            FIND_NONUM := I;
        END_IF;
    END_IF;
END_FOR;
END_FUNCTION

FUNCTION FIND_NUM : INT
VAR_INPUT
    STR : STRING[STRING_LENGTH];
    POS : INT;
END_VAR
VAR
    I : INT;
    StartPos : INT;
    CurrentChar : STRING[1];
END_VAR

StartPos := POS;
IF StartPos < INT#1 THEN
    StartPos := INT#1;
END_IF;
FIND_NUM := INT#0;
FOR I := StartPos TO LEN(STR) DO
    IF FIND_NUM = INT#0 THEN
        CurrentChar := MID(IN := STR, L := INT#1, P := I);
        IF FIND(IN1 := '0123456789.', IN2 := CurrentChar) > INT#0 THEN
            FIND_NUM := I;
        END_IF;
    END_IF;
END_FOR;
END_FUNCTION

FUNCTION FINDB : INT
VAR_INPUT
    STR1 : STRING[STRING_LENGTH];
    STR2 : STRING[STRING_LENGTH];
END_VAR
VAR
    Pos : INT;
    Length : INT;
END_VAR

Length := LEN(STR2);
FINDB := INT#0;
IF Length > INT#0 THEN
    FOR Pos := LEN(STR1) - Length + INT#1 TO INT#1 BY -INT#1 DO
        IF FINDB = INT#0 AND MID(IN := STR1, L := Length, P := Pos) = STR2 THEN
            FINDB := Pos;
        END_IF;
    END_FOR;
END_IF;
END_FUNCTION

FUNCTION FINDB_NONUM : INT
VAR_INPUT
    STR : STRING[STRING_LENGTH];
END_VAR
VAR
    Pos : INT;
    CurrentChar : STRING[1];
END_VAR

FINDB_NONUM := INT#0;
FOR Pos := LEN(STR) TO INT#1 BY -INT#1 DO
    IF FINDB_NONUM = INT#0 THEN
        CurrentChar := MID(IN := STR, L := INT#1, P := Pos);
        IF FIND(IN1 := '0123456789.', IN2 := CurrentChar) = INT#0 THEN
            FINDB_NONUM := Pos;
        END_IF;
    END_IF;
END_FOR;
END_FUNCTION

FUNCTION FINDB_NUM : INT
VAR_INPUT
    STR : STRING[STRING_LENGTH];
END_VAR
VAR
    Pos : INT;
    CurrentChar : STRING[1];
END_VAR

FINDB_NUM := INT#0;
FOR Pos := LEN(STR) TO INT#1 BY -INT#1 DO
    IF FINDB_NUM = INT#0 THEN
        CurrentChar := MID(IN := STR, L := INT#1, P := Pos);
        IF FIND(IN1 := '0123456789.', IN2 := CurrentChar) > INT#0 THEN
            FINDB_NUM := Pos;
        END_IF;
    END_IF;
END_FOR;
END_FUNCTION

FUNCTION FINDP : INT
VAR_INPUT
    STR : STRING[STRING_LENGTH];
    SRC : STRING[STRING_LENGTH];
    POS : INT;
END_VAR
VAR
    I : INT;
    StartPos : INT;
    StopPos : INT;
    MatchLength : INT;
END_VAR

MatchLength := LEN(SRC);
FINDP := INT#0;
IF MatchLength > INT#0 AND LEN(STR) >= MatchLength THEN
    StartPos := POS;
    IF StartPos < INT#1 THEN
        StartPos := INT#1;
    END_IF;
    StopPos := LEN(STR) - MatchLength + INT#1;
    FOR I := StartPos TO StopPos DO
        IF FINDP = INT#0 AND MID(IN := STR, L := MatchLength, P := I) = SRC THEN
            FINDP := I;
        END_IF;
    END_FOR;
END_IF;
END_FUNCTION

FUNCTION CAPITALIZE : STRING[STRING_LENGTH]
VAR_INPUT
    STR : STRING[STRING_LENGTH];
END_VAR
VAR
    Pos : INT;
    CurrentChar : STRING[1];
    First : BOOL := TRUE;
END_VAR

CAPITALIZE := '';
FOR Pos := INT#1 TO LEN(STR) DO
    CurrentChar := MID(IN := STR, L := INT#1, P := Pos);
    IF First THEN
        CAPITALIZE := CONCAT(IN1 := CAPITALIZE, IN2 := UPPERCASE(STR := CurrentChar));
    ELSE
        CAPITALIZE := CONCAT(IN1 := CAPITALIZE, IN2 := CurrentChar);
    END_IF;
    First := FIND(IN1 := ' ', IN2 := CurrentChar) > INT#0;
END_FOR;
END_FUNCTION

FUNCTION CLEAN : STRING[STRING_LENGTH]
VAR_INPUT
    IN : STRING[STRING_LENGTH];
    CX : STRING[80];
END_VAR
VAR
    Pos : INT;
    CurrentChar : STRING[1];
END_VAR

CLEAN := '';
FOR Pos := INT#1 TO LEN(IN) DO
    CurrentChar := MID(IN := IN, L := INT#1, P := Pos);
    IF FIND(IN1 := CX, IN2 := CurrentChar) > INT#0 THEN
        CLEAN := CONCAT(IN1 := CLEAN, IN2 := CurrentChar);
    END_IF;
END_FOR;
END_FUNCTION

FUNCTION COUNT_CHAR : INT
VAR_INPUT
    STR : STRING[STRING_LENGTH];
    CHR : BYTE;
END_VAR
VAR
    Pos : INT;
    SearchChar : STRING[1];
END_VAR

SearchChar := CHR_TO_STRING(C := CHR);
COUNT_CHAR := INT#0;
FOR Pos := INT#1 TO LEN(STR) DO
    IF FIND(IN1 := SearchChar, IN2 := MID(IN := STR, L := INT#1, P := Pos)) > INT#0 THEN
        COUNT_CHAR := COUNT_CHAR + INT#1;
    END_IF;
END_FOR;
END_FUNCTION

FUNCTION COUNT_SUBSTRING : INT
VAR_INPUT
    SEARCH : STRING[STRING_LENGTH];
    STR : STRING[STRING_LENGTH];
END_VAR
VAR
    Pos : INT;
    Size : INT;
    Remaining : STRING[STRING_LENGTH];
END_VAR

COUNT_SUBSTRING := INT#0;
Size := LEN(SEARCH);
Remaining := STR;
IF Size > INT#0 THEN
    REPEAT
        Pos := FIND(IN1 := Remaining, IN2 := SEARCH);
        IF Pos > INT#0 THEN
            Remaining := REPLACE(IN1 := Remaining, IN2 := '', L := Size, P := Pos);
            COUNT_SUBSTRING := COUNT_SUBSTRING + INT#1;
        END_IF;
    UNTIL Pos = INT#0
    END_REPEAT;
END_IF;
END_FUNCTION

FUNCTION CODE : BYTE
VAR_INPUT
    STR : STRING[STRING_LENGTH];
    POS : INT;
END_VAR
VAR
    CurrentChar : CHAR;
END_VAR

CODE := BYTE#0;
IF POS >= INT#1 AND POS <= LEN(STR) THEN
    CurrentChar := STRING_TO_CHAR(IN := MID(IN := STR, L := INT#1, P := POS));
    CODE := CHAR_TO_BYTE(IN := CurrentChar);
END_IF;
END_FUNCTION

FUNCTION CHARCODE : BYTE
VAR_INPUT
    STR : STRING[10];
END_VAR

IF LEN(STR) = INT#1 THEN
    CHARCODE := CODE(STR := STR, POS := INT#1);
ELSE
    CASE STR OF
        'quot': CHARCODE := BYTE#34;
        'amp': CHARCODE := BYTE#38;
        'lt': CHARCODE := BYTE#60;
        'gt': CHARCODE := BYTE#62;
        'euro': CHARCODE := BYTE#128;
        'nbsp': CHARCODE := BYTE#160;
        'iexcl': CHARCODE := BYTE#161;
        'cent': CHARCODE := BYTE#162;
        'pound': CHARCODE := BYTE#163;
        'curren': CHARCODE := BYTE#164;
        'yen': CHARCODE := BYTE#165;
        'brvbar': CHARCODE := BYTE#166;
        'sect': CHARCODE := BYTE#167;
        'uml': CHARCODE := BYTE#168;
        'copy': CHARCODE := BYTE#169;
        'ordf': CHARCODE := BYTE#170;
        'laquo': CHARCODE := BYTE#171;
        'not': CHARCODE := BYTE#172;
        'shy': CHARCODE := BYTE#173;
        'reg': CHARCODE := BYTE#174;
        'macr': CHARCODE := BYTE#175;
        'deg': CHARCODE := BYTE#176;
        'plusmn': CHARCODE := BYTE#177;
        'sup2': CHARCODE := BYTE#178;
        'sup3': CHARCODE := BYTE#179;
        'acute': CHARCODE := BYTE#180;
        'micro': CHARCODE := BYTE#181;
        'para': CHARCODE := BYTE#182;
        'middot': CHARCODE := BYTE#183;
        'cedil': CHARCODE := BYTE#184;
        'sup1': CHARCODE := BYTE#185;
        'ordm': CHARCODE := BYTE#186;
        'raquo': CHARCODE := BYTE#187;
        'frac14': CHARCODE := BYTE#188;
        'frac12': CHARCODE := BYTE#189;
        'frac34': CHARCODE := BYTE#190;
        'iquest': CHARCODE := BYTE#191;
        'Agrave': CHARCODE := BYTE#192;
        'Aacute': CHARCODE := BYTE#193;
        'Acirc': CHARCODE := BYTE#194;
        'Atilde': CHARCODE := BYTE#195;
        'Auml': CHARCODE := BYTE#196;
        'Aring': CHARCODE := BYTE#197;
        'AElig': CHARCODE := BYTE#198;
        'Ccedil': CHARCODE := BYTE#199;
        'Egrave': CHARCODE := BYTE#200;
        'Eacute': CHARCODE := BYTE#201;
        'Ecirc': CHARCODE := BYTE#202;
        'Euml': CHARCODE := BYTE#203;
        'Igrave': CHARCODE := BYTE#204;
        'Iacute': CHARCODE := BYTE#205;
        'Icirc': CHARCODE := BYTE#206;
        'Iuml': CHARCODE := BYTE#207;
        'ETH': CHARCODE := BYTE#208;
        'Ntilde': CHARCODE := BYTE#209;
        'Ograve': CHARCODE := BYTE#210;
        'Oacute': CHARCODE := BYTE#211;
        'Ocirc': CHARCODE := BYTE#212;
        'Otilde': CHARCODE := BYTE#213;
        'Ouml': CHARCODE := BYTE#214;
        'times': CHARCODE := BYTE#215;
        'Oslash': CHARCODE := BYTE#216;
        'Ugrave': CHARCODE := BYTE#217;
        'Uacute': CHARCODE := BYTE#218;
        'Ucirc': CHARCODE := BYTE#219;
        'Uuml': CHARCODE := BYTE#220;
        'Yacute': CHARCODE := BYTE#221;
        'THORN': CHARCODE := BYTE#222;
        'szlig': CHARCODE := BYTE#223;
        'agrave': CHARCODE := BYTE#224;
        'aacute': CHARCODE := BYTE#225;
        'acirc': CHARCODE := BYTE#226;
        'atilde': CHARCODE := BYTE#227;
        'auml': CHARCODE := BYTE#228;
        'aring': CHARCODE := BYTE#229;
        'aelig': CHARCODE := BYTE#230;
        'ccedil': CHARCODE := BYTE#231;
        'egrave': CHARCODE := BYTE#232;
        'eacute': CHARCODE := BYTE#233;
        'ecirc': CHARCODE := BYTE#234;
        'euml': CHARCODE := BYTE#235;
        'igrave': CHARCODE := BYTE#236;
        'iacute': CHARCODE := BYTE#237;
        'icirc': CHARCODE := BYTE#238;
        'iuml': CHARCODE := BYTE#239;
        'eth': CHARCODE := BYTE#240;
        'ntilde': CHARCODE := BYTE#241;
        'ograve': CHARCODE := BYTE#242;
        'oacute': CHARCODE := BYTE#243;
        'ocirc': CHARCODE := BYTE#244;
        'otilde': CHARCODE := BYTE#245;
        'ouml': CHARCODE := BYTE#246;
        'divide': CHARCODE := BYTE#247;
        'oslash': CHARCODE := BYTE#248;
        'ugrave': CHARCODE := BYTE#249;
        'uacute': CHARCODE := BYTE#250;
        'ucirc': CHARCODE := BYTE#251;
        'uuml': CHARCODE := BYTE#252;
        'yacute': CHARCODE := BYTE#253;
        'thorn': CHARCODE := BYTE#254;
        'yuml': CHARCODE := BYTE#255;
    ELSE
        CHARCODE := BYTE#0;
    END_CASE;
END_IF;
END_FUNCTION

FUNCTION CHARNAME : STRING[10]
VAR_INPUT
    C : BYTE;
END_VAR

IF C = BYTE#0 THEN
    CHARNAME := '';
ELSE
    CASE C OF
        BYTE#34: CHARNAME := 'quot';
        BYTE#38: CHARNAME := 'amp';
        BYTE#60: CHARNAME := 'lt';
        BYTE#62: CHARNAME := 'gt';
        BYTE#128: CHARNAME := 'euro';
        BYTE#160: CHARNAME := 'nbsp';
        BYTE#161: CHARNAME := 'iexcl';
        BYTE#162: CHARNAME := 'cent';
        BYTE#163: CHARNAME := 'pound';
        BYTE#164: CHARNAME := 'curren';
        BYTE#165: CHARNAME := 'yen';
        BYTE#166: CHARNAME := 'brvbar';
        BYTE#167: CHARNAME := 'sect';
        BYTE#168: CHARNAME := 'uml';
        BYTE#169: CHARNAME := 'copy';
        BYTE#170: CHARNAME := 'ordf';
        BYTE#171: CHARNAME := 'laquo';
        BYTE#172: CHARNAME := 'not';
        BYTE#173: CHARNAME := 'shy';
        BYTE#174: CHARNAME := 'reg';
        BYTE#175: CHARNAME := 'macr';
        BYTE#176: CHARNAME := 'deg';
        BYTE#177: CHARNAME := 'plusmn';
        BYTE#178: CHARNAME := 'sup2';
        BYTE#179: CHARNAME := 'sup3';
        BYTE#180: CHARNAME := 'acute';
        BYTE#181: CHARNAME := 'micro';
        BYTE#182: CHARNAME := 'para';
        BYTE#183: CHARNAME := 'middot';
        BYTE#184: CHARNAME := 'cedil';
        BYTE#185: CHARNAME := 'sup1';
        BYTE#186: CHARNAME := 'ordm';
        BYTE#187: CHARNAME := 'raquo';
        BYTE#188: CHARNAME := 'frac14';
        BYTE#189: CHARNAME := 'frac12';
        BYTE#190: CHARNAME := 'frac34';
        BYTE#191: CHARNAME := 'iquest';
        BYTE#192: CHARNAME := 'Agrave';
        BYTE#193: CHARNAME := 'Aacute';
        BYTE#194: CHARNAME := 'Acirc';
        BYTE#195: CHARNAME := 'Atilde';
        BYTE#196: CHARNAME := 'Auml';
        BYTE#197: CHARNAME := 'Aring';
        BYTE#198: CHARNAME := 'AElig';
        BYTE#199: CHARNAME := 'Ccedil';
        BYTE#200: CHARNAME := 'Egrave';
        BYTE#201: CHARNAME := 'Eacute';
        BYTE#202: CHARNAME := 'Ecirc';
        BYTE#203: CHARNAME := 'Euml';
        BYTE#204: CHARNAME := 'Igrave';
        BYTE#205: CHARNAME := 'Iacute';
        BYTE#206: CHARNAME := 'Icirc';
        BYTE#207: CHARNAME := 'Iuml';
        BYTE#208: CHARNAME := 'ETH';
        BYTE#209: CHARNAME := 'Ntilde';
        BYTE#210: CHARNAME := 'Ograve';
        BYTE#211: CHARNAME := 'Oacute';
        BYTE#212: CHARNAME := 'Ocirc';
        BYTE#213: CHARNAME := 'Otilde';
        BYTE#214: CHARNAME := 'Ouml';
        BYTE#215: CHARNAME := 'times';
        BYTE#216: CHARNAME := 'Oslash';
        BYTE#217: CHARNAME := 'Ugrave';
        BYTE#218: CHARNAME := 'Uacute';
        BYTE#219: CHARNAME := 'Ucirc';
        BYTE#220: CHARNAME := 'Uuml';
        BYTE#221: CHARNAME := 'Yacute';
        BYTE#222: CHARNAME := 'THORN';
        BYTE#223: CHARNAME := 'szlig';
        BYTE#224: CHARNAME := 'agrave';
        BYTE#225: CHARNAME := 'aacute';
        BYTE#226: CHARNAME := 'acirc';
        BYTE#227: CHARNAME := 'atilde';
        BYTE#228: CHARNAME := 'auml';
        BYTE#229: CHARNAME := 'aring';
        BYTE#230: CHARNAME := 'aelig';
        BYTE#231: CHARNAME := 'ccedil';
        BYTE#232: CHARNAME := 'egrave';
        BYTE#233: CHARNAME := 'eacute';
        BYTE#234: CHARNAME := 'ecirc';
        BYTE#235: CHARNAME := 'euml';
        BYTE#236: CHARNAME := 'igrave';
        BYTE#237: CHARNAME := 'iacute';
        BYTE#238: CHARNAME := 'icirc';
        BYTE#239: CHARNAME := 'iuml';
        BYTE#240: CHARNAME := 'eth';
        BYTE#241: CHARNAME := 'ntilde';
        BYTE#242: CHARNAME := 'ograve';
        BYTE#243: CHARNAME := 'oacute';
        BYTE#244: CHARNAME := 'ocirc';
        BYTE#245: CHARNAME := 'otilde';
        BYTE#246: CHARNAME := 'ouml';
        BYTE#247: CHARNAME := 'divide';
        BYTE#248: CHARNAME := 'oslash';
        BYTE#249: CHARNAME := 'ugrave';
        BYTE#250: CHARNAME := 'uacute';
        BYTE#251: CHARNAME := 'ucirc';
        BYTE#252: CHARNAME := 'uuml';
        BYTE#253: CHARNAME := 'yacute';
        BYTE#254: CHARNAME := 'thorn';
        BYTE#255: CHARNAME := 'yuml';
    ELSE
        CHARNAME := CHR_TO_STRING(C := C);
    END_CASE;
END_IF;
END_FUNCTION

FUNCTION DEL_CHARS : STRING[STRING_LENGTH]
VAR_INPUT
    IN : STRING[STRING_LENGTH];
    CX : STRING[80];
END_VAR
VAR
    Pos : INT;
    CurrentChar : STRING[1];
END_VAR

DEL_CHARS := '';
FOR Pos := INT#1 TO LEN(IN) DO
    CurrentChar := MID(IN := IN, L := INT#1, P := Pos);
    IF FIND(IN1 := CX, IN2 := CurrentChar) = INT#0 THEN
        DEL_CHARS := CONCAT(IN1 := DEL_CHARS, IN2 := CurrentChar);
    END_IF;
END_FOR;
END_FUNCTION

FUNCTION TO_UML : STRING[2]
VAR_INPUT
    IN : BYTE;
END_VAR

CASE IN OF
    BYTE#196:
        TO_UML := 'Ae';
    BYTE#214:
        TO_UML := 'Oe';
    BYTE#220:
        TO_UML := 'Ue';
    BYTE#223:
        TO_UML := 'ss';
    BYTE#228:
        TO_UML := 'ae';
    BYTE#246:
        TO_UML := 'oe';
    BYTE#252:
        TO_UML := 'ue';
ELSE
    TO_UML := CHR_TO_STRING(C := IN);
END_CASE;
END_FUNCTION

FUNCTION DEC_TO_BYTE : BYTE
VAR_INPUT
    DEC : STRING[10];
END_VAR
VAR
    Pos : INT;
    DigitPos : INT;
    Value : UINT;
END_VAR

Value := UINT#0;
FOR Pos := INT#1 TO LEN(DEC) DO
    DigitPos := FIND(IN1 := '0123456789', IN2 := MID(IN := DEC, L := INT#1, P := Pos));
    IF DigitPos > INT#0 THEN
        Value := Value * UINT#10 + INT_TO_UINT(DigitPos - INT#1);
    END_IF;
END_FOR;
DEC_TO_BYTE := UINT_TO_BYTE(Value);
END_FUNCTION

FUNCTION DEC_TO_DWORD : DWORD
VAR_INPUT
    DEC : STRING[20];
END_VAR
VAR
    Pos : INT;
    DigitPos : INT;
    Value : UDINT;
END_VAR

Value := UDINT#0;
FOR Pos := INT#1 TO LEN(DEC) DO
    DigitPos := FIND(IN1 := '0123456789', IN2 := MID(IN := DEC, L := INT#1, P := Pos));
    IF DigitPos > INT#0 THEN
        Value := Value * UDINT#10 + INT_TO_UDINT(DigitPos - INT#1);
    END_IF;
END_FOR;
DEC_TO_DWORD := UDINT_TO_DWORD(Value);
END_FUNCTION

FUNCTION DEC_TO_INT : INT
VAR_INPUT
    DEC : STRING[10];
END_VAR
VAR
    Pos : INT;
    CurrentChar : STRING[1];
    Digits : STRING[10];
    Sign : BOOL;
END_VAR

Digits := '';
Sign := FALSE;
FOR Pos := INT#1 TO LEN(DEC) DO
    CurrentChar := MID(IN := DEC, L := INT#1, P := Pos);
    IF FIND(IN1 := '0123456789', IN2 := CurrentChar) > INT#0 THEN
        Digits := CONCAT(IN1 := Digits, IN2 := CurrentChar);
    ELSIF LEN(Digits) = INT#0 AND FIND(IN1 := '-', IN2 := CurrentChar) > INT#0 THEN
        Sign := TRUE;
    END_IF;
END_FOR;

IF LEN(Digits) > INT#0 THEN
    DEC_TO_INT := STRING_TO_INT(Digits);
    IF Sign THEN
        DEC_TO_INT := -DEC_TO_INT;
    END_IF;
ELSE
    DEC_TO_INT := INT#0;
END_IF;
END_FUNCTION

FUNCTION BYTE_TO_STRB : STRING[8]
VAR_INPUT
    IN : BYTE;
END_VAR
VAR
    Bit : INT;
END_VAR

BYTE_TO_STRB := '';
FOR Bit := INT#7 TO INT#0 BY -INT#1 DO
    IF (SHR(IN, Bit) AND BYTE#1) = BYTE#1 THEN
        BYTE_TO_STRB := CONCAT(IN1 := BYTE_TO_STRB, IN2 := '1');
    ELSE
        BYTE_TO_STRB := CONCAT(IN1 := BYTE_TO_STRB, IN2 := '0');
    END_IF;
END_FOR;
END_FUNCTION

FUNCTION BYTE_TO_STRH : STRING[2]
VAR_INPUT
    IN : BYTE;
END_VAR
VAR
    HexDigits : STRING[16];
    HighNibble : BYTE;
    LowNibble : BYTE;
END_VAR

HexDigits := '0123456789ABCDEF';
HighNibble := SHR(IN, INT#4);
LowNibble := IN AND BYTE#16#0F;
BYTE_TO_STRH := CONCAT(
    IN1 := MID(
        IN := HexDigits,
        L := INT#1,
        P := DINT_TO_INT(BYTE_TO_DINT(HighNibble) + DINT#1)
    ),
    IN2 := MID(
        IN := HexDigits,
        L := INT#1,
        P := DINT_TO_INT(BYTE_TO_DINT(LowNibble) + DINT#1)
    )
);
END_FUNCTION

FUNCTION DWORD_TO_STRB : STRING[32]
VAR_INPUT
    IN : DWORD;
END_VAR

DWORD_TO_STRB := BYTE_TO_STRB(IN := DWORD_TO_BYTE(SHR(IN, INT#24)));
DWORD_TO_STRB := CONCAT(IN1 := DWORD_TO_STRB, IN2 := BYTE_TO_STRB(IN := DWORD_TO_BYTE(SHR(IN, INT#16))));
DWORD_TO_STRB := CONCAT(IN1 := DWORD_TO_STRB, IN2 := BYTE_TO_STRB(IN := DWORD_TO_BYTE(SHR(IN, INT#8))));
DWORD_TO_STRB := CONCAT(IN1 := DWORD_TO_STRB, IN2 := BYTE_TO_STRB(IN := DWORD_TO_BYTE(IN)));
END_FUNCTION

FUNCTION DWORD_TO_STRH : STRING[8]
VAR_INPUT
    IN : DWORD;
END_VAR

DWORD_TO_STRH := BYTE_TO_STRH(IN := DWORD_TO_BYTE(SHR(IN, INT#24)));
DWORD_TO_STRH := CONCAT(IN1 := DWORD_TO_STRH, IN2 := BYTE_TO_STRH(IN := DWORD_TO_BYTE(SHR(IN, INT#16))));
DWORD_TO_STRH := CONCAT(IN1 := DWORD_TO_STRH, IN2 := BYTE_TO_STRH(IN := DWORD_TO_BYTE(SHR(IN, INT#8))));
DWORD_TO_STRH := CONCAT(IN1 := DWORD_TO_STRH, IN2 := BYTE_TO_STRH(IN := DWORD_TO_BYTE(IN)));
END_FUNCTION

FUNCTION BIN_TO_BYTE : BYTE
VAR_INPUT
    BIN : STRING[12];
END_VAR
VAR
    Pos : INT;
    CurrentChar : STRING[1];
END_VAR

BIN_TO_BYTE := BYTE#0;
FOR Pos := INT#1 TO LEN(BIN) DO
    CurrentChar := MID(IN := BIN, L := INT#1, P := Pos);
    IF CurrentChar = '0' THEN
        BIN_TO_BYTE := SHL(BIN_TO_BYTE, INT#1);
    ELSIF CurrentChar = '1' THEN
        BIN_TO_BYTE := SHL(BIN_TO_BYTE, INT#1) OR BYTE#1;
    END_IF;
END_FOR;
END_FUNCTION

FUNCTION BIN_TO_DWORD : DWORD
VAR_INPUT
    BIN : STRING[40];
END_VAR
VAR
    Pos : INT;
    CurrentChar : STRING[1];
END_VAR

BIN_TO_DWORD := DWORD#0;
FOR Pos := INT#1 TO LEN(BIN) DO
    CurrentChar := MID(IN := BIN, L := INT#1, P := Pos);
    IF CurrentChar = '0' THEN
        BIN_TO_DWORD := SHL(BIN_TO_DWORD, INT#1);
    ELSIF CurrentChar = '1' THEN
        BIN_TO_DWORD := SHL(BIN_TO_DWORD, INT#1) OR DWORD#1;
    END_IF;
END_FOR;
END_FUNCTION

FUNCTION HEX_TO_BYTE : BYTE
VAR_INPUT
    HEX : STRING[5];
END_VAR
VAR
    Pos : INT;
    CurrentChar : STRING[1];
    DigitPos : INT;
END_VAR

HEX_TO_BYTE := BYTE#0;
FOR Pos := INT#1 TO LEN(HEX) DO
    CurrentChar := UPPERCASE(STR := MID(IN := HEX, L := INT#1, P := Pos));
    DigitPos := FIND(IN1 := '0123456789ABCDEF', IN2 := CurrentChar);
    IF DigitPos > INT#0 THEN
        HEX_TO_BYTE := SHL(HEX_TO_BYTE, INT#4) OR INT_TO_BYTE(IN := DigitPos - INT#1);
    END_IF;
END_FOR;
END_FUNCTION

FUNCTION HEX_TO_DWORD : DWORD
VAR_INPUT
    HEX : STRING[20];
END_VAR
VAR
    Pos : INT;
    CurrentChar : STRING[1];
    DigitPos : INT;
END_VAR

HEX_TO_DWORD := DWORD#0;
FOR Pos := INT#1 TO LEN(HEX) DO
    CurrentChar := UPPERCASE(STR := MID(IN := HEX, L := INT#1, P := Pos));
    DigitPos := FIND(IN1 := '0123456789ABCDEF', IN2 := CurrentChar);
    IF DigitPos > INT#0 THEN
        HEX_TO_DWORD := SHL(HEX_TO_DWORD, INT#4) OR UDINT_TO_DWORD(INT_TO_UDINT(DigitPos - INT#1));
    END_IF;
END_FOR;
END_FUNCTION

FUNCTION OCT_TO_BYTE : BYTE
VAR_INPUT
    OCT : STRING[10];
END_VAR
VAR
    Pos : INT;
    CurrentChar : STRING[1];
    DigitPos : INT;
END_VAR

OCT_TO_BYTE := BYTE#0;
FOR Pos := INT#1 TO LEN(OCT) DO
    CurrentChar := MID(IN := OCT, L := INT#1, P := Pos);
    DigitPos := FIND(IN1 := '01234567', IN2 := CurrentChar);
    IF DigitPos > INT#0 THEN
        OCT_TO_BYTE := SHL(OCT_TO_BYTE, INT#3) OR INT_TO_BYTE(IN := DigitPos - INT#1);
    END_IF;
END_FOR;
END_FUNCTION

FUNCTION OCT_TO_DWORD : DWORD
VAR_INPUT
    OCT : STRING[20];
END_VAR
VAR
    Pos : INT;
    CurrentChar : STRING[1];
    DigitPos : INT;
END_VAR

OCT_TO_DWORD := DWORD#0;
FOR Pos := INT#1 TO LEN(OCT) DO
    CurrentChar := MID(IN := OCT, L := INT#1, P := Pos);
    DigitPos := FIND(IN1 := '01234567', IN2 := CurrentChar);
    IF DigitPos > INT#0 THEN
        OCT_TO_DWORD := SHL(OCT_TO_DWORD, INT#3) OR UDINT_TO_DWORD(INT_TO_UDINT(DigitPos - INT#1));
    END_IF;
END_FOR;
END_FUNCTION

FUNCTION MIRROR : STRING[STRING_LENGTH]
VAR_INPUT
    STR : STRING[STRING_LENGTH];
END_VAR
VAR
    Pos : INT;
END_VAR

MIRROR := '';
FOR Pos := LEN(STR) TO INT#1 BY -INT#1 DO
    MIRROR := CONCAT(IN1 := MIRROR, IN2 := MID(IN := STR, L := INT#1, P := Pos));
END_FOR;
END_FUNCTION

FUNCTION REPLACE_ALL : STRING[STRING_LENGTH]
VAR_INPUT
    STR : STRING[STRING_LENGTH];
    SRC : STRING[STRING_LENGTH];
    REP : STRING[STRING_LENGTH];
END_VAR
VAR
    Pos : INT;
    SourceLength : INT;
    ReplacementLength : INT;
END_VAR

REPLACE_ALL := STR;
SourceLength := LEN(SRC);
ReplacementLength := LEN(REP);
IF SourceLength > INT#0 THEN
    Pos := FINDP(STR := REPLACE_ALL, SRC := SRC, POS := INT#1);
    WHILE Pos > INT#0 DO
        REPLACE_ALL := REPLACE(IN1 := REPLACE_ALL, IN2 := REP, L := SourceLength, P := Pos);
        Pos := FINDP(STR := REPLACE_ALL, SRC := SRC, POS := Pos + ReplacementLength);
    END_WHILE;
END_IF;
END_FUNCTION

FUNCTION REPLACE_CHARS : STRING[STRING_LENGTH]
VAR_INPUT
    STR : STRING[STRING_LENGTH];
    SRC : STRING[STRING_LENGTH];
    REP : STRING[STRING_LENGTH];
END_VAR
VAR
    Pos : INT;
    MatchPos : INT;
    MapLength : INT;
    SrcMap : STRING[STRING_LENGTH];
    RepMap : STRING[STRING_LENGTH];
    CurrentChar : STRING[1];
END_VAR

IF LEN(SRC) < LEN(REP) THEN
    MapLength := LEN(SRC);
ELSE
    MapLength := LEN(REP);
END_IF;

SrcMap := LEFT(IN := SRC, L := MapLength);
RepMap := LEFT(IN := REP, L := MapLength);
REPLACE_CHARS := STR;
FOR Pos := INT#1 TO LEN(STR) DO
    CurrentChar := MID(IN := REPLACE_CHARS, L := INT#1, P := Pos);
    MatchPos := FIND(IN1 := SrcMap, IN2 := CurrentChar);
    IF MatchPos > INT#0 THEN
        REPLACE_CHARS := REPLACE(
            IN1 := REPLACE_CHARS,
            IN2 := MID(IN := RepMap, L := INT#1, P := MatchPos),
            L := INT#1,
            P := Pos
        );
    END_IF;
END_FOR;
END_FUNCTION

FUNCTION REPLACE_UML : STRING[STRING_LENGTH]
VAR_INPUT
    STR : STRING[STRING_LENGTH];
END_VAR
VAR
    Pos : INT;
END_VAR

REPLACE_UML := '';
FOR Pos := INT#1 TO LEN(STR) DO
    REPLACE_UML := CONCAT(IN1 := REPLACE_UML, IN2 := TO_UML(IN := CODE(STR := STR, POS := Pos)));
END_FOR;
END_FUNCTION

FUNCTION FLOAT_TO_REAL : REAL
VAR_INPUT
    FLT : STRING[20];
END_VAR
VAR
    Pos : INT;
    CurrentChar : STRING[1];
    DigitPos : INT;
    Mantissa : DINT;
    FractionExponent : INT;
    ExponentValue : INT;
    Sign : DINT := DINT#1;
    ExponentSign : INT := INT#1;
    MantissaStarted : BOOL;
    DecimalSeen : BOOL;
    ExponentSeen : BOOL;
END_VAR

FLOAT_TO_REAL := REAL#0.0;
FOR Pos := INT#1 TO LEN(FLT) DO
    CurrentChar := MID(IN := FLT, L := INT#1, P := Pos);
    DigitPos := FIND(IN1 := '0123456789', IN2 := CurrentChar);

    IF NOT MantissaStarted AND NOT ExponentSeen AND CurrentChar = '-' THEN
        Sign := -DINT#1;
    ELSIF DigitPos > INT#0 THEN
        IF ExponentSeen THEN
            ExponentValue := ExponentValue * INT#10 + DigitPos - INT#1;
        ELSE
            MantissaStarted := TRUE;
            Mantissa := Mantissa * DINT#10 + INT_TO_DINT(DigitPos - INT#1);
            IF DecimalSeen THEN
                FractionExponent := FractionExponent - INT#1;
            END_IF;
        END_IF;
    ELSIF NOT ExponentSeen AND (CurrentChar = '.' OR CurrentChar = ',') THEN
        DecimalSeen := TRUE;
        MantissaStarted := TRUE;
    ELSIF MantissaStarted AND (CurrentChar = 'E' OR CurrentChar = 'e') THEN
        ExponentSeen := TRUE;
    ELSIF ExponentSeen AND ExponentValue = INT#0 AND CurrentChar = '-' THEN
        ExponentSign := -INT#1;
    END_IF;
END_FOR;

FLOAT_TO_REAL := DINT_TO_REAL(Mantissa * Sign) * EXP10(X := INT_TO_REAL(FractionExponent + ExponentSign * ExponentValue));
END_FUNCTION

FUNCTION FSTRING_TO_BYTE : BYTE
VAR_INPUT
    IN : STRING[12];
END_VAR

IF LEFT(IN := IN, L := INT#2) = '2#' THEN
    FSTRING_TO_BYTE := BIN_TO_BYTE(BIN := RIGHT(IN := IN, L := LEN(IN) - INT#2));
ELSIF LEFT(IN := IN, L := INT#2) = '8#' THEN
    FSTRING_TO_BYTE := OCT_TO_BYTE(OCT := RIGHT(IN := IN, L := LEN(IN) - INT#2));
ELSIF LEFT(IN := IN, L := INT#3) = '16#' THEN
    FSTRING_TO_BYTE := HEX_TO_BYTE(HEX := RIGHT(IN := IN, L := LEN(IN) - INT#3));
ELSE
    FSTRING_TO_BYTE := DEC_TO_BYTE(DEC := CLEAN(IN := IN, CX := '0123456789'));
END_IF;
END_FUNCTION

FUNCTION FSTRING_TO_DWORD : DWORD
VAR_INPUT
    IN : STRING[40];
END_VAR

IF LEFT(IN := IN, L := INT#2) = '2#' THEN
    FSTRING_TO_DWORD := BIN_TO_DWORD(BIN := RIGHT(IN := IN, L := LEN(IN) - INT#2));
ELSIF LEFT(IN := IN, L := INT#2) = '8#' THEN
    FSTRING_TO_DWORD := OCT_TO_DWORD(OCT := RIGHT(IN := IN, L := LEN(IN) - INT#2));
ELSIF LEFT(IN := IN, L := INT#3) = '16#' THEN
    FSTRING_TO_DWORD := HEX_TO_DWORD(HEX := RIGHT(IN := IN, L := LEN(IN) - INT#3));
ELSE
    FSTRING_TO_DWORD := DEC_TO_DWORD(DEC := CLEAN(IN := IN, CX := '0123456789'));
END_IF;
END_FUNCTION

FUNCTION FSTRING_TO_MONTH : INT
VAR_INPUT
    MTH : STRING[20];
    LANG : INT;
END_VAR
VAR
    Ly : INT;
    Normalized : STRING[20];
    Resolved : BOOL;
    I : INT;
END_VAR

IF LANG < INT#1 THEN
    Ly := LANGUAGE.DEFAULT;
ELSIF LANG > LANGUAGE.LMAX THEN
    Ly := LANGUAGE.LMAX;
ELSE
    Ly := LANG;
END_IF;

Normalized := TRIM(STR := MTH);
Normalized := LOWERCASE(STR := Normalized);
Normalized := CAPITALIZE(STR := Normalized);

FOR I := INT#1 TO INT#12 DO
    IF Normalized = LANGUAGE.MONTHS[Ly, I] THEN
        FSTRING_TO_MONTH := I;
        Resolved := TRUE;
    END_IF;
    IF Normalized = LANGUAGE.MONTHS3[Ly, I] THEN
        FSTRING_TO_MONTH := I;
        Resolved := TRUE;
    END_IF;
END_FOR;

IF NOT Resolved THEN
    FSTRING_TO_MONTH := STRING_TO_INT(Normalized);
END_IF;
END_FUNCTION

FUNCTION FSTRING_TO_WEEKDAY : INT
VAR_INPUT
    WDAY : STRING[20];
    LANG : INT;
END_VAR
VAR
    Ly : INT;
    Normalized : STRING[20];
    I : INT;
    Resolved : BOOL;
END_VAR

IF LANG < INT#1 THEN
    Ly := LANGUAGE.DEFAULT;
ELSIF LANG > LANGUAGE.LMAX THEN
    Ly := LANGUAGE.LMAX;
ELSE
    Ly := LANG;
END_IF;

Normalized := TRIM(STR := WDAY);
Normalized := LOWERCASE(STR := Normalized);
Normalized := CAPITALIZE(STR := Normalized);

FOR I := INT#1 TO INT#7 DO
    IF LANGUAGE.WEEKDAYS2[Ly, I] = Normalized THEN
        FSTRING_TO_WEEKDAY := I;
        Resolved := TRUE;
    END_IF;
END_FOR;

IF NOT Resolved THEN
    FSTRING_TO_WEEKDAY := STRING_TO_INT(WDAY);
END_IF;
END_FUNCTION

FUNCTION FSTRING_TO_WEEK : BYTE
VAR_INPUT
    WEEK : STRING[60];
    LANG : INT;
END_VAR
VAR
    Remaining : STRING[60];
    Token : STRING[20];
    Pos : INT;
    DayIndex : INT;
END_VAR

FSTRING_TO_WEEK := BYTE#0;
Remaining := WEEK;
WHILE LEN(Remaining) > INT#0 DO
    Pos := FIND(IN1 := Remaining, IN2 := ',');
    IF Pos > INT#0 THEN
        Token := LEFT(IN := Remaining, L := Pos - INT#1);
        Remaining := DELETE(IN := Remaining, L := Pos, P := INT#1);
    ELSE
        Token := Remaining;
        Remaining := '';
    END_IF;

    DayIndex := FSTRING_TO_WEEKDAY(WDAY := Token, LANG := LANG);
    IF DayIndex >= INT#1 AND DayIndex <= INT#7 THEN
        FSTRING_TO_WEEK := FSTRING_TO_WEEK OR SHR(BYTE#128, DayIndex);
    END_IF;
END_WHILE;
FSTRING_TO_WEEK := FSTRING_TO_WEEK AND BYTE#127;
END_FUNCTION

FUNCTION FSTRING_TO_DT : DT
VAR_INPUT
    SDT : STRING[60];
    FMT : STRING[60];
END_VAR
VAR
    RemainingInput : STRING[60];
    RemainingFormat : STRING[60];
    Marker : STRING[1];
    Token : STRING[1];
    NextLiteral : STRING[1];
    Chunk : STRING[20];
    SplitPos : INT;
    YearValue : INT := INT#1970;
    MonthValue : INT := INT#1;
    DayValue : INT := INT#1;
    HourValue : INT;
    MinuteValue : INT;
    SecondValue : INT;
    ParseOk : BOOL := TRUE;
END_VAR

RemainingInput := SDT;
RemainingFormat := FMT;
WHILE LEN(RemainingFormat) > INT#0 DO
    Marker := LEFT(IN := RemainingFormat, L := INT#1);
    IF Marker = '*' THEN
        RemainingFormat := DELETE(IN := RemainingFormat, L := INT#1, P := INT#1);
        RemainingInput := DELETE(IN := RemainingInput, L := INT#1, P := INT#1);
    ELSIF Marker = '#' THEN
        Token := MID(IN := RemainingFormat, L := INT#1, P := INT#2);
        RemainingFormat := DELETE(IN := RemainingFormat, L := INT#2, P := INT#1);
        IF LEN(RemainingFormat) = INT#0 THEN
            Chunk := RemainingInput;
            RemainingInput := '';
        ELSE
            NextLiteral := LEFT(IN := RemainingFormat, L := INT#1);
            SplitPos := FIND(IN1 := RemainingInput, IN2 := NextLiteral);
            IF SplitPos = INT#0 THEN
                Chunk := RemainingInput;
                RemainingInput := '';
            ELSE
                Chunk := LEFT(IN := RemainingInput, L := SplitPos - INT#1);
                RemainingInput := DELETE(IN := RemainingInput, L := SplitPos - INT#1, P := INT#1);
            END_IF;
        END_IF;

        CASE Token OF
            'Y':
                YearValue := STRING_TO_INT(Chunk);
                IF YearValue < INT#100 THEN
                    YearValue := YearValue + INT#2000;
                END_IF;
            'M':
                MonthValue := STRING_TO_INT(Chunk);
            'N':
                MonthValue := FSTRING_TO_MONTH(MTH := Chunk, LANG := INT#0);
            'D':
                DayValue := STRING_TO_INT(Chunk);
            'h':
                HourValue := STRING_TO_INT(Chunk);
            'm':
                MinuteValue := STRING_TO_INT(Chunk);
            's':
                SecondValue := STRING_TO_INT(Chunk);
        END_CASE;
    ELSIF Marker = LEFT(IN := RemainingInput, L := INT#1) THEN
        RemainingFormat := DELETE(IN := RemainingFormat, L := INT#1, P := INT#1);
        RemainingInput := DELETE(IN := RemainingInput, L := INT#1, P := INT#1);
    ELSE
        ParseOk := FALSE;
        RemainingFormat := '';
    END_IF;
END_WHILE;

IF ParseOk THEN
    FSTRING_TO_DT := SET_DT(
        YEAR := YearValue,
        MONTH := MonthValue,
        DAY := DayValue,
        HOUR := HourValue,
        MINUTE := MinuteValue,
        SECOND := SecondValue
    );
END_IF;
END_FUNCTION

FUNCTION EXEC : STRING[STRING_LENGTH]
VAR_INPUT
    STR : STRING[STRING_LENGTH];
END_VAR
VAR
    Pos : INT;
    R1 : REAL;
    R2 : REAL;
    Operator : STRING[10];
END_VAR

EXEC := UPPERCASE(STR := TRIM(STR := STR));
Pos := FINDB_NONUM(STR := EXEC);
IF Pos > INT#1 THEN
    R1 := FLOAT_TO_REAL(FLT := LEFT(IN := EXEC, L := Pos - INT#1));
END_IF;
R2 := FLOAT_TO_REAL(FLT := RIGHT(IN := EXEC, L := LEN(EXEC) - Pos));
EXEC := LEFT(IN := EXEC, L := Pos);
Pos := FINDB_NUM(STR := EXEC);
Operator := RIGHT(IN := EXEC, L := LEN(EXEC) - Pos);
IF Operator = '' AND LEN(STR) = INT#0 THEN
    EXEC := '';
ELSIF Operator = '' THEN
    EXEC := STR;
ELSIF Operator = '^' THEN
    EXEC := REAL_TO_STRING(EXPT(R1, R2));
ELSIF Operator = 'SQRT' THEN
    EXEC := REAL_TO_STRING(SQRT(R2));
ELSIF Operator = 'SIN' THEN
    EXEC := REAL_TO_STRING(SIN(R2));
ELSIF Operator = 'COS' THEN
    EXEC := REAL_TO_STRING(COS(R2));
ELSIF Operator = 'TAN' THEN
    EXEC := REAL_TO_STRING(TAN(R2));
ELSIF Operator = '*' THEN
    EXEC := REAL_TO_STRING(R1 * R2);
ELSIF Operator = '/' THEN
    IF R2 <> REAL#0.0 THEN
        EXEC := REAL_TO_STRING(R1 / R2);
    ELSE
        EXEC := 'ERROR';
    END_IF;
ELSIF Operator = '+' THEN
    EXEC := REAL_TO_STRING(R1 + R2);
ELSIF Operator = '-' THEN
    EXEC := REAL_TO_STRING(R1 - R2);
ELSE
    EXEC := 'ERROR';
END_IF;

IF EXEC <> 'ERROR' THEN
    IF FIND(IN1 := EXEC, IN2 := '.') = INT#0 THEN
        EXEC := CONCAT(IN1 := EXEC, IN2 := '.0');
    ELSIF RIGHT(IN := EXEC, L := INT#1) = '.' THEN
        EXEC := CONCAT(IN1 := EXEC, IN2 := '0');
    END_IF;
END_IF;
END_FUNCTION

FUNCTION_BLOCK MESSAGE_4R
VAR_INPUT
    M0 : STRING[STRING_LENGTH];
    M1 : STRING[STRING_LENGTH];
    M2 : STRING[STRING_LENGTH];
    M3 : STRING[STRING_LENGTH];
    MM : INT := INT#3;
    ENQ : BOOL := TRUE;
    CLK : BOOL := TRUE;
    T1 : TIME := T#3s;
END_VAR
VAR_OUTPUT
    MX : STRING[STRING_LENGTH];
    MN : INT;
    TR : BOOL;
END_VAR
VAR
    Timer : TON;
    Edge : BOOL;
END_VAR

TR := FALSE;
IF ENQ THEN
    IF (NOT Edge AND CLK) OR Timer.Q THEN
        MN := INC1(X := MN, N := MM);
        TR := TRUE;
        Timer(IN := FALSE);
        CASE MN OF
            INT#0:
                MX := M0;
            INT#1:
                MX := M1;
            INT#2:
                MX := M2;
            INT#3:
                MX := M3;
        END_CASE;
    END_IF;
    Edge := CLK;
    Timer(IN := CLK, PT := T1);
ELSE
    MX := '';
    MN := INT#0;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MESSAGE_8
VAR_INPUT
    IN1 : BOOL;
    IN2 : BOOL;
    IN3 : BOOL;
    IN4 : BOOL;
    IN5 : BOOL;
    IN6 : BOOL;
    IN7 : BOOL;
    IN8 : BOOL;
    S1 : STRING[STRING_LENGTH];
    S2 : STRING[STRING_LENGTH];
    S3 : STRING[STRING_LENGTH];
    S4 : STRING[STRING_LENGTH];
    S5 : STRING[STRING_LENGTH];
    S6 : STRING[STRING_LENGTH];
    S7 : STRING[STRING_LENGTH];
    S8 : STRING[STRING_LENGTH];
END_VAR
VAR_OUTPUT
    M : STRING[STRING_LENGTH];
END_VAR

IF IN1 THEN
    M := S1;
ELSIF IN2 THEN
    M := S2;
ELSIF IN3 THEN
    M := S3;
ELSIF IN4 THEN
    M := S4;
ELSIF IN5 THEN
    M := S5;
ELSIF IN6 THEN
    M := S6;
ELSIF IN7 THEN
    M := S7;
ELSIF IN8 THEN
    M := S8;
ELSE
    M := '';
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK TICKER
VAR_IN_OUT
    Text : STRING[STRING_LENGTH];
END_VAR
VAR_INPUT
    N : INT;
    PT : TIME;
END_VAR
VAR_OUTPUT
    Display : STRING[STRING_LENGTH];
END_VAR
VAR
    Timer : TON;
    ResetTimer : BOOL;
    TickerStep : INT;
    LastText : STRING[STRING_LENGTH];
    LastN : INT;
END_VAR

IF N <= INT#0 OR N >= LEN(Text) THEN
    Display := Text;
    TickerStep := INT#1;
    ResetTimer := FALSE;
    Timer(IN := FALSE);
ELSE
    IF Text <> LastText OR N <> LastN THEN
        TickerStep := INT#1;
        Display := MID(IN := Text, L := N, P := TickerStep);
        ResetTimer := FALSE;
        Timer(IN := FALSE);
    ELSIF PT = T#0ms THEN
        TickerStep := TickerStep + INT#1;
        IF TickerStep > LEN(Text) THEN
            TickerStep := INT#1;
        END_IF;
        Display := MID(IN := Text, L := N, P := TickerStep);
    ELSIF ResetTimer THEN
        Timer(IN := FALSE);
        ResetTimer := FALSE;
    ELSE
        Timer(IN := TRUE, PT := PT);
        IF Timer.Q THEN
            TickerStep := TickerStep + INT#1;
            IF TickerStep > LEN(Text) THEN
                TickerStep := INT#1;
            END_IF;
            Display := MID(IN := Text, L := N, P := TickerStep);
            ResetTimer := TRUE;
        END_IF;
    END_IF;
END_IF;

LastText := Text;
LastN := N;
END_FUNCTION_BLOCK

FUNCTION DT_TO_STRF : STRING[STRING_LENGTH]
VAR_INPUT
    DTI : DT;
    MS : INT;
    FMT : STRING[STRING_LENGTH];
    LANG : INT;
END_VAR
VAR
    Ly : INT;
    YearValue : INT;
    MonthValue : INT;
    DayValue : INT;
    HourValue : INT;
    MinuteValue : INT;
    SecondValue : INT;
    MillisecondValue : INT;
    DateValue : DATE;
    Pos : INT;
    Token : STRING[1];
    Replacement : STRING[STRING_LENGTH];
    Hour12 : INT;
END_VAR

IF LANG < INT#1 THEN
    Ly := LANGUAGE.DEFAULT;
ELSIF LANG > LANGUAGE.LMAX THEN
    Ly := LANGUAGE.LMAX;
ELSE
    Ly := LANG;
END_IF;

SPLIT_DT(
    DTI,
    YearValue,
    MonthValue,
    DayValue,
    HourValue,
    MinuteValue,
    SecondValue,
    MillisecondValue
);
DateValue := CONCAT_DATE(YearValue, MonthValue, DayValue);
MillisecondValue := MS;
DT_TO_STRF := FMT;
Pos := FIND(DT_TO_STRF, '#');

WHILE Pos > INT#0 DO
    Token := RIGHT(LEFT(DT_TO_STRF, Pos + INT#1), INT#1);
    Replacement := '';

    CASE Token OF
        'A':
            Replacement := INT_TO_STRING(YearValue);
        'B':
            Replacement := RIGHT(INT_TO_STRING(YearValue), INT#2);
        'C':
            Replacement := INT_TO_STRING(MonthValue);
        'D':
            Replacement := INT_TO_STRING(MonthValue);
            IF LEN(Replacement) < INT#2 THEN
                Replacement := CONCAT('0', Replacement);
            END_IF;
        'E':
            Replacement := MONTH_TO_STRING(MTH := MonthValue, LANG := Ly, LX := INT#3);
        'F':
            Replacement := MONTH_TO_STRING(MTH := MonthValue, LANG := Ly, LX := INT#0);
        'G':
            Replacement := INT_TO_STRING(DayValue);
        'H':
            Replacement := INT_TO_STRING(DayValue);
            IF LEN(Replacement) < INT#2 THEN
                Replacement := CONCAT('0', Replacement);
            END_IF;
        'I':
            Replacement := INT_TO_STRING(DAY_OF_WEEK(DateValue));
        'J':
            Replacement := WEEKDAY_TO_STRING(
                WDAY := DAY_OF_WEEK(DateValue),
                LANG := Ly,
                LX := INT#2
            );
        'K':
            Replacement := WEEKDAY_TO_STRING(
                WDAY := DAY_OF_WEEK(DateValue),
                LANG := Ly,
                LX := INT#0
            );
        'L':
            IF HourValue >= INT#12 THEN
                Replacement := 'PM';
            ELSE
                Replacement := 'AM';
            END_IF;
        'M':
            Replacement := INT_TO_STRING(HourValue);
        'N':
            Replacement := INT_TO_STRING(HourValue);
            IF LEN(Replacement) < INT#2 THEN
                Replacement := CONCAT('0', Replacement);
            END_IF;
        'O':
            Hour12 := HourValue MOD INT#12;
            IF Hour12 = INT#0 THEN
                Hour12 := INT#12;
            END_IF;
            Replacement := INT_TO_STRING(Hour12);
        'P':
            Hour12 := HourValue MOD INT#12;
            IF Hour12 = INT#0 THEN
                Hour12 := INT#12;
            END_IF;
            Replacement := INT_TO_STRING(Hour12);
            IF LEN(Replacement) < INT#2 THEN
                Replacement := CONCAT('0', Replacement);
            END_IF;
        'Q':
            Replacement := INT_TO_STRING(MinuteValue);
        'R':
            Replacement := INT_TO_STRING(MinuteValue);
            IF LEN(Replacement) < INT#2 THEN
                Replacement := CONCAT('0', Replacement);
            END_IF;
        'S':
            Replacement := INT_TO_STRING(SecondValue);
        'T':
            Replacement := INT_TO_STRING(SecondValue);
            IF LEN(Replacement) < INT#2 THEN
                Replacement := CONCAT('0', Replacement);
            END_IF;
        'U':
            Replacement := INT_TO_STRING(MillisecondValue);
        'V':
            Replacement := INT_TO_STRING(MillisecondValue);
            Replacement := CONCAT('00', Replacement);
            Replacement := RIGHT(Replacement, INT#3);
        'W':
            Replacement := INT_TO_STRING(DayValue);
            IF LEN(Replacement) < INT#2 THEN
                Replacement := CONCAT(' ', Replacement);
            END_IF;
        'X':
            Replacement := INT_TO_STRING(MonthValue);
            IF LEN(Replacement) < INT#2 THEN
                Replacement := CONCAT(' ', Replacement);
            END_IF;
    END_CASE;

    DT_TO_STRF := REPLACE(DT_TO_STRF, Replacement, INT#2, Pos);
    Pos := FIND(DT_TO_STRF, '#');
END_WHILE;
END_FUNCTION
