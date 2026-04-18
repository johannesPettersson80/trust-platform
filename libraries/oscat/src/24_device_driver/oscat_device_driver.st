(* OSCAT BASIC 3.33 device-driver helpers. *)

FUNCTION_BLOCK DRIVER_1
VAR_INPUT CONSTANT
	Toggle_Mode : BOOL;
	Timeout : TIME;
END_VAR
VAR_INPUT
	SET_REQ : BOOL;
	IN : BOOL;
	RST : BOOL;
END_VAR
VAR_OUTPUT
	Q : BOOL;
END_VAR
VAR
	off : TON;
	edge: BOOL;
END_VAR
IF off.Q THEN Q := FALSE; END_IF;
IF rst THEN
	Q := FALSE;
ELSIF SET_REQ THEN
	Q := TRUE;
ELSIF IN AND NOT edge THEN
	IF toggle_mode THEN q := NOT Q; ELSE q := TRUE; END_IF;
END_IF;
edge := in;
IF timeout > t#0s THEN off(in := Q, PT := Timeout); END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK DRIVER_4
VAR_INPUT CONSTANT
	Toggle_Mode : BOOL;
	Timeout : TIME;
END_VAR
VAR_INPUT
	SET_REQ : BOOL;
	IN0, IN1, IN2, IN3 : BOOL;
	RST : BOOL;
END_VAR
VAR_OUTPUT
	Q0, Q1, Q2, Q3 : BOOL;
END_VAR
VAR
	d0, d1, d2, d3 : DRIVER_1;
END_VAR
D0(SET_REQ := SET_REQ, IN := IN0, RST := RST, Toggle_Mode := Toggle_Mode, Timeout := Timeout);
D1(SET_REQ := SET_REQ, IN := IN1, RST := RST, Toggle_Mode := Toggle_Mode, Timeout := Timeout);
D2(SET_REQ := SET_REQ, IN := IN2, RST := RST, Toggle_Mode := Toggle_Mode, Timeout := Timeout);
D3(SET_REQ := SET_REQ, IN := IN3, RST := RST, Toggle_Mode := Toggle_Mode, Timeout := Timeout);
Q0 := D0.Q;
Q1 := D1.Q;
Q2 := D2.Q;
Q3 := D3.Q;
END_FUNCTION_BLOCK

FUNCTION_BLOCK DRIVER_4C
VAR_INPUT
    IN : BOOL;
    RST : BOOL;
END_VAR
VAR_INPUT CONSTANT
    Timeout : TIME;
END_VAR
VAR_OUTPUT
    SN : INT;
    Q0 : BOOL;
    Q1 : BOOL;
    Q2 : BOOL;
    Q3 : BOOL;
END_VAR
VAR
    off : TON;
    edge : BOOL;
END_VAR
IF RST OR off.Q THEN
    SN := 0;
ELSIF IN AND NOT edge THEN
    SN := SN + 1;
    IF SN > 4 THEN
        SN := 0;
    END_IF;
END_IF;
edge := IN;
Q0 := FALSE;
Q1 := FALSE;
Q2 := FALSE;
Q3 := FALSE;
CASE SN OF
    1:
        Q0 := TRUE;
    2:
        Q0 := TRUE;
        Q1 := TRUE;
    3:
        Q0 := TRUE;
        Q1 := TRUE;
        Q2 := TRUE;
    4:
        Q0 := TRUE;
        Q1 := TRUE;
        Q2 := TRUE;
        Q3 := TRUE;
END_CASE;
IF Timeout > T#0s THEN
    off(IN := SN > 0, PT := Timeout);
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FLOW_CONTROL
VAR_INPUT
    IN : BOOL;
    REQ : BOOL;
    ENQ : BOOL;
    RST : BOOL;
END_VAR
VAR_INPUT CONSTANT
    T_AUTO : TIME := T#1h;
    T_DELAY : TIME := T#23h;
END_VAR
VAR_OUTPUT
    Q : BOOL;
    STATUS : BYTE;
END_VAR
VAR
    timer : TP_1D;
    TimerReq : BOOL;
END_VAR
STATUS := BYTE#100;
TimerReq := FALSE;
IF RST THEN
    Q := FALSE;
    STATUS := BYTE#103;
ELSIF ENQ THEN
    IF IN THEN
        STATUS := BYTE#101;
    END_IF;
    IF REQ THEN
        TimerReq := TRUE;
        STATUS := BYTE#102;
    END_IF;
END_IF;
timer(IN := TimerReq, PT1 := T_AUTO, PTD := T_DELAY, RST := RST);
Q := (IN AND ENQ) OR timer.Q;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FT_Profile
VAR_INPUT
    K : REAL := 1.0;
    O : REAL;
    M : REAL := 1.0;
    E : BOOL;
END_VAR
VAR_INPUT CONSTANT
    value_0 : REAL;
    time_1 : TIME;
    value_1 : REAL;
    time_2 : TIME;
    value_2 : REAL;
    time_3 : TIME;
    value_3 : REAL;
    time_10 : TIME;
    value_10 : REAL;
    time_11 : TIME;
    value_11 : REAL;
    time_12 : TIME;
    value_12 : REAL;
    time_13 : TIME;
    value_13 : REAL;
END_VAR
VAR_OUTPUT
    Y : REAL;
    RUN : BOOL;
    ET : TIME;
END_VAR
VAR
    tx : DWORD;
    edge : BOOL;
    state : INT;
    ta : DWORD;
    tb : DWORD;
    t0 : DWORD;
    temp : REAL;
    va : REAL;
    vb : REAL;
    ElapsedMs : UDINT;
END_VAR

tx := T_PLC_MS();
IF E AND NOT edge THEN
    RUN := TRUE;
    ET := T#0s;
    t0 := tx;
    ta := tx;
    tb := TIME_TO_DWORD(multime(time_1, M));
    va := value_0;
    vb := value_1;
    temp := value_0;
    state := 1;
END_IF;
edge := E;
IF run THEN
    ElapsedMs := DWORD_TO_UDINT(tx) - DWORD_TO_UDINT(ta);
    CASE state OF
        1:
            IF ElapsedMs >= DWORD_TO_UDINT(tb) THEN
                ta := tx;
                tb := TIME_TO_DWORD(multime(SUB_TIME(time_2, time_1), M));
                va := value_1;
                vb := value_2;
                temp := value_1;
                state := 2;
            ELSIF DWORD_TO_UDINT(tb) > UDINT#0 THEN
                temp := ((vb - va) * UDINT_TO_REAL(ElapsedMs) / DWORD_TO_REAL(tb) + va);
            ELSE
                temp := vb;
            END_IF;
        2:
            IF ElapsedMs >= DWORD_TO_UDINT(tb) THEN
                ta := tx;
                tb := TIME_TO_DWORD(multime(SUB_TIME(time_3, time_2), M));
                va := value_2;
                vb := value_3;
                temp := value_2;
                state := 3;
            ELSIF DWORD_TO_UDINT(tb) > UDINT#0 THEN
                temp := ((vb - va) * UDINT_TO_REAL(ElapsedMs) / DWORD_TO_REAL(tb) + va);
            ELSE
                temp := vb;
            END_IF;
        3:
            IF ElapsedMs >= DWORD_TO_UDINT(tb) THEN
                ta := tx;
                tb := TIME_TO_DWORD(multime(SUB_TIME(time_10, time_3), M));
                va := value_3;
                vb := value_10;
                temp := value_3;
                state := 4;
            ELSIF DWORD_TO_UDINT(tb) > UDINT#0 THEN
                temp := ((vb - va) * UDINT_TO_REAL(ElapsedMs) / DWORD_TO_REAL(tb) + va);
            ELSE
                temp := vb;
            END_IF;
        4:
            IF ElapsedMs >= DWORD_TO_UDINT(tb) THEN
                ta := tx;
                tb := TIME_TO_DWORD(multime(SUB_TIME(time_11, time_10), M));
                va := value_10;
                vb := value_11;
                temp := value_10;
                IF E THEN
                    state := 5;
                ELSE
                    state := 6;
                END_IF;
            ELSIF DWORD_TO_UDINT(tb) > UDINT#0 THEN
                temp := ((vb - va) * UDINT_TO_REAL(ElapsedMs) / DWORD_TO_REAL(tb) + va);
            ELSE
                temp := vb;
            END_IF;
        5:
            IF E THEN
                ta := tx;
            ELSE
                state := 6;
            END_IF;
        6:
            IF ElapsedMs >= DWORD_TO_UDINT(tb) THEN
                ta := tx;
                tb := TIME_TO_DWORD(multime(SUB_TIME(time_12, time_11), M));
                va := value_11;
                vb := value_12;
                temp := value_11;
                state := 7;
            ELSIF DWORD_TO_UDINT(tb) > UDINT#0 THEN
                temp := ((vb - va) * UDINT_TO_REAL(ElapsedMs) / DWORD_TO_REAL(tb) + va);
            ELSE
                temp := vb;
            END_IF;
        7:
            IF ElapsedMs >= DWORD_TO_UDINT(tb) THEN
                ta := tx;
                tb := TIME_TO_DWORD(multime(SUB_TIME(time_13, time_12), M));
                va := value_12;
                vb := value_13;
                temp := value_12;
                state := 8;
            ELSIF DWORD_TO_UDINT(tb) > UDINT#0 THEN
                temp := ((vb - va) * UDINT_TO_REAL(ElapsedMs) / DWORD_TO_REAL(tb) + va);
            ELSE
                temp := vb;
            END_IF;
        8:
            IF ElapsedMs >= DWORD_TO_UDINT(tb) THEN
                temp := value_13;
                run := FALSE;
            ELSIF DWORD_TO_UDINT(tb) > UDINT#0 THEN
                temp := ((vb - va) * UDINT_TO_REAL(ElapsedMs) / DWORD_TO_REAL(tb) + va);
            ELSE
                temp := vb;
            END_IF;
    END_CASE;
    Y := temp * K + O;
    ET := DWORD_TO_TIME(UDINT_TO_DWORD(DWORD_TO_UDINT(tx) - DWORD_TO_UDINT(t0)));
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK INC_DEC
VAR_INPUT
	CHa : BOOL;
	CHb : BOOL;
	RST : BOOL;
END_VAR
VAR_OUTPUT
	dir : BOOL;
	cnt : INT;
END_VAR
VAR
	edgea : BOOL;
	clk: BOOL;
	clka: BOOL;
	clkb: BOOL;
	edgeb: BOOL;
	axb: BOOL;
END_VAR
axb := cha XOR chb;
clka := cha XOR edgea;
edgea := cha;
clkb := chb XOR edgeb;
edgeb := chb;
clk := clka OR clkb;
IF axb AND clka THEN dir := TRUE; END_IF;
IF axb AND clkb THEN dir := FALSE; END_IF;
IF clk AND dir THEN cnt := cnt + 1; END_IF;
IF clk AND NOT dir THEN cnt := cnt -1; END_IF;
IF rst THEN cnt := 0; END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK INTERLOCK
VAR_INPUT
	I1, I2 : BOOL;
	TL : TIME;
END_VAR
VAR_OUTPUT
	Q1, Q2 : BOOL;
END_VAR
VAR
	T1, T2 : TOF;
END_VAR
T1(IN := I1, PT := TL);
T2(IN := I2, PT := TL);
Q1 := I1 AND NOT t2.Q;
Q2 := I2 AND NOT t1.Q;
END_FUNCTION_BLOCK

FUNCTION_BLOCK INTERLOCK_4
VAR_INPUT CONSTANT
END_VAR
VAR_INPUT
    I0 : BOOL;
    I1 : BOOL;
    I2 : BOOL;
    I3 : BOOL;
    E : BOOL;
    MODE : INT;
END_VAR
VAR_OUTPUT
    OUT : BYTE;
    TP : BOOL;
END_VAR
VAR
    InValue : BYTE;
    Changed : BYTE;
    Last : BYTE;
    Old : BYTE;
    LMode : INT;
END_VAR
IF E THEN
    IF MODE <> LMode THEN
        OUT := BYTE#0;
        Last := BYTE#0;
        Old := BYTE#0;
        LMode := MODE;
    END_IF;
    InValue := BYTE#0;
    IF I0 THEN InValue := InValue OR BYTE#1; END_IF;
    IF I1 THEN InValue := InValue OR BYTE#2; END_IF;
    IF I2 THEN InValue := InValue OR BYTE#4; END_IF;
    IF I3 THEN InValue := InValue OR BYTE#8; END_IF;
    IF InValue <> Last THEN
        CASE MODE OF
            0:
                OUT := InValue;
            1:
                IF (InValue AND BYTE#8) <> BYTE#0 THEN OUT := BYTE#8;
                ELSIF (InValue AND BYTE#4) <> BYTE#0 THEN OUT := BYTE#4;
                ELSIF (InValue AND BYTE#2) <> BYTE#0 THEN OUT := BYTE#2;
                ELSE OUT := InValue;
                END_IF;
            2:
                Changed := (InValue XOR Last) AND InValue;
                IF (Changed AND BYTE#8) <> BYTE#0 THEN OUT := BYTE#8;
                ELSIF (Changed AND BYTE#4) <> BYTE#0 THEN OUT := BYTE#4;
                ELSIF (Changed AND BYTE#2) <> BYTE#0 THEN OUT := BYTE#2;
                ELSE OUT := Changed;
                END_IF;
            3:
                IF (OUT AND InValue) = BYTE#0 THEN
                    IF (InValue AND BYTE#8) <> BYTE#0 THEN OUT := BYTE#8;
                    ELSIF (InValue AND BYTE#4) <> BYTE#0 THEN OUT := BYTE#4;
                    ELSIF (InValue AND BYTE#2) <> BYTE#0 THEN OUT := BYTE#2;
                    ELSE OUT := InValue;
                    END_IF;
                END_IF;
        END_CASE;
        Last := InValue;
    END_IF;
    TP := OUT <> Old;
    Old := OUT;
ELSE
    OUT := BYTE#0;
    Last := BYTE#0;
    Old := BYTE#0;
    LMode := 0;
    TP := FALSE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION MANUAL : BOOL
VAR_INPUT
	IN : BOOL;
	FORCE_ON : BOOL;
	FORCE_OFF : BOOL;
END_VAR
MANUAL := NOT FORCE_OFF AND (IN OR FORCE_ON);
END_FUNCTION

FUNCTION_BLOCK MANUAL_1
VAR_INPUT
	IN : BOOL;
	MAN : BOOL;
	M_I : BOOL;
	SET_REQ : BOOL;
	RST : BOOL;
END_VAR
VAR_OUTPUT
	Q : BOOL;
	STATUS : BYTE;
END_VAR
VAR
	S_edge : BOOL;
	reset_edge : BOOL;
	edge: BOOL;
END_VAR
IF NOT man THEN
	Q := IN;
	STATUS := BYTE#100;
	edge := FALSE;
ELSIF NOT s_edge AND SET_REQ THEN
	Q := TRUE;
	edge := TRUE;
	status := BYTE#101;
ELSIF NOT reset_edge AND RST THEN
	Q := FALSE;
	edge := TRUE;
	status := BYTE#102;
ELSIF NOT edge THEN
	Q := M_I;
	status := BYTE#103;
END_IF;
s_edge := SET_REQ;
reset_edge := RST;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MANUAL_2
VAR_INPUT
	IN : BOOL;
	ENA : BOOL;
	FORCE_ON : BOOL;
	FORCE_OFF : BOOL;
	MAN : BOOL;
END_VAR
VAR_OUTPUT
	Q : BOOL;
	STATUS : BYTE;
END_VAR
IF ena THEN
	IF NOT FORCE_ON AND NOT FORCE_OFF THEN
		Q := IN;
		STATUS := BYTE#100;
	ELSIF FORCE_ON AND NOT FORCE_OFF THEN
		Q := TRUE;
		STATUS := BYTE#101;
	ELSIF NOT FORCE_ON AND FORCE_OFF THEN
		q := FALSE;
		STATUS := BYTE#102;
	ELSE
		Q := MAN;
		STATUS := BYTE#103;
	END_IF;
ELSE
	Q := FALSE;
	STATUS := BYTE#104;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MANUAL_4
VAR_INPUT
	I0, I1, I2, I3 : BOOL;
	MAN : BOOL;
	STP : BOOL;
	M0, M1, M2, M3 : BOOL;
END_VAR
VAR_OUTPUT
	Q0, Q1, Q2, Q3 : BOOL;
	STATUS : BYTE;
END_VAR
VAR
	edge: BOOL;
	pos : INT;
	tog: BOOL;
END_VAR
IF man THEN
	IF NOT TOG THEN
		Q0 := M0;
		Q1 := M1;
		Q2 := M2;
		Q3 := M3;
		STATUS := BYTE#101;
	END_IF;
	IF STP AND NOT edge THEN
		tog := TRUE;
		CASE pos OF
			0:	Q0 := TRUE;
				Q1 := FALSE;
				Q2 := FALSE;
				Q3 := FALSE;
				STATUS := BYTE#110;
			1:	Q0 := FALSE;
				Q1 := TRUE;
				Q2 := FALSE;
				Q3 := FALSE;
				STATUS := BYTE#111;
			2:	Q0 := FALSE;
				Q1 := FALSE;
				Q2 := TRUE;
				Q3 := FALSE;
				STATUS := BYTE#112;
			3:	Q0 := FALSE;
				Q1 := FALSE;
				Q2 := FALSE;
				Q3 := TRUE;
				STATUS := BYTE#113;
		END_CASE;
		pos := INC(pos,1,3);
	END_IF;
ELSE
	Q0 := I0;
	Q1 := I1;
	Q2 := I2;
	Q3 := I3;
	STATUS := BYTE#100;
	tog := FALSE;
	pos := 0;
END_IF;
edge := STP;
END_FUNCTION_BLOCK

FUNCTION_BLOCK PARSET
VAR_INPUT
    A0, A1 : BOOL;
END_VAR
VAR_INPUT CONSTANT
    X01, X02, X03, X04 : REAL;
    X11, X12, X13, X14 : REAL;
    X21, X22, X23, X24 : REAL;
    X31, X32, X33, X34 : REAL;
    TC : TIME;
END_VAR
VAR_OUTPUT
    P1, P2, P3, P4 : REAL;
END_VAR
VAR
    X : ARRAY[0..3,1..4] OF REAL;
    S1, S2, S3, S4 : REAL;
    tx, last : DWORD;
    start : BOOL;
    SetIndex : INT;
    LastA0 : BOOL;
    LastA1 : BOOL;
    init : BOOL;
    DurationMs : UDINT;
    ElapsedMs : UDINT;
    RemainingMs : UDINT;
END_VAR

tx := T_PLC_MS();
DurationMs := DWORD_TO_UDINT(TIME_TO_DWORD(TC));
IF NOT init THEN
    LastA0 := NOT A0;
    LastA1 := A1;
    init := TRUE;
    X[0,1] := X01;
    X[0,2] := X02;
    X[0,3] := X03;
    X[0,4] := X04;
    X[1,1] := X11;
    X[1,2] := X12;
    X[1,3] := X13;
    X[1,4] := X14;
    X[2,1] := X21;
    X[2,2] := X22;
    X[2,3] := X23;
    X[2,4] := X24;
    X[3,1] := X31;
    X[3,2] := X32;
    X[3,3] := X33;
    X[3,4] := X34;
    SetIndex := 0;
    P1 := X01;
    P2 := X02;
    P3 := X03;
    P4 := X04;
END_IF;
IF (A0 <> LastA0) OR (A1 <> LastA1) THEN
    LastA0 := A0;
    LastA1 := A1;
    SetIndex := BOOL_TO_INT(A0) + BOOL_TO_INT(A1) * 2;
    IF DurationMs > UDINT#0 THEN
        start := TRUE;
        last := tx;
        S1 := (X[SetIndex,1] - P1) / UDINT_TO_REAL(DurationMs);
        S2 := (X[SetIndex,2] - P2) / UDINT_TO_REAL(DurationMs);
        S3 := (X[SetIndex,3] - P3) / UDINT_TO_REAL(DurationMs);
        S4 := (X[SetIndex,4] - P4) / UDINT_TO_REAL(DurationMs);
    ELSE
        start := FALSE;
        P1 := X[SetIndex,1];
        P2 := X[SetIndex,2];
        P3 := X[SetIndex,3];
        P4 := X[SetIndex,4];
    END_IF;
ELSIF start THEN
    ElapsedMs := DWORD_TO_UDINT(tx) - DWORD_TO_UDINT(last);
    IF ElapsedMs < DurationMs THEN
        RemainingMs := DurationMs - ElapsedMs;
        P1 := X[SetIndex,1] - S1 * UDINT_TO_REAL(RemainingMs);
        P2 := X[SetIndex,2] - S2 * UDINT_TO_REAL(RemainingMs);
        P3 := X[SetIndex,3] - S3 * UDINT_TO_REAL(RemainingMs);
        P4 := X[SetIndex,4] - S4 * UDINT_TO_REAL(RemainingMs);
    ELSE
        start := FALSE;
        P1 := X[SetIndex,1];
        P2 := X[SetIndex,2];
        P3 := X[SetIndex,3];
        P4 := X[SetIndex,4];
    END_IF;
ELSE
    P1 := X[SetIndex,1];
    P2 := X[SetIndex,2];
    P3 := X[SetIndex,3];
    P4 := X[SetIndex,4];
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK PARSET2
VAR_INPUT
	X : REAL;
END_VAR
VAR_INPUT CONSTANT
	X01, X02, X03, X04 : REAL;
	X11, X12, X13, X14 : REAL;
	X21, X22, X23, X24 : REAL;
	X31, X32, X33, X34 : REAL;
	L1, L2, L3 : REAL;
	TC : TIME;
END_VAR
VAR_OUTPUT
	P1,P2,P3,P4 : REAL;
END_VAR
VAR
	Pset : parset;
	init: BOOL;
END_VAR
IF NOT init THEN
	init := TRUE;
	pset(TC:=TC, X01:=X01, X02:=X02, X03:=X03, X04:=X04, X11:=X11, X12:=X12, X13:=X13, X14:=X14, X21:=X21, X22:=X22, X23:=X23, X24:=X24, X31:=X31, X32:=X32, X33:=X33, X34:=X34);
END_IF;
IF ABS(X) < L1 THEN
	pset(A0 := FALSE, A1 := FALSE);
ELSIF ABS(X) < L2 THEN
	pset(A0 := TRUE, A1 := FALSE);
ELSIF ABS(x) < L3 THEN
	pset(A0 := FALSE, A1 := TRUE);
ELSE
	pset(A0 := TRUE, A1 := TRUE);
END_IF;
P1 := pset.P1;
P2 := pset.P2;
P3 := pset.P3;
P4 := pset.P4;
END_FUNCTION_BLOCK

FUNCTION_BLOCK SIGNAL
VAR_INPUT
    IN : BOOL;
    SIG : BYTE;
    TS : TIME;
END_VAR
VAR_OUTPUT
    Q : BOOL;
END_VAR
VAR
    tx : DWORD;
    pattern_step : BYTE;
    step_index : UDINT;
    pattern_mask : BYTE;
END_VAR
VAR CONSTANT
    one : BYTE := 1;
END_VAR
IF in THEN
    tx := T_PLC_MS();
    IF ts > t#0s AND TIME_TO_DWORD(ts) > DWORD#0 THEN
        step_index := DWORD_TO_UDINT(tx) / DWORD_TO_UDINT(TIME_TO_DWORD(ts));
    ELSE
        step_index := DWORD_TO_UDINT(SHR(tx, 7));
    END_IF;
    pattern_step := DWORD_TO_BYTE(UDINT_TO_DWORD(step_index MOD UDINT#8));
    pattern_mask := SHL(one, BYTE_TO_INT(pattern_step));
    Q := (pattern_mask AND sig) <> BYTE#0;
ELSE
    Q := FALSE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK SIGNAL_4
VAR_INPUT
	IN1 : BOOL;
	IN2 : BOOL;
	IN3 : BOOL;
	IN4 : BOOL;
	TS : TIME;
END_VAR
VAR_INPUT CONSTANT
	S1 : BYTE := 2#1111_1111;
	S2 : BYTE := 2#1111_0000;
	S3 : BYTE := 2#1010_1010;
	S4 : BYTE := 2#1010_0000;
END_VAR
VAR_OUTPUT
	Q : BOOL;
END_VAR
VAR
	sig : SIGNAL;
END_VAR
IF in1 THEN
	sig(in := TRUE, sig := s1, TS := TS);
ELSIF in2 THEN
	sig(in := TRUE, sig := s2, TS := TS);
ELSIF in3 THEN
	sig(in := TRUE, sig := s3, TS := TS);
ELSIF in4 THEN
	sig(in := TRUE, sig := s4, TS := TS);
ELSE
	sig(in := FALSE);
END_IF;
Q := sig.Q;
END_FUNCTION_BLOCK

FUNCTION_BLOCK SRAMP
VAR_INPUT
	X : REAL;
	A_UP : REAL;
	A_DN :REAL;
	VU_MAX : REAL;
	VD_MAX : REAL;
	LIMIT_HIGH : REAL;
	LIMIT_LOW : REAL;
	RST : BOOL;
END_VAR
VAR_OUTPUT
	Y : REAL;
	V : REAL;
END_VAR
VAR
	cycle_time : TC_S;
	init: BOOL;
	AUpLimited : REAL;
	ADnLimited : REAL;
	VUpMaxLimited : REAL;
	VDnMaxLimited : REAL;
END_VAR
cycle_time();
AUpLimited := MAX(0.0, A_UP);
ADnLimited := MIN(0.0, A_DN);
VUpMaxLimited := MAX(0.0, VU_MAX);
VDnMaxLimited := MIN(0.0, VD_MAX);
IF rst OR NOT init THEN
	init := TRUE;
	Y := 0.0;
	V := 0.0;
ELSIF X = Y THEN
	v := 0.0;
ELSIF X > Y THEN
	v := MIN(v + AUpLimited * cycle_time.TC, VUpMaxLimited);
	v := MIN(SQRT((Y-X) * 2.0 * ADnLimited), v);
	y := LIMIT(limit_low, y + MIN(v * cycle_time.TC, X-Y), limit_high);
ELSIF X < Y THEN
	v := MAX(v + ADnLimited * cycle_time.TC, VDnMaxLimited);
	v := MAX(-SQRT((Y-X) * 2.0 * AUpLimited), v);
	y := LIMIT(limit_low, y + MAX(v * cycle_time.TC, X-Y), limit_high);
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK TUNE
VAR_INPUT
    SET_REQ : BOOL;
    SU, SD : BOOL;
    RST : BOOL;
END_VAR
VAR_INPUT CONSTANT
    SS : REAL := 0.1;
    Limit_L : REAL;
    LIMIT_H : REAL := 100.0;
    RST_val : REAL;
    SET_val : REAL := 100.0;
    T1 : TIME := T#500ms;
    T2 : TIME := T#2s;
    S1 : REAL := 2.0;
    S2 : REAL := 10.0;
END_VAR
VAR_OUTPUT
    Y : REAL;
END_VAR
VAR
    tx : DWORD;
    start, start2 : DWORD;
    state : INT;
    in : BOOL;
    STEP_DELTA : REAL;
    SPEED : REAL;
    Y_start : REAL;
    Y_start2 : REAL;
    ElapsedMs : UDINT;
    T1Ms : UDINT;
    T2Ms : UDINT;
END_VAR

tx := T_PLC_MS();
T1Ms := DWORD_TO_UDINT(TIME_TO_DWORD(T1));
T2Ms := DWORD_TO_UDINT(TIME_TO_DWORD(T2));
IF rst THEN
    Y := RST_val;
    state := 0;
ELSIF SET_REQ THEN
    Y := SET_val;
    state := 0;
ELSIF state > 0 THEN
    IF state = 1 THEN
        in := su;
    ELSE
        in := sd;
    END_IF;
    ElapsedMs := DWORD_TO_UDINT(tx) - DWORD_TO_UDINT(start);
    IF NOT in AND ElapsedMs <= T1Ms THEN
        Y := Y_start + STEP_DELTA;
        state := 0;
    ELSIF in AND ElapsedMs >= T2Ms THEN
        Y := Y_start2 + UDINT_TO_REAL(DWORD_TO_UDINT(tx) - DWORD_TO_UDINT(start2)) * s2 / speed;
    ELSIF in AND ElapsedMs >= T1Ms THEN
        Y := Y_start + UDINT_TO_REAL(ElapsedMs - T1Ms) * S1 / speed;
        start2 := tx;
        Y_start2 := Y;
    ELSIF NOT in THEN
        state := 0;
    END_IF;
ELSIF su THEN
    state := 1;
    start := tx;
    STEP_DELTA := ss;
    speed := 1000.0;
    Y_start := Y;
ELSIF sd THEN
    state := 2;
    start := tx;
    STEP_DELTA := -ss;
    speed := -1000.0;
    Y_start := Y;
END_IF;
Y := LIMIT(LIMIT_L, Y, LIMIT_H);
END_FUNCTION_BLOCK

FUNCTION_BLOCK TUNE2
VAR_INPUT
    SET_REQ : BOOL;
    SU, SD : BOOL;
    FU, FD : BOOL;
    RST : BOOL;
END_VAR
VAR_INPUT CONSTANT
    SS : REAL := 0.1;
    FS : REAL := 5.0;
    Limit_L : REAL;
    LIMIT_H : REAL := 100.0;
    RST_val : REAL;
    SET_val : REAL := 100.0;
    TR : TIME := T#500ms;
    S1 : REAL := 2.0;
    S2 : REAL := 10.0;
END_VAR
VAR_OUTPUT
    Y : REAL;
END_VAR
VAR
    tx : DWORD;
    start : DWORD;
    state : INT;
    in : BOOL;
    STEP_DELTA : REAL;
    SPEED : REAL;
    Y_start : REAL;
    ElapsedMs : UDINT;
    TRMs : UDINT;
END_VAR

tx := T_PLC_MS();
TRMs := DWORD_TO_UDINT(TIME_TO_DWORD(TR));
IF rst THEN
    Y := RST_val;
    state := 0;
ELSIF SET_REQ THEN
    Y := SET_val;
    state := 0;
ELSIF state > 0 THEN
    CASE state OF
        1 :
            in := su;
        2 :
            in := sd;
        3 :
            in := fu;
        4 :
            in := fd;
    END_CASE;
    ElapsedMs := DWORD_TO_UDINT(tx) - DWORD_TO_UDINT(start);
    IF NOT in AND ElapsedMs <= TRMs THEN
        Y := Y_start + STEP_DELTA;
        state := 0;
    ELSIF in AND ElapsedMs >= TRMs THEN
        Y := Y_start + UDINT_TO_REAL(ElapsedMs - TRMs) * speed;
    ELSIF NOT in THEN
        state := 0;
    END_IF;
ELSIF su THEN
    state := 1;
    start := tx;
    STEP_DELTA := ss;
    speed := s1 * 1.0E-3;
    Y_start := Y;
ELSIF sd THEN
    state := 2;
    start := tx;
    STEP_DELTA := -ss;
    speed := -s1 * 1.0E-3;
    Y_start := Y;
ELSIF fu THEN
    state := 3;
    start := tx;
    STEP_DELTA := fs;
    speed := s2 * 1.0E-3;
    Y_start := Y;
ELSIF fd THEN
    state := 4;
    start := tx;
    STEP_DELTA := -fs;
    speed := -s2 * 1.0E-3;
    Y_start := Y;
END_IF;
Y := LIMIT(LIMIT_L, Y, LIMIT_H);
END_FUNCTION_BLOCK
