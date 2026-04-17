TEST_PROGRAM oscat_basic_not_yet_ported_public_surface_is_absent
VAR
    Data : ARRAY[0..2] OF REAL;
END_VAR

ASSERT_TRUE(IN := _ARRAY_ABS(pt := ADR(Data), size := SIZEOF(Data)));
END_TEST_PROGRAM
