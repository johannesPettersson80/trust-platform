(* OSCAT BASIC 3.33 Chapter 7 complex mathematics helpers. *)

FUNCTION CABS : REAL
VAR_INPUT
    X : COMPLEX;
END_VAR
CABS := HYPOT(X := X.RE, Y := X.IM);
END_FUNCTION

FUNCTION CACOS : COMPLEX
VAR_INPUT
    X : COMPLEX;
END_VAR
VAR
    Y : COMPLEX;
    Result : COMPLEX;
END_VAR
Y := CACOSH(X := X);
Result.RE := Y.IM;
Result.IM := -Y.RE;
CACOS := Result;
END_FUNCTION

FUNCTION CACOSH : COMPLEX
VAR_INPUT
    X : COMPLEX;
END_VAR
VAR
    Y : COMPLEX;
END_VAR
Y.RE := (X.RE - X.IM) * (X.RE + X.IM) - REAL#1.0;
Y.IM := REAL#2.0 * X.RE * X.IM;
Y := CSQRT(X := Y);
Y.RE := Y.RE + X.RE;
Y.IM := Y.IM + X.IM;
CACOSH := CLOG(X := Y);
END_FUNCTION

FUNCTION CADD : COMPLEX
VAR_INPUT
    X : COMPLEX;
    Y : COMPLEX;
END_VAR
VAR
    Result : COMPLEX;
END_VAR
Result.RE := X.RE + Y.RE;
Result.IM := X.IM + Y.IM;
CADD := Result;
END_FUNCTION

FUNCTION CARG : REAL
VAR_INPUT
    X : COMPLEX;
END_VAR
CARG := ATAN2(Y := X.IM, X := X.RE);
END_FUNCTION

FUNCTION CASIN : COMPLEX
VAR_INPUT
    X : COMPLEX;
END_VAR
VAR
    Y : COMPLEX;
    Result : COMPLEX;
END_VAR
Y.RE := -X.IM;
Y.IM := X.RE;
Y := CASINH(X := Y);
Result.RE := Y.IM;
Result.IM := -Y.RE;
CASIN := Result;
END_FUNCTION

FUNCTION CASINH : COMPLEX
VAR_INPUT
    X : COMPLEX;
END_VAR
VAR
    Y : COMPLEX;
END_VAR
Y.RE := (X.RE - X.IM) * (X.RE + X.IM) + REAL#1.0;
Y.IM := REAL#2.0 * X.RE * X.IM;
Y := CSQRT(X := Y);
Y.RE := Y.RE + X.RE;
Y.IM := Y.IM + X.IM;
CASINH := CLOG(X := Y);
END_FUNCTION

FUNCTION CATAN : COMPLEX
VAR_INPUT
    X : COMPLEX;
END_VAR
VAR
    RealSquared : REAL;
    Numerator : REAL;
    Denominator : REAL;
    Result : COMPLEX;
END_VAR
RealSquared := X.RE * X.RE;
Denominator := REAL#1.0 - RealSquared - X.IM * X.IM;
Result.RE := REAL#0.5 * ATAN(REAL#2.0 * X.RE / Denominator);
Numerator := X.IM + REAL#1.0;
Numerator := RealSquared + Numerator * Numerator;
Denominator := X.IM - REAL#1.0;
Denominator := RealSquared + Denominator * Denominator;
Result.IM := REAL#0.25 * (LN(Numerator) - LN(Denominator));
CATAN := Result;
END_FUNCTION

FUNCTION CATANH : COMPLEX
VAR_INPUT
    X : COMPLEX;
END_VAR
VAR
    ImagSquared : REAL;
    Numerator : REAL;
    Denominator : REAL;
    Result : COMPLEX;
END_VAR
ImagSquared := X.IM * X.IM;
Numerator := REAL#1.0 + X.RE;
Numerator := ImagSquared + Numerator * Numerator;
Denominator := REAL#1.0 - X.RE;
Denominator := ImagSquared + Denominator * Denominator;
Result.RE := REAL#0.25 * (LN(Numerator) - LN(Denominator));
Denominator := REAL#1.0 - X.RE * X.RE - ImagSquared;
Result.IM := REAL#0.5 * ATAN(REAL#2.0 * X.IM / Denominator);
CATANH := Result;
END_FUNCTION

FUNCTION CCON : COMPLEX
VAR_INPUT
    X : COMPLEX;
END_VAR
VAR
    Result : COMPLEX;
END_VAR
Result.RE := X.RE;
Result.IM := -X.IM;
CCON := Result;
END_FUNCTION

FUNCTION CCOS : COMPLEX
VAR_INPUT
    X : COMPLEX;
END_VAR
VAR
    Result : COMPLEX;
END_VAR
Result := CCOSH(X := CSET(RE := -X.IM, IM := X.RE));
CCOS := Result;
END_FUNCTION

FUNCTION CCOSH : COMPLEX
VAR_INPUT
    X : COMPLEX;
END_VAR
VAR
    Result : COMPLEX;
END_VAR
Result.RE := COSH(X := X.RE) * COS(X.IM);
Result.IM := SINH(X := X.RE) * SIN(X.IM);
CCOSH := Result;
END_FUNCTION

FUNCTION CDIV : COMPLEX
VAR_INPUT
    X : COMPLEX;
    Y : COMPLEX;
END_VAR
VAR
    Temp : REAL;
    Result : COMPLEX;
END_VAR
Temp := Y.RE * Y.RE + Y.IM * Y.IM;
Result.RE := (X.RE * Y.RE + X.IM * Y.IM) / Temp;
Result.IM := (X.IM * Y.RE - X.RE * Y.IM) / Temp;
CDIV := Result;
END_FUNCTION

FUNCTION CEXP : COMPLEX
VAR_INPUT
    X : COMPLEX;
END_VAR
VAR
    Temp : REAL;
    Result : COMPLEX;
END_VAR
Temp := EXP(X.RE);
Result.RE := Temp * COS(X.IM);
Result.IM := Temp * SIN(X.IM);
CEXP := Result;
END_FUNCTION

FUNCTION CINV : COMPLEX
VAR_INPUT
    X : COMPLEX;
END_VAR
VAR
    Temp : REAL;
    Result : COMPLEX;
END_VAR
Temp := X.RE * X.RE + X.IM * X.IM;
Result.RE := X.RE / Temp;
Result.IM := -X.IM / Temp;
CINV := Result;
END_FUNCTION

FUNCTION CLOG : COMPLEX
VAR_INPUT
    X : COMPLEX;
END_VAR
VAR
    Result : COMPLEX;
END_VAR
Result.RE := LN(HYPOT(X := X.RE, Y := X.IM));
Result.IM := ATAN2(Y := X.IM, X := X.RE);
CLOG := Result;
END_FUNCTION

FUNCTION CMUL : COMPLEX
VAR_INPUT
    X : COMPLEX;
    Y : COMPLEX;
END_VAR
VAR
    Result : COMPLEX;
END_VAR
Result.RE := X.RE * Y.RE - X.IM * Y.IM;
Result.IM := X.RE * Y.IM + X.IM * Y.RE;
CMUL := Result;
END_FUNCTION

FUNCTION CPOL : COMPLEX
VAR_INPUT
    L : REAL;
    A : REAL;
END_VAR
VAR
    Result : COMPLEX;
END_VAR
Result.RE := L * COS(A);
Result.IM := L * SIN(A);
CPOL := Result;
END_FUNCTION

FUNCTION CPOW : COMPLEX
VAR_INPUT
    X : COMPLEX;
    Y : COMPLEX;
END_VAR
VAR
    Result : COMPLEX;
END_VAR
Result := CEXP(X := CMUL(X := Y, Y := CLOG(X := X)));
CPOW := Result;
END_FUNCTION

FUNCTION CSET : COMPLEX
VAR_INPUT
    RE : REAL;
    IM : REAL;
END_VAR
VAR
    Result : COMPLEX;
END_VAR
Result.RE := RE;
Result.IM := IM;
CSET := Result;
END_FUNCTION

FUNCTION CSIN : COMPLEX
VAR_INPUT
    X : COMPLEX;
END_VAR
VAR
    Result : COMPLEX;
END_VAR
Result.RE := COSH(X := X.IM) * SIN(X.RE);
Result.IM := SINH(X := X.IM) * COS(X.RE);
CSIN := Result;
END_FUNCTION

FUNCTION CSINH : COMPLEX
VAR_INPUT
    X : COMPLEX;
END_VAR
VAR
    Result : COMPLEX;
END_VAR
Result.RE := SINH(X := X.RE) * COS(X.IM);
Result.IM := COSH(X := X.RE) * SIN(X.IM);
CSINH := Result;
END_FUNCTION

FUNCTION CSQRT : COMPLEX
VAR_INPUT
    X : COMPLEX;
END_VAR
VAR
    Temp : REAL;
    Result : COMPLEX;
END_VAR
Temp := HYPOT(X := X.RE, Y := X.IM);
Result.RE := SQRT(REAL#0.5 * (Temp + X.RE));
Result.IM := INT_TO_REAL(SGN(X := X.IM)) * SQRT(REAL#0.5 * (Temp - X.RE));
CSQRT := Result;
END_FUNCTION

FUNCTION CSUB : COMPLEX
VAR_INPUT
    X : COMPLEX;
    Y : COMPLEX;
END_VAR
VAR
    Result : COMPLEX;
END_VAR
Result.RE := X.RE - Y.RE;
Result.IM := X.IM - Y.IM;
CSUB := Result;
END_FUNCTION

FUNCTION CTAN : COMPLEX
VAR_INPUT
    X : COMPLEX;
END_VAR
VAR
    Temp : REAL;
    Xi2 : REAL;
    Xr2 : REAL;
    Result : COMPLEX;
END_VAR
Xi2 := REAL#2.0 * X.IM;
Xr2 := REAL#2.0 * X.RE;
Temp := REAL#1.0 / (COS(Xr2) + COSH(X := Xi2));
Result.RE := Temp * SIN(Xr2);
Result.IM := Temp * SINH(X := Xi2);
CTAN := Result;
END_FUNCTION

FUNCTION CTANH : COMPLEX
VAR_INPUT
    X : COMPLEX;
END_VAR
VAR
    Temp : REAL;
    Xi2 : REAL;
    Xr2 : REAL;
    Result : COMPLEX;
END_VAR
Xi2 := REAL#2.0 * X.IM;
Xr2 := REAL#2.0 * X.RE;
Temp := REAL#1.0 / (COSH(X := Xr2) + COS(Xi2));
Result.RE := Temp * SINH(X := Xr2);
Result.IM := Temp * SIN(Xi2);
CTANH := Result;
END_FUNCTION
