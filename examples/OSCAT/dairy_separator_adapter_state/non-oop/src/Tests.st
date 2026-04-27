TEST_PROGRAM ClassicSeparatorRunsProduction
VAR
    Separator : ClassicDairySeparator;
END_VAR

ASSERT_EQUAL(EXPECTED := INT#10, ACTUAL := Separator.Update(StartCommand := TRUE, DischargeRequest := FALSE, CipMode := FALSE, UseSiemens := FALSE, StatusWord := WORD#16#0004, SpeedFeedbackRpm := REAL#0.0, VibrationMmS := REAL#2.0, FaultRaw := WORD#16#0044));
ASSERT_EQUAL(EXPECTED := INT#20, ACTUAL := Separator.Update(StartCommand := TRUE, DischargeRequest := FALSE, CipMode := FALSE, UseSiemens := FALSE, StatusWord := WORD#16#0004, SpeedFeedbackRpm := REAL#6500.0, VibrationMmS := REAL#2.0, FaultRaw := WORD#16#0044));
ASSERT_TRUE(IN := Separator.FeedValveOpen);
END_TEST_PROGRAM

TEST_PROGRAM ClassicSeparatorTripsOnVibration
VAR
    Separator : ClassicDairySeparator;
END_VAR

ASSERT_EQUAL(EXPECTED := INT#90, ACTUAL := Separator.Update(StartCommand := TRUE, DischargeRequest := FALSE, CipMode := FALSE, UseSiemens := TRUE, StatusWord := WORD#16#0400, SpeedFeedbackRpm := REAL#6100.0, VibrationMmS := REAL#9.5, FaultRaw := WORD#16#0012));
ASSERT_EQUAL(EXPECTED := DWORD#16#500000F1, ACTUAL := Separator.FaultCode);
END_TEST_PROGRAM
