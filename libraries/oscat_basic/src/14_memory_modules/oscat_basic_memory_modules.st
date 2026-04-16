(* OSCAT BASIC 3.33 memory modules. *)

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
