(* Main sets the switch and the queue up once, then trips, queues and reads the alarm code every scan. *)
PROGRAM Main
VAR
    LevelAlarm : HysteresisSwitch;
    AlarmQueue : DwordFifo16;
    AlarmLimits : RealRange;
    FeedwaterLevelPercent : REAL := REAL#12.0;
    AlarmCode : DWORD;
    Initialized : BOOL;
END_VAR

IF NOT Initialized THEN
    AlarmLimits.Low := REAL#3.0;
    AlarmLimits.High := REAL#10.0;
    LevelAlarm.Initialize();
    LevelAlarm.SetLimits(Limits := AlarmLimits);
    AlarmQueue.Initialize();
    AlarmQueue.Reset();
    Initialized := TRUE;
END_IF;

IF LevelAlarm.Update(MeasuredValue := FeedwaterLevelPercent) THEN
    IF AlarmQueue.Push(Value := DWORD#16#B001) THEN
    END_IF;
END_IF;
IF AlarmQueue.TryPop() THEN
    AlarmCode := AlarmQueue.Value;
END_IF;
END_PROGRAM
