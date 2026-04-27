(* OSCAT BASIC 3.33 and OSCAT BUILDING control modules by manual chapter. *)

FUNCTION_BLOCK ACTUATOR_COIL
VAR_INPUT
	IN : BOOL;
END_VAR
VAR_INPUT CONSTANT
	self_act_cycle : TIME := t#10d;
	self_act_time : TIME := t#1s;
END_VAR
VAR_OUTPUT
	OUT :BOOL;
	STATUS : BYTE;
END_VAR
VAR
	last : UDINT;
	init: BOOL;
	tx: UDINT;
	now: UDINT;
END_VAR
now := DWORD_TO_UDINT(T_PLC_MS());
IF NOT init THEN
	last := now;
	init := TRUE;
END_IF;
IF IN THEN
	OUT := TRUE;
	STATUS := BYTE#101;
	LAST := now;
ELSE
	OUT := FALSE;
	STATUS := BYTE#100;
	tx := now - last;
	IF (self_act_cycle > T#0s) AND (tx >= DWORD_TO_UDINT(TIME_TO_DWORD(self_act_cycle))) THEN
		OUT := TRUE;
		STATUS := BYTE#102;
		IF tx >= DWORD_TO_UDINT(TIME_TO_DWORD(self_act_cycle)) + DWORD_TO_UDINT(TIME_TO_DWORD(self_act_time)) THEN
			last := now;
			OUT := FALSE;
			STATUS := BYTE#100;
		END_IF;
	END_IF;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK TIMER_1
VAR_INPUT
	E : BOOL := TRUE;
	DTI : DT;
	START : TOD;
	DURATION : TIME;
	DAY : BYTE := 2#0111_1111;
END_VAR
VAR_OUTPUT
	Q : BOOL;
	stop : TIME;
END_VAR
VAR
	StopTod : TOD;
END_VAR
IF E THEN
	StopTod := ADD_TOD_TIME(START, DURATION);
	stop := ADD_TIME(
		HOUR_TO_TIME(INT_TO_REAL(HOUR(StopTod))),
		ADD_TIME(
			MINUTE_TO_TIME(INT_TO_REAL(MINUTE(StopTod))),
			SECOND_TO_TIME(SECOND(StopTod))
		)
	);
	Q := TIMECHECK(TD := DT_TO_TOD(DTI), START := START, STOP := StopTod)
		AND (SHR(BYTE#128, DAY_OF_WEEK(DT_TO_DATE(DTI))) AND DAY) > BYTE#0;
ELSE
	Q := FALSE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION BAND_B : BYTE
VAR_INPUT
	X : BYTE;
	B : BYTE;
END_VAR
IF X < B THEN
	BAND_B := BYTE#0;
ELSIF BYTE_TO_INT(X) > INT#255 - BYTE_TO_INT(B) THEN
	BAND_B := BYTE#255;
ELSE
	BAND_B := X;
END_IF;
END_FUNCTION

FUNCTION_BLOCK CONTROL_SET1
VAR_INPUT
	Kt : REAL;
	Tt : REAL;
	PI : BOOL;
	PID : BOOL;
END_VAR
VAR_INPUT CONSTANT
	P_K : REAL := 0.5;
	PI_K : REAL := 0.45;
	PI_TN : REAL := 0.83;
	PID_K : REAL := 0.6;
	PID_TN : REAL := 0.5;
	PID_TV : REAL := 0.125;
END_VAR
VAR_OUTPUT
	KP : REAL;
	TN : REAL;
	TV : REAL;
	KI : REAL;
	KD : REAL;
END_VAR
IF pi AND PID THEN
	KP := 0.0;
	TN := 0.0;
	TV := 0.0;
ELSIF PID THEN
	KP := PID_K * Kt;
	TN := PID_TN * Tt;
	TV := PID_TV * Tt;
ELSIF PI THEN
	KP := PI_K * Kt;
	TN := PI_TN * Tt;
ELSE
	KP := P_K * Kt;
END_IF;
IF tn > 0.0 THEN KI := KP / TN; ELSE KI := 0.0; END_IF;
KD := KP * TV;
END_FUNCTION_BLOCK

FUNCTION_BLOCK CONTROL_SET2
VAR_INPUT
	KS : REAL;
	TU : REAL;
	TG : REAL;
	PI : BOOL;
	PID : BOOL;
END_VAR
VAR_INPUT CONSTANT
	P_K : REAL := 1.0;
	PI_K : REAL := 0.9;
	PI_TN : REAL := 3.33;
	PID_K : REAL := 1.2;
	PID_TN : REAL := 2.0;
	PID_TV : REAL := 0.5;
END_VAR
VAR_OUTPUT
	KP : REAL;
	TN : REAL;
	TV : REAL;
	KI : REAL;
	KD : REAL;
END_VAR
VAR
	TX : REAL;
END_VAR
IF TU > 0.0 AND KS > 0.0 THEN TX := TG / TU / KS; END_IF;
IF pi AND PID THEN
	KP := 0.0;
	TN := 0.0;
	TV := 0.0;
ELSIF PID THEN
	KP := PID_K * TX;
	TN := PID_TN * TU;
	TV := PID_TV * TU;
ELSIF PI THEN
	KP := PI_K * TX;
	TN := PI_TN * TU;
ELSE
	KP := P_K * TX;
END_IF;
IF TN > 0.0 THEN KI := KP / TN; ELSE KI := 0.0; END_IF;
KD := KP * TV;
END_FUNCTION_BLOCK

FUNCTION CTRL_IN : REAL
VAR_INPUT
	SET_POINT, ACTUAL, NOISE : REAL;
END_VAR
CTRL_IN := DEAD_BAND(SET_POINT - ACTUAL, NOISE);
END_FUNCTION

FUNCTION_BLOCK CTRL_OUT
VAR_INPUT
	CI, OFFSET, MAN_IN, LIM_L, LIM_H : REAL;
	MANUAL : BOOL;
END_VAR
VAR_OUTPUT
	Y : REAL;
	LIM : BOOL;
END_VAR
Y := SEL(MANUAL, CI, MAN_IN) + OFFSET;
IF Y > LIM_L AND Y < LIM_H THEN
	LIM := FALSE;
ELSE
	Y := LIMIT(LIM_L, Y, LIM_H);
	LIM := TRUE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK CTRL_PI
VAR_INPUT
	ACT, SET_POINT, SUP, OFS, M_I : REAL;
	MAN : BOOL;
	RST : BOOL;
	KP : REAL := 1.0;
	KI : REAL := 1.0;
	LL : REAL := -1000.0;
	LH : REAL := 1000.0;
END_VAR
VAR_OUTPUT
	Y : REAL;
	DIFF : REAL;
	LIM : BOOL;
END_VAR
VAR
	pi : FT_PIWL;
	co : CTRL_OUT;
END_VAR
DIFF := CTRL_IN(SET_POINT, ACT, SUP);
pi(in := DIFF, kp := KP, ki := KI, lim_l := LL, lim_h := LH, rst := RST);
co(ci := pi.Y, OFFSET := OFS, man_in := M_I, lim_l := LL, lim_h := LH, MANUAL := MAN);
Y := co.Y;
LIM := co.LIM;
END_FUNCTION_BLOCK

FUNCTION_BLOCK CTRL_PID
VAR_INPUT
	ACT, SET_POINT, SUP, OFS, M_I : REAL;
	MAN : BOOL;
	RST : BOOL;
	KP : REAL := 1.0;
	TN : REAL := 1.0;
	TV : REAL := 1.0;
	LL : REAL := -1000.0;
	LH : REAL := 1000.0;
END_VAR
VAR_OUTPUT
	Y : REAL;
	DIFF : REAL;
	LIM : BOOL;
END_VAR
VAR
	pid : FT_PIDWL;
	co : CTRL_OUT;
END_VAR
DIFF := CTRL_IN(SET_POINT, ACT, SUP);
pid(in := DIFF, kp := KP, tn := TN, tv := TV, lim_l := LL, lim_h := LH, rst := RST);
co(ci := pid.Y, OFFSET := OFS, man_in := M_I, lim_l := LL, lim_h := LH, MANUAL := MAN);
Y := co.Y;
LIM := co.LIM;
END_FUNCTION_BLOCK

FUNCTION_BLOCK CTRL_PWM
VAR_INPUT
	CI, MAN_IN : REAL;
	MANUAL : BOOL;
	F : REAL;
END_VAR
VAR_OUTPUT
	Q : BOOL;
END_VAR
VAR
	PW : PWM_DC;
END_VAR
PW(F := F, DC := SEL(MANUAL,CI,MAN_IN));
Q := PW.Q;
END_FUNCTION_BLOCK

FUNCTION DEAD_BAND : REAL
VAR_INPUT
	X : REAL;
	L : REAL;
END_VAR
IF X > L THEN
	DEAD_BAND := X - L;
ELSIF X < -L THEN
	DEAD_BAND := X + L;
ELSE
	DEAD_BAND := 0.0;
END_IF;
END_FUNCTION

FUNCTION_BLOCK DEAD_BAND_A
VAR_INPUT
	X : REAL;
	T : TIME;
	KL : REAL := 1.0;
	LM : REAL;
END_VAR
VAR_OUTPUT
	Y : REAL;
	L : REAL;
END_VAR
VAR
	tp1 : FT_PT1;
	tp2 : FT_PT1;
END_VAR
tp1(in := X, T:= T);
tp2(in := ABS(tp1.out - X), T := MULTIME(T, 4.0));
L := MIN(KL * tp2.out, LM);
IF X > L THEN
	Y := X - L;
ELSIF X < -L THEN
	Y := X + L;
ELSE
	Y := 0.0;
END_IF;
END_FUNCTION_BLOCK

FUNCTION DEAD_ZONE : REAL
VAR_INPUT
	X : REAL;
	L : REAL;
END_VAR
IF X > L OR X < -L THEN
	dead_zone := X;
ELSE
	DEAD_ZONE := 0.0;
END_IF;
END_FUNCTION

FUNCTION_BLOCK DEAD_ZONE2
VAR_INPUT
	X : REAL;
	L : REAL;
END_VAR
VAR_OUTPUT
	Y: REAL;
END_VAR
IF X > L OR X < -L THEN
	Y := X;
ELSIF Y > 0.0 THEN
	Y := L;
ELSE
	Y := -L;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FT_DERIV
VAR_INPUT
	in : REAL;
	K : REAL := 1.0;
	run : BOOL := TRUE;
END_VAR
VAR_OUTPUT
	out : REAL;
END_VAR
VAR
	old: REAL;
	tx: DWORD;
	last: DWORD;
	init: BOOL;
	tc: REAL;
END_VAR
tx := T_PLC_US();
tc := UDINT_TO_REAL(DWORD_TO_UDINT(tx) - DWORD_TO_UDINT(last));
last := tx;
IF NOT init THEN
	init := TRUE;
	old := in;
ELSIF run AND tc > 0.0 THEN
	out := (in - old) / tc * 1000000.0 * K;
	old := in;
ELSE
	out := 0.0;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FT_IMP
VAR_INPUT
	in : REAL;
	T : TIME;
	K : REAL := 1.0;
END_VAR
VAR_OUTPUT
	out : REAL;
END_VAR
VAR
	t1 : FT_PT1;
END_VAR
T1(in:= in, T:=T);
out := (in - t1.out) * K;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FT_INT
VAR_INPUT
	IN : REAL;
	K : REAL := 1;
	RUN : BOOL := TRUE;
	RST : BOOL;
	OUT_MIN : REAL := -1.0E37;
	OUT_MAX : REAL := 1.0E37;
END_VAR
VAR_OUTPUT
	OUT : REAL;
	LIM : BOOL;
END_VAR
VAR
	INTeg : INTEGRATE;
END_VAR
IF rst THEN
	out := 0.0;
ELSE
	integ(X := IN, E := RUN, K := K, Y := out);
END_IF;
IF out >= OUT_MAX THEN
	out := out_max;
	LIM := TRUE;
ELSIF out <= out_min THEN
	out := out_min;
	lim := TRUE;
ELSE
	lim := FALSE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FT_INT2
VAR_INPUT
	IN : REAL;
	K : REAL := 1.0;
	RUN : BOOL := TRUE;
	RST : BOOL;
	OUT_MIN : REAL := -1.0E38;
	OUT_MAX : REAL := 1.0E38;
END_VAR
VAR_OUTPUT
	OUT : REAL;
	LIM : BOOL;
END_VAR
VAR
	integ: INTEGRATE;
	ix : REAL;
	val : REAL2;
END_VAR
IF RST THEN
	val := R2_SET(0.0);
	out := 0.0;
ELSE
	integ(X := IN, E := RUN, K := K, Y := ix);
	val := R2_ADD(val, ix);
	ix := 0.0;
	OUT := val.RX;
END_IF;
IF out > OUT_MIN AND out < OUT_MAX THEN
	LIM := FALSE;
ELSE
	OUT := LIMIT(OUT_MIN, OUT, OUT_MAX);
	val := R2_SET(OUT);
	LIM := TRUE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FT_PD
VAR_INPUT
	IN : REAL;
	KP : REAL := 1.0;
	TV : REAL := 1.0;
END_VAR
VAR_OUTPUT
	Y : REAL;
END_VAR
VAR
	diff : FT_DERIV;
END_VAR
diff(IN := IN, K := TV);
Y := KP * (diff.out + IN);
END_FUNCTION_BLOCK

FUNCTION_BLOCK FT_PDT1
VAR_INPUT
	IN : REAL;
	KP : REAL := 1.0;
	TV : REAL := 1.0;
	T1 : REAL := 1.0;
END_VAR
VAR_OUTPUT
	Y : REAL;
END_VAR
VAR
	diff : FT_DERIV;
	TP : FT_PT1;
END_VAR
diff(IN := IN, K := TV);
tp(in := diff.out, T := DWORD_TO_TIME(UDINT_TO_DWORD(REAL_TO_UDINT(T1))));
Y := KP * (tp.out + IN);
END_FUNCTION_BLOCK

FUNCTION_BLOCK FT_PI
VAR_INPUT
	IN : REAL;
	KP : REAL := 1.0;
	KI : REAL := 1.0;
	ILIM_L : REAL := -1.0E38;
	ILIM_H : REAL := 1.0E38;
	IEN : BOOL := TRUE;
	RST : BOOL;
END_VAR
VAR_OUTPUT
	Y : REAL;
	LIM : BOOL;
END_VAR
VAR
	integ : FT_INT;
END_VAR
integ(IN := IN, K := KI, RUN := IEN, RST := RST, OUT_MIN := ILIM_L, OUT_MAX := ILIM_H);
LIM := integ.LIM;
Y := KP * IN + integ.Out;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FT_PID
VAR_INPUT
	IN : REAL;
	KP : REAL := 1.0;
	TN : REAL := 1.0;
	TV : REAL := 1.0;
	ILIM_L : REAL := -1.0E38;
	ILIM_H : REAL := 1.0E38;
	IEN : BOOL := TRUE;
	RST : BOOL;
END_VAR
VAR_OUTPUT
	Y : REAL;
	LIM : BOOL;
END_VAR
VAR
	integ : FT_INT;
	diff : FT_DERIV;
END_VAR
IF TN > 0.0 THEN
	integ(IN := IN, K := 1.0 / TN, RUN := IEN, RST := RST, OUT_MIN := ILIM_L, OUT_MAX := ILIM_H);
ELSE
	integ(RST := FALSE);
END_IF;
diff(IN := IN, K := TV);
Y := KP * (integ.Out + diff.out + IN);
LIM := integ.LIM;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FT_PIDW
VAR_INPUT
	IN : REAL;
	KP : REAL := 1.0;
	TN : REAL := 1.0;
	TV : REAL := 1.0;
	LIM_L : REAL := -1.0E38;
	LIM_H : REAL := 1.0E38;
	RST : BOOL;
END_VAR
VAR_OUTPUT
	Y : REAL;
	LIM : BOOL;
END_VAR
VAR
	integ : INTEGRATE;
	diff : FT_DERIV;
	YI: REAL;
END_VAR
IF tn = 0.0 OR rst THEN
	integ(E := FALSE, Y := YI);
	YI := 0.0;
ELSE
	integ(X := IN, K := 1.0 / TN, E := NOT LIM, Y := YI);
END_IF;
Y := KP * (IN + YI);
diff(IN := IN, K := TV);
IF Y > LIM_L AND Y < LIM_H THEN
	LIM := FALSE;
ELSE
	LIM := TRUE;
END_IF;
Y := LIMIT(LIM_L, Y + KP * diff.out, LIM_H);
END_FUNCTION_BLOCK

FUNCTION_BLOCK FT_PIDWL
VAR_INPUT
	IN : REAL;
	KP : REAL := 1.0;
	TN : REAL := 1.0;
	TV : REAL := 1.0;
	LIM_L : REAL := -1.0E38;
	LIM_H : REAL := 1.0E38;
	RST : BOOL;
END_VAR
VAR_OUTPUT
	Y : REAL;
	LIM : BOOL;
END_VAR
VAR
	piwl : FT_PIWL;
	diff : FT_DERIV;
END_VAR
IF rst THEN
	piwl(rst := TRUE);
ELSE
	IF TN = 0.0 THEN
		piwl(in := IN * KP, KP := 1.0, KI := 0.0, LIM_L := LIM_L, LIM_H := LIM_H);
	ELSE
		piwl(in := IN * KP, KP := 1.0, KI := 1.0 / TN, LIM_L := LIM_L, LIM_H := LIM_H);
	END_IF;
	diff(IN := IN, K := KP * TV);
	Y := piwl.Y + diff.out;
	IF Y < LIM_L THEN
		LIM := TRUE;
		Y := LIM_L;
	ELSIF Y > LIM_H THEN
		LIM := TRUE;
		Y := LIM_H;
	ELSE
		LIM := FALSE;
	END_IF;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FT_PIW
VAR_INPUT
	IN : REAL;
	KP : REAL := 1.0;
	KI : REAL := 1.0;
	LIM_L : REAL := -1.0E38;
	LIM_H : REAL := 1.0E38;
	RST : BOOL;
END_VAR
VAR_OUTPUT
	Y : REAL;
	LIM : BOOL;
END_VAR
VAR
	integ : FT_INT;
END_VAR
integ(IN := IN, K := KI, RUN := NOT LIM, RST := RST);
Y := KP * IN + integ.Out;
IF Y < LIM_L THEN
	Y := LIM_L;
	LIM := TRUE;
ELSIF Y > LIM_H THEN
	Y := LIM_H;
	LIM := TRUE;
ELSE
	LIM := FALSE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FT_PIWL
VAR_INPUT
	IN : REAL;
	KP : REAL := 1.0;
	KI : REAL := 1.0;
	LIM_L : REAL := -1.0E38;
	LIM_H : REAL := 1.0E38;
	RST : BOOL;
END_VAR
VAR_OUTPUT
	Y : REAL;
	LIM : BOOL;
END_VAR
VAR
	init: BOOL;
	tx: DWORD;
	tc : REAL;
	t_last: DWORD;
	in_last : REAL;
	i: REAL;
	p: REAL;
END_VAR
IF NOT init OR RST THEN
	init := TRUE;
	in_last := in;
	t_last := T_PLC_US();
	i := 0.0;
	tc := 0.0;
ELSE
	tx := T_PLC_US();
	tc := UDINT_TO_REAL(DWORD_TO_UDINT(tx) - DWORD_TO_UDINT(t_last));
	t_last := tx;
	p := KP * IN;
	i := (IN + in_last) * 5.0E-7 * KI * tc + i;
	in_last := IN;
	Y := p + i;
	IF Y >= LIM_H THEN
		Y := LIM_H;
		IF ki <> 0.0 THEN
			i := LIM_H - p;
		ELSE
			i := 0.0;
		END_IF;
		LIM := TRUE;
	ELSIF Y <= LIM_L THEN
		Y := LIM_L;
		IF ki <> 0.0 THEN
			i := LIM_L - p;
		ELSE
			i := 0.0;
		END_IF;
		LIM := TRUE;
	ELSE
		LIM := FALSE;
	END_IF;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FT_PT1
VAR_INPUT
	in : REAL;
	T : TIME;
	K : REAL := 1.0;
END_VAR
VAR_OUTPUT
	out : REAL;
END_VAR
VAR
	last : DWORD;
	tx: DWORD;
	init: BOOL;
END_VAR
tx := T_PLC_US();
IF NOT init OR T = t#0s THEN
	init := TRUE;
	out := K * in;
ELSE
	out := out + (in * K - out) * UDINT_TO_REAL(DWORD_TO_UDINT(Tx) - DWORD_TO_UDINT(last)) / DWORD_TO_REAL(TIME_TO_DWORD(T)) * 1.0E-3;
	IF ABS(out) < 1.0E-20 THEN out := 0.0; END_IF;
END_IF;
last := tx;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FT_PT2
VAR_INPUT
	in : REAL;
	T : TIME;
	D : REAL;
	K : REAL := 1.0;
END_VAR
VAR_OUTPUT
	out : REAL;
END_VAR
VAR
	init: BOOL;
	int1 : INTEGRATE;
	int2 : INTEGRATE;
	tn: REAL;
	I1, I2 : REAL;
	tn2: REAL;
END_VAR
IF NOT init OR T = T#0s THEN
	init := TRUE;
	out := K * in;
	I2 := out;
ELSE
	TN := DWORD_TO_REAL(TIME_TO_DWORD(T)) * 1.0E-3;
	tn2 := TN * TN;
	int1(X := in * K / tn2 - I1 * 0.5 * D / TN - I2 / TN2, Y := I1);
	int2(X := I1,Y := I2);
	out := I2;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FT_TN16
VAR_INPUT
	in : REAL;
	T : TIME;
END_VAR
VAR_OUTPUT
	out : REAL;
	trig : BOOL;
END_VAR
VAR
	length : INT := 16;
	X : ARRAY[0..15] OF REAL;
	cnt : INT;
	last : DWORD;
	tx: DWORD;
	init: BOOL;
END_VAR
tx := T_PLC_MS();
trig := FALSE;
IF NOT init THEN
	x[cnt] := in;
	init := TRUE;
	last := tx;
ELSIF DWORD_TO_UDINT(tx) - DWORD_TO_UDINT(last) >= DWORD_TO_UDINT(TIME_TO_DWORD(T)) / INT_TO_UDINT(length) THEN
	IF cnt = length - 1 THEN cnt := 0; ELSE cnt := cnt + 1; END_IF;
	Out := X[cnt];
	x[cnt] := in;
	last := tx;
	trig := TRUE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FT_TN64
VAR_INPUT
	in : REAL;
	T : TIME;
END_VAR
VAR_OUTPUT
	out : REAL;
	trig : BOOL;
END_VAR
VAR
	length : INT := 64;
	X : ARRAY[0..63] OF REAL;
	cnt : INT;
	last : DWORD;
	tx: DWORD;
	init: BOOL;
END_VAR
tx := T_PLC_MS();
trig := FALSE;
IF NOT init THEN
	x[cnt] := in;
	init := TRUE;
	last := tx;
ELSIF DWORD_TO_UDINT(tx) - DWORD_TO_UDINT(last) >= DWORD_TO_UDINT(TIME_TO_DWORD(T)) / INT_TO_UDINT(length) THEN
	IF cnt = length - 1 THEN cnt := 0; ELSE cnt := cnt + 1; END_IF;
	Out := X[cnt];
	x[cnt] := in;
	last := tx;
	trig := TRUE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FT_TN8
VAR_INPUT
	in : REAL;
	T : TIME;
END_VAR
VAR_OUTPUT
	out : REAL;
	trig : BOOL;
END_VAR
VAR
	length : INT := 8;
	X : ARRAY[0..7] OF REAL;
	cnt : INT;
	last : DWORD;
	tx: DWORD;
	init: BOOL;
END_VAR
tx := T_PLC_MS();
trig := FALSE;
IF NOT init THEN
	x[cnt] := in;
	init := TRUE;
	last := tx;
ELSIF DWORD_TO_UDINT(tx) - DWORD_TO_UDINT(last) >= DWORD_TO_UDINT(TIME_TO_DWORD(T)) / INT_TO_UDINT(length) THEN
	IF cnt = length - 1 THEN cnt := 0; ELSE cnt := cnt + 1; END_IF;
	Out := X[cnt];
	x[cnt] := in;
	last := tx;
	trig := TRUE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK HYST
VAR_INPUT
	In : REAL;
	HIGH_LIMIT : REAL;
	LOW_LIMIT : REAL;
END_VAR
VAR_OUTPUT
	Q : BOOL;
	WIN : BOOL;
END_VAR
IF HIGH_LIMIT >= LOW_LIMIT THEN
	IF IN < LOW_LIMIT THEN
		Q := FALSE;
		WIN := FALSE;
	ELSIF IN > HIGH_LIMIT THEN
		Q := TRUE;
		WIN := FALSE;
	ELSE
		WIN := TRUE;
	END_IF;
ELSE
	IF IN > LOW_LIMIT THEN
		Q := FALSE;
		WIN := FALSE;
	ELSIF IN < HIGH_LIMIT THEN
		Q := TRUE;
		WIN := FALSE;
	ELSE
		WIN := TRUE;
	END_IF;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK HYST_1
VAR_INPUT
	In : REAL;
	high : REAL;
	low : REAL;
END_VAR
VAR_OUTPUT
	Q : BOOL;
	WIN : BOOL;
END_VAR
IF in < low THEN
	Q := FALSE;
	win := FALSE;
ELSIF in > high THEN
	Q := TRUE;
	win := FALSE;
ELSE
	win := TRUE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK HYST_2
VAR_INPUT
	IN : REAL;
	VAL : REAL;
	HYS : REAL;
END_VAR
VAR_OUTPUT
	Q : BOOL;
	WIN : BOOL;
END_VAR
VAR
	tmp: REAL;
END_VAR
tmp := val - hys * 0.5;
IF in < tmp THEN
	Q := FALSE;
	win := FALSE;
ELSIF in > tmp + hys THEN
	Q := TRUE;
	win := FALSE;
ELSE
	win := TRUE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK HYST_3
VAR_INPUT
	in : REAL;
	hyst : REAL;
	val1 : REAL;
	val2 : REAL;
END_VAR
VAR_OUTPUT
	Q1 : BOOL;
	Q2 : BOOL;
END_VAR
VAR
	X: REAL;
END_VAR
X := hyst * 0.5;
IF in < val1 - X THEN
	Q1 := TRUE;
ELSIF in > val1 + X THEN
	Q1 := FALSE;
END_IF;
IF in < val2 - X THEN
	Q2 := FALSE;
ELSIF in > val2 + X THEN
	Q2 := TRUE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK INTEGRATE
VAR_INPUT
	E : BOOL := TRUE;
	X : REAL;
	K : REAL := 1.0;
END_VAR
VAR_IN_OUT
	Y : REAL;
END_VAR
VAR
	X_last : REAL;
	init: BOOL;
	last: DWORD;
	tx: DWORD;
END_VAR
tx := T_PLC_MS();
IF NOT init THEN
	init := TRUE;
	X_last := X;
ELSIF E THEN
	Y := (X + X_LAST) * 0.5E-3 * UDINT_TO_REAL(DWORD_TO_UDINT(tx) - DWORD_TO_UDINT(last)) * K + Y;
	X_last := X;
END_IF;
last := tx;
END_FUNCTION_BLOCK

FUNCTION AIR_DENSITY : REAL
VAR_INPUT
    T : REAL;
    P : REAL;
    RH : REAL;
END_VAR
VAR CONSTANT
    RL : REAL := 287.05;
    RX : REAL := 3.773319E-3;
END_VAR
VAR
    SddValue : REAL;
END_VAR
SddValue := SDD(T + 0.0, TRUE);
AIR_DENSITY := phys.PN * (1.0 - RH * SddValue * RX / P) / (RL * (T - phys.T0));
END_FUNCTION

FUNCTION AIR_ENTHALPY : REAL
VAR_INPUT
    T : REAL;
    RH : REAL;
END_VAR
VAR CONSTANT
    CPL : REAL := 1.00482;
    CW : REAL := 1.86;
    LH : REAL := 2500.78;
END_VAR
VAR
    DewCondensation : REAL;
END_VAR
DewCondensation := DEW_CON(RH, T);
AIR_ENTHALPY := CPL * T + (CW * T + LH) * DewCondensation;
END_FUNCTION

FUNCTION_BLOCK BOILER
VAR_INPUT
	T_upper : REAL;
	T_lower : REAL;
	Pressure : BOOL := TRUE;
	enable : BOOL := TRUE;
	Req_1 : BOOL;
	Req_2 : BOOL;
	Boost : BOOL;
END_VAR
VAR_INPUT CONSTANT
	t_upper_min : REAL := 50.0;
	t_upper_max : REAL := 60.0;
	T_lower_enable : BOOL;
	T_lower_max : REAL := 60.0;
	T_request_1 : REAL := 70.0;
	T_request_2 : REAL := 50.0;
	T_request_hys : REAL := 5.0;
	T_protect_high : REAL := 80.0;
	T_protect_low : REAL := 10.0;
END_VAR
VAR_OUTPUT
	Heat : BOOL;
	error : BOOL;
	status : BYTE;
END_VAR
VAR
	edge: BOOL;
	boost_mode: BOOL;
	flag_0 : BOOL;
	flag_1: BOOL;
	flag_2: BOOL;
END_VAR
IF t_upper > t_protect_high THEN
	status := BYTE#1;
	heat := FALSE;
	error := TRUE;
ELSIF t_upper < t_protect_low THEN
	status := BYTE#2;
	heat :=	TRUE;
	error := TRUE;
ELSIF t_lower > T_protect_high AND t_lower_enable THEN
	status := BYTE#3;
	heat := FALSE;
	error := TRUE;
ELSIF t_lower < t_protect_low AND t_lower_enable THEN
	status := BYTE#4;
	heat := TRUE;
	error := TRUE;
ELSIF NOT pressure THEN
	status := BYTE#5;
	heat := FALSE;
	error := TRUE;
ELSIF req_1 OR req_2 OR enable OR boost THEN
	error := FALSE;
	IF boost AND NOT edge AND t_upper < t_upper_max THEN
		status := BYTE#101;
		heat := TRUE;
		boost_mode := TRUE;
	ELSIF enable AND t_upper < T_upper_min THEN
		status := BYTE#102;
		heat := TRUE;
	ELSIF req_1 AND t_upper < T_request_1 THEN
		status := BYTE#103;
		heat := TRUE;
	ELSIF req_2 AND t_upper < t_request_2 THEN
		status := BYTE#104;
		heat := TRUE;
	END_IF;
	IF heat THEN
		IF (enable OR boost_mode) THEN
			flag_0 := TRUE;
			IF T_lower_enable AND T_lower > T_lower_max THEN
				flag_0 := FALSE;
				boost_mode := FALSE;
			ELSIF NOT T_lower_enable AND T_upper > T_upper_max THEN
				flag_0 := FALSE;
				boost_mode := FALSE;
			END_IF;
		ELSE
			flag_0 := FALSE;
		END_IF;
		flag_1 := (req_1 AND T_upper > T_request_1 + T_request_hys) AND req_1;
		flag_2 := (req_2 AND T_upper > T_request_2 + T_request_hys) AND req_2;
		heat := flag_0 OR flag_1 OR flag_2;
		IF heat = FALSE THEN status := BYTE#100; END_IF;
	END_IF;
ELSE
	status := BYTE#100;
	heat := FALSE;
	error := FALSE;
END_IF;
edge := boost;
END_FUNCTION_BLOCK

FUNCTION_BLOCK BURNER
VAR_INPUT
	in : BOOL;
	stage2 : BOOL;
	over_temp : BOOL;
	oil_temp : BOOL := TRUE;
	Flame : BOOL;
	rst : BOOL;
	rst_timer : BOOL;
END_VAR
VAR_INPUT CONSTANT
	pre_heat_time : TIME := t#5s;
	pre_vent_time : TIME := t#15s;
	pre_ignite_time : TIME := t#5s;
	post_ignite_time : TIME := t#25s;
	stage2_delay : TIME := t#10s;
	safety_time : TIME := t#5s;
	lockout_time : TIME := t#10s;
	multiple_ignition : BOOL := TRUE;
	KW1 : REAL;
	KW2 : REAL;
END_VAR
VAR_OUTPUT
	motor : BOOL;
	coil1 : BOOL;
	coil2 : BOOL;
	pre_heat: BOOL;
	ignite : BOOL;
	KWh : REAL;
	status : BYTE;
	fail : BOOL;
END_VAR
VAR_IN_OUT
	runtime1 : UDINT;
	runtime2 : UDINT;
	cycles : UDINT;
END_VAR
VAR
	state : INT;
	last : DWORD;
	tx: DWORD;
	last_change: DWORD;
	timer1 : ONTIME;
	timer2 : ONTIME;
	oil_temp_last: BOOL;
	cycles2 : UDINT;
END_VAR
tx := T_PLC_MS();
IF rst OR over_temp OR state = 0 THEN
	IF status > BYTE#0 AND DWORD_TO_UDINT(tx) - DWORD_TO_UDINT(last_change) >= DWORD_TO_UDINT(TIME_TO_DWORD(lockout_time)) AND rst THEN
		status := BYTE#110;
		fail := FALSE;
		state := 1;
	ELSE
		motor := FALSE;
		coil1 := FALSE;
		coil2 := FALSE;
		ignite := FALSE;
		pre_heat := FALSE;
		IF over_temp THEN
			status := BYTE#9;
			fail := TRUE;
		END_IF;
		last_change := tx;
		last := tx;
		state := 1;
	END_IF;
END_IF;
IF rst_timer THEN
	runtime1 := 0;
	runtime2 := 0;
	cycles := 0;
	cycles2 := 0;
END_IF;
IF NOT ((status > BYTE#0 AND status < BYTE#100) OR rst) THEN
    CASE state OF
    1:
        IF in AND flame THEN
            state := 7;
            pre_heat := FALSE;
            status := BYTE#2;
            last_change := tx;
        ELSIF in THEN
            pre_heat := TRUE;
            state := 2;
            last_change := tx;
        END_IF;
    2:
        IF (DWORD_TO_UDINT(tx) - DWORD_TO_UDINT(last_change) >= DWORD_TO_UDINT(TIME_TO_DWORD(pre_heat_time)) AND oil_temp) OR (oil_temp AND NOT oil_temp_last) THEN
            motor := TRUE;
            state := 3;
            last_change := tx;
        ELSIF DWORD_TO_UDINT(tx) - DWORD_TO_UDINT(last_change) >= DWORD_TO_UDINT(TIME_TO_DWORD(pre_heat_time)) AND NOT oil_temp THEN
            state := 7;
            pre_heat := FALSE;
            status := BYTE#1;
            last_change := tx;
        ELSIF flame THEN
            state := 7;
            pre_heat := FALSE;
            status := BYTE#2;
            last_change := tx;
        END_IF;
    3:
        IF DWORD_TO_UDINT(tx) - DWORD_TO_UDINT(last_change) >= DWORD_TO_UDINT(TIME_TO_DWORD(pre_vent_time)) - DWORD_TO_UDINT(TIME_TO_DWORD(pre_ignite_time)) THEN
            ignite := TRUE;
            state := 4;
            last_change := tx;
        ELSIF flame THEN
            state := 7;
            pre_heat := FALSE;
            motor := FALSE;
            status := BYTE#3;
            last_change := tx;
        END_IF;
    4:
        IF DWORD_TO_UDINT(tx) - DWORD_TO_UDINT(last_change) >= DWORD_TO_UDINT(TIME_TO_DWORD(pre_ignite_time)) THEN
            coil1 := TRUE;
            state := 5;
            last_change := tx;
        END_IF;
    5:
        IF DWORD_TO_UDINT(tx) - DWORD_TO_UDINT(last_change) >= DWORD_TO_UDINT(TIME_TO_DWORD(safety_time)) OR flame THEN
            IF NOT flame THEN
                state := 7;
                motor := FALSE;
                coil1 := FALSE;
                pre_heat := FALSE;
                ignite := FALSE;
                status := BYTE#4;
                last_change := tx;
            ELSE
                state := 6;
                last_change := tx;
            END_IF;
        END_IF;
    6:
        IF NOT flame AND NOT multiple_ignition THEN
            state := 7;
            motor := FALSE;
            coil1 := FALSE;
            coil2 := FALSE;
            pre_heat := FALSE;
            ignite := FALSE;
            status := BYTE#5;
            last_change := tx;
        ELSIF NOT flame AND multiple_ignition THEN
            ignite := TRUE;
            state := 5;
            coil2 := FALSE;
            last_change := tx;
        ELSE
            IF DWORD_TO_UDINT(tx) - DWORD_TO_UDINT(last_change) >= DWORD_TO_UDINT(TIME_TO_DWORD(post_ignite_time)) THEN
                ignite := FALSE;
            END_IF;
            IF DWORD_TO_UDINT(tx) - DWORD_TO_UDINT(last_change) >= DWORD_TO_UDINT(TIME_TO_DWORD(stage2_delay)) AND stage2 THEN
                coil2 := TRUE;
            ELSE
                coil2 := FALSE;
            END_IF;
        END_IF;
    END_CASE;
    IF NOT in THEN
        state := 1;
        motor := FALSE;
        coil1 := FALSE;
        coil2 := FALSE;
        ignite := FALSE;
        pre_heat := FALSE;
        last_change := tx;
    END_IF;
    timer1(in := flame AND in AND motor AND coil1 AND NOT coil2, seconds := runtime1, cycles := cycles);
    timer2(in := flame AND in AND motor AND coil1 AND coil2, seconds := runtime2, cycles := cycles2);
    KWH := UDINT_TO_REAL(runtime1) * KW1 / 3600.0 + UDINT_TO_REAL(runtime2) * KW2 / 3600.0;
    last := tx;
    IF status > BYTE#0 AND status < BYTE#100 THEN
        fail := TRUE;
    ELSE
        fail := FALSE;
        IF NOT in THEN
            status := BYTE#110;
        ELSIF flame AND in AND motor AND coil2 AND coil1 THEN
            status := BYTE#113;
        ELSIF flame AND in AND motor AND coil1 THEN
            status := BYTE#112;
        ELSE
            status := BYTE#111;
        END_IF;
    END_IF;
END_IF;
END_FUNCTION_BLOCK

FUNCTION DEW_CON : REAL
VAR_INPUT
    RH : REAL;
    T : REAL;
END_VAR
VAR
    SddValue : REAL;
END_VAR
IF RH > 0.0 AND T > -50.0 THEN
    SddValue := SDD(T + 0.0, FALSE);
    DEW_CON := 2.166824303E-2 * RH * SddValue / (T - phys.T0);
ELSE
    DEW_CON := 0.0;
END_IF;
END_FUNCTION

FUNCTION DEW_RH : REAL
VAR_INPUT
    VC : REAL;
    T : REAL;
END_VAR
VAR
    DewCondensation : REAL;
END_VAR
DewCondensation := DEW_CON(1.0, T);
DEW_RH := LIMIT(0.0, VC / DewCondensation, 100.0);
END_FUNCTION


FUNCTION DEW_TEMP : REAL
VAR_INPUT
    RH : REAL;
    T : REAL;
END_VAR
VAR CONSTANT
    a : REAL := 7.5;
    b : REAL := 237.3;
END_VAR
VAR
    V : REAL;
    SaturationTerm : REAL;
END_VAR
IF rh > 0.0 THEN
    SaturationTerm := EXP10((a * T) / (b + T));
    V := LOG(RH * 0.01 * SaturationTerm);
    DEW_TEMP := b * V / (a - V);
ELSE
    DEW_TEMP := phys.T0;
END_IF;
END_FUNCTION

FUNCTION HEAT_INDEX : REAL
VAR_INPUT
	T : REAL;
	RH : REAL;
END_VAR
VAR
	RH2 : REAL;
	T2 : REAL;
	TF : REAL;
END_VAR
IF RH < 20.0 OR T < 20.0 THEN
	HEAT_INDEX := T;
ELSE
	RH2 := RH * RH;
	TF := C_TO_F(T);
	T2 := TF * TF;
	HEAT_INDEX := -42.379 + 2.04901523 * TF - 6.83783E-3 * T2
			+ RH * (10.1433127 - 0.22475541 * TF + 1.22874E-3 * T2)
			+ RH2 * (8.5282E-4 * TF - 5.481717E-2 - 1.99E-6 * T2);
	HEAT_INDEX := F_TO_C(HEAT_INDEX);
END_IF;
END_FUNCTION

FUNCTION_BLOCK HEAT_METER
VAR_INPUT
	TF, TR : REAL;
	LPH : REAL;
	E : BOOL;
	RST : BOOL;
END_VAR
VAR_INPUT CONSTANT
	CP : REAL;
	Density : REAL;
	Content : REAL;
	Pulse_Mode : BOOL;
	Return_meter : BOOL;
	AVG_time : TIME := T#5s;
END_VAR
VAR_OUTPUT
	C : REAL;
END_VAR
VAR_IN_OUT
	Y : REAL;
END_VAR
VAR
	tx, last : DWORD;
	int1 : FT_INT2;
	edge : BOOL;
	x: REAL;
	init: BOOL;
	y_last: REAL;
END_VAR
IF rst THEN
	int1(rst := TRUE);
	C := 0.0;
	Y := 0.0;
ELSIF e THEN
	X := (WATER_DENSITY(SEL(return_meter, TF, TR), FALSE) * (WATER_ENTHALPY(TF) - WATER_ENTHALPY(TR)) * (1.0 - content) + CP * Density * content * (TF-TR));
END_IF;
int1(run := NOT pulse_mode AND e, in := X * LPH * 2.77777777777E-4);
IF pulse_mode THEN
	IF NOT edge AND E THEN Y := Y + X * LPH; END_IF;
ELSE
	Y := int1.Out;
END_IF;
edge := e;
tx := T_PLC_MS();
IF NOT init THEN
	init := TRUE;
	last := tx;
END_IF;
IF (DWORD_TO_UDINT(tx) - DWORD_TO_UDINT(last) >= DWORD_TO_UDINT(TIME_TO_DWORD(AVG_TIME))) AND (avg_time > T#0s) THEN
	last := tx;
	C := (Y - Y_last) * 3.6E6 / DWORD_TO_REAL(TIME_TO_DWORD(AVG_TIME));
	Y_last := Y;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK HEAT_TEMP
VAR_INPUT
	T_EXT : REAL;
	T_INT : REAL;
	OFFSET : REAL;
	T_REQ : REAL;
END_VAR
VAR_INPUT CONSTANT
	TY_MAX : REAL := 70.0;
	TY_MIN : REAL := 25.0;
	TY_CONFIG : REAL := 70.0;
	T_INT_CONFIG : REAL := 20.0;
	T_EXT_CONFIG : REAL := -15.0;
	T_DIFF : REAL := 10.0;
	C : REAL := 1.33;
	H : REAL := 3.0;
END_VAR
VAR_OUTPUT
	TY : REAL;
	HEAT : BOOL;
END_VAR
VAR
	tr : REAL;
	tx: REAL;
END_VAR
tr := T_INT + OFFSET;
tx := (tr - T_EXT) / (T_INT_CONFIG - T_EXT_CONFIG);
IF T_EXT + H > tr THEN
	TY := 0.0;
ELSE
	TY := LIMIT(TY_MIN, tr + T_DIFF * 0.5 * tx + (TY_CONFIG - T_DIFF * 0.5 - tr) * EXPT(tx, 1.0 / C), TY_MAX);
END_IF;
TY := MAX(TY, T_REQ);
HEAT := TY > 0.0;
END_FUNCTION_BLOCK

FUNCTION_BLOCK LEGIONELLA
VAR_INPUT
	manual : BOOL;
	temp_boiler : REAL;
	temp_return : REAL := 100.0;
	DT_in: DT;
	rst : BOOL;
END_VAR
VAR_INPUT CONSTANT
	T_start: TOD := TOD#03:00:00;
	day : INT := 7;
	temp_set : REAL := 70.0;
	temp_offset : REAL := 10.0;
	temp_hys : REAL := 5.0;
	t_max_heat: TIME := t#10m;
	t_max_ret : TIME := t#10m;
	tp_0: TIME := t#5m;
	TP_1: TIME := t#5m;
	tp_2: TIME := t#5m;
	tp_3: TIME := t#5m;
	tp_4: TIME := t#5m;
	tp_5: TIME := t#5m;
	tp_6: TIME := t#5m;
	tp_7: TIME := t#5m;
END_VAR
VAR_OUTPUT
	Heat : BOOL;
	pump : BOOL;
	valve0 : BOOL;
	valve1 : BOOL;
	valve2 : BOOL;
	valve3 : BOOL;
	valve4 : BOOL;
	valve5 : BOOL;
	valve6 : BOOL;
	valve7 : BOOL;
	run: BOOL;
	Status : BYTE;
END_VAR
VAR
	X1: TIMER_1;
	x2: SEQUENCE_8;
	x3: HYST_1;
	init: BOOL;
END_VAR
IF NOT init THEN
	init := TRUE;
END_IF;
X1(E := TRUE, DTI := DT_in, START := T_start, DURATION := T#0s, DAY := SHR(BYTE#128, day));
X3(IN := temp_boiler, low := Temp_offset + temp_set, high := Temp_hys + Temp_offset + temp_set);
X2(
	IN0 := X3.Q OR X3.win,
	IN1 := temp_return >= temp_set,
	IN2 := temp_return >= temp_set,
	IN3 := temp_return >= temp_set,
	IN4 := temp_return >= temp_set,
	IN5 := temp_return >= temp_set,
	IN6 := temp_return >= temp_set,
	IN7 := temp_return >= temp_set,
	START := X1.Q OR MANUAL,
	RST := rst,
	WAIT0 := T_max_heat,
	DELAY0 := TP_0,
	WAIT1 := T_max_ret,
	DELAY1 := TP_1,
	WAIT2 := T_max_ret,
	DELAY2 := TP_2,
	WAIT3 := T_max_ret,
	DELAY3 := TP_3,
	WAIT4 := T_max_ret,
	DELAY4 := TP_4,
	WAIT5 := T_max_ret,
	DELAY5 := TP_5,
	WAIT6 := T_max_ret,
	DELAY6 := TP_6,
	WAIT7 := T_max_ret,
	DELAY7 := TP_7
);
IF X1.Q OR MANUAL OR x2.run THEN
	run := x2.run;
	pump := x2.qx;
	Heat := NOT X3.Q AND x2.run;
	valve0 := X2.Q0;
	valve1 := X2.Q1;
	valve2 := X2.Q2;
	valve3 := X2.Q3;
	valve4 := X2.Q4;
	valve5 := X2.Q5;
	valve6 := X2.Q6;
	valve7 := X2.Q7;
	pump := X2.qx;
	status := X2.status;
ELSE
	status := x2.status;
END_IF;
END_FUNCTION_BLOCK

FUNCTION SDD : REAL
VAR_INPUT
	T : REAL;
	ICE: BOOL;
END_VAR
IF ice THEN
	SDD := 611.153 * EXP(22.4433 * T / (272.186 + T));
ELSE
	SDD := 611.213 * EXP(17.5043 * T / (241.2 + T));
END_IF;
END_FUNCTION

FUNCTION SDD_NH3 : REAL
VAR_INPUT
	T : REAL;
END_VAR
IF T < -33.65 THEN
	SDD_NH3 := EXP(7.3396511649 - ( 1166.7498002 / ( T + 192.37 )));
ELSE
	SDD_NH3 := EXP(11.210964456 - ( 2564.9140075 / ( T + 262.741 )));
END_IF;
END_FUNCTION

FUNCTION SDT_NH3 : REAL
VAR_INPUT
	P : REAL;
END_VAR
IF P < 1.0E-3 THEN
	SDT_NH3 := -110.0;
ELSIF P < 1.0 THEN
	SDT_NH3 := 506.713 / (3.18757 - LOG(P)) - 192.37 ;
ELSE
	SDT_NH3 := 1113.928 / (4.86886 - LOG(P)) - 262.71 ;
END_IF;
END_FUNCTION

FUNCTION_BLOCK T_AVG24
VAR_INPUT
	TS : INT;
	DTI : DT;
	RST : BOOL;
END_VAR
VAR_INPUT CONSTANT
	T_FILTER : TIME := T#10m;
	SCALE : REAL := 1.0;
	OFS : REAL;
END_VAR
VAR_OUTPUT
	TA : REAL;
	TP : BOOL;
END_VAR
VAR_IN_OUT
	T24 : REAL;
	T24_MAX : REAL;
	T24_MIN : REAL;
END_VAR
VAR
	samples : ARRAY[0..47] OF INT;
	pos : INT;
	init: BOOL;
	ft1 : FILTER_I;
	sum : DINT;
	last: DT;
END_VAR
VAR_TEMP
	tmp_max : INT;
	tmp_min : INT;
	i: INT;
	MinuteOfDay : INT;
END_VAR
ft1(X := TS, T := T_FILTER);
IF rst OR NOT init THEN
	init := TRUE;
	IF T24 = -1000.0 THEN t24 := INT_TO_REAL(ft1.Y) * 0.1; END_IF;
	FOR pos := 0 TO 47 DO
		samples[pos] := REAL_TO_INT(T24 * 10.0);
	END_FOR;
	pos := 0;
	sum := INT_TO_DINT(samples[0]) * 48;
	TA := (INT_TO_REAL(ft1.Y) * 0.1 + OFS) * SCALE;
	T24 := (DINT_TO_REAL(sum) * 0.00208333333333 + OFS) * SCALE;
	tp := TRUE;
ELSIF DTI > last THEN
	MinuteOfDay := HOUR(DT_TO_TOD(DTI)) * 60 + MINUTE(DT_TO_TOD(DTI));
	IF MinuteOfDay MOD 30 = 0 THEN
		last := ADD_DT_TIME(DTI, T#1m);
		sum := sum - INT_TO_DINT(samples[pos]);
		samples[pos] := ft1.Y;
		sum := sum + INT_TO_DINT(samples[pos]);
		pos := INC1(pos, 48);
		TA := (INT_TO_REAL(ft1.Y) * 0.1 + OFS) * SCALE;
		T24 := (DINT_TO_REAL(sum) * 0.00208333333333 + OFS) * SCALE;
		tmp_max := -32000;
		tmp_min := 32000;
		FOR i := 0 TO 47 DO
			tmp_max := MAX(tmp_max, samples[i]);
			tmp_min := MIN(tmp_min, samples[i]);
		END_FOR;
		T24_MAX := (INT_TO_REAL(tmp_max) * 0.1 + OFS) * SCALE;
		T24_MIN := (INT_TO_REAL(tmp_min) * 0.1 + OFS) * SCALE;
		tp := TRUE;
	ELSE
		tp := FALSE;
	END_IF;
ELSE
	tp := FALSE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK TANK_LEVEL
VAR_INPUT
    LEVEL : BOOL;
    LEAK : BOOL;
    ACLR : BOOL;
END_VAR
VAR_INPUT CONSTANT
    MAX_VALVE_TIME : TIME;
    LEVEL_DELAY_TIME : TIME;
END_VAR
VAR_OUTPUT
    VALVE : BOOL;
    ALARM : BOOL;
    STATUS : BYTE;
END_VAR
VAR
    cx : ACTUATOR_COIL;
    tn : TON;
    tl : TONOF;
    open : BOOL;
END_VAR

tl(in := level, T_ON := level_delay_time, T_OFF := level_delay_time);
open := tl.Q;
IF ALARM THEN
    IF ACLR THEN
        ALARM := FALSE;
        STATUS := BYTE#101;
        cx(in := FALSE);
    END_IF;
ELSE
    IF LEAK THEN
        cx(in := FALSE);
        ALARM := TRUE;
        STATUS := BYTE#1;
    ELSIF open THEN
        cx(in := TRUE);
        STATUS := BYTE#102;
    ELSE
        cx(in := FALSE);
        STATUS := BYTE#100;
    END_IF;
    tn(in := cx.OUT AND (MAX_VALVE_TIME > T#0s), PT := MAX_VALVE_TIME);
    IF tn.Q THEN
        ALARM := TRUE;
        STATUS := BYTE#2;
        cx(in := FALSE);
    END_IF;
    VALVE := cx.OUT;
END_IF;
END_FUNCTION_BLOCK

FUNCTION TANK_VOL1 : REAL
VAR_INPUT
    TR : REAL;
    TL : REAL;
    H : REAL;
END_VAR
TANK_VOL1 := CIRCLE_SEG(TR, H) * TL;
END_FUNCTION

FUNCTION TANK_VOL2 : REAL
VAR_INPUT
	TR : REAL;
	H : REAL;
END_VAR
TANK_VOL2 := math.PI * H * H * (TR - H/3.0);
END_FUNCTION

FUNCTION_BLOCK TEMP_EXT
VAR_INPUT
	t_ext1 : REAL;
	t_ext2 : REAL;
	t_ext3 : REAL;
	t_ext_config : BYTE;
	dt_in : DT;
END_VAR
VAR_INPUT CONSTANT
	t_ext_min : REAL := -40.0;
	t_ext_max : REAL := 60.0;
	t_ext_default : REAL := -10.0;
	heat_period_start : DATE := DATE#1970-09-01;
	heat_period_stop : DATE := DATE#1970-04-30;
	cool_period_start : DATE := DATE#1970-04-01;
	cool_period_stop : DATE := DATE#1970-09-30;
	heat_start_temp_day : REAL := 15.0;
	heat_start_temp_night : REAL := 10.0;
	heat_stop_temp : REAL := 18.0;
	cool_start_temp_day : REAL := 26.0;
	cool_start_temp_night : REAL := 26.0;
	cool_stop_temp : REAL := 24.0;
	start_day : TOD := TOD#09:00:00;
	start_night : TOD := TOD#21:00:00;
	CYCLE_TIME : TIME := t#10m;
END_VAR
VAR_OUTPUT
	t_ext : REAL;
	heat : BOOL;
	cool : BOOL;
END_VAR
VAR
	tx: DWORD;
	last_run: DWORD;
	init: BOOL;
	force_run: BOOL;
	cool_start: DATE;
	cool_stop: DATE;
	heat_start: DATE;
	heat_stop: DATE;
	xdate: DATE;
	day: BOOL;
END_VAR
IF NOT init THEN
    init := TRUE;
    force_run := TRUE;
    heat_start := SET_DATE(1972,MONTH_OF_DATE(heat_period_start),DAY_OF_MONTH(heat_period_start));
    heat_stop := SET_DATE(1972,MONTH_OF_DATE(heat_period_stop),DAY_OF_MONTH(heat_period_stop));
    cool_start := SET_DATE(1972,MONTH_OF_DATE(cool_period_start),DAY_OF_MONTH(cool_period_start));
    cool_stop := SET_DATE(1972,MONTH_OF_DATE(cool_period_stop),DAY_OF_MONTH(cool_period_stop));
END_IF;
tx := T_PLC_MS();
IF force_run OR DWORD_TO_UDINT(tx) - DWORD_TO_UDINT(last_run) >= DWORD_TO_UDINT(TIME_TO_DWORD(cycle_time)) THEN
    xdate := SET_DATE(1972,MONTH_OF_DATE(DT_TO_DATE(dt_in)),DAY_OF_MONTH(DT_TO_DATE(dt_in)));
    day := DT_TO_TOD(dt_in) >= start_day AND DT_TO_TOD(dt_in) < start_night;
    t_ext := MULTI_IN(t_ext1, t_ext2, t_ext3, t_ext_default, t_ext_min, t_ext_max, t_ext_config);
    IF (heat_start <= heat_stop AND xdate >= heat_start AND xdate <= heat_stop) OR (heat_start > heat_stop AND (xdate >= heat_start OR xdate <= heat_stop)) THEN
        IF day AND t_ext <= heat_start_temp_day THEN
            heat := TRUE;
        ELSIF NOT day AND t_ext <= heat_start_temp_night THEN
            heat := TRUE;
        ELSIF t_ext >= heat_stop_temp THEN
            heat := FALSE;
        END_IF;
    ELSE
        heat := FALSE;
    END_IF;
    IF (cool_start <= cool_stop AND xdate >= cool_start AND xdate <= cool_stop) OR (cool_start > cool_stop AND (xdate >= cool_start OR xdate <= cool_stop)) THEN
        IF day AND t_ext >= cool_start_temp_day THEN
            cool := TRUE;
        ELSIF NOT day AND t_ext >= cool_start_temp_night THEN
            cool := TRUE;
        ELSIF t_ext <= cool_stop_temp THEN
            cool := FALSE;
        END_IF;
    ELSE
        cool := FALSE;
    END_IF;
    last_run := tx;
    force_run := FALSE;
END_IF;
END_FUNCTION_BLOCK
FUNCTION WATER_CP : REAL
VAR_INPUT
	T : REAL;
END_VAR
VAR
	Data : ARRAY[1..20, 0..1] OF REAL;
	Pts : INT := 10;
END_VAR
Data[1,0] := 0.0;
Data[1,1] := 4.228;
Data[2,0] := 5.0;
Data[2,1] := 4.20;
Data[3,0] := 10.0;
Data[3,1] := 4.188;
Data[4,0] := 15.0;
Data[4,1] := 4.184;
Data[5,0] := 50.0;
Data[5,1] := 4.181;
Data[6,0] := 60.0;
Data[6,1] := 4.183;
Data[7,0] := 70.0;
Data[7,1] := 4.187;
Data[8,0] := 80.0;
Data[8,1] := 4.194;
Data[9,0] := 90.0;
Data[9,1] := 4.204;
Data[10,0] := 100.0;
Data[10,1] := 4.22;
WATER_CP := LINEAR_INT(T, Data, Pts);
END_FUNCTION

FUNCTION WATER_DENSITY : REAL
VAR_INPUT
	T : REAL;
	Sat : BOOL;
END_VAR
VAR CONSTANT
	a0 : REAL := 999.83952;
	a1 : REAL := 16.952577;
	a2 : REAL := -7.9905127E-3;
	a3 : REAL := -4.6241757E-5;
	a4 : REAL := 1.0584601E-7;
	a5 : REAL := -2.8103006E-10;
	b  : REAL := 0.0168872;
END_VAR
VAR
	T2: REAL;
	T4: REAL;
END_VAR
T2 := T * T;
T4 := T2 * T2;
WATER_DENSITY := (a0 + a1*T + a2*T2 + a3*T2*T + a4*T4 + a5*T4*T) / (1.0 + b*T);
IF sat THEN WATER_DENSITY := WATER_DENSITY - 0.004612 + 0.000106 * T; END_IF;
END_FUNCTION

FUNCTION WATER_ENTHALPY : REAL
VAR_INPUT
	T : REAL;
END_VAR
VAR
	Data : ARRAY[1..20, 0..1] OF REAL;
	Pts : INT := 11;
END_VAR
Data[1,0] := 0.0;
Data[1,1] := 0.06;
Data[2,0] := 10.0;
Data[2,1] := 42.1;
Data[3,0] := 20.0;
Data[3,1] := 83.9;
Data[4,0] := 30.0;
Data[4,1] := 125.8;
Data[5,0] := 40.0;
Data[5,1] := 167.58;
Data[6,0] := 50.0;
Data[6,1] := 209.4;
Data[7,0] := 60.0;
Data[7,1] := 251.2;
Data[8,0] := 70.0;
Data[8,1] := 293.1;
Data[9,0] := 80.0;
Data[9,1] := 335.0;
Data[10,0] := 90.0;
Data[10,1] := 377.0;
Data[11,0] := 100.0;
Data[11,1] := 419.1;
WATER_ENTHALPY := LINEAR_INT(T, Data, Pts);
END_FUNCTION

FUNCTION WCT : REAL
VAR_INPUT
	T : REAL;
	V : REAL;
END_VAR
IF V < 5.0 OR T > 10.0 THEN
	WCT := T;
ELSE
	WCT := 13.12 + 0.6215 * T +(0.3965 * T - 11.37) * EXP(LN(v) * 0.16);
END_IF;
END_FUNCTION
