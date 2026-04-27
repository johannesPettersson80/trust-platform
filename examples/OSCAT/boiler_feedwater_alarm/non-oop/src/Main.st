PROGRAM Main
VAR
    LevelAlarm : HYST;
    AlarmQueue : FIFO_16;
    FeedwaterLevelPercent : REAL := REAL#12.0;
    AlarmCode : DWORD;
END_VAR

LevelAlarm(In := FeedwaterLevelPercent, HIGH_LIMIT := REAL#10.0, LOW_LIMIT := REAL#3.0);
AlarmQueue(DIN := DWORD#16#B001, E := TRUE, RD := FALSE, WD := LevelAlarm.Q, RST := FALSE);
AlarmQueue(DIN := DWORD#0, E := TRUE, RD := TRUE, WD := FALSE, RST := FALSE);
AlarmCode := AlarmQueue.DOUT;
END_PROGRAM
