(* OSCAT BASIC 3.33 signal-processing helpers. *)

FUNCTION BYTE_TO_RANGE : REAL
VAR_INPUT
    X : BYTE;
    low : REAL;
    high : REAL;
END_VAR
BYTE_TO_RANGE := (high - low) * DINT_TO_REAL(BYTE_TO_DINT(X)) / 255.0 + low;
END_FUNCTION

FUNCTION RANGE_TO_BYTE : BYTE
VAR_INPUT
    X : REAL;
    low : REAL;
    high : REAL;
END_VAR
VAR
    LimitedValue : REAL;
    ScaledValue : REAL;
    TruncatedValue : DINT;
END_VAR
LimitedValue := LIMIT(MN := low, IN := X, MX := high);
ScaledValue := (LimitedValue - low) * 255.0 / (high - low);
TruncatedValue := FLOOR2(X := ScaledValue);
RANGE_TO_BYTE := DINT_TO_BYTE(TruncatedValue);
END_FUNCTION

FUNCTION RANGE_TO_WORD : WORD
VAR_INPUT
    X : REAL;
    low : REAL;
    high : REAL;
END_VAR
VAR
    LimitedValue : REAL;
    ScaledValue : REAL;
    TruncatedValue : DINT;
END_VAR
LimitedValue := LIMIT(MN := low, IN := X, MX := high);
ScaledValue := (LimitedValue - low) * 65535.0 / (high - low);
TruncatedValue := FLOOR2(X := ScaledValue);
RANGE_TO_WORD := DINT_TO_WORD(TruncatedValue);
END_FUNCTION

FUNCTION SCALE : REAL
VAR_INPUT
    X : REAL;
    K : REAL;
    O : REAL;
    MX : REAL;
    MN : REAL;
END_VAR
SCALE := LIMIT(MN := MN, IN := X * K + O, MX := MX);
END_FUNCTION

FUNCTION WORD_TO_RANGE : REAL
VAR_INPUT
    X : WORD;
    low : REAL;
    high : REAL;
END_VAR
WORD_TO_RANGE := (high - low) * DINT_TO_REAL(WORD_TO_DINT(X)) / 65535.0 + low;
END_FUNCTION

FUNCTION_BLOCK DELAY
VAR_INPUT
    IN : REAL;
    N : INT;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    OUT : REAL;
END_VAR
VAR
    BUF : ARRAY[0..31] OF REAL;
    I : INT;
    INIT : BOOL;
    STOP : INT;
    WINDOW : INT;
END_VAR

WINDOW := LIMIT(INT#0, N, INT#32);
STOP := WINDOW - INT#1;
IF RST OR NOT INIT THEN
    INIT := TRUE;
    FOR I := INT#0 TO STOP DO
        BUF[I] := IN;
    END_FOR;
    OUT := IN;
    I := INT#0;
ELSIF STOP < INT#0 THEN
    OUT := IN;
ELSE
    OUT := BUF[I];
    BUF[I] := IN;
    I := INC1(X := I, N := WINDOW);
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK TREND_DW
VAR_INPUT
    X : DWORD;
END_VAR
VAR_OUTPUT
    Q : BOOL;
    TU : BOOL;
    TD : BOOL;
    D : DWORD;
END_VAR
VAR
    LastX : DWORD;
END_VAR

IF X > LastX THEN
    TU := TRUE;
    TD := FALSE;
    D := UDINT_TO_DWORD(DWORD_TO_UDINT(X) - DWORD_TO_UDINT(LastX));
    Q := TRUE;
ELSIF X < LastX THEN
    TD := TRUE;
    TU := FALSE;
    D := UDINT_TO_DWORD(DWORD_TO_UDINT(LastX) - DWORD_TO_UDINT(X));
    Q := FALSE;
ELSE
    TU := FALSE;
    TD := FALSE;
    D := DWORD#0;
END_IF;
LastX := X;
END_FUNCTION_BLOCK

FUNCTION AIN : REAL
VAR_INPUT
    in : DWORD;
END_VAR
VAR_INPUT CONSTANT
    Bits : BYTE;
    sign : BYTE := BYTE#255;
    low : REAL;
    high : REAL := 10.0;
END_VAR
VAR CONSTANT
    ff : DWORD := DWORD#16#FFFFFFFF;
END_VAR
VAR
    Temp1 : DWORD;
    Temp2 : DWORD;
    SignedValue : BOOL;
END_VAR

IF sign < BYTE#32 THEN
    Temp1 := SHR(in, BYTE_TO_INT(sign));
    SignedValue := (Temp1 AND DWORD#1) = DWORD#1;
ELSE
    SignedValue := FALSE;
END_IF;
Temp1 := SHR(ff, INT#32 - BYTE_TO_INT(Bits));
Temp2 := in AND Temp1;
AIN := (high - low) * DWORD_TO_REAL(Temp2) / DWORD_TO_REAL(Temp1) + low;
IF SignedValue THEN
    AIN := -AIN;
END_IF;
END_FUNCTION

FUNCTION_BLOCK AIN1
VAR_INPUT
    in : DWORD;
END_VAR
VAR_INPUT CONSTANT
    sign_bit : INT := INT#255;
    error_bit : INT := INT#255;
    error_code_en : BOOL;
    error_code : DWORD;
    overflow_bit : INT := INT#255;
    overflow_code_en : BOOL;
    overflow_code : DWORD;
    Bit_0 : INT;
    Bit_N : INT := INT#31;
    out_min : REAL;
    out_max : REAL := 10.0;
    code_min : DWORD;
    code_max : DWORD := DWORD#16#FFFFFFFF;
    error_output : REAL;
    overflow_output : REAL := 10.0;
END_VAR
VAR_OUTPUT
    out : REAL;
    sign : BOOL;
    error : BOOL;
    overflow : BOOL;
END_VAR
VAR
    DataBits : DWORD;
END_VAR

error := ((SHR(in, error_bit) AND DWORD#1) = DWORD#1) OR (error_code_en AND error_code = in);
IF error THEN
    out := error_output;
ELSE
    DataBits := SHR(SHL(in, INT#31 - Bit_N), INT#31 - Bit_N + Bit_0);
    overflow := ((SHR(in, overflow_bit) AND DWORD#1) = DWORD#1)
        OR (overflow_code_en AND overflow_code = in)
        OR (DataBits < code_min OR DataBits > code_max);
    IF overflow THEN
        out := overflow_output;
    ELSE
        sign := (SHR(in, sign_bit) AND DWORD#1) = DWORD#1;
        out := UDINT_TO_REAL(DWORD_TO_UDINT(DataBits) - DWORD_TO_UDINT(code_min)) * (out_max - out_min) / UDINT_TO_REAL(DWORD_TO_UDINT(code_max) - DWORD_TO_UDINT(code_min)) + out_min;
        IF sign THEN
            out := out * -1.0;
        END_IF;
    END_IF;
END_IF;
END_FUNCTION_BLOCK

FUNCTION AOUT : DWORD
VAR_INPUT
    in : REAL;
END_VAR
VAR_INPUT CONSTANT
    Bits : BYTE;
    sign : BYTE := BYTE#255;
    low : REAL;
    high : REAL := 10.0;
END_VAR
VAR CONSTANT
    ff : DWORD := DWORD#1;
END_VAR
VAR
    InputValue : REAL;
    SignedValue : BOOL;
END_VAR

IF sign < BYTE#32 THEN
    SignedValue := SIGN_R(IN := in);
    InputValue := ABS(in);
ELSE
    InputValue := in;
END_IF;

InputValue := LIMIT(MN := low, IN := InputValue, MX := high);
AOUT := UDINT_TO_DWORD(
    REAL_TO_UDINT(
        (InputValue - low) / (high - low) * UDINT_TO_REAL(DWORD_TO_UDINT(SHL(ff, BYTE_TO_INT(Bits))) - UDINT#1)
    )
);
IF SignedValue THEN
    AOUT := SHL(ff, BYTE_TO_INT(sign)) OR AOUT;
END_IF;
END_FUNCTION

FUNCTION AOUT1 : DWORD
VAR_INPUT
    in : REAL;
END_VAR
VAR_INPUT CONSTANT
    Bit_0 : INT;
    Bit_N : INT := INT#31;
    sign : INT := INT#255;
    low : REAL;
    high : REAL := 10.0;
END_VAR
VAR CONSTANT
    ff : DWORD := DWORD#1;
END_VAR
VAR
    SignedValue : BOOL;
    InputValue : REAL;
END_VAR

IF sign < INT#32 THEN
    SignedValue := SIGN_R(IN := in);
    InputValue := ABS(in);
ELSE
    InputValue := in;
END_IF;

InputValue := LIMIT(MN := low, IN := InputValue, MX := high);
AOUT1 := SHL(
    UDINT_TO_DWORD(
        REAL_TO_UDINT(
            (InputValue - low) / (high - low) * UDINT_TO_REAL(DWORD_TO_UDINT(SHL(ff, Bit_N - Bit_0 + INT#1)) - UDINT#1)
        )
    ),
    Bit_0
);
IF SignedValue THEN
    AOUT1 := SHL(ff, sign) OR AOUT1;
END_IF;
END_FUNCTION

FUNCTION_BLOCK DELAY_4
VAR_INPUT
    in : REAL;
END_VAR
VAR_OUTPUT
    out1 : REAL;
    out2 : REAL;
    out3 : REAL;
    out4 : REAL;
END_VAR
VAR
    Temp : REAL;
END_VAR

out4 := out3;
out3 := out2;
out2 := out1;
out1 := Temp;
Temp := in;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FADE
VAR_INPUT
    IN1 : REAL;
    IN2 : REAL;
    F : BOOL;
    TF : TIME;
    rst : BOOL;
END_VAR
VAR_OUTPUT
    Y : REAL;
END_VAR
VAR
    Ramp : RMP_W;
END_VAR

Ramp(RST := rst AND NOT F, SET_HIGH := rst AND F, PT := TF, UP := F);
Y := (IN2 - IN1) / 65535.0 * DINT_TO_REAL(WORD_TO_DINT(Ramp.OUT)) + IN1;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FILTER_DW
VAR_INPUT
    X : DWORD;
    T : TIME;
END_VAR
VAR_OUTPUT
    Y : DWORD;
END_VAR
VAR
    Last : DWORD;
    Tx : DWORD;
    Init : BOOL;
    Yi : REAL;
    PeriodMs : REAL;
END_VAR

Tx := T_PLC_MS();
IF NOT Init OR T = T#0s THEN
    Init := TRUE;
    Yi := DWORD_TO_REAL(X);
ELSE
    PeriodMs := DWORD_TO_REAL(TIME_TO_DWORD(T));
    Yi := Yi + (DWORD_TO_REAL(X) - DWORD_TO_REAL(Y)) * UDINT_TO_REAL(DWORD_TO_UDINT(Tx) - DWORD_TO_UDINT(Last)) / PeriodMs;
END_IF;
Last := Tx;
Y := REAL_TO_DWORD(Yi);
END_FUNCTION_BLOCK

FUNCTION_BLOCK FILTER_I
VAR_INPUT
    X : INT;
    T : TIME;
END_VAR
VAR_OUTPUT
    Y : INT;
END_VAR
VAR
    Yi : DINT;
    Last : DWORD;
    Tx : DWORD;
    Init : BOOL;
END_VAR

Tx := T_PLC_MS();
IF NOT Init OR T = T#0s THEN
    Init := TRUE;
    Yi := INT_TO_DINT(X) * DINT#1000;
ELSE
    Yi := Yi + INT_TO_DINT(X - Y) * (DWORD_TO_DINT(Tx) - DWORD_TO_DINT(Last)) * DINT#1000 / DWORD_TO_DINT(TIME_TO_DWORD(T));
END_IF;
Last := Tx;
Y := DINT_TO_INT(Yi / DINT#1000);
END_FUNCTION_BLOCK

FUNCTION_BLOCK FILTER_MAV_DW
VAR_INPUT
    X : DWORD;
    N : UINT;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    Y : DWORD;
END_VAR
VAR
    Init : BOOL;
    Buffer : ARRAY[0..31] OF DWORD;
    I : INT;
    Window : UINT;
    WindowInt : INT;
    YReal : REAL;
END_VAR

Window := LIMIT(MN := UINT#0, IN := N, MX := UINT#32);
WindowInt := UINT_TO_INT(Window);
IF NOT Init OR RST OR Window = UINT#0 THEN
    Init := TRUE;
    IF WindowInt > INT#0 THEN
        FOR I := INT#0 TO WindowInt - INT#1 DO
            Buffer[I] := X;
        END_FOR;
    END_IF;
    I := INT#0;
    Y := X;
ELSIF WindowInt > INT#0 THEN
    I := INC1(X := I, N := WindowInt);
    YReal := DWORD_TO_REAL(Y) + (DWORD_TO_REAL(X) - DWORD_TO_REAL(Buffer[I])) / UINT_TO_REAL(Window);
    Y := REAL_TO_DWORD(YReal);
    Buffer[I] := X;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FILTER_MAV_W
VAR_INPUT
    X : WORD;
    N : UINT;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    Y : WORD;
END_VAR
VAR
    Init : BOOL;
    Buffer : ARRAY[0..31] OF WORD;
    I : INT;
    Sum : UDINT;
    Window : UINT;
    WindowInt : INT;
END_VAR

Window := LIMIT(MN := UINT#0, IN := N, MX := UINT#32);
WindowInt := UINT_TO_INT(Window);
IF NOT Init OR RST OR Window = UINT#0 THEN
    Init := TRUE;
    IF WindowInt > INT#0 THEN
        FOR I := INT#0 TO WindowInt - INT#1 DO
            Buffer[I] := X;
        END_FOR;
        Sum := WORD_TO_UDINT(X) * UINT_TO_UDINT(Window);
    ELSE
        Sum := UDINT#0;
    END_IF;
    I := INT#0;
    Y := X;
ELSIF WindowInt > INT#0 THEN
    I := INC1(X := I, N := WindowInt);
    Sum := Sum + WORD_TO_UDINT(X) - WORD_TO_UDINT(Buffer[I]);
    Buffer[I] := X;
    Y := UDINT_TO_WORD(Sum / UINT_TO_UDINT(Window));
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FILTER_W
VAR_INPUT
    X : WORD;
    T : TIME;
END_VAR
VAR_OUTPUT
    Y : WORD;
END_VAR
VAR
    Last : DWORD;
    Tx : DWORD;
    Init : BOOL;
    Delta : DINT;
    StepValue : DINT;
END_VAR

Tx := T_PLC_MS();
IF NOT Init OR T = T#0s THEN
    Init := TRUE;
    Last := Tx;
    Y := X;
ELSIF Y = X THEN
    Last := Tx;
ELSE
    Delta := WORD_TO_DINT(X) - WORD_TO_DINT(Y);
    StepValue := Delta * (DWORD_TO_DINT(Tx) - DWORD_TO_DINT(Last)) / DWORD_TO_DINT(TIME_TO_DWORD(T));
    IF StepValue <> DINT#0 THEN
        Y := DINT_TO_WORD(LIMIT(MN := DINT#0, IN := WORD_TO_DINT(Y) + StepValue, MX := DINT#65535));
        Last := Tx;
    END_IF;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FILTER_WAV
VAR_INPUT
    X : REAL;
    W : ARRAY[0..15] OF REAL;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    Y : REAL;
END_VAR
VAR
    Init : BOOL;
    Buffer : ARRAY[0..15] OF REAL;
    I : INT;
    N : INT;
    Cursor : INT;
END_VAR

IF NOT Init OR RST THEN
    Init := TRUE;
    FOR I := INT#0 TO INT#15 DO
        Buffer[I] := X;
    END_FOR;
    I := INT#15;
    Y := X;
ELSE
    I := INC1(X := I, N := INT#16);
    Buffer[I] := X;
END_IF;

Y := 0.0;
Cursor := I;
FOR N := INT#0 TO INT#15 DO
    Y := Buffer[Cursor] * W[N] + Y;
    Cursor := DEC1(X := Cursor, N := INT#16);
END_FOR;
END_FUNCTION_BLOCK

FUNCTION MIX : REAL
VAR_INPUT
    A : REAL;
    B : REAL;
    M : REAL;
END_VAR
MIX := (1.0 - M) * A + M * B;
END_FUNCTION

FUNCTION MUX_R2 : REAL
VAR_INPUT
    IN0 : REAL;
    IN1 : REAL;
    A : BOOL;
END_VAR
MUX_R2 := SEL(G := A, IN0 := IN0, IN1 := IN1);
END_FUNCTION

FUNCTION MUX_R4 : REAL
VAR_INPUT
    IN0 : REAL;
    IN1 : REAL;
    IN2 : REAL;
    IN3 : REAL;
    A0 : BOOL;
    A1 : BOOL;
END_VAR
IF A1 THEN
    MUX_R4 := SEL(G := A0, IN0 := IN2, IN1 := IN3);
ELSE
    MUX_R4 := SEL(G := A0, IN0 := IN0, IN1 := IN1);
END_IF;
END_FUNCTION

FUNCTION OFFSET : REAL
VAR_INPUT
    X : REAL;
    O1 : BOOL;
    O2 : BOOL;
    O3 : BOOL;
    O4 : BOOL;
    D : BOOL;
END_VAR
VAR_INPUT CONSTANT
    Offset_1 : REAL;
    Offset_2 : REAL;
    Offset_3 : REAL;
    Offset_4 : REAL;
    DEFAULT_VALUE : REAL;
END_VAR

IF D THEN
    OFFSET := DEFAULT_VALUE;
ELSE
    OFFSET := X;
END_IF;
IF O1 THEN OFFSET := OFFSET + Offset_1; END_IF;
IF O2 THEN OFFSET := OFFSET + Offset_2; END_IF;
IF O3 THEN OFFSET := OFFSET + Offset_3; END_IF;
IF O4 THEN OFFSET := OFFSET + Offset_4; END_IF;
END_FUNCTION

FUNCTION OFFSET2 : REAL
VAR_INPUT
    X : REAL;
    O1 : BOOL;
    O2 : BOOL;
    O3 : BOOL;
    O4 : BOOL;
    D : BOOL;
END_VAR
VAR_INPUT CONSTANT
    Offset_1 : REAL;
    Offset_2 : REAL;
    Offset_3 : REAL;
    Offset_4 : REAL;
    DEFAULT_VALUE : REAL;
END_VAR

IF D THEN
    OFFSET2 := DEFAULT_VALUE;
ELSE
    OFFSET2 := X;
END_IF;
IF O4 THEN
    OFFSET2 := OFFSET2 + Offset_4;
ELSIF O3 THEN
    OFFSET2 := OFFSET2 + Offset_3;
ELSIF O2 THEN
    OFFSET2 := OFFSET2 + Offset_2;
ELSIF O1 THEN
    OFFSET2 := OFFSET2 + Offset_1;
END_IF;
END_FUNCTION

FUNCTION OVERRIDE_3 : REAL
VAR_INPUT
    X1 : REAL;
    X2 : REAL;
    X3 : REAL;
    E1 : BOOL;
    E2 : BOOL;
    E3 : BOOL;
END_VAR

OVERRIDE_3 := 0.0;
IF E1 THEN OVERRIDE_3 := X1; END_IF;
IF E2 AND ABS(X2) > ABS(OVERRIDE_3) THEN OVERRIDE_3 := X2; END_IF;
IF E3 AND ABS(X3) > ABS(OVERRIDE_3) THEN OVERRIDE_3 := X3; END_IF;
END_FUNCTION

FUNCTION SCALE_B : REAL
VAR_INPUT
    X : BYTE;
    I_LO : BYTE;
    I_HI : BYTE;
    O_LO : REAL;
    O_HI : REAL;
END_VAR
VAR
    Limited : BYTE;
END_VAR

IF I_HI = I_LO THEN
    SCALE_B := O_LO;
ELSE
    Limited := LIMIT(MN := I_LO, IN := X, MX := I_HI);
    SCALE_B := (O_HI - O_LO) / DINT_TO_REAL(BYTE_TO_DINT(I_HI) - BYTE_TO_DINT(I_LO))
        * DINT_TO_REAL(BYTE_TO_DINT(Limited) - BYTE_TO_DINT(I_LO)) + O_LO;
END_IF;
END_FUNCTION

FUNCTION SCALE_B2 : REAL
VAR_INPUT
    in1 : BYTE;
    in2 : BYTE;
    K : REAL;
    O : REAL;
END_VAR
VAR_INPUT CONSTANT
    in1_min : REAL;
    in1_max : REAL := 1000.0;
    in2_min : REAL;
    in2_max : REAL := 1000.0;
END_VAR

SCALE_B2 := (((in1_max - in1_min) * DINT_TO_REAL(BYTE_TO_DINT(in1))
    + (in2_max - in2_min) * DINT_TO_REAL(BYTE_TO_DINT(in2))) * 0.003921569
    + in1_min + in2_min) * K + O;
END_FUNCTION

FUNCTION SCALE_B4 : REAL
VAR_INPUT
    in1 : BYTE;
    in2 : BYTE;
    in3 : BYTE;
    in4 : BYTE;
    K : REAL;
    O : REAL;
END_VAR
VAR_INPUT CONSTANT
    in1_min : REAL;
    in1_max : REAL := 1000.0;
    in2_min : REAL;
    in2_max : REAL := 1000.0;
    in3_min : REAL;
    in3_max : REAL := 1000.0;
    in4_min : REAL;
    in4_max : REAL := 1000.0;
END_VAR

SCALE_B4 := (((in1_max - in1_min) * DINT_TO_REAL(BYTE_TO_DINT(in1))
    + (in2_max - in2_min) * DINT_TO_REAL(BYTE_TO_DINT(in2))
    + (in3_max - in3_min) * DINT_TO_REAL(BYTE_TO_DINT(in3))
    + (in4_max - in4_min) * DINT_TO_REAL(BYTE_TO_DINT(in4))) * 0.003921569
    + in1_min + in2_min + in3_min + in4_min) * K + O;
END_FUNCTION

FUNCTION SCALE_B8 : REAL
VAR_INPUT
    in1 : BYTE;
    in2 : BYTE;
    in3 : BYTE;
    in4 : BYTE;
    in5 : BYTE;
    in6 : BYTE;
    in7 : BYTE;
    in8 : BYTE;
    K : REAL;
    O : REAL;
END_VAR
VAR_INPUT CONSTANT
    in1_min : REAL;
    in1_max : REAL := 1000.0;
    in2_min : REAL;
    in2_max : REAL := 1000.0;
    in3_min : REAL;
    in3_max : REAL := 1000.0;
    in4_min : REAL;
    in4_max : REAL := 1000.0;
    in5_min : REAL;
    in5_max : REAL := 1000.0;
    in6_min : REAL;
    in6_max : REAL := 1000.0;
    in7_min : REAL;
    in7_max : REAL := 1000.0;
    in8_min : REAL;
    in8_max : REAL := 1000.0;
END_VAR

SCALE_B8 := (((in1_max - in1_min) * DINT_TO_REAL(BYTE_TO_DINT(in1))
    + (in2_max - in2_min) * DINT_TO_REAL(BYTE_TO_DINT(in2))
    + (in3_max - in3_min) * DINT_TO_REAL(BYTE_TO_DINT(in3))
    + (in4_max - in4_min) * DINT_TO_REAL(BYTE_TO_DINT(in4))
    + (in5_max - in5_min) * DINT_TO_REAL(BYTE_TO_DINT(in5))
    + (in6_max - in6_min) * DINT_TO_REAL(BYTE_TO_DINT(in6))
    + (in7_max - in7_min) * DINT_TO_REAL(BYTE_TO_DINT(in7))
    + (in8_max - in8_min) * DINT_TO_REAL(BYTE_TO_DINT(in8))) * 0.003921569
    + in1_min + in2_min + in3_min + in4_min + in5_min + in6_min + in7_min + in8_min) * K + O;
END_FUNCTION

FUNCTION SCALE_D : REAL
VAR_INPUT
    X : DWORD;
    I_LO : DWORD;
    I_HI : DWORD;
    O_LO : REAL;
    O_HI : REAL;
END_VAR

IF I_HI = I_LO THEN
    SCALE_D := O_LO;
ELSE
    SCALE_D := (O_HI - O_LO) / UDINT_TO_REAL(DWORD_TO_UDINT(I_HI) - DWORD_TO_UDINT(I_LO))
        * UDINT_TO_REAL(DWORD_TO_UDINT(LIMIT(MN := I_LO, IN := X, MX := I_HI)) - DWORD_TO_UDINT(I_LO)) + O_LO;
END_IF;
END_FUNCTION

FUNCTION SCALE_R : REAL
VAR_INPUT
    X : REAL;
    I_LO : REAL;
    I_HI : REAL;
    O_LO : REAL;
    O_HI : REAL;
END_VAR

IF I_LO = I_HI THEN
    SCALE_R := O_LO;
ELSE
    SCALE_R := (O_HI - O_LO) / (I_HI - I_LO) * (LIMIT(MN := I_LO, IN := X, MX := I_HI) - I_LO) + O_LO;
END_IF;
END_FUNCTION

FUNCTION SCALE_X2 : REAL
VAR_INPUT
    IN1 : BOOL;
    IN2 : BOOL;
    K : REAL;
    O : REAL;
END_VAR
VAR_INPUT CONSTANT
    IN1_MIN : REAL;
    IN1_MAX : REAL := 1000.0;
    IN2_MIN : REAL;
    IN2_MAX : REAL := 1000.0;
END_VAR

SCALE_X2 := (SEL(G := IN1, IN0 := IN1_MIN, IN1 := IN1_MAX) + SEL(G := IN2, IN0 := IN2_MIN, IN1 := IN2_MAX)) * K + O;
END_FUNCTION

FUNCTION SCALE_X4 : REAL
VAR_INPUT
    IN1 : BOOL;
    IN2 : BOOL;
    IN3 : BOOL;
    IN4 : BOOL;
    K : REAL;
    O : REAL;
END_VAR
VAR_INPUT CONSTANT
    IN1_MIN : REAL;
    IN1_MAX : REAL := 1000.0;
    IN2_MIN : REAL;
    IN2_MAX : REAL := 1000.0;
    IN3_MIN : REAL;
    IN3_MAX : REAL := 1000.0;
    IN4_MIN : REAL;
    IN4_MAX : REAL := 1000.0;
END_VAR

SCALE_X4 := (
    SEL(G := IN1, IN0 := IN1_MIN, IN1 := IN1_MAX)
    + SEL(G := IN2, IN0 := IN2_MIN, IN1 := IN2_MAX)
    + SEL(G := IN3, IN0 := IN3_MIN, IN1 := IN3_MAX)
    + SEL(G := IN4, IN0 := IN4_MIN, IN1 := IN4_MAX)
) * K + O;
END_FUNCTION

FUNCTION SCALE_X8 : REAL
VAR_INPUT
    IN1 : BOOL;
    IN2 : BOOL;
    IN3 : BOOL;
    IN4 : BOOL;
    IN5 : BOOL;
    IN6 : BOOL;
    IN7 : BOOL;
    IN8 : BOOL;
    K : REAL;
    O : REAL;
END_VAR
VAR_INPUT CONSTANT
    IN1_MIN : REAL;
    IN1_MAX : REAL := 1000.0;
    IN2_MIN : REAL;
    IN2_MAX : REAL := 1000.0;
    IN3_MIN : REAL;
    IN3_MAX : REAL := 1000.0;
    IN4_MIN : REAL;
    IN4_MAX : REAL := 1000.0;
    IN5_MIN : REAL;
    IN5_MAX : REAL := 1000.0;
    IN6_MIN : REAL;
    IN6_MAX : REAL := 1000.0;
    IN7_MIN : REAL;
    IN7_MAX : REAL := 1000.0;
    IN8_MIN : REAL;
    IN8_MAX : REAL := 1000.0;
END_VAR

SCALE_X8 := (
    SEL(G := IN1, IN0 := IN1_MIN, IN1 := IN1_MAX)
    + SEL(G := IN2, IN0 := IN2_MIN, IN1 := IN2_MAX)
    + SEL(G := IN3, IN0 := IN3_MIN, IN1 := IN3_MAX)
    + SEL(G := IN4, IN0 := IN4_MIN, IN1 := IN4_MAX)
    + SEL(G := IN5, IN0 := IN5_MIN, IN1 := IN5_MAX)
    + SEL(G := IN6, IN0 := IN6_MIN, IN1 := IN6_MAX)
    + SEL(G := IN7, IN0 := IN7_MIN, IN1 := IN7_MAX)
    + SEL(G := IN8, IN0 := IN8_MIN, IN1 := IN8_MAX)
) * K + O;
END_FUNCTION

FUNCTION_BLOCK SEL2_OF_3
VAR_INPUT
    IN1 : REAL;
    IN2 : REAL;
    IN3 : REAL;
    TOL : REAL;
END_VAR
VAR_OUTPUT
    Y : REAL;
    W : INT;
    E : BOOL;
END_VAR
VAR
    D12 : BOOL;
    D23 : BOOL;
    D31 : BOOL;
END_VAR

D12 := ABS(IN1 - IN2) <= TOL;
D23 := ABS(IN2 - IN3) <= TOL;
D31 := ABS(IN3 - IN1) <= TOL;

IF (D12 AND D23) OR (D12 AND D31) OR (D23 AND D31) THEN
    Y := (IN1 + IN2 + IN3) * 0.333333333333;
    E := FALSE;
    W := INT#0;
ELSIF D12 THEN
    Y := (IN1 + IN2) * 0.5;
    E := FALSE;
    W := INT#3;
ELSIF D23 THEN
    Y := (IN2 + IN3) * 0.5;
    E := FALSE;
    W := INT#1;
ELSIF D31 THEN
    Y := (IN3 + IN1) * 0.5;
    E := FALSE;
    W := INT#2;
ELSE
    E := TRUE;
    W := INT#4;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK SEL2_OF_3B
VAR_INPUT
    IN1 : BOOL;
    IN2 : BOOL;
    IN3 : BOOL;
    TD : TIME;
END_VAR
VAR_OUTPUT
    Q : BOOL;
    W : BOOL;
END_VAR
VAR
    Delay : TON;
END_VAR

Q := (IN1 AND IN2) OR (IN1 AND IN3) OR (IN2 AND IN3);
Delay(IN := (IN1 XOR IN2) OR (IN1 XOR IN3) OR (IN2 XOR IN3), PT := TD);
W := Delay.Q;
END_FUNCTION_BLOCK

FUNCTION_BLOCK SH
VAR_INPUT
    in : REAL;
    CLK : BOOL;
END_VAR
VAR_OUTPUT
    out : REAL;
    trig : BOOL;
END_VAR
VAR
    Edge : BOOL;
END_VAR

IF CLK AND NOT Edge THEN
    out := in;
    trig := TRUE;
ELSE
    trig := FALSE;
END_IF;
Edge := CLK;
END_FUNCTION_BLOCK

FUNCTION_BLOCK SH_1
VAR_INPUT
    in : REAL;
    PT : TIME;
END_VAR
VAR_OUTPUT
    out : REAL;
    trig : BOOL;
END_VAR
VAR
    LastMs : UDINT;
    TxMs : UDINT;
END_VAR

TxMs := DWORD_TO_UDINT(T_PLC_MS());
IF TxMs - LastMs >= DWORD_TO_UDINT(TIME_TO_DWORD(PT)) THEN
    LastMs := TxMs;
    out := in;
    trig := TRUE;
ELSE
    trig := FALSE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK SH_2
VAR_INPUT
    in : REAL;
    PT : TIME;
    N : INT := 16;
    disc : INT;
END_VAR
VAR_OUTPUT
    out : REAL;
    trig : BOOL;
    avg : REAL;
    high : REAL;
    low : REAL;
END_VAR
VAR
    M : INT;
    buf : ARRAY[0..15] OF REAL;
    buf2 : ARRAY[0..15] OF REAL;
    LastMs : UDINT;
    I : INT;
    StartIdx : INT;
    Temp : REAL;
    StopIdx : INT;
    TxMs : UDINT;
    D2 : INT;
END_VAR

TxMs := DWORD_TO_UDINT(T_PLC_MS());
D2 := disc / 2;

IF TxMs - LastMs >= DWORD_TO_UDINT(TIME_TO_DWORD(PT)) THEN
    LastMs := TxMs;
    trig := TRUE;
    M := LIMIT(MN := INT#1, IN := N, MX := INT#16);
    FOR I := M - 1 TO 1 BY -1 DO
        buf2[I] := buf2[I - 1];
    END_FOR;
    buf2[0] := in;
    out := in;
    buf := buf2;

    FOR StartIdx := 0 TO M - 2 DO
        FOR I := StartIdx + 1 TO M - 1 DO
            IF buf[StartIdx] > buf[I] THEN
                Temp := buf[StartIdx];
                buf[StartIdx] := buf[I];
                buf[I] := Temp;
            END_IF;
        END_FOR;
    END_FOR;

    StopIdx := M - 1 - D2;
    StartIdx := D2;
    IF NOT EVEN(IN := INT_TO_DINT(disc)) THEN
        StartIdx := StartIdx + 1;
    END_IF;
    avg := 0.0;
    FOR I := StartIdx TO StopIdx DO
        avg := avg + buf[I];
    END_FOR;
    avg := avg / INT_TO_REAL(StopIdx - StartIdx + 1);
    low := buf[StartIdx];
    high := buf[StopIdx];
ELSE
    trig := FALSE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK SH_T
VAR_INPUT
    IN : REAL;
    E : BOOL;
END_VAR
VAR_OUTPUT
    OUT : REAL;
END_VAR

IF E THEN
    OUT := IN;
END_IF;
END_FUNCTION_BLOCK

FUNCTION STAIR : REAL
VAR_INPUT
    X : REAL;
    STEP_SIZE : REAL;
END_VAR

IF STEP_SIZE > 0.0 THEN
    STAIR := DINT_TO_REAL(REAL_TO_DINT(X / STEP_SIZE)) * STEP_SIZE;
ELSE
    STAIR := X;
END_IF;
END_FUNCTION

FUNCTION_BLOCK STAIR2
VAR_INPUT
    X : REAL;
    STEP_SIZE : REAL;
END_VAR
VAR_OUTPUT
    Y : REAL;
END_VAR

IF STEP_SIZE > 0.0 THEN
    IF X >= Y + STEP_SIZE OR X <= Y - STEP_SIZE THEN
        Y := FLOOR(X := X / STEP_SIZE) * STEP_SIZE;
    END_IF;
ELSE
    Y := X;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK TREND
VAR_INPUT
    X : REAL;
END_VAR
VAR_OUTPUT
    Q : BOOL;
    TU : BOOL;
    TD : BOOL;
    D : REAL;
END_VAR
VAR
    LastX : REAL;
END_VAR

TU := X > LastX;
TD := X < LastX;
Q := TU OR TD;
D := X - LastX;
LastX := X;
END_FUNCTION_BLOCK
