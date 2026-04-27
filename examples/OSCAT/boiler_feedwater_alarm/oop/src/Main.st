(* Compact showcase: Main keeps the component call sequence visible for the ST tests. *)
PROGRAM Main
VAR
    LevelAlarm : HysteresisSwitch;
    AlarmQueue : DwordFifo16;
    AlarmLimits : RealRange;
    FeedwaterLevelPercent : REAL := REAL#12.0;
    AlarmCode : DWORD;
END_VAR

AlarmLimits.Low := REAL#3.0;
AlarmLimits.High := REAL#10.0;
LevelAlarm.Initialize();
LevelAlarm.SetLimits(Limits := AlarmLimits);
AlarmQueue.Initialize();
AlarmQueue.Reset();

IF LevelAlarm.Update(MeasuredValue := FeedwaterLevelPercent) THEN
    IF AlarmQueue.Push(Value := DWORD#16#B001) THEN
    END_IF;
END_IF;
IF AlarmQueue.TryPop() THEN
    AlarmCode := AlarmQueue.Value;
END_IF;
END_PROGRAM
