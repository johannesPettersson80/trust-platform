TEST_PROGRAM oscat_basic_not_yet_ported_public_surface_is_absent
VAR
    Value : BYTE;
END_VAR

Value := FRMP_B(START := BYTE#10, DIR := TRUE, TD := T#100ms, TR := T#1s);
END_TEST_PROGRAM
