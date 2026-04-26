PROGRAM Main
VAR
    WindAlarm : HYST;
    WindMetersPerSecond : REAL := REAL#10.0;
    BeaufortIndex : INT;
    AlarmActive : BOOL;
END_VAR

BeaufortIndex := MS_TO_BFT(MS := WindMetersPerSecond);
WindAlarm(In := WindMetersPerSecond, HIGH_LIMIT := REAL#8.0, LOW_LIMIT := REAL#3.0);
AlarmActive := WindAlarm.Q;
END_PROGRAM
