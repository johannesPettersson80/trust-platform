TEST_PROGRAM ClassicDosingBranchesOnCommandType
VAR
    Scheduler : ClassicDosingScheduler;
END_VAR

ASSERT_TRUE(IN := Scheduler.Enqueue(CommandType := INT#1));
ASSERT_TRUE(IN := Scheduler.ExecuteNext(CalibrationValid := TRUE, TankLevelMl := REAL#1000.0, CurrentPH := REAL#7.2));
ASSERT_EQUAL(EXPECTED := DWORD#16#C001, ACTUAL := Scheduler.LastCommandId);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Scheduler.ExecutedCount);
END_TEST_PROGRAM
