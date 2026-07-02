(* OSCAT BASIC 3.33 sensor helpers. *)

FUNCTION MULTI_IN : REAL
VAR_INPUT
    in_1 : REAL;
    in_2 : REAL;
    in_3 : REAL;
    default : REAL;
    in_min : REAL;
    in_max : REAL;
    mode : BYTE;
END_VAR
VAR
    count : INT;
    F1 : BOOL;
    F2 : BOOL;
    F3 : BOOL;
END_VAR
F1 := in_1 > in_min AND in_1 < in_max;
F2 := in_2 > in_min AND in_2 < in_max;
F3 := in_3 > in_min AND in_3 < in_max;

CASE mode OF
BYTE#0:
    count := 0;
    IF F1 THEN
        count := count + 1;
        MULTI_IN := in_1;
    ELSE
        MULTI_IN := 0.0;
    END_IF;
    IF F2 THEN
        count := count + 1;
        MULTI_IN := MULTI_IN + in_2;
    END_IF;
    IF F3 THEN
        count := count + 1;
        MULTI_IN := MULTI_IN + in_3;
    END_IF;
    IF count = 0 THEN
        MULTI_IN := default;
    ELSE
        MULTI_IN := MULTI_IN / DINT_TO_REAL(INT_TO_DINT(count));
    END_IF;
BYTE#1:
    MULTI_IN := SEL(G := F1, IN0 := default, IN1 := in_1);
BYTE#2:
    MULTI_IN := SEL(G := F2, IN0 := default, IN1 := in_2);
BYTE#3:
    MULTI_IN := SEL(G := F3, IN0 := default, IN1 := in_3);
BYTE#4:
    MULTI_IN := default;
BYTE#5:
    MULTI_IN := SEL(G := F1, IN0 := in_max, IN1 := in_1);
    IF F2 AND in_2 < MULTI_IN THEN
        MULTI_IN := in_2;
    END_IF;
    IF F3 AND in_3 < MULTI_IN THEN
        MULTI_IN := in_3;
    END_IF;
    IF MULTI_IN = in_max THEN
        MULTI_IN := default;
    END_IF;
BYTE#6:
    MULTI_IN := SEL(G := F1, IN0 := in_min, IN1 := in_1);
    IF F2 AND in_2 > MULTI_IN THEN
        MULTI_IN := in_2;
    END_IF;
    IF F3 AND in_3 > MULTI_IN THEN
        MULTI_IN := in_3;
    END_IF;
    IF MULTI_IN = in_min THEN
        MULTI_IN := default;
    END_IF;
BYTE#7:
    IF F1 AND F2 AND F3 THEN
        MULTI_IN := MID3(IN1 := in_1, IN2 := in_2, IN3 := in_3);
    ELSIF F1 AND F2 THEN
        MULTI_IN := MIN(IN1 := in_1, IN2 := in_2);
    ELSIF F1 AND F3 THEN
        MULTI_IN := MIN(IN1 := in_1, IN2 := in_3);
    ELSIF F2 AND F3 THEN
        MULTI_IN := MIN(IN1 := in_2, IN2 := in_3);
    ELSIF F1 THEN
        MULTI_IN := in_1;
    ELSIF F2 THEN
        MULTI_IN := in_2;
    ELSIF F3 THEN
        MULTI_IN := in_3;
    ELSE
        MULTI_IN := default;
    END_IF;
ELSE
    MULTI_IN := 0.0;
END_CASE;
END_FUNCTION

FUNCTION RES_NI : REAL
VAR_INPUT
    T : REAL;
    R0 : REAL;
END_VAR
VAR CONSTANT
    A : REAL := 0.5485;
    B : REAL := 0.665E-3;
    C : REAL := 2.805E-9;
END_VAR
VAR
    T2 : REAL;
END_VAR
T2 := T * T;
RES_NI := R0 + A * T + B * T2 + C * T2 * T2;
END_FUNCTION

FUNCTION RES_NTC : REAL
VAR_INPUT
    T : REAL;
    RN : REAL;
    B : REAL;
END_VAR
RES_NTC := RN * EXP(B * (1.0 / (T + 273.15) - 0.00335401643468053));
END_FUNCTION

FUNCTION RES_PT : REAL
VAR_INPUT
    T : REAL;
    R0 : REAL;
END_VAR
VAR CONSTANT
    A : REAL := 3.90802E-3;
    B : REAL := -5.802E-7;
    C : REAL := -4.27350E-12;
END_VAR
VAR
    T2 : REAL;
END_VAR
T2 := T * T;
IF T >= 0.0 THEN
    RES_PT := R0 * (1.0 + A * T + B * T2);
ELSE
    RES_PT := R0 * (1.0 + A * T + B * T2 + C * (T - 100.0) * T2 * T);
END_IF;
END_FUNCTION

FUNCTION RES_SI : REAL
VAR_INPUT
    T : REAL;
    RS : REAL;
    TS : REAL;
END_VAR
VAR CONSTANT
    A : REAL := 7.64E-3;
    B : REAL := 1.66E-5;
END_VAR
VAR
    TX : REAL;
END_VAR
TX := T - TS;
RES_SI := RS * (1.0 + A * TX + B * TX * TX);
END_FUNCTION

FUNCTION SENSOR_INT : REAL
VAR_INPUT
    Voltage : REAL;
    Current : REAL;
    RP : REAL;
    RS : REAL;
END_VAR
VAR
    RG : REAL;
END_VAR
IF Current <> 0.0 THEN
    RG := Voltage / Current;
    SENSOR_INT := RP * (RG - RS) / (RP + RS - RG);
END_IF;
END_FUNCTION

FUNCTION TEMP_NI : REAL
VAR_INPUT
    Res : REAL;
    R0 : REAL;
END_VAR
TEMP_NI := (SQRT(0.30085225 - 2.66E-3 * (R0 - Res)) - 0.5485) * 751.8796992;
END_FUNCTION

FUNCTION TEMP_NTC : REAL
VAR_INPUT
    Res : REAL;
    RN : REAL;
    B : REAL;
END_VAR
IF Res > 0.0 THEN
    TEMP_NTC := B * 298.15 / (B + LN(Res / RN) * 298.15) - 273.15;
END_IF;
END_FUNCTION

FUNCTION TEMP_PT : REAL
VAR_INPUT
    Res : REAL;
    R0 : REAL;
END_VAR
VAR CONSTANT
    A : REAL := 3.9083E-3;
    B : REAL := -5.775E-7;
    accuracy : REAL := 0.01;
END_VAR
VAR
    StepSize : REAL := 50.0;
    X : REAL;
    Y : REAL;
    t1 : REAL;
END_VAR
X := A * R0;
Y := B * R0;
IF Res >= R0 THEN
    t1 := X * X - 4.0 * Y * (R0 - Res);
    IF t1 < 0.0 THEN
        TEMP_PT := 10000.0;
    ELSE
        TEMP_PT := (-X + SQRT(t1)) / (2.0 * Y);
    END_IF;
ELSE
    TEMP_PT := -100.0;
    WHILE StepSize > accuracy DO
        IF RES_PT(T := TEMP_PT, R0 := R0) < Res THEN
            TEMP_PT := TEMP_PT + StepSize;
        ELSE
            TEMP_PT := TEMP_PT - StepSize;
        END_IF;
        StepSize := StepSize * 0.5;
    END_WHILE;
END_IF;
END_FUNCTION

FUNCTION TEMP_SI : REAL
VAR_INPUT
    Res : REAL;
    RS : REAL;
    TS : REAL;
END_VAR
TEMP_SI := (-7.64E-3 + SQRT(Res / RS * 6.64E-5 - 0.803E-5)) * 30120.48193 + TS;
END_FUNCTION
