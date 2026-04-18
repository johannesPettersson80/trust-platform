(* OSCAT BASIC 3.33 arithmetic helpers with REAL2 double-precision tuples. *)

FUNCTION R2_ABS : REAL2
VAR_INPUT
    X : REAL2;
END_VAR
VAR
    Result : REAL2;
END_VAR

IF X.RX >= REAL#0.0 THEN
    Result.RX := X.RX;
    Result.R1 := X.R1;
ELSE
    Result.RX := -X.RX;
    Result.R1 := -X.R1;
END_IF;
R2_ABS := Result;
END_FUNCTION

FUNCTION R2_ADD : REAL2
VAR_INPUT
    X : REAL2;
    Y : REAL;
END_VAR
VAR
    Temp : REAL;
    Result : REAL2;
END_VAR

Temp := X.RX;
Result.RX := Y + X.R1 + X.RX;
Result.R1 := Temp - Result.RX + Y + X.R1;
R2_ADD := Result;
END_FUNCTION

FUNCTION R2_ADD2 : REAL2
VAR_INPUT
    X : REAL2;
    Y : REAL2;
END_VAR
VAR
    Result : REAL2;
END_VAR

Result.R1 := X.R1 + Y.R1;
Result.RX := X.RX + Y.RX;
R2_ADD2 := Result;
END_FUNCTION

FUNCTION R2_MUL : REAL2
VAR_INPUT
    X : REAL2;
    Y : REAL;
END_VAR
VAR
    Result : REAL2;
END_VAR

Result.RX := X.RX * Y;
Result.R1 := X.R1 * Y;
R2_MUL := Result;
END_FUNCTION

FUNCTION R2_SET : REAL2
VAR_INPUT
    X : REAL;
END_VAR
VAR
    Result : REAL2;
END_VAR

Result.RX := X;
Result.R1 := REAL#0.0;
R2_SET := Result;
END_FUNCTION
