PROGRAM Main
VAR
    LevelAlarm : HysteresisSwitch;
    AlarmObject : IHysteresisSwitch;
    AlarmQueue : DwordFifo16;
    QueueObject : IDwordQueue;
    AlarmLimits : RealRange;
    FeedwaterLevelPercent : REAL := REAL#12.0;
    AlarmCode : DWORD;
END_VAR

AlarmLimits.Low := REAL#3.0;
AlarmLimits.High := REAL#10.0;
AlarmObject := LevelAlarm;
AlarmObject.Initialize();
AlarmObject.SetLimits(Limits := AlarmLimits);
QueueObject := AlarmQueue;
QueueObject.Initialize();
QueueObject.Reset();

IF AlarmObject.Update(MeasuredValue := FeedwaterLevelPercent) THEN
    IF QueueObject.Push(Value := DWORD#16#B001) THEN
    END_IF;
END_IF;
IF QueueObject.TryPop() THEN
    AlarmCode := QueueObject.Value;
END_IF;
END_PROGRAM
