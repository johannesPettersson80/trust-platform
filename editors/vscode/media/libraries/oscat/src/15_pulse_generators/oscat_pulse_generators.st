(* OSCAT BASIC 3.33 pulse-generator helpers. *)

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


FUNCTION_BLOCK CLK_PRG
VAR_INPUT
    PT : TIME := T#10ms;
END_VAR
VAR_OUTPUT
    Q : BOOL;
END_VAR
VAR
    Init : BOOL;
    LastMs : DINT;
    TxMs : DINT;
END_VAR

TxMs := DWORD_TO_DINT(T_PLC_MS());

IF NOT Init THEN
    Init := TRUE;
    LastMs := TxMs - DWORD_TO_DINT(TIME_TO_DWORD(PT));
END_IF;

Q := TxMs - LastMs >= DWORD_TO_DINT(TIME_TO_DWORD(PT));
IF Q THEN
    LastMs := TxMs;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK TP_1D
VAR_INPUT
    IN : BOOL;
    PT1 : TIME;
    PTD : TIME;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    Q : BOOL;
    W : BOOL;
END_VAR
VAR
    TxMs : DINT;
    StartMs : DINT;
    Edge : BOOL;
END_VAR

TxMs := DWORD_TO_DINT(T_PLC_MS());

IF RST THEN
    Q := FALSE;
    W := FALSE;
ELSIF W THEN
    IF TxMs - StartMs >= DWORD_TO_DINT(TIME_TO_DWORD(PTD)) THEN
        W := FALSE;
    END_IF;
ELSIF IN AND NOT Edge THEN
    Q := TRUE;
    StartMs := TxMs;
ELSIF Q AND TxMs - StartMs >= DWORD_TO_DINT(TIME_TO_DWORD(PT1)) THEN
    Q := FALSE;
    W := TRUE;
    StartMs := TxMs;
END_IF;

Edge := IN;
END_FUNCTION_BLOCK
