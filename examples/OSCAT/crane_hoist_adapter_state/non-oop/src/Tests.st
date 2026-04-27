TEST_PROGRAM ClassicCraneDecodesFaultAndStateInline
VAR
    Crane : ClassicCraneController;
END_VAR

ASSERT_EQUAL(EXPECTED := INT#90, ACTUAL := Crane.Update(HoistStatus := WORD#16#0008, TravelStatus := WORD#16#0400, LiftCommand := TRUE, TravelCommand := FALSE, LowerCommand := FALSE, UpperLimit := FALSE, Overload := FALSE));
ASSERT_EQUAL(EXPECTED := DWORD#16#A0000044, ACTUAL := Crane.NormalizedFaultCode);
END_TEST_PROGRAM
