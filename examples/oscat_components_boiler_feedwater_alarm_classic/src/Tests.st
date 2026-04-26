TEST_PROGRAM BoilerFeedwaterAlarmClassicQueuesAlarmCode
VAR
    LevelAlarm : HYST;
    AlarmQueue : FIFO_16;
END_VAR

AlarmQueue(DIN := DWORD#0, E := TRUE, RD := FALSE, WD := FALSE, RST := TRUE);
LevelAlarm(In := REAL#12.0, HIGH_LIMIT := REAL#10.0, LOW_LIMIT := REAL#3.0);
AlarmQueue(DIN := DWORD#16#B001, E := TRUE, RD := FALSE, WD := LevelAlarm.Q, RST := FALSE);
AlarmQueue(DIN := DWORD#0, E := TRUE, RD := TRUE, WD := FALSE, RST := FALSE);

ASSERT_TRUE(IN := LevelAlarm.Q);
ASSERT_EQUAL(EXPECTED := DWORD#16#B001, ACTUAL := AlarmQueue.DOUT);
END_TEST_PROGRAM
