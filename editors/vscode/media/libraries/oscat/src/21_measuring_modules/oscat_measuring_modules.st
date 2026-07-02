(* OSCAT BASIC 3.33 measuring-module helpers. *)

FUNCTION T_PLC_MS : DWORD
VAR
    tx : TIME;
END_VAR
tx := TIME();
T_PLC_MS := TIME_TO_DWORD(tx);
END_FUNCTION

FUNCTION T_PLC_US : DWORD
VAR
    tx : TIME;
END_VAR
tx := TIME();
T_PLC_US := UDINT_TO_DWORD(DWORD_TO_UDINT(TIME_TO_DWORD(tx)) * UDINT#1000);
END_FUNCTION

FUNCTION_BLOCK TC_MS
VAR_OUTPUT
    TC : DWORD;
END_VAR
VAR
    Init : BOOL;
    Tx : DINT;
    Last : DINT;
END_VAR

Tx := DWORD_TO_DINT(T_PLC_MS());
IF NOT Init THEN
    Init := TRUE;
    TC := DWORD#0;
ELSE
    TC := DINT_TO_DWORD(Tx - Last);
END_IF;
Last := Tx;
END_FUNCTION_BLOCK

FUNCTION_BLOCK TC_S
VAR_OUTPUT
    TC : REAL;
END_VAR
VAR
    Init : BOOL;
    Tx : DINT;
    Last : DINT;
END_VAR

Tx := DWORD_TO_DINT(T_PLC_US());
IF NOT Init THEN
    Init := TRUE;
    TC := REAL#0.0;
ELSE
    TC := DINT_TO_REAL(Tx - Last) * 1.0E-6;
END_IF;
Last := Tx;
END_FUNCTION_BLOCK

FUNCTION_BLOCK TC_US
VAR_OUTPUT
    TC : DWORD;
END_VAR
VAR
    Init : BOOL;
    Tx : DINT;
    Last : DINT;
END_VAR

Tx := DWORD_TO_DINT(T_PLC_US());
IF NOT Init THEN
    Init := TRUE;
    TC := DWORD#0;
ELSE
    TC := DINT_TO_DWORD(Tx - Last);
END_IF;
Last := Tx;
END_FUNCTION_BLOCK

FUNCTION_BLOCK ONTIME
VAR_INPUT
	IN : BOOL;
	RST : BOOL;
END_VAR
VAR_IN_OUT
	SECONDS : UDINT;
	CYCLES : UDINT;
END_VAR
VAR
	tx: DINT;
	last : DINT;
	edge : BOOL;
	init: BOOL;
	ms: DINT;
END_VAR
tx := DWORD_TO_DINT(T_PLC_MS());
IF NOT init THEN
	init := TRUE;
	last := tx;
	ms := 0;
END_IF;
IF RST THEN
	SECONDS := 0;
	CYCLES := 0;
	last := tx;
	ms := 0;
ELSIF IN THEN
	ms := (tx - last) + ms;
	IF ms >= 1000 THEN
		seconds := seconds + 1;
		ms := ms - DINT#1000;
	END_IF;
	cycles := cycles + BOOL_TO_UINT(NOT edge);
END_IF;
last := tx;
edge := in;
END_FUNCTION_BLOCK

FUNCTION_BLOCK ALARM_2
VAR_INPUT
    X : REAL;
    LO_1 : REAL;
    HI_1 : REAL;
    LO_2 : REAL;
    HI_2 : REAL;
    HYS : REAL;
END_VAR
VAR_OUTPUT
    Q1_LO : BOOL;
    Q1_HI : BOOL;
    Q2_LO : BOOL;
    Q2_HI : BOOL;
END_VAR
VAR
    tmp : REAL;
END_VAR

tmp := X - HYS * 0.5;
IF tmp > LO_1 THEN Q1_LO := FALSE; END_IF;
IF tmp > LO_2 THEN Q2_LO := FALSE; END_IF;
IF tmp > HI_1 THEN Q1_HI := TRUE; END_IF;
IF tmp > HI_2 THEN Q2_HI := TRUE; END_IF;
tmp := tmp + HYS;
IF tmp < LO_1 THEN Q1_LO := TRUE; END_IF;
IF tmp < LO_2 THEN Q2_LO := TRUE; END_IF;
IF tmp < HI_1 THEN Q1_HI := FALSE; END_IF;
IF tmp < HI_2 THEN Q2_HI := FALSE; END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK BAR_GRAPH
VAR_INPUT
    X : REAL;
    rst : BOOL;
END_VAR
VAR_INPUT CONSTANT
    trigger_Low : REAL;
    trigger_High : REAL;
    Alarm_low : BOOL;
    Alarm_high : BOOL;
    log_scale : BOOL;
END_VAR
VAR_OUTPUT
    LOW : BOOL;
    Q1 : BOOL;
    Q2 : BOOL;
    Q3 : BOOL;
    Q4 : BOOL;
    Q5 : BOOL;
    Q6 : BOOL;
    HIGH : BOOL;
    Alarm : BOOL;
    Status : BYTE;
END_VAR
VAR
    init : BOOL;
    T1 : REAL;
    T2 : REAL;
    T3 : REAL;
    T4 : REAL;
    T5 : REAL;
    temp : REAL;
END_VAR

IF NOT init THEN
    init := TRUE;
    IF log_scale THEN
        temp := EXP(LN(trigger_High / trigger_Low) * 0.16666666666666667);
        T1 := trigger_Low * temp;
        T2 := T1 * temp;
        T3 := T2 * temp;
        T4 := T3 * temp;
        T5 := T4 * temp;
    ELSE
        temp := (trigger_High - trigger_Low) * 0.142857142;
        T1 := trigger_Low + temp;
        T2 := T1 + temp;
        T3 := T2 + temp;
        T4 := T3 + temp;
        T5 := T4 + temp;
    END_IF;
END_IF;

Q1 := FALSE;
Q2 := FALSE;
Q3 := FALSE;
Q4 := FALSE;
Q5 := FALSE;
Q6 := FALSE;
Status := BYTE#110;

IF NOT Alarm_low THEN LOW := FALSE; END_IF;
IF NOT Alarm_high THEN HIGH := FALSE; END_IF;
IF rst THEN
    Alarm := FALSE;
    LOW := FALSE;
    HIGH := FALSE;
END_IF;

IF X < trigger_Low THEN
    LOW := TRUE;
    Status := BYTE#111;
    IF Alarm_low THEN
        Alarm := TRUE;
        Status := BYTE#1;
    END_IF;
ELSIF X < T1 THEN
    Q1 := TRUE;
ELSIF X < T2 THEN
    Q2 := TRUE;
ELSIF X < T3 THEN
    Q3 := TRUE;
ELSIF X < T4 THEN
    Q4 := TRUE;
ELSIF X < T5 THEN
    Q5 := TRUE;
ELSIF X < trigger_High THEN
    Q6 := TRUE;
ELSE
    HIGH := TRUE;
    Status := BYTE#112;
    IF Alarm_high THEN
        Alarm := TRUE;
        Status := BYTE#2;
    END_IF;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK CALIBRATE
VAR_INPUT
    X : REAL;
    CO : BOOL;
    CS : BOOL;
END_VAR
VAR_INPUT CONSTANT
    Y_Offset : REAL;
    Y_Scale : REAL;
END_VAR
VAR_OUTPUT
    Y : REAL;
END_VAR
VAR RETAIN
    offset : REAL;
    Scale : REAL := 1.0;
END_VAR

IF CO THEN
    offset := Y_Offset - X;
ELSIF CS THEN
    Scale := Y_Scale / (X + offset);
END_IF;
Y := (X + offset) * Scale;
END_FUNCTION_BLOCK

FUNCTION_BLOCK CYCLE_TIME
VAR_INPUT
    RST : BOOL;
END_VAR
VAR_OUTPUT
    ct_min : TIME;
    ct_max : TIME;
    ct_last : TIME;
    systime : TIME;
    sysdays : INT;
    cycles : DWORD;
END_VAR
VAR
    last_cycle : TIME;
    tx : TIME;
    init : BOOL;
END_VAR

tx := SUB_TIME(DWORD_TO_TIME(T_PLC_MS()), last_cycle);
IF RST THEN
    ct_min := T#10h;
    ct_max := T#0ms;
    cycles := DWORD#0;
ELSIF last_cycle > T#0s THEN
    IF tx < ct_min THEN
        ct_min := tx;
    ELSIF tx > ct_max THEN
        ct_max := tx;
    END_IF;
    ct_last := tx;
ELSIF ct_min = T#0s THEN
    ct_min := SUB_TIME(T#0s, T#1ms);
END_IF;
IF init THEN
    systime := ADD_TIME(systime, tx);
    IF systime >= T#1d THEN
        systime := SUB_TIME(systime, T#1d);
        sysdays := sysdays + 1;
    END_IF;
END_IF;
init := TRUE;
last_cycle := ADD_TIME(last_cycle, tx);
cycles := UDINT_TO_DWORD(DWORD_TO_UDINT(cycles) + UDINT#1);
END_FUNCTION_BLOCK

FUNCTION_BLOCK DT_SIMU
VAR_INPUT
    START : DT;
    SPEED : REAL := 1.0;
END_VAR
VAR_OUTPUT
    DTS : DT;
END_VAR
VAR
    tc : DWORD;
    init : BOOL;
    last : DWORD;
    tx : DWORD;
    td : DWORD;
END_VAR

tx := T_PLC_MS();
tc := REAL_TO_DWORD(UDINT_TO_REAL(DWORD_TO_UDINT(tx) - DWORD_TO_UDINT(last)) * SPEED);

IF NOT init THEN
    init := TRUE;
    DTS := START;
    tc := DWORD#0;
    last := tx;
ELSIF SPEED = REAL#0.0 THEN
    DTS := ADD_DT_TIME(DTS, T#1s);
ELSIF tc >= DWORD#1000 THEN
    td := UDINT_TO_DWORD((DWORD_TO_UDINT(tc) / UDINT#1000) * UDINT#1000);
    DTS := ADD_DT_TIME(DTS, DWORD_TO_TIME(td));
    last := UDINT_TO_DWORD(DWORD_TO_UDINT(last) + DWORD_TO_UDINT(REAL_TO_DWORD(DWORD_TO_REAL(td) / SPEED)));
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FLOW_METER
VAR_INPUT
    VX : REAL;
    E : BOOL;
    RST : BOOL;
END_VAR
VAR_INPUT CONSTANT
    PULSE_MODE : BOOL;
    UPDATE_TIME : TIME := T#1s;
END_VAR
VAR_OUTPUT
    F : REAL;
END_VAR
VAR_IN_OUT
    X : REAL;
    Y : UDINT;
END_VAR
VAR
    tx : TIME;
    tl : TIME;
    int1 : INTEGRATE;
    init : BOOL;
    e_last : BOOL;
    tmp : INT;
    x_last : REAL;
    y_last : UDINT;
END_VAR

IF NOT init THEN
    init := TRUE;
    tl := tx;
    x_last := X;
    y_last := Y;
END_IF;

int1(E := NOT (RST OR PULSE_MODE) AND E, X := VX, K := 2.7777777777777777E-4, Y := X);

IF RST THEN
    X := REAL#0.0;
    Y := UDINT#0;
    tl := tx;
    x_last := REAL#0.0;
    y_last := UDINT#0;
ELSIF E AND PULSE_MODE THEN
    IF NOT e_last THEN
        X := X + VX;
    END_IF;
END_IF;
e_last := E;

IF X > REAL#1.0 THEN
    tmp := FLOOR(X);
    Y := Y + INT_TO_UDINT(tmp);
    X := X - INT_TO_REAL(tmp);
END_IF;

tx := DWORD_TO_TIME(T_PLC_MS());
IF SUB_TIME(tx, tl) >= UPDATE_TIME AND UPDATE_TIME > T#0s THEN
    F := (UDINT_TO_REAL(Y - y_last) + X - x_last) / DWORD_TO_REAL(TIME_TO_DWORD(SUB_TIME(tx, tl))) * 3.6E6;
    y_last := Y;
    x_last := X;
    tl := tx;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK M_D
VAR_INPUT
    start : BOOL;
    stop : BOOL;
    tmax : TIME := T#10d;
    rst : BOOL;
END_VAR
VAR_OUTPUT
    PT : TIME;
    ET : TIME;
    run : BOOL;
END_VAR
VAR
    edge : BOOL;
    T0 : TIME;
    tx : TIME;
    startup : BOOL;
END_VAR

IF rst OR ET >= tmax THEN
    PT := T#0ms;
    ET := T#0ms;
    startup := FALSE;
    run := FALSE;
END_IF;

IF NOT startup THEN
    edge := start;
    startup := TRUE;
END_IF;

tx := DWORD_TO_TIME(T_PLC_MS());

IF start AND NOT edge AND NOT stop THEN
    T0 := tx;
    run := TRUE;
    PT := T#0s;
ELSIF stop AND run THEN
    PT := ET;
    run := FALSE;
END_IF;
edge := start;
IF run THEN
    ET := SUB_TIME(tx, T0);
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK M_T
VAR_INPUT
    IN : BOOL;
    TMAX : TIME := T#10d;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    PT : TIME;
    ET : TIME;
END_VAR
VAR
    edge : BOOL;
    start : TIME;
    tx : TIME;
END_VAR

tx := DWORD_TO_TIME(T_PLC_MS());

IF RST OR ET >= TMAX THEN
    PT := T#0s;
    ET := T#0s;
ELSIF IN THEN
    IF NOT edge THEN
        start := tx;
    END_IF;
    ET := SUB_TIME(tx, start);
ELSE
    PT := ET;
END_IF;
edge := IN;
END_FUNCTION_BLOCK

FUNCTION_BLOCK M_TX
VAR_INPUT
    IN : BOOL;
    tmax : TIME := T#10d;
    rst : BOOL;
END_VAR
VAR_OUTPUT
    TH : TIME;
    TL : TIME;
    DC : REAL;
    F : REAL;
    ET : TIME;
END_VAR
VAR
    edge : BOOL;
    start : TIME;
    stop : TIME;
    tx : TIME;
    rise : BOOL;
    fall : BOOL;
    startup : BOOL;
END_VAR

IF rst OR ET >= tmax THEN
    rise := FALSE;
    fall := FALSE;
    startup := FALSE;
    TH := T#0ms;
    TL := T#0ms;
    DC := REAL#0.0;
    F := REAL#0.0;
    ET := T#0s;
END_IF;

IF NOT startup THEN
    edge := IN;
    startup := TRUE;
END_IF;

tx := DWORD_TO_TIME(T_PLC_MS());

IF IN XOR edge THEN
    edge := IN;
    IF IN THEN
        start := tx;
        rise := TRUE;
        IF fall THEN
            TL := SUB_TIME(start, stop);
        END_IF;
        IF TH > T#0ms AND TL > T#0ms THEN
            DC := DWORD_TO_REAL(TIME_TO_DWORD(TH)) / DWORD_TO_REAL(TIME_TO_DWORD(ADD_TIME(TH, TL)));
            F := REAL#1000.0 / DWORD_TO_REAL(TIME_TO_DWORD(ADD_TIME(TH, TL)));
        END_IF;
    ELSE
        stop := tx;
        fall := TRUE;
        IF rise THEN
            TH := SUB_TIME(stop, start);
        END_IF;
        IF TH > T#0ms AND TL > T#0ms THEN
            DC := DWORD_TO_REAL(TIME_TO_DWORD(TH)) / DWORD_TO_REAL(TIME_TO_DWORD(ADD_TIME(TH, TL)));
            F := REAL#1000.0 / DWORD_TO_REAL(TIME_TO_DWORD(ADD_TIME(TH, TL)));
        END_IF;
    END_IF;
END_IF;
IF rise THEN
    ET := SUB_TIME(tx, start);
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK METER
VAR_INPUT
    M1 : REAL;
    M2 : REAL;
    I1 : BOOL;
    I2 : BOOL;
    D : REAL := 1.0;
    RST : BOOL;
END_VAR
VAR_IN_OUT
    MX : REAL;
END_VAR
VAR
    MR : REAL2;
    MX1 : REAL;
    MX2 : REAL;
    tx : DWORD;
    last : DWORD;
    tc : REAL;
    init : BOOL;
END_VAR

tx := T_PLC_MS();
IF NOT init THEN
    init := TRUE;
    last := tx;
    MR.RX := MX;
    MR.R1 := REAL#0.0;
ELSIF tx <> last THEN
    tc := UDINT_TO_REAL(DWORD_TO_UDINT(tx) - DWORD_TO_UDINT(last)) * 0.001;
    last := tx;

    IF RST THEN
        MR.R1 := REAL#0.0;
        MR.RX := REAL#0.0;
        MX := REAL#0.0;
    ELSE
        IF I1 THEN
            MX1 := M1;
        ELSE
            MX1 := REAL#0.0;
        END_IF;
        IF I2 THEN
            MX2 := M2;
        ELSE
            MX2 := REAL#0.0;
        END_IF;
        MR := R2_ADD(MR, (SEL(I1, REAL#0.0, MX1) + SEL(I2, REAL#0.0, MX2)) / D * tc);
        MX := MR.RX;
    END_IF;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK METER_STAT
VAR_INPUT
    IN : REAL;
    DI : DATE;
    RST : BOOL;
END_VAR
VAR_IN_OUT
    Last_Day : REAL;
    Current_Day : REAL;
    Last_Week : REAL;
    Current_Week : REAL;
    Last_Month : REAL;
    Current_Month : REAL;
    Last_Year : REAL;
    Current_Year : REAL;
END_VAR
VAR RETAIN
    Year_Start : REAL;
    Month_Start : REAL;
    Week_Start : REAL;
    Day_Start : REAL;
    last_run : DATE;
END_VAR

IF RST THEN
    Last_Day := REAL#0.0;
    Current_Day := REAL#0.0;
    Day_Start := IN;
    Last_Week := REAL#0.0;
    Current_Week := REAL#0.0;
    Week_Start := IN;
    Last_Month := REAL#0.0;
    Current_Month := REAL#0.0;
    Month_Start := IN;
    Last_Year := REAL#0.0;
    Current_Year := REAL#0.0;
    Year_Start := IN;
ELSE
    Current_Day := IN - Day_Start;
    Current_Week := IN - Week_Start;
    Current_Month := IN - Month_Start;
    Current_Year := IN - Year_Start;
END_IF;

IF YEAR_OF_DATE(DI) > YEAR_OF_DATE(last_run) THEN
    Last_Year := Current_Year;
    Current_Year := REAL#0.0;
    Year_Start := IN;
    Last_Month := Current_Month;
    Current_Month := REAL#0.0;
    Month_Start := IN;
    Last_Day := Current_Day;
    Current_Day := REAL#0.0;
    Day_Start := IN;
ELSIF MONTH_OF_DATE(DI) > MONTH_OF_DATE(last_run) THEN
    Last_Month := Current_Month;
    Current_Month := REAL#0.0;
    Month_Start := IN;
    Last_Day := Current_Day;
    Current_Day := REAL#0.0;
    Day_Start := IN;
ELSIF DAY_OF_YEAR(DI) > DAY_OF_YEAR(last_run) THEN
    Last_Day := Current_Day;
    Current_Day := REAL#0.0;
    Day_Start := IN;
END_IF;
IF DAY_OF_WEEK(DI) < DAY_OF_WEEK(last_run) THEN
    Last_Week := Current_Week;
    Current_Week := REAL#0.0;
    Week_Start := IN;
END_IF;
last_run := DI;
END_FUNCTION_BLOCK
