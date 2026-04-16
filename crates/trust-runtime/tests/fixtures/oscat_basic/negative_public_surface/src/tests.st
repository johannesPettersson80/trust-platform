TEST_PROGRAM oscat_basic_not_yet_ported_public_surface_is_absent
VAR
    Coefficients : ARRAY[0..7] OF REAL;
    Value : REAL;
END_VAR

Coefficients[0] := REAL#1.0;
Coefficients[1] := REAL#2.0;
Value := F_LIN(X := REAL#2.0, A := REAL#3.0, B := REAL#4.0)
    + F_LIN2(X := REAL#2.0, X1 := REAL#0.0, Y1 := REAL#0.0, X2 := REAL#4.0, Y2 := REAL#8.0)
    + F_POLY(X := REAL#2.0, C := Coefficients)
    + F_POWER(A := REAL#2.0, X := REAL#3.0, N := REAL#2.0)
    + F_QUAD(X := REAL#2.0, A := REAL#1.0, B := REAL#2.0, C := REAL#3.0);
END_TEST_PROGRAM
