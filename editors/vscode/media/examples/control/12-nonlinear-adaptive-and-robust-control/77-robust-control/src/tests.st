(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson77GustAndControllerRequestsAreExplicit
ASSERT_NEAR(EXPECTED := LREAL#0.0,
    ACTUAL := Lesson77ReviewedGust(
        ScanCount := DINT#250, ControlAuthority := FALSE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0,
    ACTUAL := Lesson77ReviewedGust(
        ScanCount := DINT#249, ControlAuthority := TRUE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.10,
    ACTUAL := Lesson77ReviewedGust(
        ScanCount := DINT#250, ControlAuthority := TRUE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0,
    ACTUAL := Lesson77ReviewedGust(
        ScanCount := DINT#300, ControlAuthority := TRUE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#-4.0,
    ACTUAL := Lesson77NominalRequest(Bending := LREAL#0.10),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#-2.5,
    ACTUAL := Lesson77RobustRequest(
        FilteredBending := LREAL#0.10,
        PreviousFilteredBending := LREAL#0.09),
    DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
