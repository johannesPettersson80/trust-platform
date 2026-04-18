(* OSCAT BASIC 3.33 Chapter 5 Mathematics helpers. *)

FUNCTION ACOSH : REAL
VAR_INPUT
    X : REAL;
END_VAR
ACOSH := LN(SQRT(X * X - 1.0) + X);
END_FUNCTION

FUNCTION ACOTH : REAL
VAR_INPUT
    X : REAL;
END_VAR
ACOTH := LN((X + 1.0) / (X - 1.0)) * 0.5;
END_FUNCTION

FUNCTION AGDF : REAL
VAR_INPUT
    X : REAL;
END_VAR
AGDF := LN((1.0 + SIN(X)) / COS(X));
END_FUNCTION

FUNCTION ASINH : REAL
VAR_INPUT
    X : REAL;
END_VAR
ASINH := LN(SQRT(X * X + 1.0) + X);
END_FUNCTION

FUNCTION ATAN2 : REAL
VAR_INPUT
    Y : REAL;
    X : REAL;
END_VAR
IF X > 0.0 THEN
    ATAN2 := ATAN(Y / X);
ELSIF X < 0.0 THEN
    IF Y >= 0.0 THEN
        ATAN2 := ATAN(Y / X) + MATH.PI;
    ELSE
        ATAN2 := ATAN(Y / X) - MATH.PI;
    END_IF;
ELSIF Y > 0.0 THEN
    ATAN2 := MATH.PI05;
ELSIF Y < 0.0 THEN
    ATAN2 := -MATH.PI05;
ELSE
    ATAN2 := 0.0;
END_IF;
END_FUNCTION

FUNCTION ATANH : REAL
VAR_INPUT
    X : REAL;
END_VAR
ATANH := LN((X + 1.0) / (1.0 - X)) * 0.5;
END_FUNCTION

FUNCTION BETA : REAL
VAR_INPUT
    X : REAL;
    Y : REAL;
END_VAR
BETA := GAMMA(X := X) * GAMMA(X := Y) / GAMMA(X := X + Y);
END_FUNCTION

FUNCTION BINOM : DINT
VAR_INPUT
    N : INT;
    K : INT;
END_VAR
VAR
    Result : DINT;
    WorkN : INT;
    WorkK : INT;
    I : INT;
END_VAR
WorkN := N;
WorkK := K;
IF 2 * WorkK > WorkN THEN
    WorkK := WorkN - WorkK;
END_IF;
IF WorkK > WorkN THEN
    BINOM := DINT#0;
ELSIF WorkK = 0 OR WorkK = WorkN THEN
    BINOM := DINT#1;
ELSIF WorkK = 1 THEN
    BINOM := INT_TO_DINT(WorkN);
ELSE
    Result := INT_TO_DINT(WorkN);
    WorkN := WorkN + 1;
    FOR I := 2 TO WorkK DO
        Result := Result * INT_TO_DINT(WorkN - I) / INT_TO_DINT(I);
    END_FOR;
    BINOM := Result;
END_IF;
END_FUNCTION

FUNCTION CAUCHY : REAL
VAR_INPUT
    X : REAL;
    T : REAL;
    U : REAL;
END_VAR
VAR
    Temp : REAL;
END_VAR
Temp := X - T;
CAUCHY := MATH.PI_INV * U / (U * U + Temp * Temp);
END_FUNCTION

FUNCTION CAUCHYCD : REAL
VAR_INPUT
    X : REAL;
    T : REAL;
    U : REAL;
END_VAR
CAUCHYCD := 0.5 + MATH.PI_INV * ATAN((X - T) / U);
END_FUNCTION

FUNCTION CEIL : INT
VAR_INPUT
    X : REAL;
END_VAR
CEIL := REAL_TO_INT(X);
IF INT_TO_REAL(CEIL) < X THEN
    CEIL := CEIL + 1;
END_IF;
END_FUNCTION

FUNCTION CEIL2 : DINT
VAR_INPUT
    X : REAL;
END_VAR
CEIL2 := REAL_TO_DINT(X);
IF DINT_TO_REAL(CEIL2) < X THEN
    CEIL2 := CEIL2 + 1;
END_IF;
END_FUNCTION

FUNCTION CMP : BOOL
VAR_INPUT
    X : REAL;
    Y : REAL;
    N : INT;
END_VAR
VAR
    ScaleFactor : REAL;
    LogValue : REAL;
    DecadeExponent : INT;
    XDigits : DINT;
    YDigits : DINT;
END_VAR
ScaleFactor := ABS(X);
IF ScaleFactor > 0.0 THEN
    LogValue := LOG(ScaleFactor);
    DecadeExponent := FLOOR(X := LogValue) - N + 1;
    ScaleFactor := EXP10(X := INT_TO_REAL(DecadeExponent));
    XDigits := FLOOR2(X := X / ScaleFactor);
    YDigits := FLOOR2(X := Y / ScaleFactor);
    CMP := XDigits = YDigits;
ELSE
    CMP := X = Y;
END_IF;
END_FUNCTION

FUNCTION COSH : REAL
VAR_INPUT
    X : REAL;
END_VAR
VAR
    T : REAL;
END_VAR
T := EXP(X);
COSH := (1.0 / T + T) * 0.5;
END_FUNCTION

FUNCTION COTH : REAL
VAR_INPUT
    X : REAL;
END_VAR
IF X > 20.0 THEN
    COTH := 1.0;
ELSIF X < -20.0 THEN
    COTH := -1.0;
ELSE
    COTH := 1.0 + 2.0 / (EXP(X * 2.0) - 1.0);
END_IF;
END_FUNCTION

FUNCTION D_TRUNC : DINT
VAR_INPUT
    X : REAL;
END_VAR
D_TRUNC := REAL_TO_DINT(X);
IF X > 0.0 THEN
    IF DINT_TO_REAL(D_TRUNC) > X THEN
        D_TRUNC := D_TRUNC - 1;
    END_IF;
ELSE
    IF DINT_TO_REAL(D_TRUNC) < X THEN
        D_TRUNC := D_TRUNC + 1;
    END_IF;
END_IF;
END_FUNCTION

FUNCTION DEC1 : INT
VAR_INPUT
    X : INT;
    N : INT;
END_VAR
IF X = 0 THEN
    DEC1 := N - 1;
ELSE
    DEC1 := X - 1;
END_IF;
END_FUNCTION

FUNCTION DEG : REAL
VAR_INPUT
    RAD : REAL;
END_VAR
DEG := MODR(IN := 57.29577951308232 * RAD, DIVI := 360.0);
END_FUNCTION

FUNCTION DIFFER : BOOL
VAR_INPUT
    IN1 : REAL;
    IN2 : REAL;
    X : REAL;
END_VAR
DIFFER := ABS(IN1 - IN2) > X;
END_FUNCTION

FUNCTION ERF : REAL
VAR_INPUT
    X : REAL;
END_VAR
VAR
    X2 : REAL;
    AX2 : REAL;
END_VAR
X2 := X * X;
AX2 := 0.147 * X2 + 1.0;
ERF := SQRT(1.0 - EXP(-X2 * ((0.27323954473516 + AX2) / AX2))) * INT_TO_REAL(SGN(X := X));
END_FUNCTION

FUNCTION ERFC : REAL
VAR_INPUT
    X : REAL;
END_VAR
ERFC := 1.0 - ERF(X := X);
END_FUNCTION

FUNCTION EVEN : BOOL
VAR_INPUT
    IN : DINT;
END_VAR
EVEN := (IN MOD 2) = 0;
END_FUNCTION

FUNCTION EXP10 : REAL
VAR_INPUT
    X : REAL;
END_VAR
EXP10 := EXP(X * 2.30258509299405);
END_FUNCTION

FUNCTION EXPN : REAL
VAR_INPUT
    X : REAL;
    N : INT;
END_VAR
VAR
    Base : REAL;
    Exponent : INT;
    NegativeExponent : BOOL;
END_VAR
Base := X;
Exponent := N;
NegativeExponent := Exponent < 0;
Exponent := ABS(Exponent);
IF (Exponent MOD 2) = 1 THEN
    EXPN := Base;
ELSE
    EXPN := 1.0;
END_IF;
Exponent := Exponent / 2;
WHILE Exponent > 0 DO
    Base := Base * Base;
    IF (Exponent MOD 2) = 1 THEN
        EXPN := EXPN * Base;
    END_IF;
    Exponent := Exponent / 2;
END_WHILE;
IF NegativeExponent THEN
    EXPN := 1.0 / EXPN;
END_IF;
END_FUNCTION

FUNCTION FACT : DINT
VAR_INPUT
    X : INT;
END_VAR
VAR
    I : INT;
END_VAR
IF X < 0 OR X > 12 THEN
    FACT := -1;
ELSIF X < 2 THEN
    FACT := 1;
ELSE
    FACT := 1;
    FOR I := 2 TO X DO
        FACT := FACT * INT_TO_DINT(I);
    END_FOR;
END_IF;
END_FUNCTION

FUNCTION FIB : DINT
VAR_INPUT
    X : INT;
END_VAR
VAR
    WorkX : INT;
    T1 : DINT;
    T2 : DINT;
END_VAR
WorkX := X;
IF WorkX < 0 OR WorkX > 46 THEN
    FIB := -1;
ELSIF WorkX < 2 THEN
    FIB := INT_TO_DINT(WorkX);
ELSE
    T2 := 1;
    WHILE WorkX > 3 DO
        WorkX := WorkX - 2;
        T1 := T1 + T2;
        T2 := T1 + T2;
    END_WHILE;
    IF WorkX > 2 THEN
        T1 := T1 + T2;
    END_IF;
    FIB := T1 + T2;
END_IF;
END_FUNCTION

FUNCTION FLOOR : INT
VAR_INPUT
    X : REAL;
END_VAR
FLOOR := REAL_TO_INT(X);
IF INT_TO_REAL(FLOOR) > X THEN
    FLOOR := FLOOR - 1;
END_IF;
END_FUNCTION

FUNCTION FLOOR2 : DINT
VAR_INPUT
    X : REAL;
END_VAR
FLOOR2 := REAL_TO_DINT(X);
IF DINT_TO_REAL(FLOOR2) > X THEN
    FLOOR2 := FLOOR2 - 1;
END_IF;
END_FUNCTION

FUNCTION FRACT : REAL
VAR_INPUT
    X : REAL;
END_VAR
VAR
    IntegerPart : DINT;
END_VAR
IF ABS(X) < 2.0E9 THEN
    IntegerPart := D_TRUNC(X := X);
    FRACT := X - DINT_TO_REAL(IntegerPart);
ELSE
    FRACT := 0.0;
END_IF;
END_FUNCTION

FUNCTION GAMMA : REAL
VAR_INPUT
    X : REAL;
END_VAR
IF X > 0.0 THEN
    GAMMA := SQRT(MATH.PI2 / X) * EXPT(MATH.E_INV * (X + 1.0 / (12.0 * X - 0.1 / X)), X);
END_IF;
END_FUNCTION

FUNCTION GAUSS : REAL
VAR_INPUT
    X : REAL;
    U : REAL;
    SI : REAL;
END_VAR
VAR
    Temp : REAL;
    SIInv : REAL;
END_VAR
IF SI = 0.0 THEN
    GAUSS := 0.0;
ELSE
    Temp := X - U;
    SIInv := 1.0 / SI;
    GAUSS := EXP(Temp * Temp * SIInv * SIInv * -0.5) * 0.39894228 * SIInv;
END_IF;
END_FUNCTION

FUNCTION GAUSSCD : REAL
VAR_INPUT
    X : REAL;
    U : REAL;
    SI : REAL;
END_VAR
IF SI = 0.0 THEN
    IF X < U THEN
        GAUSSCD := 0.0;
    ELSE
        GAUSSCD := 1.0;
    END_IF;
ELSE
    GAUSSCD := (ERF(X := (X - U) / (SI * 1.414213562)) + 1.0) * 0.5;
END_IF;
END_FUNCTION

FUNCTION GCD : INT
VAR_INPUT
    A : DINT;
    B : DINT;
END_VAR
VAR
    Left : DINT;
    Right : DINT;
    Temp : DINT;
END_VAR
Left := ABS(A);
Right := ABS(B);
WHILE Right <> 0 DO
    Temp := Left MOD Right;
    Left := Right;
    Right := Temp;
END_WHILE;
GCD := DINT_TO_INT(Left);
END_FUNCTION

FUNCTION GDF : REAL
VAR_INPUT
    X : REAL;
END_VAR
IF X = 0.0 THEN
    GDF := 0.0;
ELSIF X > 15.0 THEN
    GDF := MATH.PI05;
ELSIF X < -15.0 THEN
    GDF := -MATH.PI05;
ELSE
    GDF := ATAN(EXP(X)) * 2.0 - MATH.PI05;
END_IF;
END_FUNCTION

FUNCTION GOLD : REAL
VAR_INPUT
    X : REAL;
END_VAR
GOLD := (X + SQRT(X * X + 4.0)) * 0.5;
END_FUNCTION

FUNCTION HYPOT : REAL
VAR_INPUT
    X : REAL;
    Y : REAL;
END_VAR
HYPOT := SQRT(X * X + Y * Y);
END_FUNCTION

FUNCTION INC : INT
VAR_INPUT
    X : INT;
    D : INT;
    M : INT;
END_VAR
INC := (X + D + M + 1) MOD (M + 1);
END_FUNCTION

FUNCTION INC1 : INT
VAR_INPUT
    X : INT;
    N : INT;
END_VAR
IF X >= N - 1 THEN
    INC1 := 0;
ELSE
    INC1 := X + 1;
END_IF;
END_FUNCTION

FUNCTION INC2 : INT
VAR_INPUT
    X : INT;
    D : INT;
    L : INT;
    U : INT;
END_VAR
VAR
    Span : INT;
END_VAR
Span := U - L + 1;
INC2 := (X + D - L + Span) MOD Span + L;
END_FUNCTION

FUNCTION INV : REAL
VAR_INPUT
    X : REAL;
END_VAR
IF X <> 0.0 THEN
    INV := 1.0 / X;
ELSE
    INV := 0.0;
END_IF;
END_FUNCTION

FUNCTION LAMBERT_W : REAL
VAR_INPUT
    X : REAL;
END_VAR
VAR
    W : REAL;
    NextW : REAL;
    We : REAL;
    W1e : REAL;
    Ewx : REAL;
    I : INT;
END_VAR
IF X < -0.367879441171442 THEN
    LAMBERT_W := -1000.0;
ELSIF X = 0.0 THEN
    LAMBERT_W := 0.0;
ELSE
    IF X <= 500.0 THEN
        W := LN(X + 1.0);
        W := 0.665 * (1.0 + 0.0195 * W) * W + 0.04;
    ELSE
        W := LN(X - 4.0) - (1.0 - 1.0 / LN(X)) * LN(LN(X));
    END_IF;
    FOR I := 0 TO 5 DO
        Ewx := EXP(W);
        We := W * Ewx - X;
        W1e := (W + 1.0) * Ewx;
        NextW := W - (We / (W1e - (W + 2.0) * We / (2.0 * W + 2.0)));
        IF ABS(NextW - W) < 1.0E-6 THEN
            W := NextW;
            EXIT;
        END_IF;
        W := NextW;
    END_FOR;
    LAMBERT_W := W;
END_IF;
END_FUNCTION

FUNCTION LANGEVIN : REAL
VAR_INPUT
    X : REAL;
END_VAR
IF X = 0.0 THEN
    LANGEVIN := 0.0;
ELSE
    LANGEVIN := COTH(X := X) - 1.0 / X;
END_IF;
END_FUNCTION

FUNCTION MAX3 : REAL
VAR_INPUT
    IN1 : REAL;
    IN2 : REAL;
    IN3 : REAL;
END_VAR
MAX3 := MAX(MAX(IN1, IN2), IN3);
END_FUNCTION

FUNCTION MID3 : REAL
VAR_INPUT
    IN1 : REAL;
    IN2 : REAL;
    IN3 : REAL;
END_VAR
VAR
    A : REAL;
    B : REAL;
END_VAR
A := IN1;
B := IN2;
IF A > B THEN
    MID3 := A;
    A := B;
    B := MID3;
END_IF;
IF B > IN3 THEN
    B := IN3;
END_IF;
MID3 := SEL(A > B, B, A);
END_FUNCTION

FUNCTION MIN3 : REAL
VAR_INPUT
    IN1 : REAL;
    IN2 : REAL;
    IN3 : REAL;
END_VAR
MIN3 := MIN(MIN(IN1, IN2), IN3);
END_FUNCTION

FUNCTION MODR : REAL
VAR_INPUT
    IN : REAL;
    DIVI : REAL;
END_VAR
VAR
    QuotientFloor : DINT;
END_VAR
IF DIVI = 0.0 THEN
    MODR := 0.0;
ELSE
    QuotientFloor := FLOOR2(X := IN / DIVI);
    MODR := IN - DINT_TO_REAL(QuotientFloor) * DIVI;
END_IF;
END_FUNCTION

FUNCTION MUL_ADD : REAL
VAR_INPUT
    X : REAL;
    K : REAL;
    O : REAL;
END_VAR
MUL_ADD := X * K + O;
END_FUNCTION

FUNCTION NEGX : REAL
VAR_INPUT
    X : REAL;
END_VAR
NEGX := -X;
END_FUNCTION

FUNCTION RAD : REAL
VAR_INPUT
    DEG : REAL;
END_VAR
RAD := MODR(IN := 0.0174532925199433 * DEG, DIVI := MATH.PI2);
END_FUNCTION

FUNCTION RDM : REAL
VAR_INPUT
    LAST : REAL;
END_VAR
VAR
    Tn : DWORD;
    Tc : INT;
    Seed : REAL;
END_VAR
Tn := T_PLC_MS();
Tc := BIT_COUNT(IN := Tn);
Tn := ROL(Tn OR DWORD#16#80000001, Tc MOD 32);
Seed := LIMIT(0.0, LAST, 1.0);
RDM := FRACT(X := ABS(SIN(DWORD_TO_REAL(Tn) * 0.0001 + Seed * MATH.PI2)) * 10000.0);
END_FUNCTION

FUNCTION RDM2 : INT
VAR_INPUT
    LAST : INT;
    LOW : INT;
    HIGH : INT;
END_VAR
VAR
    Span : INT;
END_VAR
Span := HIGH - LOW + 1;
RDM2 := DINT_TO_INT(D_TRUNC(X := RDM(LAST := FRACT(X := INT_TO_REAL(LAST) * MATH.PI)) * INT_TO_REAL(Span))) + LOW;
END_FUNCTION

FUNCTION RDMDW : DWORD
VAR_INPUT
    LAST : DWORD;
END_VAR
VAR
    RX : REAL;
    M : REAL;
END_VAR
M := INT_TO_REAL(BIT_COUNT(IN := LAST));
RX := RDM(LAST := FRACT(X := M * MATH.PI));
RDMDW := SHL(REAL_TO_DWORD(RX * 65535.0), 16);
RX := RDM(LAST := FRACT(X := M * MATH.E));
RDMDW := RDMDW OR (REAL_TO_DWORD(RX * 65535.0) AND DWORD#16#0000FFFF);
END_FUNCTION

FUNCTION REAL_TO_FRAC : FRACTION
VAR_INPUT
    X : REAL;
    N : INT;
END_VAR
VAR
    Result : FRACTION;
    Temp : DINT;
    IsNegative : BOOL;
    WorkX : REAL;
    Rounded : DINT;
    IntegerPart : REAL;
    Numerator : DINT := 1;
    Denominator : DINT := 0;
    NumeratorOld : DINT := 0;
    DenominatorOld : DINT := 1;
END_VAR
WorkX := X;
IF WorkX < 0.0 THEN
    IsNegative := TRUE;
    WorkX := ABS(WorkX);
END_IF;
REPEAT
    Rounded := REAL_TO_DINT(WorkX);
    Temp := Numerator * Rounded + NumeratorOld;
    NumeratorOld := Numerator;
    Numerator := Temp;
    Temp := Denominator * Rounded + DenominatorOld;
    DenominatorOld := Denominator;
    Denominator := Temp;
    IntegerPart := DINT_TO_REAL(Rounded);
    IF WorkX = IntegerPart THEN
        IF ABS(Denominator) <= N THEN
            NumeratorOld := Numerator;
            DenominatorOld := Denominator;
        END_IF;
        EXIT;
    ELSE
        WorkX := 1.0 / (WorkX - IntegerPart);
    END_IF;
UNTIL ABS(Denominator) > N
END_REPEAT;
IF IsNegative THEN
    Result.NUMERATOR := -ABS(DINT_TO_INT(NumeratorOld));
ELSE
    Result.NUMERATOR := ABS(DINT_TO_INT(NumeratorOld));
END_IF;
Result.DENOMINATOR := ABS(DINT_TO_INT(DenominatorOld));
REAL_TO_FRAC := Result;
END_FUNCTION

FUNCTION RND : REAL
VAR_INPUT
    X : REAL;
    N : INT;
END_VAR
VAR
    M : REAL;
END_VAR
IF X = 0.0 THEN
    RND := 0.0;
ELSE
    M := EXPN(X := 10.0, N := N - CEIL(X := LOG(ABS(X))));
    RND := DINT_TO_REAL(D_TRUNC(X := X * M)) / M;
END_IF;
END_FUNCTION

FUNCTION ROUND : REAL
VAR_INPUT
    IN : REAL;
    N : INT;
END_VAR
VAR
    Scale : REAL;
    Rounded : DINT;
END_VAR
Scale := EXPN(X := 10.0, N := LIMIT(0, N, 8));
IF IN >= 0.0 THEN
    Rounded := D_TRUNC(X := IN * Scale + 0.5);
ELSE
    Rounded := D_TRUNC(X := IN * Scale - 0.5);
END_IF;
ROUND := DINT_TO_REAL(Rounded) / Scale;
END_FUNCTION

FUNCTION SGN : INT
VAR_INPUT
    X : REAL;
END_VAR
IF X > 0.0 THEN
    SGN := 1;
ELSIF X < 0.0 THEN
    SGN := -1;
ELSE
    SGN := 0;
END_IF;
END_FUNCTION

FUNCTION SIGMOID : REAL
VAR_INPUT
    X : REAL;
END_VAR
IF X > 20.0 THEN
    SIGMOID := 1.0;
ELSIF X < -85.0 THEN
    SIGMOID := 0.0;
ELSE
    SIGMOID := 1.0 / (1.0 + EXP(-X));
END_IF;
END_FUNCTION

FUNCTION SIGN_I : BOOL
VAR_INPUT
    IN : DINT;
END_VAR
SIGN_I := IN < 0;
END_FUNCTION

FUNCTION SIGN_R : BOOL
VAR_INPUT
    IN : REAL;
END_VAR
SIGN_R := IN < 0.0;
END_FUNCTION

FUNCTION SINC : REAL
VAR_INPUT
    X : REAL;
END_VAR
IF X = 0.0 THEN
    SINC := 1.0;
ELSE
    SINC := SIN(X) / X;
END_IF;
END_FUNCTION

FUNCTION SINH : REAL
VAR_INPUT
    X : REAL;
END_VAR
IF ABS(X) < 2.0E-3 THEN
    SINH := X;
ELSE
    SINH := (EXP(X) - EXP(-X)) * 0.5;
END_IF;
END_FUNCTION

FUNCTION SQRTN : REAL
VAR_INPUT
    X : REAL;
    N : INT;
END_VAR
IF N > 0 THEN
    IF X = 0.0 THEN
        SQRTN := 0.0;
    ELSIF X > 0.0 THEN
        SQRTN := EXP(LN(X) / INT_TO_REAL(N));
    ELSE
        SQRTN := 0.0;
    END_IF;
ELSE
    SQRTN := 0.0;
END_IF;
END_FUNCTION

FUNCTION TANC : REAL
VAR_INPUT
    X : REAL;
END_VAR
IF X = 0.0 THEN
    TANC := 1.0;
ELSE
    TANC := TAN(X) / X;
END_IF;
END_FUNCTION

FUNCTION TANH : REAL
VAR_INPUT
    X : REAL;
END_VAR
IF X > 20.0 THEN
    TANH := 1.0;
ELSIF X < -20.0 THEN
    TANH := -1.0;
ELSE
    TANH := 1.0 - 2.0 / (EXP(2.0 * X) + 1.0);
END_IF;
END_FUNCTION

FUNCTION WINDOW : BOOL
VAR_INPUT
    LOW : REAL;
    IN : REAL;
    HIGH : REAL;
END_VAR
WINDOW := IN < HIGH AND IN > LOW;
END_FUNCTION

FUNCTION WINDOW2 : BOOL
VAR_INPUT
    LOW : REAL;
    IN : REAL;
    HIGH : REAL;
END_VAR
WINDOW2 := IN >= LOW AND IN <= HIGH;
END_FUNCTION
