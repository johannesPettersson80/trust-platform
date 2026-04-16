FUNCTION_BLOCK LTCH
VAR_INPUT
    D : BOOL;
    L : BOOL;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    Q : BOOL;
END_VAR

IF RST THEN
    Q := FALSE;
ELSIF L THEN
    Q := D;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK LTCH_4
VAR_INPUT
    D0 : BOOL;
    D1 : BOOL;
    D2 : BOOL;
    D3 : BOOL;
    L : BOOL;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    Q0 : BOOL;
    Q1 : BOOL;
    Q2 : BOOL;
    Q3 : BOOL;
END_VAR

IF RST THEN
    Q0 := FALSE;
    Q1 := FALSE;
    Q2 := FALSE;
    Q3 := FALSE;
ELSIF L THEN
    Q0 := D0;
    Q1 := D1;
    Q2 := D2;
    Q3 := D3;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK STORE_8
VAR_INPUT
    SetIn : BOOL;
    D0 : BOOL;
    D1 : BOOL;
    D2 : BOOL;
    D3 : BOOL;
    D4 : BOOL;
    D5 : BOOL;
    D6 : BOOL;
    D7 : BOOL;
    ClrIn : BOOL;
    Rst : BOOL;
END_VAR
VAR_OUTPUT
    Q0 : BOOL;
    Q1 : BOOL;
    Q2 : BOOL;
    Q3 : BOOL;
    Q4 : BOOL;
    Q5 : BOOL;
    Q6 : BOOL;
    Q7 : BOOL;
END_VAR
VAR
    Edge : BOOL;
END_VAR

IF Rst OR SetIn THEN
    Q0 := NOT Rst;
    Q1 := Q0;
    Q2 := Q0;
    Q3 := Q0;
    Q4 := Q0;
    Q5 := Q0;
    Q6 := Q0;
    Q7 := Q0;
ELSE
    IF D0 THEN Q0 := TRUE; END_IF;
    IF D1 THEN Q1 := TRUE; END_IF;
    IF D2 THEN Q2 := TRUE; END_IF;
    IF D3 THEN Q3 := TRUE; END_IF;
    IF D4 THEN Q4 := TRUE; END_IF;
    IF D5 THEN Q5 := TRUE; END_IF;
    IF D6 THEN Q6 := TRUE; END_IF;
    IF D7 THEN Q7 := TRUE; END_IF;

    IF ClrIn AND NOT Edge THEN
        IF Q0 THEN
            Q0 := FALSE;
        ELSIF Q1 THEN
            Q1 := FALSE;
        ELSIF Q2 THEN
            Q2 := FALSE;
        ELSIF Q3 THEN
            Q3 := FALSE;
        ELSIF Q4 THEN
            Q4 := FALSE;
        ELSIF Q5 THEN
            Q5 := FALSE;
        ELSIF Q6 THEN
            Q6 := FALSE;
        ELSE
            Q7 := FALSE;
        END_IF;
    END_IF;
    Edge := ClrIn;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK COUNT_BR
VAR_INPUT
    SetIn : BOOL;
    IN : BYTE;
    UP : BOOL;
    DN : BOOL;
    StepSize : BYTE := BYTE#1;
    MX : BYTE := BYTE#255;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    CNT : BYTE;
END_VAR
VAR
    LastUp : BOOL;
    LastDn : BOOL;
END_VAR

IF RST THEN
    CNT := BYTE#0;
ELSIF SetIn THEN
    IF IN > MX THEN
        CNT := MX;
    ELSE
        CNT := IN;
    END_IF;
ELSIF UP AND NOT LastUp THEN
    CNT := INT_TO_BYTE(IN := INC(
        X := BYTE_TO_INT(IN := CNT),
        D := BYTE_TO_INT(IN := StepSize),
        M := BYTE_TO_INT(IN := MX)
    ));
ELSIF DN AND NOT LastDn THEN
    CNT := INT_TO_BYTE(IN := INC(
        X := BYTE_TO_INT(IN := CNT),
        D := -BYTE_TO_INT(IN := StepSize),
        M := BYTE_TO_INT(IN := MX)
    ));
END_IF;
LastUp := UP;
LastDn := DN;
END_FUNCTION_BLOCK

FUNCTION_BLOCK COUNT_DR
VAR_INPUT
    SetIn : BOOL;
    IN : DWORD;
    UP : BOOL;
    DN : BOOL;
    StepSize : DWORD := DWORD#1;
    MX : DWORD := DWORD#16#FFFFFFFF;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    CNT : DWORD;
END_VAR
VAR
    LastUp : BOOL;
    LastDn : BOOL;
    CntValue : UDINT;
    StepValue : UDINT;
    MaxValue : UDINT;
    InputValue : UDINT;
END_VAR

CntValue := DWORD_TO_UDINT(IN := CNT);
StepValue := DWORD_TO_UDINT(IN := StepSize);
MaxValue := DWORD_TO_UDINT(IN := MX);
InputValue := DWORD_TO_UDINT(IN := IN);

IF RST THEN
    CNT := DWORD#0;
ELSIF SetIn THEN
    IF InputValue > MaxValue THEN
        CNT := MX;
    ELSE
        CNT := IN;
    END_IF;
ELSIF UP AND NOT LastUp THEN
    IF StepValue > MaxValue - CntValue THEN
        CntValue := StepValue - (MaxValue - CntValue) - UDINT#1;
    ELSE
        CntValue := CntValue + StepValue;
    END_IF;
    CNT := UDINT_TO_DWORD(IN := CntValue);
ELSIF DN AND NOT LastDn THEN
    IF StepValue > CntValue THEN
        CntValue := MaxValue - (StepValue - CntValue) + UDINT#1;
    ELSE
        CntValue := CntValue - StepValue;
    END_IF;
    CNT := UDINT_TO_DWORD(IN := CntValue);
END_IF;
LastUp := UP;
LastDn := DN;
END_FUNCTION_BLOCK

FUNCTION_BLOCK TOGGLE
VAR_INPUT
    CLK : BOOL;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    Q : BOOL;
END_VAR
VAR
    Edge : BOOL;
END_VAR

IF RST THEN
    Q := FALSE;
ELSIF CLK AND NOT Edge THEN
    Q := NOT Q;
END_IF;
Edge := CLK;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FF_D2E
VAR_INPUT
    D0 : BOOL;
    D1 : BOOL;
    CLK : BOOL;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    Q0 : BOOL;
    Q1 : BOOL;
END_VAR
VAR
    Edge : BOOL;
END_VAR

IF RST THEN
    Q0 := FALSE;
    Q1 := FALSE;
ELSIF CLK AND NOT Edge THEN
    Q0 := D0;
    Q1 := D1;
END_IF;
Edge := CLK;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FF_D4E
VAR_INPUT
    D0 : BOOL;
    D1 : BOOL;
    D2 : BOOL;
    D3 : BOOL;
    CLK : BOOL;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    Q0 : BOOL;
    Q1 : BOOL;
    Q2 : BOOL;
    Q3 : BOOL;
END_VAR
VAR
    Edge : BOOL;
END_VAR

IF RST THEN
    Q0 := FALSE;
    Q1 := FALSE;
    Q2 := FALSE;
    Q3 := FALSE;
ELSIF CLK AND NOT Edge THEN
    Q0 := D0;
    Q1 := D1;
    Q2 := D2;
    Q3 := D3;
END_IF;
Edge := CLK;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FF_DRE
VAR_INPUT
    SetIn : BOOL;
    D : BOOL;
    CLK : BOOL;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    Q : BOOL;
END_VAR
VAR
    Edge : BOOL;
END_VAR

IF RST OR SetIn THEN
    Q := NOT RST;
ELSIF CLK AND NOT Edge THEN
    Q := D;
END_IF;
Edge := CLK;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FF_JKE
VAR_INPUT
    SetIn : BOOL;
    J : BOOL;
    CLK : BOOL;
    K : BOOL;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    Q : BOOL;
END_VAR
VAR
    Edge : BOOL;
END_VAR

IF RST OR SetIn THEN
    Q := NOT RST;
ELSIF CLK AND NOT Edge THEN
    IF J XOR K THEN
        Q := J;
    ELSE
        Q := K XOR Q;
    END_IF;
END_IF;
Edge := CLK;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FF_RSE
VAR_INPUT
    CS : BOOL;
    CR : BOOL;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    Q : BOOL;
END_VAR
VAR
    EdgeSet : BOOL;
    EdgeReset : BOOL;
END_VAR

IF RST THEN
    Q := FALSE;
ELSIF CR AND NOT EdgeReset THEN
    Q := FALSE;
ELSIF CS AND NOT EdgeSet THEN
    Q := TRUE;
END_IF;

EdgeSet := CS;
EdgeReset := CR;
END_FUNCTION_BLOCK

FUNCTION_BLOCK SELECT_8
VAR_INPUT
    E : BOOL;
    SetIn : BOOL;
    IN : BYTE;
    UP : BOOL;
    DN : BOOL;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    Q0 : BOOL;
    Q1 : BOOL;
    Q2 : BOOL;
    Q3 : BOOL;
    Q4 : BOOL;
    Q5 : BOOL;
    Q6 : BOOL;
    Q7 : BOOL;
    STATE : INT;
END_VAR
VAR
    LastUp : BOOL;
    LastDn : BOOL;
END_VAR

IF RST THEN
    STATE := INT#0;
ELSIF SetIn THEN
    STATE := BYTE_TO_INT(IN := IN);
ELSIF UP AND NOT LastUp THEN
    STATE := INC(X := STATE, D := INT#1, M := INT#7);
ELSIF DN AND NOT LastDn THEN
    STATE := INC(X := STATE, D := INT#-1, M := INT#7);
END_IF;
LastUp := UP;
LastDn := DN;

Q0 := FALSE;
Q1 := FALSE;
Q2 := FALSE;
Q3 := FALSE;
Q4 := FALSE;
Q5 := FALSE;
Q6 := FALSE;
Q7 := FALSE;

IF E THEN
    CASE STATE OF
        INT#0:
            Q0 := TRUE;
        INT#1:
            Q1 := TRUE;
        INT#2:
            Q2 := TRUE;
        INT#3:
            Q3 := TRUE;
        INT#4:
            Q4 := TRUE;
        INT#5:
            Q5 := TRUE;
        INT#6:
            Q6 := TRUE;
        INT#7:
            Q7 := TRUE;
    END_CASE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK SHR_4E
VAR_INPUT
    SetIn : BOOL;
    D0 : BOOL;
    CLK : BOOL;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    Q0 : BOOL;
    Q1 : BOOL;
    Q2 : BOOL;
    Q3 : BOOL;
END_VAR
VAR
    Edge : BOOL;
END_VAR

IF SetIn OR RST THEN
    Q0 := NOT RST;
    Q1 := Q0;
    Q2 := Q0;
    Q3 := Q0;
ELSIF CLK AND NOT Edge THEN
    Q3 := Q2;
    Q2 := Q1;
    Q1 := Q0;
    Q0 := D0;
END_IF;
Edge := CLK;
END_FUNCTION_BLOCK

FUNCTION_BLOCK SHR_4UDE
VAR_INPUT
    SetIn : BOOL;
    D0 : BOOL;
    D3 : BOOL;
    CLK : BOOL;
    DN : BOOL;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    Q0 : BOOL;
    Q1 : BOOL;
    Q2 : BOOL;
    Q3 : BOOL;
END_VAR
VAR
    Edge : BOOL;
END_VAR

IF SetIn OR RST THEN
    Q0 := NOT RST;
    Q1 := Q0;
    Q2 := Q0;
    Q3 := Q0;
ELSIF CLK AND NOT Edge THEN
    IF DN THEN
        Q0 := Q1;
        Q1 := Q2;
        Q2 := Q3;
        Q3 := D3;
    ELSE
        Q3 := Q2;
        Q2 := Q1;
        Q1 := Q0;
        Q0 := D0;
    END_IF;
END_IF;
Edge := CLK;
END_FUNCTION_BLOCK

FUNCTION_BLOCK SHR_8PLE
VAR_INPUT
    Din : BOOL;
    Dload : BYTE;
    CLK : BOOL;
    UP : BOOL := TRUE;
    LoadIn : BOOL;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    DOut : BOOL;
END_VAR
VAR
    Edge : BOOL;
    RegisterValue : BYTE;
END_VAR

IF RST THEN
    RegisterValue := BYTE#0;
    DOut := FALSE;
ELSIF CLK AND NOT Edge THEN
    IF UP THEN
        RegisterValue := SHL(RegisterValue, INT#1);
        IF Din THEN
            RegisterValue := RegisterValue OR BYTE#1;
        END_IF;
    ELSE
        RegisterValue := SHR(RegisterValue, INT#1);
        IF Din THEN
            RegisterValue := RegisterValue OR BYTE#16#80;
        END_IF;
    END_IF;

    IF LoadIn THEN
        RegisterValue := Dload;
    END_IF;

    IF UP THEN
        DOut := (RegisterValue AND BYTE#16#80) = BYTE#16#80;
    ELSE
        DOut := (RegisterValue AND BYTE#1) = BYTE#1;
    END_IF;
END_IF;
Edge := CLK;
END_FUNCTION_BLOCK

FUNCTION_BLOCK SHR_8UDE
VAR_INPUT
    SetIn : BOOL;
    D0 : BOOL;
    D7 : BOOL;
    CLK : BOOL;
    DN : BOOL;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    Q0 : BOOL;
    Q1 : BOOL;
    Q2 : BOOL;
    Q3 : BOOL;
    Q4 : BOOL;
    Q5 : BOOL;
    Q6 : BOOL;
    Q7 : BOOL;
END_VAR
VAR
    Edge : BOOL;
END_VAR

IF SetIn OR RST THEN
    Q0 := NOT RST;
    Q1 := Q0;
    Q2 := Q0;
    Q3 := Q0;
    Q4 := Q0;
    Q5 := Q0;
    Q6 := Q0;
    Q7 := Q0;
ELSIF CLK AND NOT Edge THEN
    IF DN THEN
        Q0 := Q1;
        Q1 := Q2;
        Q2 := Q3;
        Q3 := Q4;
        Q4 := Q5;
        Q5 := Q6;
        Q6 := Q7;
        Q7 := D7;
    ELSE
        Q7 := Q6;
        Q6 := Q5;
        Q5 := Q4;
        Q4 := Q3;
        Q3 := Q2;
        Q2 := Q1;
        Q1 := Q0;
        Q0 := D0;
    END_IF;
END_IF;
Edge := CLK;
END_FUNCTION_BLOCK

FUNCTION_BLOCK DEC_2
VAR_INPUT
    D : BOOL;
    A : BOOL;
END_VAR
VAR_OUTPUT
    Q0 : BOOL;
    Q1 : BOOL;
END_VAR

Q0 := D AND NOT A;
Q1 := D AND A;
END_FUNCTION_BLOCK

FUNCTION_BLOCK DEC_4
VAR_INPUT
    D : BOOL;
    A0 : BOOL;
    A1 : BOOL;
END_VAR
VAR_OUTPUT
    Q0 : BOOL;
    Q1 : BOOL;
    Q2 : BOOL;
    Q3 : BOOL;
END_VAR

Q0 := D AND NOT A0 AND NOT A1;
Q1 := D AND A0 AND NOT A1;
Q2 := D AND NOT A0 AND A1;
Q3 := D AND A0 AND A1;
END_FUNCTION_BLOCK

FUNCTION_BLOCK DEC_8
VAR_INPUT
    D : BOOL;
    A0 : BOOL;
    A1 : BOOL;
    A2 : BOOL;
END_VAR
VAR_OUTPUT
    Q0 : BOOL;
    Q1 : BOOL;
    Q2 : BOOL;
    Q3 : BOOL;
    Q4 : BOOL;
    Q5 : BOOL;
    Q6 : BOOL;
    Q7 : BOOL;
END_VAR

Q0 := D AND NOT A0 AND NOT A1 AND NOT A2;
Q1 := D AND A0 AND NOT A1 AND NOT A2;
Q2 := D AND NOT A0 AND A1 AND NOT A2;
Q3 := D AND A0 AND A1 AND NOT A2;
Q4 := D AND NOT A0 AND NOT A1 AND A2;
Q5 := D AND A0 AND NOT A1 AND A2;
Q6 := D AND NOT A0 AND A1 AND A2;
Q7 := D AND A0 AND A1 AND A2;
END_FUNCTION_BLOCK

FUNCTION DW_TO_REAL : REAL
VAR_INPUT
    X : DWORD;
END_VAR

DW_TO_REAL := DWORD_TO_REAL(X);
END_FUNCTION

FUNCTION MUX_2 : BOOL
VAR_INPUT
    D0 : BOOL;
    D1 : BOOL;
    A0 : BOOL;
END_VAR

MUX_2 := SEL(A0, D0, D1);
END_FUNCTION

FUNCTION MUX_4 : BOOL
VAR_INPUT
    D0 : BOOL;
    D1 : BOOL;
    D2 : BOOL;
    D3 : BOOL;
    A0 : BOOL;
    A1 : BOOL;
END_VAR

IF A1 THEN
    MUX_4 := SEL(A0, D2, D3);
ELSE
    MUX_4 := SEL(A0, D0, D1);
END_IF;
END_FUNCTION

FUNCTION BCDC_TO_INT : INT
VAR_INPUT
    IN : BYTE;
END_VAR

BCDC_TO_INT := BYTE_TO_INT(IN := IN AND BYTE#16#0F) + BYTE_TO_INT(IN := SHR(IN, INT#4)) * INT#10;
END_FUNCTION

FUNCTION BIT_COUNT : INT
VAR_INPUT
    IN : DWORD;
END_VAR
VAR
    Work : DWORD;
END_VAR

Work := IN;
WHILE Work > DWORD#0 DO
    IF (Work AND DWORD#1) = DWORD#1 THEN
        BIT_COUNT := BIT_COUNT + INT#1;
    END_IF;
    Work := SHR(Work, INT#1);
END_WHILE;
END_FUNCTION

FUNCTION BIT_LOAD_B : BYTE
VAR_INPUT
    IN : BYTE;
    VAL : BOOL;
    POS : INT;
END_VAR

IF VAL THEN
    BIT_LOAD_B := IN OR SHL(BYTE#1, POS);
ELSE
    BIT_LOAD_B := IN AND NOT SHL(BYTE#1, POS);
END_IF;
END_FUNCTION

FUNCTION BIT_LOAD_B2 : BYTE
VAR_INPUT
    I : BYTE;
    D : BOOL;
    P : INT;
    N : INT;
END_VAR
VAR
    Index : INT;
    TargetPos : INT;
END_VAR

BIT_LOAD_B2 := I;
FOR Index := INT#0 TO N - INT#1 DO
    TargetPos := P + Index;
    WHILE TargetPos >= INT#8 DO
        TargetPos := TargetPos - INT#8;
    END_WHILE;
    WHILE TargetPos < INT#0 DO
        TargetPos := TargetPos + INT#8;
    END_WHILE;
    BIT_LOAD_B2 := BIT_LOAD_B(IN := BIT_LOAD_B2, VAL := D, POS := TargetPos);
END_FOR;
END_FUNCTION

FUNCTION BIT_LOAD_DW : DWORD
VAR_INPUT
    IN : DWORD;
    VAL : BOOL;
    POS : INT;
END_VAR

IF VAL THEN
    BIT_LOAD_DW := IN OR SHL(DWORD#1, POS);
ELSE
    BIT_LOAD_DW := IN AND NOT SHL(DWORD#1, POS);
END_IF;
END_FUNCTION

FUNCTION BIT_LOAD_DW2 : DWORD
VAR_INPUT
    I : DWORD;
    D : BOOL;
    P : INT;
    N : INT;
END_VAR
VAR
    Index : INT;
    TargetPos : INT;
END_VAR

BIT_LOAD_DW2 := I;
FOR Index := INT#0 TO N - INT#1 DO
    TargetPos := P + Index;
    WHILE TargetPos >= INT#32 DO
        TargetPos := TargetPos - INT#32;
    END_WHILE;
    WHILE TargetPos < INT#0 DO
        TargetPos := TargetPos + INT#32;
    END_WHILE;
    BIT_LOAD_DW2 := BIT_LOAD_DW(IN := BIT_LOAD_DW2, VAL := D, POS := TargetPos);
END_FOR;
END_FUNCTION

FUNCTION BIT_LOAD_W : WORD
VAR_INPUT
    IN : WORD;
    VAL : BOOL;
    POS : INT;
END_VAR

IF VAL THEN
    BIT_LOAD_W := IN OR SHL(WORD#1, POS);
ELSE
    BIT_LOAD_W := IN AND NOT SHL(WORD#1, POS);
END_IF;
END_FUNCTION

FUNCTION BIT_LOAD_W2 : WORD
VAR_INPUT
    I : WORD;
    D : BOOL;
    P : INT;
    N : INT;
END_VAR
VAR
    Index : INT;
    TargetPos : INT;
END_VAR

BIT_LOAD_W2 := I;
FOR Index := INT#0 TO N - INT#1 DO
    TargetPos := P + Index;
    WHILE TargetPos >= INT#16 DO
        TargetPos := TargetPos - INT#16;
    END_WHILE;
    WHILE TargetPos < INT#0 DO
        TargetPos := TargetPos + INT#16;
    END_WHILE;
    BIT_LOAD_W2 := BIT_LOAD_W(IN := BIT_LOAD_W2, VAL := D, POS := TargetPos);
END_FOR;
END_FUNCTION

FUNCTION BIT_OF_DWORD : BOOL
VAR_INPUT
    IN : DWORD;
    N : INT;
END_VAR

BIT_OF_DWORD := (SHR(IN, N) AND DWORD#1) = DWORD#1;
END_FUNCTION

FUNCTION BIT_TOGGLE_B : BYTE
VAR_INPUT
    IN : BYTE;
    POS : INT;
END_VAR

BIT_TOGGLE_B := SHL(BYTE#1, POS) XOR IN;
END_FUNCTION

FUNCTION BIT_TOGGLE_DW : DWORD
VAR_INPUT
    IN : DWORD;
    POS : INT;
END_VAR

BIT_TOGGLE_DW := SHL(DWORD#1, POS) XOR IN;
END_FUNCTION

FUNCTION BIT_TOGGLE_W : WORD
VAR_INPUT
    IN : WORD;
    POS : INT;
END_VAR

BIT_TOGGLE_W := SHL(WORD#1, POS) XOR IN;
END_FUNCTION

FUNCTION BYTE_OF_BIT : BYTE
VAR_INPUT
    B0 : BOOL;
    B1 : BOOL;
    B2 : BOOL;
    B3 : BOOL;
    B4 : BOOL;
    B5 : BOOL;
    B6 : BOOL;
    B7 : BOOL;
END_VAR

BYTE_OF_BIT := BYTE#0;
IF B0 THEN BYTE_OF_BIT := BYTE_OF_BIT OR BYTE#1; END_IF;
IF B1 THEN BYTE_OF_BIT := BYTE_OF_BIT OR BYTE#2; END_IF;
IF B2 THEN BYTE_OF_BIT := BYTE_OF_BIT OR BYTE#4; END_IF;
IF B3 THEN BYTE_OF_BIT := BYTE_OF_BIT OR BYTE#8; END_IF;
IF B4 THEN BYTE_OF_BIT := BYTE_OF_BIT OR BYTE#16; END_IF;
IF B5 THEN BYTE_OF_BIT := BYTE_OF_BIT OR BYTE#32; END_IF;
IF B6 THEN BYTE_OF_BIT := BYTE_OF_BIT OR BYTE#64; END_IF;
IF B7 THEN BYTE_OF_BIT := BYTE_OF_BIT OR BYTE#16#80; END_IF;
END_FUNCTION

FUNCTION BYTE_OF_DWORD : BYTE
VAR_INPUT
    IN : DWORD;
    N : BYTE;
END_VAR
VAR
    ShiftBits : INT;
END_VAR

ShiftBits := BYTE_TO_INT(IN := N) * INT#8;
BYTE_OF_DWORD := DWORD_TO_BYTE(SHR(IN, ShiftBits));
END_FUNCTION

FUNCTION_BLOCK BYTE_TO_BITS
VAR_INPUT
    IN : BYTE;
END_VAR
VAR_OUTPUT
    B0 : BOOL;
    B1 : BOOL;
    B2 : BOOL;
    B3 : BOOL;
    B4 : BOOL;
    B5 : BOOL;
    B6 : BOOL;
    B7 : BOOL;
END_VAR

B0 := (IN AND BYTE#1) = BYTE#1;
B1 := (IN AND BYTE#2) = BYTE#2;
B2 := (IN AND BYTE#4) = BYTE#4;
B3 := (IN AND BYTE#8) = BYTE#8;
B4 := (IN AND BYTE#16) = BYTE#16;
B5 := (IN AND BYTE#32) = BYTE#32;
B6 := (IN AND BYTE#64) = BYTE#64;
B7 := (IN AND BYTE#16#80) = BYTE#16#80;
END_FUNCTION_BLOCK

FUNCTION BYTE_TO_GRAY : BYTE
VAR_INPUT
    IN : BYTE;
END_VAR

BYTE_TO_GRAY := IN XOR SHR(IN, INT#1);
END_FUNCTION

FUNCTION CHECK_PARITY : BOOL
VAR_INPUT
    IN : DWORD;
    P : BOOL;
END_VAR

CHECK_PARITY := PARITY(IN) = P;
END_FUNCTION

FUNCTION DWORD_OF_BYTE : DWORD
VAR_INPUT
    B3 : BYTE;
    B2 : BYTE;
    B1 : BYTE;
    B0 : BYTE;
END_VAR

DWORD_OF_BYTE :=
    SHL(BYTE_TO_DWORD(B3), INT#24) OR
    SHL(BYTE_TO_DWORD(B2), INT#16) OR
    SHL(BYTE_TO_DWORD(B1), INT#8) OR
    BYTE_TO_DWORD(B0);
END_FUNCTION

FUNCTION DWORD_OF_WORD : DWORD
VAR_INPUT
    W1 : WORD;
    W0 : WORD;
END_VAR

DWORD_OF_WORD := SHL(WORD_TO_DWORD(W1), INT#16) OR WORD_TO_DWORD(W0);
END_FUNCTION

FUNCTION CHK_REAL : BYTE
VAR_INPUT
    X : REAL;
END_VAR
VAR
    Bits : DWORD;
    Magnitude : DWORD;
END_VAR

Bits := REAL_TO_DWORD(X);
Magnitude := Bits AND DWORD#16#7FFFFFFF;

IF Magnitude < DWORD#16#7F800000 THEN
    CHK_REAL := BYTE#16#00;
ELSIF Bits = DWORD#16#7F800000 THEN
    CHK_REAL := BYTE#16#20;
ELSIF Bits = DWORD#16#FF800000 THEN
    CHK_REAL := BYTE#16#40;
ELSE
    CHK_REAL := BYTE#16#80;
END_IF;
END_FUNCTION

FUNCTION GRAY_TO_BYTE : BYTE
VAR_INPUT
    IN : BYTE;
END_VAR
VAR
    Work : BYTE;
END_VAR

Work := IN;
Work := SHR(Work, INT#4) XOR Work;
Work := SHR(Work, INT#2) XOR Work;
GRAY_TO_BYTE := SHR(Work, INT#1) XOR Work;
END_FUNCTION

FUNCTION INT_TO_BCDC : BYTE
VAR_INPUT
    IN : INT;
END_VAR

INT_TO_BCDC := SHL(INT_TO_BYTE(IN / INT#10), INT#4) OR INT_TO_BYTE(IN MOD INT#10);
END_FUNCTION

FUNCTION PARITY : BOOL
VAR_INPUT
    IN : DWORD;
END_VAR

PARITY := (BIT_COUNT(IN) MOD INT#2) = INT#1;
END_FUNCTION

FUNCTION REAL_TO_DW : DWORD
VAR_INPUT
    X : REAL;
END_VAR

REAL_TO_DW := REAL_TO_DWORD(X);
END_FUNCTION

FUNCTION REFLECT : DWORD
VAR_INPUT
    D : DWORD;
    L : INT;
END_VAR
VAR
    Index : INT;
    Work : DWORD;
END_VAR

Work := D;
REFLECT := DWORD#0;
FOR Index := INT#1 TO L DO
    REFLECT := SHL(REFLECT, INT#1);
    IF (Work AND DWORD#1) = DWORD#1 THEN
        REFLECT := REFLECT OR DWORD#1;
    END_IF;
    Work := SHR(Work, INT#1);
END_FOR;
REFLECT := REFLECT OR SHL(Work, L);
END_FUNCTION

FUNCTION REVERSE : BYTE
VAR_INPUT
    IN : BYTE;
END_VAR
VAR
    Index : INT;
    Work : BYTE;
END_VAR

Work := IN;
REVERSE := BYTE#0;
FOR Index := INT#0 TO INT#7 DO
    REVERSE := SHL(REVERSE, INT#1);
    IF (Work AND BYTE#1) = BYTE#1 THEN
        REVERSE := REVERSE OR BYTE#1;
    END_IF;
    Work := SHR(Work, INT#1);
END_FOR;
END_FUNCTION

FUNCTION SHL1 : DWORD
VAR_INPUT
    IN : DWORD;
    N : INT;
END_VAR

IF N <= INT#0 THEN
    SHL1 := IN;
ELSIF N >= INT#32 THEN
    SHL1 := DWORD#16#FFFFFFFF;
ELSE
    SHL1 := SHL(IN, N) OR SHR(DWORD#16#FFFFFFFF, INT#32 - N);
END_IF;
END_FUNCTION

FUNCTION SHR1 : DWORD
VAR_INPUT
    IN : DWORD;
    N : INT;
END_VAR

IF N <= INT#0 THEN
    SHR1 := IN;
ELSIF N >= INT#32 THEN
    SHR1 := DWORD#16#FFFFFFFF;
ELSE
    SHR1 := SHR(IN, N) OR SHL(DWORD#16#FFFFFFFF, INT#32 - N);
END_IF;
END_FUNCTION

FUNCTION SWAP_BYTE : WORD
VAR_INPUT
    IN : WORD;
END_VAR

SWAP_BYTE := SHL(IN AND WORD#16#00FF, INT#8) OR SHR(IN AND WORD#16#FF00, INT#8);
END_FUNCTION

FUNCTION SWAP_BYTE2 : DWORD
VAR_INPUT
    IN : DWORD;
END_VAR

SWAP_BYTE2 :=
    SHL(IN AND DWORD#16#000000FF, INT#24) OR
    SHL(IN AND DWORD#16#0000FF00, INT#8) OR
    SHR(IN AND DWORD#16#00FF0000, INT#8) OR
    SHR(IN AND DWORD#16#FF000000, INT#24);
END_FUNCTION

FUNCTION WORD_OF_BYTE : WORD
VAR_INPUT
    B1 : BYTE;
    B0 : BYTE;
END_VAR

WORD_OF_BYTE := SHL(BYTE_TO_WORD(B1), INT#8) OR BYTE_TO_WORD(B0);
END_FUNCTION

FUNCTION WORD_OF_DWORD : WORD
VAR_INPUT
    IN : DWORD;
    N : BYTE;
END_VAR
VAR
    ShiftBits : INT;
END_VAR

ShiftBits := BYTE_TO_INT(IN := N) * INT#16;
WORD_OF_DWORD := DWORD_TO_WORD(SHR(IN, ShiftBits));
END_FUNCTION

FUNCTION_BLOCK A_TRIG
VAR_INPUT
    IN : REAL;
    RES : REAL;
END_VAR
VAR_OUTPUT
    Q : BOOL;
    D : REAL;
END_VAR
VAR
    LastIn : REAL;
END_VAR

D := IN - LastIn;
Q := ABS(D) > RES;
IF Q THEN
    LastIn := IN;
END_IF;
D := IN - LastIn;
END_FUNCTION_BLOCK

FUNCTION_BLOCK B_TRIG
VAR_INPUT
    CLK : BOOL;
END_VAR
VAR_OUTPUT
    Q : BOOL;
END_VAR
VAR
    Edge : BOOL;
END_VAR

Q := CLK XOR Edge;
Edge := CLK;
END_FUNCTION_BLOCK

FUNCTION_BLOCK D_TRIG
VAR_INPUT
    IN : DWORD;
END_VAR
VAR_OUTPUT
    Q : BOOL;
    X : DWORD;
END_VAR
VAR
    LastIn : DWORD;
END_VAR

Q := IN <> LastIn;
X := UDINT_TO_DWORD(IN := DWORD_TO_UDINT(IN) - DWORD_TO_UDINT(LastIn));
LastIn := IN;
END_FUNCTION_BLOCK

FUNCTION_BLOCK CLICK_CNT
VAR_INPUT
    IN : BOOL;
    N : INT;
    TC : TIME;
END_VAR
VAR_OUTPUT
    Q : BOOL;
END_VAR
VAR
    Timer : TP;
    Edge : BOOL;
    Count : INT := INT#-1;
END_VAR

Q := FALSE;

IF IN AND NOT Edge AND NOT Timer.Q THEN
    Count := INT#0;
ELSIF Timer.Q AND NOT IN AND Edge THEN
    Count := Count + INT#1;
ELSIF NOT Timer.Q THEN
    Q := Count = N;
    Count := INT#-1;
END_IF;

Edge := IN;
Timer(IN := IN, PT := TC);
END_FUNCTION_BLOCK

FUNCTION_BLOCK CLICK_DEC
VAR_INPUT
    IN : BOOL;
    TC : TIME;
END_VAR
VAR_OUTPUT
    Q0 : BOOL;
    Q1 : BOOL;
    Q2 : BOOL;
    Q3 : BOOL;
END_VAR
VAR
    Timer : TP;
    Edge : BOOL;
    Count : INT := INT#-1;
END_VAR

IF NOT IN THEN
    Q0 := FALSE;
    Q1 := FALSE;
    Q2 := FALSE;
    Q3 := FALSE;
END_IF;

IF IN AND NOT Edge AND NOT Timer.Q THEN
    Count := INT#0;
ELSIF Timer.Q AND NOT IN AND Edge THEN
    Count := Count + INT#1;
ELSIF NOT Timer.Q THEN
    CASE Count OF
        INT#0:
            Q0 := TRUE;
        INT#1:
            Q1 := TRUE;
        INT#2:
            Q2 := TRUE;
        INT#3:
            Q3 := TRUE;
    END_CASE;
    Count := INT#-1;
END_IF;

Edge := IN;
Timer(IN := IN, PT := TC);
END_FUNCTION_BLOCK

FUNCTION_BLOCK CLK_DIV
VAR_INPUT
    CLK : BOOL;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    Q0 : BOOL;
    Q1 : BOOL;
    Q2 : BOOL;
    Q3 : BOOL;
    Q4 : BOOL;
    Q5 : BOOL;
    Q6 : BOOL;
    Q7 : BOOL;
END_VAR
VAR
    Count : BYTE;
END_VAR

IF RST THEN
    Count := BYTE#0;
    Q0 := FALSE;
    Q1 := FALSE;
    Q2 := FALSE;
    Q3 := FALSE;
    Q4 := FALSE;
    Q5 := FALSE;
    Q6 := FALSE;
    Q7 := FALSE;
ELSIF CLK THEN
    Count := INT_TO_BYTE(IN := BYTE_TO_INT(IN := Count) + INT#1);
    Q0 := (Count AND BYTE#1) = BYTE#1;
    Q1 := (Count AND BYTE#2) = BYTE#2;
    Q2 := (Count AND BYTE#4) = BYTE#4;
    Q3 := (Count AND BYTE#8) = BYTE#8;
    Q4 := (Count AND BYTE#16) = BYTE#16;
    Q5 := (Count AND BYTE#32) = BYTE#32;
    Q6 := (Count AND BYTE#64) = BYTE#64;
    Q7 := (Count AND BYTE#16#80) = BYTE#16#80;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK CLK_N
VAR_INPUT
    N : INT;
END_VAR
VAR_OUTPUT
    Q : BOOL;
END_VAR
VAR
    Edge : BOOL;
    Sample : DWORD;
    Clk : BOOL;
END_VAR

Sample := SHR(T_PLC_MS(), N);
Clk := (Sample AND DWORD#1) = DWORD#1;
Q := Clk XOR Edge;
Edge := Clk;
END_FUNCTION_BLOCK

FUNCTION_BLOCK CLK_PULSE
VAR_INPUT
    PT : TIME;
    N : INT;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    Q : BOOL;
    CNT : INT;
    RUN : BOOL;
END_VAR
VAR
    Tx : DWORD;
    Tn : DWORD;
    Init : BOOL;
    Armed : BOOL;
    TxValue : UDINT;
    TnValue : UDINT;
    PeriodValue : UDINT;
END_VAR

Tx := T_PLC_MS();
TxValue := DWORD_TO_UDINT(Tx);
TnValue := DWORD_TO_UDINT(Tn);
PeriodValue := DWORD_TO_UDINT(TIME_TO_DWORD(PT));
Q := FALSE;
RUN := CNT < N;

IF NOT Init OR RST THEN
    Init := TRUE;
    CNT := INT#0;
    Tn := Tx;
    RUN := FALSE;
    Armed := TRUE;
ELSIF (CNT < N OR N = INT#0) AND (Armed OR TxValue - TnValue >= PeriodValue) THEN
    CNT := CNT + INT#1;
    Q := TRUE;
    IF Armed THEN
        Tn := Tx;
        Armed := FALSE;
    ELSE
        Tn := UDINT_TO_DWORD(TnValue + PeriodValue);
    END_IF;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK CYCLE_4
VAR_INPUT
    E : BOOL := TRUE;
    T0 : TIME;
    T1 : TIME;
    T2 : TIME;
    T3 : TIME;
    S0 : BOOL;
    SX : INT;
    SL : BOOL;
END_VAR
VAR_OUTPUT
    STATE : INT;
END_VAR
VAR
    Tx : DWORD;
    Last : DWORD;
    Init : BOOL;
    StartEdge : BOOL;
    TxValue : UDINT;
    LastValue : UDINT;
    T0Value : UDINT;
    T1Value : UDINT;
    T2Value : UDINT;
    T3Value : UDINT;
END_VAR

Tx := T_PLC_MS();
TxValue := DWORD_TO_UDINT(Tx);
LastValue := DWORD_TO_UDINT(Last);
T0Value := DWORD_TO_UDINT(TIME_TO_DWORD(T0));
T1Value := DWORD_TO_UDINT(TIME_TO_DWORD(T1));
T2Value := DWORD_TO_UDINT(TIME_TO_DWORD(T2));
T3Value := DWORD_TO_UDINT(TIME_TO_DWORD(T3));

IF NOT Init THEN
    Init := TRUE;
    Last := Tx;
END_IF;

IF E THEN
    IF SL AND NOT StartEdge THEN
        IF SX < INT#0 THEN
            STATE := INT#0;
        ELSIF SX > INT#3 THEN
            STATE := INT#3;
        ELSE
            STATE := SX;
        END_IF;
        Last := Tx;
    ELSE
        CASE STATE OF
            INT#0:
                IF TxValue - LastValue >= T0Value THEN
                    STATE := INT#1;
                    Last := Tx;
                END_IF;
            INT#1:
                IF TxValue - LastValue >= T1Value THEN
                    STATE := INT#2;
                    Last := Tx;
                END_IF;
            INT#2:
                IF TxValue - LastValue >= T2Value THEN
                    STATE := INT#3;
                    Last := Tx;
                END_IF;
            INT#3:
                IF TxValue - LastValue >= T3Value THEN
                    IF S0 THEN
                        STATE := INT#0;
                    END_IF;
                    Last := Tx;
                END_IF;
        END_CASE;
    END_IF;
ELSE
    STATE := INT#0;
    Last := Tx;
END_IF;

StartEdge := SL;
END_FUNCTION_BLOCK

FUNCTION_BLOCK GEN_BIT
VAR_INPUT
    IN0 : DWORD;
    IN1 : DWORD;
    IN2 : DWORD;
    IN3 : DWORD;
    CLK : BOOL;
    STEPS : INT;
    REP : INT;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    Q0 : BOOL;
    Q1 : BOOL;
    Q2 : BOOL;
    Q3 : BOOL;
    CNT : INT;
    RUN : BOOL;
END_VAR
VAR
    R0 : DWORD;
    R1 : DWORD;
    R2 : DWORD;
    R3 : DWORD;
    Rx : INT := INT#1;
END_VAR

IF CLK AND NOT RST THEN
    RUN := (REP = INT#0) OR (Rx <= REP);
    IF RUN THEN
        IF CNT = STEPS THEN
            CNT := INT#0;
        END_IF;

        IF CNT = INT#0 THEN
            R0 := IN0;
            R1 := IN1;
            R2 := IN2;
            R3 := IN3;
        END_IF;

        IF CNT < STEPS THEN
            Q0 := (R0 AND DWORD#1) = DWORD#1;
            Q1 := (R1 AND DWORD#1) = DWORD#1;
            Q2 := (R2 AND DWORD#1) = DWORD#1;
            Q3 := (R3 AND DWORD#1) = DWORD#1;
            R0 := SHR(R0, INT#1);
            R1 := SHR(R1, INT#1);
            R2 := SHR(R2, INT#1);
            R3 := SHR(R3, INT#1);
        END_IF;

        CNT := CNT + INT#1;
        IF CNT = STEPS AND REP <> INT#0 THEN
            Rx := Rx + INT#1;
        END_IF;
        IF Rx > REP AND REP <> INT#0 THEN
            RUN := FALSE;
        END_IF;
    END_IF;
ELSIF RST THEN
    RUN := FALSE;
    Q0 := FALSE;
    Q1 := FALSE;
    Q2 := FALSE;
    Q3 := FALSE;
    R0 := DWORD#0;
    R1 := DWORD#0;
    R2 := DWORD#0;
    R3 := DWORD#0;
    CNT := INT#0;
    Rx := INT#1;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK GEN_SQ
VAR_INPUT
    PT : TIME;
END_VAR
VAR_OUTPUT
    Q : BOOL;
END_VAR
VAR
    Tn : DWORD;
    Tx : DWORD;
    Init : BOOL;
    TxValue : UDINT;
    TnValue : UDINT;
    HalfPeriod : UDINT;
END_VAR

Tx := T_PLC_MS();
TxValue := DWORD_TO_UDINT(Tx);
TnValue := DWORD_TO_UDINT(Tn);
HalfPeriod := DWORD_TO_UDINT(SHR(TIME_TO_DWORD(PT), INT#1));

IF NOT Init THEN
    Init := TRUE;
    Tn := Tx;
    Q := TRUE;
ELSIF TxValue - TnValue >= HalfPeriod THEN
    Q := NOT Q;
    Tn := UDINT_TO_DWORD(TnValue + HalfPeriod);
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK SCHEDULER
VAR_INPUT
    E0 : BOOL;
    E1 : BOOL;
    E2 : BOOL;
    E3 : BOOL;
    T0 : TIME;
    T1 : TIME;
    T2 : TIME;
    T3 : TIME;
END_VAR
VAR_OUTPUT
    Q0 : BOOL;
    Q1 : BOOL;
    Q2 : BOOL;
    Q3 : BOOL;
END_VAR
VAR
    Init : BOOL;
    S0 : DWORD;
    S1 : DWORD;
    S2 : DWORD;
    S3 : DWORD;
    Tx : DWORD;
    C : INT;
    Armed0 : BOOL;
    Armed1 : BOOL;
    Armed2 : BOOL;
    Armed3 : BOOL;
    TxValue : UDINT;
END_VAR

Tx := T_PLC_MS();
TxValue := DWORD_TO_UDINT(Tx);

IF NOT Init THEN
    Init := TRUE;
    S0 := Tx;
    S1 := Tx;
    S2 := Tx;
    S3 := Tx;
    Armed0 := TRUE;
    Armed1 := TRUE;
    Armed2 := TRUE;
    Armed3 := TRUE;
END_IF;

Q0 := FALSE;
Q1 := FALSE;
Q2 := FALSE;
Q3 := FALSE;

CASE C OF
    INT#0:
        IF Armed0 OR TxValue - DWORD_TO_UDINT(S0) >= DWORD_TO_UDINT(TIME_TO_DWORD(T0)) THEN
            Q0 := E0;
            S0 := Tx;
            Armed0 := FALSE;
        END_IF;
        C := INT#1;
    INT#1:
        IF Armed1 OR TxValue - DWORD_TO_UDINT(S1) >= DWORD_TO_UDINT(TIME_TO_DWORD(T1)) THEN
            Q1 := E1;
            S1 := Tx;
            Armed1 := FALSE;
        END_IF;
        C := INT#2;
    INT#2:
        IF Armed2 OR TxValue - DWORD_TO_UDINT(S2) >= DWORD_TO_UDINT(TIME_TO_DWORD(T2)) THEN
            Q2 := E2;
            S2 := Tx;
            Armed2 := FALSE;
        END_IF;
        C := INT#3;
    ELSE
        IF Armed3 OR TxValue - DWORD_TO_UDINT(S3) >= DWORD_TO_UDINT(TIME_TO_DWORD(T3)) THEN
            Q3 := E3;
            S3 := Tx;
            Armed3 := FALSE;
        END_IF;
        C := INT#0;
END_CASE;
END_FUNCTION_BLOCK

FUNCTION_BLOCK SCHEDULER_2
VAR_INPUT
    E0 : BOOL;
    E1 : BOOL;
    E2 : BOOL;
    E3 : BOOL;
    C0 : UINT;
    C1 : UINT;
    C2 : UINT;
    C3 : UINT;
    O0 : UINT;
    O1 : UINT;
    O2 : UINT;
    O3 : UINT;
END_VAR
VAR_OUTPUT
    Q0 : BOOL;
    Q1 : BOOL;
    Q2 : BOOL;
    Q3 : BOOL;
END_VAR
VAR
    Sx : UINT;
END_VAR

Q0 := E0 AND C0 > UINT#0 AND (Sx MOD C0) = O0;
Q1 := E1 AND C1 > UINT#0 AND (Sx MOD C1) = O1;
Q2 := E2 AND C2 > UINT#0 AND (Sx MOD C2) = O2;
Q3 := E3 AND C3 > UINT#0 AND (Sx MOD C3) = O3;
Sx := Sx + UINT#1;
END_FUNCTION_BLOCK

FUNCTION_BLOCK SEQUENCE_4
VAR_INPUT
    IN0 : BOOL := TRUE;
    IN1 : BOOL := TRUE;
    IN2 : BOOL := TRUE;
    IN3 : BOOL := TRUE;
    START : BOOL;
    RST : BOOL;
    WAIT0 : TIME;
    DELAY0 : TIME;
    WAIT1 : TIME;
    DELAY1 : TIME;
    WAIT2 : TIME;
    DELAY2 : TIME;
    WAIT3 : TIME;
    DELAY3 : TIME;
    STOP_ON_ERROR : BOOL := FALSE;
END_VAR
VAR_OUTPUT
    Q0 : BOOL;
    Q1 : BOOL;
    Q2 : BOOL;
    Q3 : BOOL;
    QX : BOOL;
    RUN : BOOL;
    STATE : INT;
    STATUS : BYTE;
END_VAR
VAR
    Last : DWORD;
    Edge : BOOL;
    Tx : DWORD;
    Init : BOOL;
    TxValue : UDINT;
END_VAR

Tx := T_PLC_MS();
TxValue := DWORD_TO_UDINT(Tx);

IF NOT Init THEN
    Last := Tx;
    Init := TRUE;
    STATUS := BYTE#110;
END_IF;

IF RST THEN
    STATE := INT#-1;
    Q0 := FALSE;
    Q1 := FALSE;
    Q2 := FALSE;
    Q3 := FALSE;
    STATUS := BYTE#110;
    RUN := FALSE;
ELSIF START AND NOT Edge THEN
    STATE := INT#0;
    Last := Tx;
    STATUS := BYTE#111;
    Q0 := FALSE;
    Q1 := FALSE;
    Q2 := FALSE;
    Q3 := FALSE;
    RUN := TRUE;
END_IF;
Edge := START;

IF NOT (STATUS > BYTE#0 AND STATUS < BYTE#100 AND STOP_ON_ERROR) THEN
    IF RUN AND STATE = INT#0 THEN
        IF NOT Q0 AND IN0 AND TxValue - DWORD_TO_UDINT(Last) <= DWORD_TO_UDINT(TIME_TO_DWORD(WAIT0)) THEN
            Q0 := TRUE;
            Last := Tx;
        ELSIF NOT Q0 AND TxValue - DWORD_TO_UDINT(Last) > DWORD_TO_UDINT(TIME_TO_DWORD(WAIT0)) THEN
            STATUS := BYTE#1;
            RUN := FALSE;
        ELSIF Q0 AND TxValue - DWORD_TO_UDINT(Last) >= DWORD_TO_UDINT(TIME_TO_DWORD(DELAY0)) THEN
            STATE := INT#1;
            Last := Tx;
        END_IF;
    END_IF;

    IF RUN AND STATE = INT#1 THEN
        IF NOT Q1 AND IN1 AND TxValue - DWORD_TO_UDINT(Last) <= DWORD_TO_UDINT(TIME_TO_DWORD(WAIT1)) THEN
            Q0 := FALSE;
            Q1 := TRUE;
            Last := Tx;
        ELSIF NOT Q1 AND TxValue - DWORD_TO_UDINT(Last) > DWORD_TO_UDINT(TIME_TO_DWORD(WAIT1)) THEN
            STATUS := BYTE#2;
            Q0 := FALSE;
            RUN := FALSE;
        ELSIF Q1 AND TxValue - DWORD_TO_UDINT(Last) >= DWORD_TO_UDINT(TIME_TO_DWORD(DELAY1)) THEN
            STATE := INT#2;
            Last := Tx;
        END_IF;
    END_IF;

    IF RUN AND STATE = INT#2 THEN
        IF NOT Q2 AND IN2 AND TxValue - DWORD_TO_UDINT(Last) <= DWORD_TO_UDINT(TIME_TO_DWORD(WAIT2)) THEN
            Q1 := FALSE;
            Q2 := TRUE;
            Last := Tx;
        ELSIF NOT Q2 AND TxValue - DWORD_TO_UDINT(Last) > DWORD_TO_UDINT(TIME_TO_DWORD(WAIT2)) THEN
            STATUS := BYTE#3;
            Q1 := FALSE;
            RUN := FALSE;
        ELSIF Q2 AND TxValue - DWORD_TO_UDINT(Last) >= DWORD_TO_UDINT(TIME_TO_DWORD(DELAY2)) THEN
            STATE := INT#3;
            Last := Tx;
        END_IF;
    END_IF;

    IF RUN AND STATE = INT#3 THEN
        IF NOT Q3 AND IN3 AND TxValue - DWORD_TO_UDINT(Last) <= DWORD_TO_UDINT(TIME_TO_DWORD(WAIT3)) THEN
            Q2 := FALSE;
            Q3 := TRUE;
            Last := Tx;
        ELSIF NOT Q3 AND TxValue - DWORD_TO_UDINT(Last) > DWORD_TO_UDINT(TIME_TO_DWORD(WAIT3)) THEN
            STATUS := BYTE#4;
            Q2 := FALSE;
            RUN := FALSE;
        ELSIF Q3 AND TxValue - DWORD_TO_UDINT(Last) >= DWORD_TO_UDINT(TIME_TO_DWORD(DELAY3)) THEN
            STATE := INT#-1;
            Q3 := FALSE;
            RUN := FALSE;
            STATUS := BYTE#110;
        END_IF;
    END_IF;
END_IF;

QX := Q0 OR Q1 OR Q2 OR Q3;
END_FUNCTION_BLOCK

FUNCTION_BLOCK SEQUENCE_8
VAR_INPUT
    IN0 : BOOL := TRUE;
    IN1 : BOOL := TRUE;
    IN2 : BOOL := TRUE;
    IN3 : BOOL := TRUE;
    IN4 : BOOL := TRUE;
    IN5 : BOOL := TRUE;
    IN6 : BOOL := TRUE;
    IN7 : BOOL := TRUE;
    START : BOOL;
    RST : BOOL;
    WAIT0 : TIME;
    DELAY0 : TIME;
    WAIT1 : TIME;
    DELAY1 : TIME;
    WAIT2 : TIME;
    DELAY2 : TIME;
    WAIT3 : TIME;
    DELAY3 : TIME;
    WAIT4 : TIME;
    DELAY4 : TIME;
    WAIT5 : TIME;
    DELAY5 : TIME;
    WAIT6 : TIME;
    DELAY6 : TIME;
    WAIT7 : TIME;
    DELAY7 : TIME;
    STOP_ON_ERROR : BOOL := FALSE;
END_VAR
VAR_OUTPUT
    Q0 : BOOL;
    Q1 : BOOL;
    Q2 : BOOL;
    Q3 : BOOL;
    Q4 : BOOL;
    Q5 : BOOL;
    Q6 : BOOL;
    Q7 : BOOL;
    QX : BOOL;
    RUN : BOOL;
    STATE : INT;
    STATUS : BYTE;
END_VAR
VAR
    Last : DWORD;
    Edge : BOOL;
    Tx : DWORD;
    Init : BOOL;
    TxValue : UDINT;
END_VAR

Tx := T_PLC_MS();
TxValue := DWORD_TO_UDINT(Tx);

IF NOT Init THEN
    Last := Tx;
    Init := TRUE;
    STATUS := BYTE#110;
END_IF;

IF RST THEN
    STATE := INT#-1;
    Q0 := FALSE;
    Q1 := FALSE;
    Q2 := FALSE;
    Q3 := FALSE;
    Q4 := FALSE;
    Q5 := FALSE;
    Q6 := FALSE;
    Q7 := FALSE;
    STATUS := BYTE#110;
    RUN := FALSE;
ELSIF START AND NOT Edge THEN
    STATE := INT#0;
    Last := Tx;
    STATUS := BYTE#111;
    Q0 := FALSE;
    Q1 := FALSE;
    Q2 := FALSE;
    Q3 := FALSE;
    Q4 := FALSE;
    Q5 := FALSE;
    Q6 := FALSE;
    Q7 := FALSE;
    RUN := TRUE;
END_IF;
Edge := START;

IF NOT (STATUS > BYTE#0 AND STATUS < BYTE#100 AND STOP_ON_ERROR) THEN
    IF RUN AND STATE = INT#0 THEN
        IF NOT Q0 AND IN0 AND TxValue - DWORD_TO_UDINT(Last) <= DWORD_TO_UDINT(TIME_TO_DWORD(WAIT0)) THEN
            Q0 := TRUE;
            Last := Tx;
        ELSIF NOT Q0 AND TxValue - DWORD_TO_UDINT(Last) > DWORD_TO_UDINT(TIME_TO_DWORD(WAIT0)) THEN
            STATUS := BYTE#1;
            RUN := FALSE;
        ELSIF Q0 AND TxValue - DWORD_TO_UDINT(Last) >= DWORD_TO_UDINT(TIME_TO_DWORD(DELAY0)) THEN
            STATE := INT#1;
            Last := Tx;
        END_IF;
    END_IF;

    IF RUN AND STATE = INT#1 THEN
        IF NOT Q1 AND IN1 AND TxValue - DWORD_TO_UDINT(Last) <= DWORD_TO_UDINT(TIME_TO_DWORD(WAIT1)) THEN
            Q0 := FALSE;
            Q1 := TRUE;
            Last := Tx;
        ELSIF NOT Q1 AND TxValue - DWORD_TO_UDINT(Last) > DWORD_TO_UDINT(TIME_TO_DWORD(WAIT1)) THEN
            STATUS := BYTE#2;
            Q0 := FALSE;
            RUN := FALSE;
        ELSIF Q1 AND TxValue - DWORD_TO_UDINT(Last) >= DWORD_TO_UDINT(TIME_TO_DWORD(DELAY1)) THEN
            STATE := INT#2;
            Last := Tx;
        END_IF;
    END_IF;

    IF RUN AND STATE = INT#2 THEN
        IF NOT Q2 AND IN2 AND TxValue - DWORD_TO_UDINT(Last) <= DWORD_TO_UDINT(TIME_TO_DWORD(WAIT2)) THEN
            Q1 := FALSE;
            Q2 := TRUE;
            Last := Tx;
        ELSIF NOT Q2 AND TxValue - DWORD_TO_UDINT(Last) > DWORD_TO_UDINT(TIME_TO_DWORD(WAIT2)) THEN
            STATUS := BYTE#3;
            Q1 := FALSE;
            RUN := FALSE;
        ELSIF Q2 AND TxValue - DWORD_TO_UDINT(Last) >= DWORD_TO_UDINT(TIME_TO_DWORD(DELAY2)) THEN
            STATE := INT#3;
            Last := Tx;
        END_IF;
    END_IF;

    IF RUN AND STATE = INT#3 THEN
        IF NOT Q3 AND IN3 AND TxValue - DWORD_TO_UDINT(Last) <= DWORD_TO_UDINT(TIME_TO_DWORD(WAIT3)) THEN
            Q2 := FALSE;
            Q3 := TRUE;
            Last := Tx;
        ELSIF NOT Q3 AND TxValue - DWORD_TO_UDINT(Last) > DWORD_TO_UDINT(TIME_TO_DWORD(WAIT3)) THEN
            STATUS := BYTE#4;
            Q2 := FALSE;
            RUN := FALSE;
        ELSIF Q3 AND TxValue - DWORD_TO_UDINT(Last) >= DWORD_TO_UDINT(TIME_TO_DWORD(DELAY3)) THEN
            STATE := INT#4;
            Last := Tx;
        END_IF;
    END_IF;

    IF RUN AND STATE = INT#4 THEN
        IF NOT Q4 AND IN4 AND TxValue - DWORD_TO_UDINT(Last) <= DWORD_TO_UDINT(TIME_TO_DWORD(WAIT4)) THEN
            Q3 := FALSE;
            Q4 := TRUE;
            Last := Tx;
        ELSIF NOT Q4 AND TxValue - DWORD_TO_UDINT(Last) > DWORD_TO_UDINT(TIME_TO_DWORD(WAIT4)) THEN
            STATUS := BYTE#5;
            Q3 := FALSE;
            RUN := FALSE;
        ELSIF Q4 AND TxValue - DWORD_TO_UDINT(Last) >= DWORD_TO_UDINT(TIME_TO_DWORD(DELAY4)) THEN
            STATE := INT#5;
            Last := Tx;
        END_IF;
    END_IF;

    IF RUN AND STATE = INT#5 THEN
        IF NOT Q5 AND IN5 AND TxValue - DWORD_TO_UDINT(Last) <= DWORD_TO_UDINT(TIME_TO_DWORD(WAIT5)) THEN
            Q4 := FALSE;
            Q5 := TRUE;
            Last := Tx;
        ELSIF NOT Q5 AND TxValue - DWORD_TO_UDINT(Last) > DWORD_TO_UDINT(TIME_TO_DWORD(WAIT5)) THEN
            STATUS := BYTE#6;
            Q4 := FALSE;
            RUN := FALSE;
        ELSIF Q5 AND TxValue - DWORD_TO_UDINT(Last) >= DWORD_TO_UDINT(TIME_TO_DWORD(DELAY5)) THEN
            STATE := INT#6;
            Last := Tx;
        END_IF;
    END_IF;

    IF RUN AND STATE = INT#6 THEN
        IF NOT Q6 AND IN6 AND TxValue - DWORD_TO_UDINT(Last) <= DWORD_TO_UDINT(TIME_TO_DWORD(WAIT6)) THEN
            Q5 := FALSE;
            Q6 := TRUE;
            Last := Tx;
        ELSIF NOT Q6 AND TxValue - DWORD_TO_UDINT(Last) > DWORD_TO_UDINT(TIME_TO_DWORD(WAIT6)) THEN
            STATUS := BYTE#7;
            Q5 := FALSE;
            RUN := FALSE;
        ELSIF Q6 AND TxValue - DWORD_TO_UDINT(Last) >= DWORD_TO_UDINT(TIME_TO_DWORD(DELAY6)) THEN
            STATE := INT#7;
            Last := Tx;
        END_IF;
    END_IF;

    IF RUN AND STATE = INT#7 THEN
        IF NOT Q7 AND IN7 AND TxValue - DWORD_TO_UDINT(Last) <= DWORD_TO_UDINT(TIME_TO_DWORD(WAIT7)) THEN
            Q6 := FALSE;
            Q7 := TRUE;
            Last := Tx;
        ELSIF NOT Q7 AND TxValue - DWORD_TO_UDINT(Last) > DWORD_TO_UDINT(TIME_TO_DWORD(WAIT7)) THEN
            STATUS := BYTE#8;
            Q6 := FALSE;
            RUN := FALSE;
        ELSIF Q7 AND TxValue - DWORD_TO_UDINT(Last) >= DWORD_TO_UDINT(TIME_TO_DWORD(DELAY7)) THEN
            STATE := INT#-1;
            Q7 := FALSE;
            RUN := FALSE;
            STATUS := BYTE#110;
        END_IF;
    END_IF;
END_IF;

QX := Q0 OR Q1 OR Q2 OR Q3 OR Q4 OR Q5 OR Q6 OR Q7;
END_FUNCTION_BLOCK

FUNCTION_BLOCK TONOF
VAR_INPUT
    IN : BOOL;
    T_ON : TIME;
    T_OFF : TIME;
END_VAR
VAR_OUTPUT
    Q : BOOL;
END_VAR
VAR
    X : TON;
    Old : BOOL;
    Mode : BOOL;
END_VAR

IF IN XOR Old THEN
    X(IN := FALSE, PT := SEL(G := IN, IN0 := T_OFF, IN1 := T_ON));
    Mode := IN;
    Old := IN;
END_IF;
X(IN := TRUE);
IF X.Q THEN
    Q := Mode;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK TP_X
VAR_INPUT
    IN : BOOL;
    PT : TIME;
END_VAR
VAR_OUTPUT
    Q : BOOL;
    ET : TIME;
END_VAR
VAR
    Edge : BOOL;
    Start : DWORD;
    Tx : DWORD;
    TxValue : UDINT;
    StartValue : UDINT;
    PeriodValue : UDINT;
END_VAR

Tx := T_PLC_MS();
TxValue := DWORD_TO_UDINT(Tx);
StartValue := DWORD_TO_UDINT(Start);
PeriodValue := DWORD_TO_UDINT(TIME_TO_DWORD(PT));

IF IN AND NOT Edge THEN
    Start := Tx;
    ET := T#0ms;
    Q := PT > T#0ms;
ELSIF Q THEN
    ET := DWORD_TO_TIME(UDINT_TO_DWORD(TxValue - StartValue));
    IF TxValue - StartValue >= PeriodValue THEN
        Q := FALSE;
        ET := T#0ms;
    END_IF;
END_IF;
Edge := IN;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FIFO_16
VAR_INPUT
    DIN : DWORD;
    E : BOOL := TRUE;
    RD : BOOL;
    WD : BOOL;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    DOUT : DWORD;
    EMPTY : BOOL;
    FULL : BOOL;
END_VAR
VAR
    Fifo : ARRAY[0..15] OF DWORD;
    Pr : INT;
    Pw : INT;
END_VAR

IF RST THEN
    Pw := Pr;
    FULL := FALSE;
    EMPTY := TRUE;
    DOUT := DWORD#0;
ELSIF E THEN
    IF NOT EMPTY AND RD THEN
        DOUT := Fifo[Pr];
        IF Pr >= INT#15 THEN
            Pr := INT#0;
        ELSE
            Pr := Pr + INT#1;
        END_IF;
        EMPTY := Pr = Pw;
        FULL := FALSE;
    END_IF;
    IF NOT FULL AND WD THEN
        Fifo[Pw] := DIN;
        IF Pw >= INT#15 THEN
            Pw := INT#0;
        ELSE
            Pw := Pw + INT#1;
        END_IF;
        FULL := Pw = Pr;
        EMPTY := FALSE;
    END_IF;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FIFO_32
VAR_INPUT
    DIN : DWORD;
    E : BOOL := TRUE;
    RD : BOOL;
    WD : BOOL;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    DOUT : DWORD;
    EMPTY : BOOL;
    FULL : BOOL;
END_VAR
VAR
    Fifo : ARRAY[0..31] OF DWORD;
    Pr : INT;
    Pw : INT;
END_VAR

IF RST THEN
    Pw := Pr;
    FULL := FALSE;
    EMPTY := TRUE;
    DOUT := DWORD#0;
ELSIF E THEN
    IF NOT EMPTY AND RD THEN
        DOUT := Fifo[Pr];
        IF Pr >= INT#31 THEN
            Pr := INT#0;
        ELSE
            Pr := Pr + INT#1;
        END_IF;
        EMPTY := Pr = Pw;
        FULL := FALSE;
    END_IF;
    IF NOT FULL AND WD THEN
        Fifo[Pw] := DIN;
        IF Pw >= INT#31 THEN
            Pw := INT#0;
        ELSE
            Pw := Pw + INT#1;
        END_IF;
        FULL := Pw = Pr;
        EMPTY := FALSE;
    END_IF;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK STACK_16
VAR_INPUT
    DIN : DWORD;
    E : BOOL := TRUE;
    RD : BOOL;
    WD : BOOL;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    DOUT : DWORD;
    EMPTY : BOOL;
    FULL : BOOL;
END_VAR
VAR
    Stack : ARRAY[0..15] OF DWORD;
    Pt : INT;
END_VAR

IF RST THEN
    Pt := INT#0;
    EMPTY := TRUE;
    FULL := FALSE;
    DOUT := DWORD#0;
ELSIF E THEN
    IF NOT EMPTY AND RD THEN
        Pt := Pt - INT#1;
        DOUT := Stack[Pt];
        EMPTY := Pt = INT#0;
        FULL := FALSE;
    END_IF;
    IF NOT FULL AND WD THEN
        Stack[Pt] := DIN;
        Pt := Pt + INT#1;
        FULL := Pt > INT#15;
        EMPTY := FALSE;
    END_IF;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK STACK_32
VAR_INPUT
    DIN : DWORD;
    E : BOOL := TRUE;
    RD : BOOL;
    WD : BOOL;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    DOUT : DWORD;
    EMPTY : BOOL;
    FULL : BOOL;
END_VAR
VAR
    Stack : ARRAY[0..31] OF DWORD;
    Pt : INT;
END_VAR

IF RST THEN
    Pt := INT#0;
    EMPTY := TRUE;
    FULL := FALSE;
    DOUT := DWORD#0;
ELSIF E THEN
    IF NOT EMPTY AND RD THEN
        Pt := Pt - INT#1;
        DOUT := Stack[Pt];
        EMPTY := Pt = INT#0;
        FULL := FALSE;
    END_IF;
    IF NOT FULL AND WD THEN
        Stack[Pt] := DIN;
        Pt := Pt + INT#1;
        FULL := Pt > INT#31;
        EMPTY := FALSE;
    END_IF;
END_IF;
END_FUNCTION_BLOCK

FUNCTION CRC_GEN : DWORD
VAR_INPUT
    PT : POINTER TO ARRAY[*] OF BYTE;
    SIZE : INT;
    PL : INT;
    PN : DWORD;
    INIT : DWORD;
    REV_IN : BOOL;
    REV_OUT : BOOL;
    XOR_OUT : DWORD;
END_VAR
VAR
    Pos : INT;
    Shift : INT;
    Dx : BYTE;
    Bits : INT;
    Poly : DWORD;
    TopSet : BOOL;
    InputBit : BOOL;
END_VAR

Shift := INT#32 - PL;
Poly := SHL(PN, Shift);
CRC_GEN := DWORD#0;

FOR Pos := INT#0 TO INT#3 DO
    IF Pos < SIZE THEN
        Dx := PT^[Pos];
    ELSE
        Dx := BYTE#0;
    END_IF;
    IF REV_IN THEN
        Dx := REVERSE(IN := Dx);
    END_IF;
    CRC_GEN := SHL(CRC_GEN, INT#8) OR BYTE_TO_DWORD(Dx);
END_FOR;
Pos := INT#4;

CRC_GEN := CRC_GEN XOR SHL(INIT, Shift);

WHILE Pos < SIZE DO
    Dx := PT^[Pos];
    IF REV_IN THEN
        Dx := REVERSE(IN := Dx);
    END_IF;
    Pos := Pos + INT#1;

    FOR Bits := INT#0 TO INT#7 DO
        TopSet := (CRC_GEN AND DWORD#16#8000_0000) <> DWORD#0;
        InputBit := (Dx AND BYTE#16#80) <> BYTE#0;
        CRC_GEN := SHL(CRC_GEN, INT#1);
        IF InputBit THEN
            CRC_GEN := CRC_GEN OR DWORD#1;
        END_IF;
        IF TopSet THEN
            CRC_GEN := CRC_GEN XOR Poly;
        END_IF;
        Dx := SHL(Dx, INT#1);
    END_FOR;
END_WHILE;

FOR Bits := INT#0 TO INT#31 DO
    TopSet := (CRC_GEN AND DWORD#16#8000_0000) <> DWORD#0;
    CRC_GEN := SHL(CRC_GEN, INT#1);
    IF TopSet THEN
        CRC_GEN := CRC_GEN XOR Poly;
    END_IF;
END_FOR;

CRC_GEN := SHR(CRC_GEN, Shift) XOR XOR_OUT;
IF REV_OUT THEN
    CRC_GEN := REFLECT(D := CRC_GEN, L := PL);
END_IF;
END_FUNCTION

FUNCTION_BLOCK MATRIX
VAR_INPUT
    X1 : BOOL;
    X2 : BOOL;
    X3 : BOOL;
    X4 : BOOL;
    X5 : BOOL;
    Release : BOOL := FALSE;
END_VAR
VAR_OUTPUT
    CODE : BYTE;
    TP : BOOL;
    Y1 : BOOL := TRUE;
    Y2 : BOOL;
    Y3 : BOOL;
    Y4 : BOOL;
END_VAR
VAR
    Line : INT;
    X : ARRAY[0..3] OF BYTE;
    L : ARRAY[0..3] OF BYTE;
    I : INT;
    Temp : BYTE;
    Found : BOOL;
END_VAR

TP := FALSE;
CODE := BYTE#0;
Temp := BYTE#0;

IF X1 THEN
    Temp := Temp OR BYTE#1;
END_IF;
IF X2 THEN
    Temp := Temp OR BYTE#2;
END_IF;
IF X3 THEN
    Temp := Temp OR BYTE#4;
END_IF;
IF X4 THEN
    Temp := Temp OR BYTE#8;
END_IF;
IF X5 THEN
    Temp := Temp OR BYTE#16;
END_IF;
X[Line] := Temp;

Found := FALSE;
FOR I := INT#0 TO INT#3 DO
    IF NOT Found AND X[I] <> L[I] THEN
        Temp := X[I] XOR L[I];
        IF (Temp AND BYTE#1) = BYTE#1 THEN
            CODE := BYTE#1;
            IF (X[I] AND BYTE#1) = BYTE#1 THEN
                CODE := CODE OR BYTE#16#80;
                L[I] := L[I] OR BYTE#1;
            ELSE
                L[I] := L[I] AND BYTE#16#FE;
            END_IF;
        ELSIF (Temp AND BYTE#2) = BYTE#2 THEN
            CODE := BYTE#2;
            IF (X[I] AND BYTE#2) = BYTE#2 THEN
                CODE := CODE OR BYTE#16#80;
                L[I] := L[I] OR BYTE#2;
            ELSE
                L[I] := L[I] AND BYTE#16#FD;
            END_IF;
        ELSIF (Temp AND BYTE#4) = BYTE#4 THEN
            CODE := BYTE#3;
            IF (X[I] AND BYTE#4) = BYTE#4 THEN
                CODE := CODE OR BYTE#16#80;
                L[I] := L[I] OR BYTE#4;
            ELSE
                L[I] := L[I] AND BYTE#16#FB;
            END_IF;
        ELSIF (Temp AND BYTE#8) = BYTE#8 THEN
            CODE := BYTE#4;
            IF (X[I] AND BYTE#8) = BYTE#8 THEN
                CODE := CODE OR BYTE#16#80;
                L[I] := L[I] OR BYTE#8;
            ELSE
                L[I] := L[I] AND BYTE#16#F7;
            END_IF;
        ELSIF (Temp AND BYTE#16) = BYTE#16 THEN
            CODE := BYTE#5;
            IF (X[I] AND BYTE#16) = BYTE#16 THEN
                CODE := CODE OR BYTE#16#80;
                L[I] := L[I] OR BYTE#16;
            ELSE
                L[I] := L[I] AND BYTE#16#EF;
            END_IF;
        END_IF;

        TP := TRUE;
        CODE := CODE OR SHL(INT_TO_BYTE(I), INT#4);
        IF NOT Release AND (CODE AND BYTE#16#80) = BYTE#0 THEN
            CODE := BYTE#0;
            TP := FALSE;
        END_IF;
        Found := TRUE;
    END_IF;
END_FOR;

Line := (Line + INT#1) MOD INT#4;
Y1 := Line = INT#0;
Y2 := Line = INT#1;
Y3 := Line = INT#2;
Y4 := Line = INT#3;
END_FUNCTION_BLOCK

FUNCTION_BLOCK PIN_CODE
VAR_INPUT
    CB : BYTE;
    E : BOOL;
    PIN : STRING[8];
END_VAR
VAR_OUTPUT
    TP : BOOL;
END_VAR
VAR
    POS : INT := INT#1;
END_VAR

TP := FALSE;
IF E THEN
    IF CB = CODE(STR := PIN, POS := POS) THEN
        POS := POS + INT#1;
        IF POS > LEN(PIN) THEN
            TP := TRUE;
            POS := INT#1;
        END_IF;
    ELSE
        POS := INT#1;
    END_IF;
END_IF;
END_FUNCTION_BLOCK
