TEST_PROGRAM oscat_negative_public_surface
VAR
    WrongOverride : REAL;
END_VAR

WrongOverride := OVERRIDE(
    X1 := REAL#1.0,
    E1 := TRUE,
    X2 := REAL#2.0,
    E2 := FALSE,
    X3 := REAL#3.0,
    E3 := FALSE
);
END_TEST_PROGRAM
