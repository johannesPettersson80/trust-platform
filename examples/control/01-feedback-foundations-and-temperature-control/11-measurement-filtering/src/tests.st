(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM FilterMovesPartwayTowardRawMeasurement
VAR
    Fraction : LREAL;
    Filtered : LREAL;
END_VAR
Fraction := FilterFraction(CycleTime := LREAL#0.1, FilterTime := LREAL#1.0);
Filtered := FilterUpdate(PreviousFilteredValue := LREAL#35.0, RawValue := LREAL#36.0, FilterFraction := Fraction);
ASSERT_NEAR(EXPECTED := LREAL#0.0909090909090909, ACTUAL := Fraction, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#35.0909090909091, ACTUAL := Filtered, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM FilteredMeasurementReducesRequestMovement
VAR
    RawRequest : LREAL;
    FilteredRequest : LREAL;
END_VAR
RawRequest := ProportionalRequest(HoldingOutput := LREAL#75.0, ProportionalGain := LREAL#2.0, Setpoint := LREAL#35.0, Measurement := LREAL#36.0);
FilteredRequest := ProportionalRequest(HoldingOutput := LREAL#75.0, ProportionalGain := LREAL#2.0, Setpoint := LREAL#35.0, Measurement := LREAL#35.0909090909091);
ASSERT_TRUE(IN := RawRequest = LREAL#73.0);
ASSERT_NEAR(EXPECTED := LREAL#74.8181818181818, ACTUAL := FilteredRequest, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
