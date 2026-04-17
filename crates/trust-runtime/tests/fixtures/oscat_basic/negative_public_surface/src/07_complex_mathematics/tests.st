TEST_PROGRAM oscat_basic_not_yet_ported_public_surface_is_absent
VAR
    Value : COMPLEX;
END_VAR

Value.RE := REAL#3.0;
Value.IM := REAL#4.0;
ASSERT_NEAR(EXPECTED := REAL#5.0, ACTUAL := CABS(X := Value), DELTA := REAL#0.001);
END_TEST_PROGRAM
