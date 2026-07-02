(* OSCAT BASIC 3.33 latches, flip-flops, and shift-register helpers. *)

FUNCTION_BLOCK LTCH
VAR_INPUT
    DATA : BOOL;
    L : BOOL;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    Q : BOOL;
END_VAR

IF RST THEN
    Q := FALSE;
ELSIF L THEN
    Q := DATA;
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
    CNT := INT_TO_BYTE(IN := INC(BYTE_TO_INT(IN := CNT), BYTE_TO_INT(IN := StepSize), BYTE_TO_INT(IN := MX)));
ELSIF DN AND NOT LastDn THEN
    CNT := INT_TO_BYTE(IN := INC(BYTE_TO_INT(IN := CNT), -BYTE_TO_INT(IN := StepSize), BYTE_TO_INT(IN := MX)));
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
    DATA : BOOL;
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
    Q := DATA;
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
    STATE := INC(STATE, INT#1, INT#7);
ELSIF DN AND NOT LastDn THEN
    STATE := INC(STATE, INT#-1, INT#7);
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

